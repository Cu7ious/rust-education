use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    match fs::write("openapi.json", edurust_api::http::openapi_json()) {
        Ok(()) => {
            println!("wrote openapi.json");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("export failed: {error}");
            ExitCode::FAILURE
        }
    }
}
