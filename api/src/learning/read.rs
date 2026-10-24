use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::content::model::{Localized, PublicTaskPayload};
use crate::dto::{
    AttemptResponse, BlockView, ChoiceView, CourseDetail, CourseListResponse, CourseSummary,
    LessonLink, LessonProgressBody, LessonProgressItem, LessonResponse, ProgressResponse,
    SubtopicNode, TaskView, TopicNode,
};
use crate::error::AppError;

pub async fn list_courses(pool: &PgPool, locale: &str) -> Result<CourseListResponse, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT c.id, c.default_locale, l.locale, l.title, l.description
        FROM courses c
        JOIN course_localizations l ON l.course_id = c.id
        WHERE c.retired_at IS NULL
        ORDER BY c.id, l.locale
        "#
    )
    .fetch_all(pool)
    .await?;
    type LocaleTexts = (String, Vec<(String, String, String)>);
    let mut grouped: HashMap<String, LocaleTexts> = HashMap::new();
    for row in rows {
        grouped
            .entry(row.id.clone())
            .or_insert_with(|| (row.default_locale.clone(), Vec::new()))
            .1
            .push((row.locale, row.title, row.description));
    }
    let mut courses: Vec<CourseSummary> = grouped
        .into_iter()
        .map(|(id, (default_locale, texts))| {
            let (title, description, locale_used) = pick_pair(&texts, locale, &default_locale);
            CourseSummary {
                id,
                title,
                description,
                locale_used,
            }
        })
        .collect();
    courses.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(CourseListResponse { courses })
}

