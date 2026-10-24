use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use edurust_api::config::{Config, load_dotenv};
use edurust_api::content::{ImportOutcome, import_course};
use edurust_api::db;

#[tokio::main]
async fn main() -> ExitCode {
    load_dotenv();
    match run().await {
        Ok(outcome) => {
            match outcome {
                ImportOutcome::Unchanged {
                    course_id,
                    revision,
                } => {
                    println!("unchanged {course_id} revision {revision}");
                }
                ImportOutcome::Imported {
                    course_id,
                    revision,
                    release_id,
                } => {
                    println!("imported {course_id} revision {revision} release {release_id}");
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("import failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<ImportOutcome, edurust_api::error::AppError> {
    let path = course_path()?;
    let config = Config::from_env()?;
    let pool = db::connect(&config.database_url).await?;
    db::migrate(&pool).await?;
    import_course(&pool, &path).await
}

fn course_path() -> Result<PathBuf, edurust_api::error::AppError> {
    let mut args = env::args().skip(1);
    let mut path = None;
    while let Some(arg) = args.next() {
        if arg == "--path" {
            path = args.next();
        } else if let Some(value) = arg.strip_prefix("--path=") {
            path = Some(value.to_string());
        }
    }
    path.map(PathBuf::from).ok_or_else(|| {
        edurust_api::error::AppError::validation("usage: import-content --path <course-dir>")
    })
}
