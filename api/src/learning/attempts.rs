use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::content::model::{GradingSpec, TaskType};
use crate::dto::AttemptRequest;
use crate::dto::AttemptResponse;
use crate::error::AppError;
use crate::learning::grade::grade;

pub async fn submit_attempt(
    pool: &PgPool,
    user_id: Uuid,
    task_id: &str,
    idempotency_key: Uuid,
    locale: &str,
    answer: &AttemptRequest,
) -> Result<AttemptResponse, AppError> {
    let mut tx = pool.begin().await?;
    let task = sqlx::query!(
        r#"
        SELECT t.id, t.task_type, t.retired_at, g.spec, b.lesson_id, c.current_release_id, c.default_locale
        FROM tasks t
        JOIN task_grading g ON g.task_id = t.id
        JOIN blocks b ON b.id = t.block_id
        JOIN lessons l ON l.id = b.lesson_id
        JOIN subtopics s ON s.id = l.subtopic_id
        JOIN topics top ON top.id = s.topic_id
        JOIN courses c ON c.id = top.course_id
        WHERE t.id = $1
          AND t.retired_at IS NULL
          AND b.retired_at IS NULL
          AND l.retired_at IS NULL
          AND s.retired_at IS NULL
          AND top.retired_at IS NULL
          AND c.retired_at IS NULL
        "#,
        task_id
    )
    .fetch_optional(&mut *tx)
    .await?;
    let Some(task) = task else {
        return Err(AppError::NotFound);
    };
    let lesson_id = task.lesson_id.clone();
    let task_type = TaskType::parse(&task.task_type).ok_or(AppError::Internal)?;
    let spec: GradingSpec = serde_json::from_value(task.spec).map_err(|_| AppError::Internal)?;
    let existing = sqlx::query!(
        r#"
        SELECT id, correct, feedback, correct_choice_ids, submitted_answer, created_at
        FROM task_attempts
        WHERE user_id = $1 AND idempotency_key = $2
        "#,
        user_id,
        idempotency_key
    )
    .fetch_optional(&mut *tx)
    .await?;
    let stored = if let Some(existing) = existing {
        StoredAttempt::from_row(
            existing.id,
            existing.correct,
            existing.feedback,
            existing.correct_choice_ids,
            existing.submitted_answer,
            existing.created_at,
        )
    } else {
        let outcome = grade(task_type, &spec, answer)?;
        let submitted = serde_json::to_value(answer).map_err(|_| AppError::Internal)?;
        let feedback = serde_json::to_value(&spec.feedback).map_err(|_| AppError::Internal)?;
        let correct_choice_ids = outcome
            .correct_choice_ids
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|_| AppError::Internal)?;
        let attempt_id = Uuid::new_v4();
        let inserted = sqlx::query!(
            r#"
            INSERT INTO task_attempts (
                id, user_id, task_id, release_id, idempotency_key, submitted_answer,
                correct, feedback, correct_choice_ids, context, context_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'lesson', $10)
            ON CONFLICT (user_id, idempotency_key) DO NOTHING
            RETURNING id, correct, feedback, correct_choice_ids, submitted_answer, created_at
            "#,
            attempt_id,
            user_id,
            task.id,
            task.current_release_id,
            idempotency_key,
            submitted,
            outcome.correct,
            feedback,
            correct_choice_ids,
            lesson_id
        )
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(inserted) = inserted {
            StoredAttempt::from_row(
                inserted.id,
                inserted.correct,
                inserted.feedback,
                inserted.correct_choice_ids,
                inserted.submitted_answer,
                inserted.created_at,
            )
        } else {
            let row = sqlx::query!(
                r#"
                SELECT id, correct, feedback, correct_choice_ids, submitted_answer, created_at
                FROM task_attempts
                WHERE user_id = $1 AND idempotency_key = $2
                "#,
                user_id,
                idempotency_key
            )
            .fetch_one(&mut *tx)
            .await?;
            StoredAttempt::from_row(
                row.id,
                row.correct,
                row.feedback,
                row.correct_choice_ids,
                row.submitted_answer,
                row.created_at,
            )
        }
    };

    sqlx::query!(
        r#"
        INSERT INTO lesson_progress (user_id, lesson_id, status, completed_at)
        VALUES ($1, $2, 'in_progress', NULL)
        ON CONFLICT (user_id, lesson_id) DO UPDATE
        SET status = CASE
                WHEN lesson_progress.completed_at IS NULL THEN 'in_progress'
                ELSE lesson_progress.status
            END,
            updated_at = now()
        "#,
        user_id,
        lesson_id
    )
    .execute(&mut *tx)
    .await?;
    let complete = sqlx::query_scalar!(
        r#"
        SELECT NOT EXISTS (
            SELECT 1
            FROM blocks b
            JOIN tasks t ON t.block_id = b.id
            WHERE b.lesson_id = $1
              AND b.kind = 'task'
              AND b.retired_at IS NULL
              AND t.retired_at IS NULL
              AND NOT EXISTS (
                  SELECT 1 FROM task_attempts a
                  WHERE a.user_id = $2 AND a.task_id = t.id AND a.correct
              )
        )
        "#,
        lesson_id,
        user_id
    )
    .fetch_one(&mut *tx)
    .await?
    .unwrap_or(false);
    if complete {
        sqlx::query!(
            r#"
            UPDATE lesson_progress
            SET status = 'completed',
                completed_at = COALESCE(completed_at, now()),
                updated_at = now()
            WHERE user_id = $1 AND lesson_id = $2 AND completed_at IS NULL
            "#,
            user_id,
            lesson_id
        )
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    Ok(localize_attempt(
        stored,
        &task.id,
        locale,
        &task.default_locale,
    ))
}

struct StoredAttempt {
    id: Uuid,
    correct: bool,
    feedback: Value,
    correct_choice_ids: Option<Value>,
    submitted_answer: Value,
    created_at: DateTime<Utc>,
}

impl StoredAttempt {
    fn from_row(
        id: Uuid,
        correct: bool,
        feedback: Value,
        correct_choice_ids: Option<Value>,
        submitted_answer: Value,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            correct,
            feedback,
            correct_choice_ids,
            submitted_answer,
            created_at,
        }
    }
}

fn localize_attempt(
    stored: StoredAttempt,
    task_id: &str,
    locale: &str,
    default_locale: &str,
) -> AttemptResponse {
    let feedback_spec: Option<crate::content::model::FeedbackSpec> =
        serde_json::from_value(stored.feedback).ok();
    let (feedback, locale_used) = if let Some(feedback_spec) = feedback_spec {
        let source = if stored.correct {
            &feedback_spec.correct
        } else {
            &feedback_spec.incorrect
        };
        let (text, used) = source.pick(locale, default_locale);
        (
            crate::content::model::visible_feedback(text).to_string(),
            used.to_string(),
        )
    } else {
        (String::new(), default_locale.to_string())
    };
    AttemptResponse {
        attempt_id: stored.id,
        task_id: task_id.to_string(),
        correct: stored.correct,
        feedback,
        locale_used,
        correct_choice_ids: stored
            .correct_choice_ids
            .and_then(|value| serde_json::from_value(value).ok()),
        submitted_answer: stored.submitted_answer,
        created_at: stored.created_at.to_rfc3339(),
    }
}