pub async fn course_detail(
    pool: &PgPool,
    user_id: Uuid,
    course_id: &str,
    locale: &str,
) -> Result<CourseDetail, AppError> {
    let course = sqlx::query!(
        r#"
        SELECT id, default_locale
        FROM courses
        WHERE id = $1 AND retired_at IS NULL
        "#,
        course_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let texts = sqlx::query!(
        r#"SELECT locale, title, description FROM course_localizations WHERE course_id = $1"#,
        course_id
    )
    .fetch_all(pool)
    .await?;
    let text_rows: Vec<(String, String, String)> = texts
        .iter()
        .map(|row| {
            (
                row.locale.clone(),
                row.title.clone(),
                row.description.clone(),
            )
        })
        .collect();
    let (title, description, locale_used) = pick_pair(&text_rows, locale, &course.default_locale);
    let topics = sqlx::query!(
        r#"
        SELECT t.id, t.position, l.locale, l.title, l.description
        FROM topics t
        JOIN topic_localizations l ON l.topic_id = t.id
        WHERE t.course_id = $1 AND t.retired_at IS NULL
        ORDER BY t.position, l.locale
        "#,
        course_id
    )
    .fetch_all(pool)
    .await?;
    let subtopics = sqlx::query!(
        r#"
        SELECT s.id, s.topic_id, s.position, l.locale, l.title, l.description
        FROM subtopics s
        JOIN topics t ON t.id = s.topic_id
        JOIN subtopic_localizations l ON l.subtopic_id = s.id
        WHERE t.course_id = $1 AND s.retired_at IS NULL AND t.retired_at IS NULL
        ORDER BY s.position, l.locale
        "#,
        course_id
    )
    .fetch_all(pool)
    .await?;
    let lessons = sqlx::query!(
        r#"
        SELECT les.id, les.subtopic_id, les.position, l.locale, l.title, l.description,
               COALESCE(p.status, 'not_started') AS status
        FROM lessons les
        JOIN subtopics s ON s.id = les.subtopic_id
        JOIN topics t ON t.id = s.topic_id
        JOIN lesson_localizations l ON l.lesson_id = les.id
        LEFT JOIN lesson_progress p ON p.lesson_id = les.id AND p.user_id = $2
        WHERE t.course_id = $1 AND les.retired_at IS NULL AND s.retired_at IS NULL AND t.retired_at IS NULL
        ORDER BY les.position, l.locale
        "#,
        course_id,
        user_id
    )
    .fetch_all(pool)
    .await?;

    let mut topic_nodes: Vec<TopicNode> = Vec::new();
    let mut topic_index: HashMap<String, usize> = HashMap::new();
    for row in topics {
        if let Some(index) = topic_index.get(&row.id) {
            if row.locale == "en" {
                let node = &mut topic_nodes[*index];
                node.title = row.title;
                node.description = row.description;
            }
            continue;
        }
        topic_index.insert(row.id.clone(), topic_nodes.len());
        let english = row.locale == "en";
        topic_nodes.push(TopicNode {
            id: row.id,
            title: if english { row.title } else { String::new() },
            description: if english {
                row.description
            } else {
                String::new()
            },
            subtopics: Vec::new(),
        });
    }
    let _ = locale_used;
    // Rebuild topic text with the requested locale after the scan above.
    fill_topic_text(
        &mut topic_nodes,
        &topic_index,
        locale,
        &course.default_locale,
        pool,
        course_id,
    )
    .await?;

    let mut subtopic_index: HashMap<String, (usize, usize)> = HashMap::new();
    for row in &subtopics {
        let Some(topic_at) = topic_index.get(&row.topic_id).copied() else {
            continue;
        };
        if subtopic_index.contains_key(&row.id) {
            continue;
        }
        let node = SubtopicNode {
            id: row.id.clone(),
            title: String::new(),
            description: String::new(),
            lessons: Vec::new(),
        };
        let sub_at = topic_nodes[topic_at].subtopics.len();
        topic_nodes[topic_at].subtopics.push(node);
        subtopic_index.insert(row.id.clone(), (topic_at, sub_at));
    }
    for row in &subtopics {
        let Some((topic_at, sub_at)) = subtopic_index.get(&row.id).copied() else {
            continue;
        };
        let node = &mut topic_nodes[topic_at].subtopics[sub_at];
        if row.locale == "en" {
            node.title = row.title.clone();
            node.description = row.description.clone();
        }
    }
    let mut lesson_seen = std::collections::BTreeSet::new();
    for row in &lessons {
        if !lesson_seen.insert(row.id.clone()) {
            let Some((topic_at, sub_at)) = subtopic_index.get(&row.subtopic_id).copied() else {
                continue;
            };
            if let Some(link) = topic_nodes[topic_at].subtopics[sub_at]
                .lessons
                .iter_mut()
                .find(|lesson| lesson.id == row.id)
                && row.locale == locale
            {
                link.title = row.title.clone();
                link.description = row.description.clone();
            }
            continue;
        }
        let Some((topic_at, sub_at)) = subtopic_index.get(&row.subtopic_id).copied() else {
            continue;
        };
        topic_nodes[topic_at].subtopics[sub_at]
            .lessons
            .push(LessonLink {
                id: row.id.clone(),
                title: row.title.clone(),
                description: row.description.clone(),
                status: row
                    .status
                    .clone()
                    .unwrap_or_else(|| "not_started".to_string()),
            });
    }

    Ok(CourseDetail {
        id: course.id,
        title,
        description,
        locale_used,
        topics: topic_nodes,
    })
}

async fn fill_topic_text(
    topics: &mut [TopicNode],
    index: &HashMap<String, usize>,
    _locale: &str,
    _default_locale: &str,
    pool: &PgPool,
    course_id: &str,
) -> Result<(), AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT t.id, l.locale, l.title, l.description
        FROM topics t
        JOIN topic_localizations l ON l.topic_id = t.id
        WHERE t.course_id = $1 AND t.retired_at IS NULL
        "#,
        course_id
    )
    .fetch_all(pool)
    .await?;
    for row in rows {
        let Some(at) = index.get(&row.id).copied() else {
            continue;
        };
        if row.locale == "en" {
            topics[at].title = row.title;
            topics[at].description = row.description;
        }
    }
    Ok(())
}

