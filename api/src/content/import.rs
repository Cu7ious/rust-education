use std::path::Path;

use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::content::load::load_course;
use crate::content::model::{BlockPackage, CoursePackage, Localized};
use crate::error::AppError;

#[derive(Debug, PartialEq, Eq)]
pub enum ImportOutcome {
    Unchanged {
        course_id: String,
        revision: i32,
    },
    Imported {
        course_id: String,
        revision: i32,
        release_id: Uuid,
    },
}

pub async fn import_course(pool: &sqlx::PgPool, path: &Path) -> Result<ImportOutcome, AppError> {
    let package = load_course(path)?;
    let mut tx = pool.begin().await?;
    let existing = sqlx::query!(
        r#"
        SELECT manifest_hash
        FROM content_releases
        WHERE course_id = $1 AND revision = $2
        "#,
        package.course_id,
        package.revision
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(existing) = existing {
        if existing.manifest_hash == package.manifest_hash {
            return Ok(ImportOutcome::Unchanged {
                course_id: package.course_id,
                revision: package.revision,
            });
        }
        return Err(AppError::RevisionConflict);
    }
    let max_revision = sqlx::query_scalar!(
        r#"SELECT MAX(revision) FROM content_releases WHERE course_id = $1"#,
        package.course_id
    )
    .fetch_one(&mut *tx)
    .await?;
    if let Some(max_revision) = max_revision
        && package.revision < max_revision
    {
        return Err(AppError::RevisionNotNewer);
    }

    let release_id = Uuid::new_v4();
    let snapshot = serde_json::to_value(&package).map_err(|_| AppError::Internal)?;
    sqlx::query!(
        r#"
        INSERT INTO content_releases (id, course_id, revision, manifest_hash, snapshot)
        VALUES ($1, $2, $3, $4, $5)
        "#,
        release_id,
        package.course_id,
        package.revision,
        package.manifest_hash,
        snapshot
    )
    .execute(&mut *tx)
    .await?;
    project(&mut tx, &package, release_id).await?;
    tx.commit().await?;
    Ok(ImportOutcome::Imported {
        course_id: package.course_id,
        revision: package.revision,
        release_id,
    })
}

async fn project(
    tx: &mut Transaction<'_, Postgres>,
    package: &CoursePackage,
    release_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        INSERT INTO courses (id, current_release_id, default_locale, retired_at)
        VALUES ($1, $2, $3, NULL)
        ON CONFLICT (id) DO UPDATE
        SET current_release_id = EXCLUDED.current_release_id,
            default_locale = EXCLUDED.default_locale,
            retired_at = NULL
        "#,
        package.course_id,
        release_id,
        package.default_locale
    )
    .execute(&mut **tx)
    .await?;
    replace_course_text(tx, &package.course_id, &package.title, &package.description).await?;
    negate_positions(tx, &package.course_id).await?;

    let mut topic_ids = Vec::new();
    let mut subtopic_ids = Vec::new();
    let mut lesson_ids = Vec::new();
    let mut block_ids = Vec::new();
    let mut task_ids = Vec::new();
    let mut assessment_ids = Vec::new();

    upsert_assessment(
        tx,
        &package.assessment.id,
        "course",
        &package.course_id,
        &package.course_id,
        package.assessment.pass_percent,
    )
    .await?;
    assessment_ids.push(package.assessment.id.clone());

    for topic in &package.topics {
        ensure_same_course(tx, "topics", &topic.id, &package.course_id).await?;
        sqlx::query!(
            r#"
            INSERT INTO topics (id, course_id, position, retired_at)
            VALUES ($1, $2, $3, NULL)
            ON CONFLICT (id) DO UPDATE
            SET course_id = EXCLUDED.course_id,
                position = EXCLUDED.position,
                retired_at = NULL
            "#,
            topic.id,
            package.course_id,
            topic.position
        )
        .execute(&mut **tx)
        .await?;
        replace_topic_text(tx, &topic.id, &topic.title, &topic.description).await?;
        topic_ids.push(topic.id.clone());

        for subtopic in &topic.subtopics {
            sqlx::query!(
                r#"
                INSERT INTO subtopics (id, topic_id, position, retired_at)
                VALUES ($1, $2, $3, NULL)
                ON CONFLICT (id) DO UPDATE
                SET topic_id = EXCLUDED.topic_id,
                    position = EXCLUDED.position,
                    retired_at = NULL
                "#,
                subtopic.id,
                topic.id,
                subtopic.position
            )
            .execute(&mut **tx)
            .await?;
            replace_subtopic_text(tx, &subtopic.id, &subtopic.title, &subtopic.description).await?;
            subtopic_ids.push(subtopic.id.clone());
            upsert_assessment(
                tx,
                &subtopic.assessment.id,
                "subtopic",
                &subtopic.id,
                &package.course_id,
                subtopic.assessment.pass_percent,
            )
            .await?;
            assessment_ids.push(subtopic.assessment.id.clone());

            for lesson in &subtopic.lessons {
                sqlx::query!(
                    r#"
                    INSERT INTO lessons (id, subtopic_id, position, retired_at)
                    VALUES ($1, $2, $3, NULL)
                    ON CONFLICT (id) DO UPDATE
                    SET subtopic_id = EXCLUDED.subtopic_id,
                        position = EXCLUDED.position,
                        retired_at = NULL
                    "#,
                    lesson.id,
                    subtopic.id,
                    lesson.position
                )
                .execute(&mut **tx)
                .await?;
                replace_lesson_text(tx, &lesson.id, &lesson.title, &lesson.description).await?;
                lesson_ids.push(lesson.id.clone());

                for block in &lesson.blocks {
                    upsert_block(tx, &lesson.id, block).await?;
                    block_ids.push(block.id().to_string());
                    if let BlockPackage::Task { task, .. } = block {
                        let public_payload = serde_json::to_value(&task.public_payload)
                            .map_err(|_| AppError::Internal)?;
                        let previous = sqlx::query!(
                            r#"SELECT course_id, grading_hash FROM tasks WHERE id = $1"#,
                            task.id
                        )
                        .fetch_optional(&mut **tx)
                        .await?;
                        if let Some(previous) = &previous
                            && previous.course_id != package.course_id
                        {
                            return Err(AppError::content(format!(
                                "task `{}` belongs to another course",
                                task.id
                            )));
                        }
                        sqlx::query!(
                            r#"
                            INSERT INTO tasks (
                                id, block_id, course_id, task_type, purpose,
                                grading_hash, public_payload, retired_at
                            )
                            VALUES ($1, $2, $3, $4, $5, $6, $7, NULL)
                            ON CONFLICT (id) DO UPDATE
                            SET block_id = EXCLUDED.block_id,
                                course_id = EXCLUDED.course_id,
                                task_type = EXCLUDED.task_type,
                                purpose = EXCLUDED.purpose,
                                grading_hash = EXCLUDED.grading_hash,
                                public_payload = EXCLUDED.public_payload,
                                retired_at = NULL
                            "#,
                            task.id,
                            block.id(),
                            package.course_id,
                            task.task_type.as_str(),
                            task.purpose.as_str(),
                            task.grading_hash,
                            public_payload
                        )
                        .execute(&mut **tx)
                        .await?;
                        let grading =
                            serde_json::to_value(&task.grading).map_err(|_| AppError::Internal)?;
                        sqlx::query!(
                            r#"
                            INSERT INTO task_grading (task_id, grading_hash, spec)
                            VALUES ($1, $2, $3)
                            ON CONFLICT (task_id) DO UPDATE
                            SET grading_hash = EXCLUDED.grading_hash,
                                spec = EXCLUDED.spec
                            "#,
                            task.id,
                            task.grading_hash,
                            grading
                        )
                        .execute(&mut **tx)
                        .await?;
                        if previous.is_some_and(|row| row.grading_hash != task.grading_hash) {
                            sqlx::query!(
                                r#"
                                UPDATE question_mastery
                                SET state = 'needs_review', updated_at = now()
                                WHERE task_id = $1
                                "#,
                                task.id
                            )
                            .execute(&mut **tx)
                            .await?;
                        }
                        task_ids.push(task.id.clone());
                    }
                }
            }
        }
    }

    sqlx::query!(
        r#"
        DELETE FROM assessment_items
        WHERE assessment_id IN (SELECT id FROM assessments WHERE course_id = $1)
        "#,
        package.course_id
    )
    .execute(&mut **tx)
    .await?;
    insert_items(tx, &package.assessment.id, &package.assessment.task_ids).await?;
    for topic in &package.topics {
        for subtopic in &topic.subtopics {
            insert_items(tx, &subtopic.assessment.id, &subtopic.assessment.task_ids).await?;
        }
    }

    retire(
        tx,
        &package.course_id,
        &PublishedIds {
            topics: &topic_ids,
            subtopics: &subtopic_ids,
            lessons: &lesson_ids,
            blocks: &block_ids,
            tasks: &task_ids,
            assessments: &assessment_ids,
        },
    )
    .await?;
    Ok(())
}

