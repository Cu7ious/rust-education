use std::sync::Arc;

use sqlx::PgPool;

use crate::auth::PasswordHasher;
use crate::config::Config;
use crate::error::AppError;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub passwords: PasswordHasher,
}

impl AppState {
    pub fn new(pool: PgPool, config: Config) -> Result<Self, AppError> {
        let passwords = PasswordHasher::new(
            config.argon2_memory_kib,
            config.argon2_iterations,
            config.argon2_parallelism,
        )?;
        Ok(Self {
            pool,
            config: Arc::new(config),
            passwords,
        })
    }
}