pub async fn lesson(
    pool: &PgPool,
    user_id: Uuid,
    lesson_id: &str,
    locale: &str,
) -> Result<LessonResponse, AppError> {
    let lesson = sqlx::query!(
        r#"
        SELECT les.id, s.id AS subtopic_id, t.id AS topic_id, c.id AS course_id, c.default_locale
        FROM lessons les
        JOIN subtopics s ON s.id = les.subtopic_id
        JOIN topics t ON t.id = s.topic_id
        JOIN courses c ON c.id = t.course_id
        WHERE les.id = $1
          AND les.retired_at IS NULL
          AND s.retired_at IS NULL
          AND t.retired_at IS NULL
          AND c.retired_at IS NULL
        "#,
        lesson_id
    )
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let texts = sqlx::query!(
        r#"SELECT locale, title, description FROM lesson_localizations WHERE lesson_id = $1"#,
        lesson_id
    )
    .fetch_all(pool)
    .await?;
    let text_rows: Vec<(String, String, String)> = texts
        .iter()
        .map(|row| {
            (
                row.locale.clone(),
                row.title.clone(),
                row.description.clone(),
            )
        })
        .collect();
    let (title, description, locale_used) = pick_pair(&text_rows, locale, &lesson.default_locale);
    let blocks = sqlx::query!(
        r#"
        SELECT id, position, kind, text_markdown, video_url, video_title
        FROM blocks
        WHERE lesson_id = $1 AND retired_at IS NULL
        ORDER BY position
        "#,
        lesson_id
    )
    .fetch_all(pool)
    .await?;
    let tasks = sqlx::query!(
        r#"
        SELECT t.id, t.block_id, t.task_type, t.public_payload
        FROM tasks t
        JOIN blocks b ON b.id = t.block_id
        WHERE b.lesson_id = $1 AND t.retired_at IS NULL AND b.retired_at IS NULL
        "#,
        lesson_id
    )
    .fetch_all(pool)
    .await?;
    let task_ids: Vec<String> = tasks.iter().map(|task| task.id.clone()).collect();
    let attempts = if task_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query!(
            r#"
            SELECT a.id, a.task_id, a.correct, a.feedback, a.correct_choice_ids,
                   a.submitted_answer, a.created_at
            FROM task_attempts a
            WHERE a.user_id = $1
              AND a.task_id = ANY($2)
              AND a.created_at = (
                  SELECT MAX(b.created_at)
                  FROM task_attempts b
                  WHERE b.user_id = a.user_id AND b.task_id = a.task_id
              )
            "#,
            user_id,
            &task_ids
        )
        .fetch_all(pool)
        .await?
    };
    let mut latest = HashMap::new();
    for attempt in attempts {
        latest.insert(
            attempt.task_id.clone(),
            AttemptResponse {
                attempt_id: attempt.id,
                task_id: attempt.task_id,
                correct: attempt.correct,
                feedback: feedback_text(
                    attempt.feedback,
                    attempt.correct,
                    locale,
                    &lesson.default_locale,
                ),
                locale_used: locale_used.clone(),
                correct_choice_ids: attempt
                    .correct_choice_ids
                    .and_then(|value| serde_json::from_value(value).ok()),
                submitted_answer: attempt.submitted_answer,
                created_at: attempt.created_at.to_rfc3339(),
            },
        );
    }
    let mut task_by_block = HashMap::new();
    for task in tasks {
        if let Some(block_id) = task.block_id.clone() {
            task_by_block.insert(block_id, task);
        }
    }
    let mut block_views = Vec::new();
    for block in blocks {
        match block.kind.as_str() {
            "text" => {
                let markdown = localized_value(block.text_markdown, locale, &lesson.default_locale);
                block_views.push(BlockView::Text {
                    id: block.id,
                    markdown,
                });
            }
            "video" => {
                block_views.push(BlockView::Video {
                    id: block.id,
                    url: block.video_url.unwrap_or_default(),
                    title: localized_value(block.video_title, locale, &lesson.default_locale),
                });
            }
            "task" => {
                let Some(task) = task_by_block.remove(&block.id) else {
                    continue;
                };
                let payload: PublicTaskPayload =
                    serde_json::from_value(task.public_payload).map_err(|_| AppError::Internal)?;
                let (prompt, _) = payload.prompt.pick(locale, &lesson.default_locale);
                let choices = payload
                    .choices
                    .iter()
                    .map(|choice| {
                        let (text, _) = choice.text.pick(locale, &lesson.default_locale);
                        ChoiceView {
                            id: choice.id.clone(),
                            text: text.to_string(),
                        }
                    })
                    .collect();
                block_views.push(BlockView::Task {
                    id: block.id,
                    task: Box::new(TaskView {
                        id: task.id.clone(),
                        task_type: task.task_type,
                        prompt: prompt.to_string(),
                        choices,
                        latest_attempt: latest.remove(&task.id),
                    }),
                });
            }
            _ => {}
        }
    }
    let progress = sqlx::query!(
        r#"
        SELECT status, completed_at
        FROM lesson_progress
        WHERE user_id = $1 AND lesson_id = $2
        "#,
        user_id,
        lesson_id
    )
    .fetch_optional(pool)
    .await?;
    let progress = match progress {
        Some(row) => LessonProgressBody {
            status: row.status,
            completed_at: row.completed_at.map(|value| value.to_rfc3339()),
        },
        None => LessonProgressBody {
            status: "not_started".to_string(),
            completed_at: None,
        },
    };
    Ok(LessonResponse {
        id: lesson.id,
        title,
        description,
        locale_used,
        course_id: lesson.course_id,
        topic_id: lesson.topic_id,
        subtopic_id: lesson.subtopic_id,
        progress,
        blocks: block_views,
    })
}