async fn ensure_same_course(
    tx: &mut Transaction<'_, Postgres>,
    _table: &str,
    topic_id: &str,
    course_id: &str,
) -> Result<(), AppError> {
    let existing = sqlx::query!(r#"SELECT course_id FROM topics WHERE id = $1"#, topic_id)
        .fetch_optional(&mut **tx)
        .await?;
    if let Some(existing) = existing
        && existing.course_id != course_id
    {
        return Err(AppError::content(format!(
            "topic `{topic_id}` belongs to another course"
        )));
    }
    Ok(())
}

async fn negate_positions(
    tx: &mut Transaction<'_, Postgres>,
    course_id: &str,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE topics
        SET position = -position
        WHERE course_id = $1 AND retired_at IS NULL AND position > 0
        "#,
        course_id
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        r#"
        UPDATE subtopics
        SET position = -position
        WHERE retired_at IS NULL AND position > 0
          AND topic_id IN (SELECT id FROM topics WHERE course_id = $1)
        "#,
        course_id
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        r#"
        UPDATE lessons
        SET position = -position
        WHERE retired_at IS NULL AND position > 0
          AND subtopic_id IN (
            SELECT s.id FROM subtopics s
            JOIN topics t ON t.id = s.topic_id
            WHERE t.course_id = $1
          )
        "#,
        course_id
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        r#"
        UPDATE blocks
        SET position = -position
        WHERE retired_at IS NULL AND position > 0
          AND lesson_id IN (
            SELECT l.id FROM lessons l
            JOIN subtopics s ON s.id = l.subtopic_id
            JOIN topics t ON t.id = s.topic_id
            WHERE t.course_id = $1
          )
        "#,
        course_id
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn upsert_block(
    tx: &mut Transaction<'_, Postgres>,
    lesson_id: &str,
    block: &BlockPackage,
) -> Result<(), AppError> {
    let (kind, markdown, video_url, video_title) = match block {
        BlockPackage::Text { markdown, .. } => (
            "text",
            Some(serde_json::to_value(markdown).map_err(|_| AppError::Internal)?),
            None,
            None,
        ),
        BlockPackage::Video { url, title, .. } => (
            "video",
            None,
            Some(url.clone()),
            Some(serde_json::to_value(title).map_err(|_| AppError::Internal)?),
        ),
        BlockPackage::Task { .. } => ("task", None, None, None),
    };
    sqlx::query!(
        r#"
        INSERT INTO blocks (
            id, lesson_id, position, kind, text_markdown, video_url, video_title, retired_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, NULL)
        ON CONFLICT (id) DO UPDATE
        SET lesson_id = EXCLUDED.lesson_id,
            position = EXCLUDED.position,
            kind = EXCLUDED.kind,
            text_markdown = EXCLUDED.text_markdown,
            video_url = EXCLUDED.video_url,
            video_title = EXCLUDED.video_title,
            retired_at = NULL
        "#,
        block.id(),
        lesson_id,
        block_position(block),
        kind,
        markdown,
        video_url,
        video_title
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn block_position(block: &BlockPackage) -> i32 {
    match block {
        BlockPackage::Text { position, .. }
        | BlockPackage::Video { position, .. }
        | BlockPackage::Task { position, .. } => *position,
    }
}

async fn upsert_assessment(
    tx: &mut Transaction<'_, Postgres>,
    id: &str,
    scope: &str,
    scope_id: &str,
    course_id: &str,
    pass_percent: i32,
) -> Result<(), AppError> {
    let existing = sqlx::query!(r#"SELECT course_id FROM assessments WHERE id = $1"#, id)
        .fetch_optional(&mut **tx)
        .await?;
    if let Some(existing) = existing
        && existing.course_id != course_id
    {
        return Err(AppError::content(format!(
            "assessment `{id}` belongs to another course"
        )));
    }
    sqlx::query!(
        r#"
        INSERT INTO assessments (id, scope, scope_id, course_id, pass_percent, retired_at)
        VALUES ($1, $2, $3, $4, $5, NULL)
        ON CONFLICT (id) DO UPDATE
        SET scope = EXCLUDED.scope,
            scope_id = EXCLUDED.scope_id,
            course_id = EXCLUDED.course_id,
            pass_percent = EXCLUDED.pass_percent,
            retired_at = NULL
        "#,
        id,
        scope,
        scope_id,
        course_id,
        pass_percent
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_items(
    tx: &mut Transaction<'_, Postgres>,
    assessment_id: &str,
    task_ids: &[String],
) -> Result<(), AppError> {
    for (index, task_id) in task_ids.iter().enumerate() {
        let position = i32::try_from(index + 1).unwrap_or(1);
        sqlx::query!(
            r#"
            INSERT INTO assessment_items (assessment_id, position, task_id)
            VALUES ($1, $2, $3)
            "#,
            assessment_id,
            position,
            task_id
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

struct PublishedIds<'a> {
    topics: &'a [String],
    subtopics: &'a [String],
    lessons: &'a [String],
    blocks: &'a [String],
    tasks: &'a [String],
    assessments: &'a [String],
}

async fn retire(
    tx: &mut Transaction<'_, Postgres>,
    course_id: &str,
    ids: &PublishedIds<'_>,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"
        UPDATE topics
        SET retired_at = now()
        WHERE course_id = $1 AND retired_at IS NULL AND NOT (id = ANY($2))
        "#,
        course_id,
        ids.topics
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        r#"
        UPDATE subtopics
        SET retired_at = now()
        WHERE retired_at IS NULL AND NOT (id = ANY($2))
          AND topic_id IN (SELECT id FROM topics WHERE course_id = $1)
        "#,
        course_id,
        ids.subtopics
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        r#"
        UPDATE lessons
        SET retired_at = now()
        WHERE retired_at IS NULL AND NOT (id = ANY($2))
          AND subtopic_id IN (
            SELECT s.id FROM subtopics s
            JOIN topics t ON t.id = s.topic_id
            WHERE t.course_id = $1
          )
        "#,
        course_id,
        ids.lessons
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        r#"
        UPDATE blocks
        SET retired_at = now()
        WHERE retired_at IS NULL AND NOT (id = ANY($2))
          AND lesson_id IN (
            SELECT l.id FROM lessons l
            JOIN subtopics s ON s.id = l.subtopic_id
            JOIN topics t ON t.id = s.topic_id
            WHERE t.course_id = $1
          )
        "#,
        course_id,
        ids.blocks
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        r#"
        UPDATE tasks
        SET retired_at = now()
        WHERE course_id = $1 AND retired_at IS NULL AND NOT (id = ANY($2))
        "#,
        course_id,
        ids.tasks
    )
    .execute(&mut **tx)
    .await?;
    sqlx::query!(
        r#"
        UPDATE assessments
        SET retired_at = now()
        WHERE course_id = $1 AND retired_at IS NULL AND NOT (id = ANY($2))
        "#,
        course_id,
        ids.assessments
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn replace_course_text(
    tx: &mut Transaction<'_, Postgres>,
    course_id: &str,
    title: &Localized,
    description: &Localized,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"DELETE FROM course_localizations WHERE course_id = $1"#,
        course_id
    )
    .execute(&mut **tx)
    .await?;
    for (locale, title, description) in locales(title, description) {
        sqlx::query!(
            r#"
            INSERT INTO course_localizations (course_id, locale, title, description)
            VALUES ($1, $2, $3, $4)
            "#,
            course_id,
            locale,
            title,
            description
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn replace_topic_text(
    tx: &mut Transaction<'_, Postgres>,
    topic_id: &str,
    title: &Localized,
    description: &Localized,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"DELETE FROM topic_localizations WHERE topic_id = $1"#,
        topic_id
    )
    .execute(&mut **tx)
    .await?;
    for (locale, title, description) in locales(title, description) {
        sqlx::query!(
            r#"
            INSERT INTO topic_localizations (topic_id, locale, title, description)
            VALUES ($1, $2, $3, $4)
            "#,
            topic_id,
            locale,
            title,
            description
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn replace_subtopic_text(
    tx: &mut Transaction<'_, Postgres>,
    subtopic_id: &str,
    title: &Localized,
    description: &Localized,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"DELETE FROM subtopic_localizations WHERE subtopic_id = $1"#,
        subtopic_id
    )
    .execute(&mut **tx)
    .await?;
    for (locale, title, description) in locales(title, description) {
        sqlx::query!(
            r#"
            INSERT INTO subtopic_localizations (subtopic_id, locale, title, description)
            VALUES ($1, $2, $3, $4)
            "#,
            subtopic_id,
            locale,
            title,
            description
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn replace_lesson_text(
    tx: &mut Transaction<'_, Postgres>,
    lesson_id: &str,
    title: &Localized,
    description: &Localized,
) -> Result<(), AppError> {
    sqlx::query!(
        r#"DELETE FROM lesson_localizations WHERE lesson_id = $1"#,
        lesson_id
    )
    .execute(&mut **tx)
    .await?;
    for (locale, title, description) in locales(title, description) {
        sqlx::query!(
            r#"
            INSERT INTO lesson_localizations (lesson_id, locale, title, description)
            VALUES ($1, $2, $3, $4)
            "#,
            lesson_id,
            locale,
            title,
            description
        )
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

fn locales<'a>(
    title: &'a Localized,
    description: &'a Localized,
) -> [(&'a str, &'a str, &'a str); 3] {
    [
        ("en", title.en.as_str(), description.en.as_str()),
        ("ru", title.ru.as_str(), description.ru.as_str()),
        ("uk", title.uk.as_str(), description.uk.as_str()),
    ]
}
