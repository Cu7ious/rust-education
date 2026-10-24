use axum::http::{HeaderMap, HeaderValue};

use crate::auth::random_token;
use crate::auth::{CurrentUser, authenticate, tokens_match};
use crate::error::AppError;
use crate::state::AppState;

pub const SESSION_COOKIE: &str = "edurust_session";
pub const CSRF_COOKIE: &str = "edurust_csrf";

pub fn session_cookie(token: &str, secure: bool, max_age: i64) -> String {
    format!(
        "{SESSION_COOKIE}={token}; HttpOnly; SameSite=Lax; Path=/; Max-Age={max_age}{}",
        secure_suffix(secure)
    )
}

pub fn clear_session_cookie(secure: bool) -> String {
    format!(
        "{SESSION_COOKIE}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0{}",
        secure_suffix(secure)
    )
}

pub fn csrf_cookie(token: &str, secure: bool) -> String {
    format!(
        "{CSRF_COOKIE}={token}; SameSite=Lax; Path=/; Max-Age=2592000{}",
        secure_suffix(secure)
    )
}

fn secure_suffix(secure: bool) -> &'static str {
    if secure { "; Secure" } else { "" }
}

pub fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let mut pieces = part.trim().splitn(2, '=');
        if pieces.next()? == name {
            return pieces.next().map(ToOwned::to_owned);
        }
    }
    None
}

pub fn bearer_token(headers: &HeaderMap) -> Option<String> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    let token = value.strip_prefix("Bearer ")?.trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

pub async fn current_user(state: &AppState, headers: &HeaderMap) -> Result<CurrentUser, AppError> {
    let token = bearer_token(headers)
        .or_else(|| cookie_value(headers, SESSION_COOKIE))
        .ok_or(AppError::Unauthorized)?;
    authenticate(&state.pool, &token, state.config.session_idle_days).await
}

pub fn enforce_csrf(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    if bearer_token(headers).is_some() {
        return Ok(());
    }
    let cookie = cookie_value(headers, CSRF_COOKIE).unwrap_or_default();
    let header = headers
        .get("x-csrf-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if cookie.is_empty() || !tokens_match(&cookie, header) {
        return Err(AppError::CsrfRejected);
    }
    let _ = state;
    Ok(())
}

pub fn issue_csrf(headers: &HeaderMap, secure: bool) -> (String, String) {
    let token = cookie_value(headers, CSRF_COOKIE).unwrap_or_else(random_token);
    (token.clone(), csrf_cookie(&token, secure))
}

pub fn append_cookie(headers: &mut HeaderMap, cookie: String) {
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        headers.append(axum::http::header::SET_COOKIE, value);
    }
}