pub async fn progress(
    pool: &PgPool,
    user_id: Uuid,
    course_id: Option<&str>,
) -> Result<ProgressResponse, AppError> {
    let rows = sqlx::query!(
        r#"
        SELECT les.id AS lesson_id, p.status, p.completed_at
        FROM lessons les
        JOIN subtopics s ON s.id = les.subtopic_id
        JOIN topics t ON t.id = s.topic_id
        LEFT JOIN lesson_progress p ON p.lesson_id = les.id AND p.user_id = $1
        WHERE les.retired_at IS NULL
          AND ($2::text IS NULL OR t.course_id = $2)
        ORDER BY t.position, s.position, les.position
        "#,
        user_id,
        course_id
    )
    .fetch_all(pool)
    .await?;
    let lessons = rows
        .into_iter()
        .map(|row| LessonProgressItem {
            lesson_id: row.lesson_id,
            status: row.status,
            completed_at: row
                .completed_at
                .map(|value: DateTime<Utc>| value.to_rfc3339()),
        })
        .collect();
    Ok(ProgressResponse { lessons })
}

fn pick_pair(
    rows: &[(String, String, String)],
    locale: &str,
    default_locale: &str,
) -> (String, String, String) {
    let _ = (locale, default_locale);
    rows.iter().find(|row| row.0 == "en").map_or_else(
        || (String::new(), String::new(), "en".to_string()),
        |(_, title, description)| (title.clone(), description.clone(), "en".to_string()),
    )
}

fn localized_value(value: Option<Value>, locale: &str, default_locale: &str) -> String {
    let Some(value) = value else {
        return String::new();
    };
    let Ok(localized) = serde_json::from_value::<Localized>(value) else {
        return String::new();
    };
    localized.pick(locale, default_locale).0.to_string()
}

fn feedback_text(value: Value, correct: bool, locale: &str, default_locale: &str) -> String {
    let Ok(feedback) = serde_json::from_value::<crate::content::model::FeedbackSpec>(value) else {
        return String::new();
    };
    let source = if correct {
        &feedback.correct
    } else {
        &feedback.incorrect
    };
    crate::content::model::visible_feedback(source.pick(locale, default_locale).0).to_string()
}
