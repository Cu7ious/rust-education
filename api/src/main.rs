use std::process::ExitCode;

use edurust_api::config::{Config, load_dotenv};
use edurust_api::db;
use edurust_api::http;
use edurust_api::state::AppState;

#[tokio::main]
async fn main() -> ExitCode {
    if let Err(error) = run().await {
        eprintln!("api failed: {error:#}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

async fn run() -> anyhow::Result<()> {
    load_dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let config = Config::from_env().map_err(|error| anyhow::anyhow!(error.to_string()))?;
    tracing::info!(
        env = config.app_env.as_str(),
        google = config.google_client_id.is_some(),
        github = config.github_client_id.is_some(),
        smtp = config.smtp_url.is_some(),
        sms = config.sms_provider.is_some(),
        dev_log_verification_codes = config.dev_log_verification_codes,
        "starting api"
    );
    let pool = db::connect(&config.database_url).await?;
    db::migrate(&pool).await?;
    let bind = config.api_bind.clone();
    let state = AppState::new(pool, config)?;
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "api listening");
    axum::serve(listener, http::router(state)).await?;
    Ok(())
}
