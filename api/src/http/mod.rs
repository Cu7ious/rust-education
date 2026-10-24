mod auth;
mod cookies;
mod learning;

use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::http::{HeaderName, HeaderValue, Method};
use axum::routing::{get, post};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;

use crate::dto::{
    AttemptRequest, AttemptResponse, CourseDetail, CourseListResponse, CsrfResponse,
    LessonResponse, LoginRequest, ProgressResponse, RegisterRequest, UserResponse,
};
use crate::error::AppError;
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    let origin = state
        .config
        .app_origin
        .parse::<HeaderValue>()
        .unwrap_or_else(|_| HeaderValue::from_static("http://localhost:3000"));
    let cors = CorsLayer::new()
        .allow_origin(origin)
        .allow_credentials(true)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            HeaderName::from_static("content-type"),
            HeaderName::from_static("authorization"),
            HeaderName::from_static("x-csrf-token"),
            HeaderName::from_static("idempotency-key"),
            HeaderName::from_static("accept-language"),
        ])
        .max_age(Duration::from_secs(600));
    Router::new()
        .route("/v1/openapi.json", get(openapi_handler))
        .route("/v1/auth/csrf", get(auth::csrf))
        .route("/v1/auth/register", post(auth::register))
        .route("/v1/auth/login", post(auth::login))
        .route("/v1/auth/logout", post(auth::logout))
        .route("/v1/auth/me", get(auth::me))
        .route("/v1/courses", get(learning::list_courses))
        .route("/v1/courses/{course_id}", get(learning::course_detail))
        .route("/v1/lessons/{lesson_id}", get(learning::lesson))
        .route(
            "/v1/tasks/{task_id}/attempts",
            post(learning::submit_attempt),
        )
        .route("/v1/progress", get(learning::progress))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

pub fn openapi_json() -> String {
    ApiDoc::openapi()
        .to_pretty_json()
        .unwrap_or_else(|_| "{}".to_string())
}

async fn openapi_handler() -> Json<serde_json::Value> {
    Json(
        ApiDoc::openapi()
            .to_pretty_json()
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_else(|| serde_json::json!({})),
    )
}

#[derive(OpenApi)]
#[openapi(
    info(title = "Edurust API", version = "0.1.0", description = "Learning platform API."),
    paths(
        auth::csrf,
        auth::register,
        auth::login,
        auth::logout,
        auth::me,
        learning::list_courses,
        learning::course_detail,
        learning::lesson,
        learning::submit_attempt,
        learning::progress
    ),
    components(schemas(
        RegisterRequest,
        LoginRequest,
        UserResponse,
        CsrfResponse,
        AttemptRequest,
        AttemptResponse,
        CourseListResponse,
        CourseDetail,
        LessonResponse,
        ProgressResponse
    )),
    tags(
        (name = "auth", description = "Registration and sessions"),
        (name = "learning", description = "Courses, lessons, and attempts")
    )
)]
struct ApiDoc;

pub fn require_idempotency(headers: &axum::http::HeaderMap) -> Result<uuid::Uuid, AppError> {
    let raw = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::IdempotencyKeyRequired)?;
    uuid::Uuid::parse_str(raw).map_err(|_| AppError::validation("idempotency key must be a UUID"))
}

pub fn requested_locale(explicit: Option<&str>, headers: &axum::http::HeaderMap) -> String {
    let _ = (explicit, headers);
    "en".to_string()
}
