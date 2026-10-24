use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::tokens::token_hash;
use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub user_id: Uuid,
    pub session_id: Uuid,
}

pub async fn authenticate(
    pool: &PgPool,
    token: &str,
    idle_days: i64,
) -> Result<CurrentUser, AppError> {
    let row = sqlx::query!(
        r#"
        SELECT id, user_id, expires_at, revoked_at, last_seen_at
        FROM sessions
        WHERE token_hash = $1
        "#,
        token_hash(token)
    )
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else {
        return Err(AppError::Unauthorized);
    };
    let now = Utc::now();
    if row.revoked_at.is_some() || row.expires_at < now {
        return Err(AppError::Unauthorized);
    }
    let idle_for = now.signed_duration_since(row.last_seen_at);
    if idle_for > Duration::days(idle_days) {
        sqlx::query!(
            r#"UPDATE sessions SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL"#,
            row.id
        )
        .execute(pool)
        .await?;
        return Err(AppError::Unauthorized);
    }
    if idle_for > Duration::seconds(60) {
        sqlx::query!(
            r#"UPDATE sessions SET last_seen_at = now() WHERE id = $1 AND revoked_at IS NULL"#,
            row.id
        )
        .execute(pool)
        .await?;
    }
    Ok(CurrentUser {
        user_id: row.user_id,
        session_id: row.id,
    })
}

pub async fn revoke(pool: &PgPool, session_id: Uuid) -> Result<(), AppError> {
    sqlx::query!(
        r#"UPDATE sessions SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL"#,
        session_id
    )
    .execute(pool)
    .await?;
    Ok(())
}
