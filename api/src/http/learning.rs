use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;

use crate::dto::{
    AttemptRequest, AttemptResponse, CourseDetail, CourseListResponse, LessonResponse, LocaleQuery,
    ProgressQuery, ProgressResponse,
};
use crate::error::AppError;
use crate::http::cookies::{current_user, enforce_csrf};
use crate::http::{requested_locale, require_idempotency};
use crate::learning::{
    course_detail as read_course, lesson as read_lesson, list_courses as read_courses,
    progress as read_progress, submit_attempt as grade_attempt,
};
use crate::state::AppState;

#[utoipa::path(
    get,
    path = "/v1/courses",
    params(("locale" = Option<String>, Query, description = "en")),
    responses((status = 200, description = "Visible courses", body = CourseListResponse)),
    tag = "learning"
)]
pub async fn list_courses(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LocaleQuery>,
) -> Result<Json<CourseListResponse>, AppError> {
    current_user(&state, &headers).await?;
    let locale = requested_locale(query.locale.as_deref(), &headers);
    Ok(Json(read_courses(&state.pool, &locale).await?))
}

#[utoipa::path(
    get,
    path = "/v1/courses/{course_id}",
    params(
        ("course_id" = String, Path, description = "Course slug"),
        ("locale" = Option<String>, Query, description = "en")
    ),
    responses((status = 200, description = "Course tree", body = CourseDetail)),
    tag = "learning"
)]
pub async fn course_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(course_id): Path<String>,
    Query(query): Query<LocaleQuery>,
) -> Result<Json<CourseDetail>, AppError> {
    let user = current_user(&state, &headers).await?;
    let locale = requested_locale(query.locale.as_deref(), &headers);
    Ok(Json(
        read_course(&state.pool, user.user_id, &course_id, &locale).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/v1/lessons/{lesson_id}",
    params(
        ("lesson_id" = String, Path, description = "Lesson slug"),
        ("locale" = Option<String>, Query, description = "en")
    ),
    responses((status = 200, description = "Public lesson", body = LessonResponse)),
    tag = "learning"
)]
pub async fn lesson(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(lesson_id): Path<String>,
    Query(query): Query<LocaleQuery>,
) -> Result<Json<LessonResponse>, AppError> {
    let user = current_user(&state, &headers).await?;
    let locale = requested_locale(query.locale.as_deref(), &headers);
    Ok(Json(
        read_lesson(&state.pool, user.user_id, &lesson_id, &locale).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/v1/tasks/{task_id}/attempts",
    params(("task_id" = String, Path, description = "Task slug")),
    request_body = AttemptRequest,
    responses((status = 200, description = "Graded attempt", body = AttemptResponse)),
    tag = "learning"
)]
pub async fn submit_attempt(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(task_id): Path<String>,
    Query(query): Query<LocaleQuery>,
    Json(body): Json<AttemptRequest>,
) -> Result<Json<AttemptResponse>, AppError> {
    enforce_csrf(&state, &headers)?;
    let user = current_user(&state, &headers).await?;
    let key = require_idempotency(&headers)?;
    let locale = requested_locale(query.locale.as_deref(), &headers);
    Ok(Json(
        grade_attempt(&state.pool, user.user_id, &task_id, key, &locale, &body).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/v1/progress",
    params(("course_id" = Option<String>, Query, description = "Optional course filter")),
    responses((status = 200, description = "Lesson progress", body = ProgressResponse)),
    tag = "learning"
)]
pub async fn progress(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ProgressQuery>,
) -> Result<Json<ProgressResponse>, AppError> {
    let user = current_user(&state, &headers).await?;
    Ok(Json(
        read_progress(&state.pool, user.user_id, query.course_id.as_deref()).await?,
    ))
}
