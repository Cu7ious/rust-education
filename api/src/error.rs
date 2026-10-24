use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("identifier taken")]
    IdentifierTaken,
    #[error("authentication is required")]
    Unauthorized,
    #[error("email is not verified")]
    EmailUnverified,
    #[error("csrf check failed")]
    CsrfRejected,
    #[error("{0}")]
    Validation(String),
    #[error("not found")]
    NotFound,
    #[error("idempotency key is required")]
    IdempotencyKeyRequired,
    #[error("content revision conflict")]
    RevisionConflict,
    #[error("content revision is not newer than the published release")]
    RevisionNotNewer,
    #[error("{0}")]
    ContentInvalid(String),
    #[error("internal error")]
    Internal,
    #[error(transparent)]
    Database(#[from] Box<sqlx::Error>),
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        Self::Database(Box::new(error))
    }
}

impl AppError {
    pub fn content(message: impl Into<String>) -> Self {
        Self::ContentInvalid(message.into())
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    fn public_parts(&self) -> (StatusCode, &'static str, String) {
        match self {
            Self::InvalidCredentials => (
                StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                self.to_string(),
            ),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", self.to_string()),
            Self::EmailUnverified => (StatusCode::FORBIDDEN, "email_unverified", self.to_string()),
            Self::IdentifierTaken => (StatusCode::CONFLICT, "identifier_taken", self.to_string()),
            Self::CsrfRejected => (StatusCode::BAD_REQUEST, "csrf_rejected", self.to_string()),
            Self::Validation(message) => {
                (StatusCode::BAD_REQUEST, "validation_error", message.clone())
            }
            Self::IdempotencyKeyRequired => (
                StatusCode::BAD_REQUEST,
                "idempotency_key_required",
                self.to_string(),
            ),
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found", self.to_string()),
            Self::RevisionConflict | Self::RevisionNotNewer => {
                (StatusCode::CONFLICT, "revision_conflict", self.to_string())
            }
            Self::ContentInvalid(message) => {
                (StatusCode::BAD_REQUEST, "content_invalid", message.clone())
            }
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "internal error".to_string(),
            ),
            Self::Database(error) => {
                tracing::error!(error = %error, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "internal error".to_string(),
                )
            }
        }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = self.public_parts();
        let body = ErrorBody {
            error: ErrorDetail { code, message },
        };
        (status, Json(body)).into_response()
    }
}
