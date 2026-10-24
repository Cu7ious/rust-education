use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::password::PasswordHasher;
use crate::auth::tokens::{random_token, token_hash};
use crate::config::AppEnv;
use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct PublicUser {
    pub id: Uuid,
    pub email: String,
}

pub struct SessionIssued {
    pub user: PublicUser,
    pub token: String,
}

pub async fn register(
    pool: &PgPool,
    hasher: &PasswordHasher,
    app_env: AppEnv,
    email: &str,
    password: &str,
) -> Result<PublicUser, AppError> {
    let email = normalize_email(email)?;
    validate_password(password)?;
    let password_hash = hasher.hash(password)?;
    let existing = sqlx::query_scalar!(
        r#"
        SELECT id
        FROM user_identities
        WHERE provider = 'password' AND provider_subject = $1
        "#,
        email
    )
    .fetch_optional(pool)
    .await?;
    if existing.is_some() {
        return Err(AppError::IdentifierTaken);
    }

    let user_id = Uuid::new_v4();
    let identity_id = Uuid::new_v4();
    let verified_at = if app_env.auto_verify_email() {
        Some(Utc::now())
    } else {
        None
    };
    let mut tx = pool.begin().await?;
    sqlx::query!(r#"INSERT INTO users (id) VALUES ($1)"#, user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query!(
        r#"
        INSERT INTO user_identities (
            id, user_id, provider, provider_subject, identifier_kind, identifier, email_verified_at
        )
        VALUES ($1, $2, 'password', $3, 'email', $3, $4)
        "#,
        identity_id,
        user_id,
        email,
        verified_at
    )
    .execute(&mut *tx)
    .await
    .map_err(map_unique)?;
    sqlx::query!(
        r#"
        INSERT INTO password_credentials (identity_id, password_hash)
        VALUES ($1, $2)
        "#,
        identity_id,
        password_hash
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(PublicUser { id: user_id, email })
}

pub async fn login(
    pool: &PgPool,
    hasher: &PasswordHasher,
    email: &str,
    password: &str,
    ttl_days: i64,
) -> Result<SessionIssued, AppError> {
    let email = normalize_email(email)?;
    validate_password(password)?;
    let identity = sqlx::query!(
        r#"
        SELECT i.user_id, i.email_verified_at, c.password_hash
        FROM user_identities i
        JOIN password_credentials c ON c.identity_id = i.id
        WHERE i.provider = 'password'
          AND i.identifier_kind = 'email'
          AND i.identifier = $1
        "#,
        email
    )
    .fetch_optional(pool)
    .await?;
    let Some(identity) = identity else {
        hasher.verify_dummy(password);
        return Err(AppError::InvalidCredentials);
    };
    if !hasher.verify(password, &identity.password_hash) {
        return Err(AppError::InvalidCredentials);
    }
    if identity.email_verified_at.is_none() {
        return Err(AppError::EmailUnverified);
    }
    let token = random_token();
    let session_id = Uuid::new_v4();
    let expires_at = Utc::now() + Duration::days(ttl_days);
    sqlx::query!(
        r#"
        INSERT INTO sessions (id, user_id, token_hash, expires_at)
        VALUES ($1, $2, $3, $4)
        "#,
        session_id,
        identity.user_id,
        token_hash(&token),
        expires_at
    )
    .execute(pool)
    .await?;
    Ok(SessionIssued {
        user: PublicUser {
            id: identity.user_id,
            email,
        },
        token,
    })
}

pub fn normalize_email(input: &str) -> Result<String, AppError> {
    let email = input.trim().to_lowercase();
    let Some((local, domain)) = email.split_once('@') else {
        return Err(AppError::validation("email is invalid"));
    };
    let Some((name, tld)) = domain.rsplit_once('.') else {
        return Err(AppError::validation("email is invalid"));
    };
    let valid = !local.is_empty()
        && !name.is_empty()
        && tld.len() >= 2
        && email.len() <= 254
        && !email.chars().any(char::is_whitespace)
        && local.chars().all(|c| c.is_ascii_graphic() && c != '@')
        && domain
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    if valid {
        Ok(email)
    } else {
        Err(AppError::validation("email is invalid"))
    }
}

fn validate_password(password: &str) -> Result<(), AppError> {
    let length = password.chars().count();
    if (8..=200).contains(&length) {
        Ok(())
    } else {
        Err(AppError::validation(
            "password must be between 8 and 200 characters",
        ))
    }
}

fn map_unique(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db) = &error
        && db.code().as_deref() == Some("23505")
    {
        return AppError::IdentifierTaken;
    }
    AppError::from(error)
}
