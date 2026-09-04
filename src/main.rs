mod domain;
mod error;
mod model;
mod protocol;
mod recurrence;
mod scheduling;
mod solve;
mod time;
mod validation;

use std::io::{self, BufRead, Read, Write};

use error::{AppError, WireError};
use protocol::{Request, Response};

const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;

fn main() {
    let input = read_input();
    let request_id = input
        .as_ref()
        .ok()
        .and_then(|bytes| serde_json::from_slice::<RequestIdHint>(bytes).ok())
        .and_then(|hint| hint.request_id)
        .unwrap_or_default();
    let result = input.and_then(|bytes| run(&bytes));
    if let Err(error) = result {
        let response = Response::failure(request_id, WireError::from(&error));
        let _ = writeln!(
            io::stdout(),
            "{}",
            serde_json::to_string(&response).unwrap_or_else(|_| {
                "{\"protocolVersion\":1,\"requestId\":\"\",\"ok\":false}".to_string()
            })
        );
        eprintln!("omarchy-calendar-solver: {error}");
        std::process::exit(
            if matches!(
                error,
                AppError::Protocol { .. }
                    | AppError::Validation { .. }
                    | AppError::Recurrence { .. }
            ) {
                2
            } else {
                1
            },
        );
    }
}

fn read_input() -> Result<Vec<u8>, AppError> {
    let mut input = Vec::new();
    let stdin = io::stdin();
    let mut reader = io::BufReader::new(stdin.lock()).take((MAX_INPUT_BYTES + 1) as u64);
    reader
        .read_until(b'\n', &mut input)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    if input.len() > MAX_INPUT_BYTES {
        Err(AppError::protocol(
            "input_too_large",
            "",
            "request exceeds the maximum input size",
        ))
    } else {
        Ok(input)
    }
}

fn run(input: &[u8]) -> Result<(), AppError> {
    let request: Request = serde_json::from_slice(input)
        .map_err(|error| AppError::protocol("invalid_json", "", error.to_string()))?;
    validation::validate_request(&request)?;
    let proposal = crate::solve::solve(&request)?;
    let response = Response::success(&request, proposal);
    serde_json::to_writer(io::stdout(), &response)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    io::stdout()
        .write_all(b"\n")
        .map_err(|error| AppError::Internal(error.to_string()))?;
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequestIdHint {
    request_id: Option<String>,
}
