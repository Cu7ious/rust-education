use std::env;
use std::path::Path;

use crate::error::AppError;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub app_env: AppEnv,
    pub app_origin: String,
    pub api_bind: String,
    pub cookie_secure: bool,
    pub dev_log_verification_codes: bool,
    pub session_ttl_days: i64,
    pub session_idle_days: i64,
    pub argon2_memory_kib: u32,
    pub argon2_iterations: u32,
    pub argon2_parallelism: u32,
    pub smtp_url: Option<String>,
    pub sms_provider: Option<String>,
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppEnv {
    Development,
    Test,
    Production,
}

impl AppEnv {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Test => "test",
            Self::Production => "production",
        }
    }

    pub fn auto_verify_email(self) -> bool {
        !matches!(self, Self::Production)
    }
}

impl Config {
    pub fn from_env() -> Result<Self, AppError> {
        let app_env = parse_env(optional("APP_ENV").as_deref().unwrap_or("development"))?;
        let dev_log_verification_codes = bool_env("DEV_LOG_VERIFICATION_CODES", false)?;
        if app_env == AppEnv::Production && dev_log_verification_codes {
            return Err(AppError::validation(
                "DEV_LOG_VERIFICATION_CODES must be false when APP_ENV=production",
            ));
        }
        let cookie_secure = match optional("COOKIE_SECURE") {
            Some(value) => parse_bool("COOKIE_SECURE", &value)?,
            None => app_env == AppEnv::Production,
        };
        Ok(Self {
            database_url: required("DATABASE_URL")?,
            app_env,
            app_origin: optional("APP_ORIGIN")
                .unwrap_or_else(|| "http://localhost:3000".to_string()),
            api_bind: optional("API_BIND").unwrap_or_else(|| "127.0.0.1:3001".to_string()),
            cookie_secure,
            dev_log_verification_codes,
            session_ttl_days: 30,
            session_idle_days: 14,
            argon2_memory_kib: 19 * 1024,
            argon2_iterations: 2,
            argon2_parallelism: 1,
            smtp_url: optional("SMTP_URL"),
            sms_provider: optional("SMS_PROVIDER"),
            google_client_id: optional("GOOGLE_CLIENT_ID"),
            google_client_secret: optional("GOOGLE_CLIENT_SECRET"),
            github_client_id: optional("GITHUB_CLIENT_ID"),
            github_client_secret: optional("GITHUB_CLIENT_SECRET"),
        })
    }

    pub fn for_tests(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
            app_env: AppEnv::Test,
            app_origin: "http://localhost:3000".to_string(),
            api_bind: "127.0.0.1:0".to_string(),
            cookie_secure: false,
            dev_log_verification_codes: false,
            session_ttl_days: 30,
            session_idle_days: 14,
            argon2_memory_kib: 19 * 1024,
            argon2_iterations: 2,
            argon2_parallelism: 1,
            smtp_url: None,
            sms_provider: None,
            google_client_id: None,
            google_client_secret: None,
            github_client_id: None,
            github_client_secret: None,
        }
    }
}

pub fn load_dotenv() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../.env");
    dotenvy::from_filename(manifest).ok();
    dotenvy::dotenv().ok();
}

fn required(name: &str) -> Result<String, AppError> {
    optional(name).ok_or_else(|| AppError::validation(format!("{name} is required")))
}

fn optional(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

fn parse_env(value: &str) -> Result<AppEnv, AppError> {
    match value {
        "development" => Ok(AppEnv::Development),
        "test" => Ok(AppEnv::Test),
        "production" => Ok(AppEnv::Production),
        _ => Err(AppError::validation(
            "APP_ENV must be development, test, or production",
        )),
    }
}

fn bool_env(name: &str, default: bool) -> Result<bool, AppError> {
    match optional(name) {
        Some(value) => parse_bool(name, &value),
        None => Ok(default),
    }
}

fn parse_bool(name: &str, value: &str) -> Result<bool, AppError> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => Err(AppError::validation(format!(
            "{name} must be true or false"
        ))),
    }
}
