use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};

use crate::auth::{login as login_user, register as register_user};
use crate::dto::{CsrfResponse, LoginRequest, RegisterRequest, UserBody, UserResponse};
use crate::error::AppError;
use crate::http::cookies::{
    append_cookie, clear_session_cookie, current_user, enforce_csrf, issue_csrf, session_cookie,
};
use crate::state::AppState;

#[utoipa::path(
    get,
    path = "/v1/auth/csrf",
    responses((status = 200, description = "CSRF token", body = CsrfResponse)),
    tag = "auth"
)]
pub async fn csrf(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> (HeaderMap, Json<CsrfResponse>) {
    let (token, cookie) = issue_csrf(&headers, state.config.cookie_secure);
    let mut response_headers = HeaderMap::new();
    append_cookie(&mut response_headers, cookie);
    (response_headers, Json(CsrfResponse { token }))
}

#[utoipa::path(
    post,
    path = "/v1/auth/register",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "Account created", body = UserResponse),
        (status = 409, description = "Email is already registered")
    ),
    tag = "auth"
)]
pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<UserResponse>), AppError> {
    enforce_csrf(&state, &headers)?;
    let user = register_user(
        &state.pool,
        &state.passwords,
        state.config.app_env,
        &body.email,
        &body.password,
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(UserResponse {
            user: UserBody {
                id: user.id,
                email: user.email,
            },
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/v1/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Session cookie set", body = UserResponse),
        (status = 401, description = "Invalid credentials")
    ),
    tag = "auth"
)]
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Result<(HeaderMap, Json<UserResponse>), AppError> {
    enforce_csrf(&state, &headers)?;
    let issued = login_user(
        &state.pool,
        &state.passwords,
        &body.email,
        &body.password,
        state.config.session_ttl_days,
    )
    .await?;
    let mut response_headers = HeaderMap::new();
    let max_age = state.config.session_ttl_days * 24 * 60 * 60;
    append_cookie(
        &mut response_headers,
        session_cookie(&issued.token, state.config.cookie_secure, max_age),
    );
    Ok((
        response_headers,
        Json(UserResponse {
            user: UserBody {
                id: issued.user.id,
                email: issued.user.email,
            },
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/v1/auth/logout",
    responses((status = 204, description = "Session revoked")),
    tag = "auth"
)]
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<(StatusCode, HeaderMap), AppError> {
    enforce_csrf(&state, &headers)?;
    let user = current_user(&state, &headers).await?;
    crate::auth::revoke(&state.pool, user.session_id).await?;
    let mut response_headers = HeaderMap::new();
    append_cookie(
        &mut response_headers,
        clear_session_cookie(state.config.cookie_secure),
    );
    Ok((StatusCode::NO_CONTENT, response_headers))
}

#[utoipa::path(
    get,
    path = "/v1/auth/me",
    responses(
        (status = 200, description = "Current user", body = UserResponse),
        (status = 401, description = "Unauthorized")
    ),
    tag = "auth"
)]
pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<UserResponse>, AppError> {
    let user = current_user(&state, &headers).await?;
    let email = sqlx::query_scalar!(
        r#"
        SELECT identifier
        FROM user_identities
        WHERE user_id = $1 AND provider = 'password' AND identifier_kind = 'email'
        "#,
        user.user_id
    )
    .fetch_optional(&state.pool)
    .await?
    .flatten()
    .ok_or(AppError::Unauthorized)?;
    Ok(Json(UserResponse {
        user: UserBody {
            id: user.user_id,
            email,
        },
    }))
}
