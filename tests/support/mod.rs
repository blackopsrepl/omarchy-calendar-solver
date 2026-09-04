use std::process::{Command, Output, Stdio};

use serde_json::{json, Value};

pub fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/minimal-request.json")).unwrap()
}

pub fn invoke(request: &Value) -> Output {
    use std::io::Write;

    let mut child = Command::new(env!("CARGO_BIN_EXE_omarchy-calendar-solver"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("solver process should start");
    child
        .stdin
        .take()
        .expect("solver stdin should be available")
        .write_all(serde_json::to_string(request).unwrap().as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[allow(dead_code)]
pub fn task(id: &str, duration_minutes: u32) -> Value {
    json!({
        "id": id,
        "title": id,
        "durationMinutes": duration_minutes,
        "priority": "normal",
        "cognitiveLoad": "medium",
        "earliestAt": null,
        "deadlineKind": "none",
        "deadlineAt": null,
        "state": "inbox",
        "linkedEventId": null,
        "createdAt": "2026-09-04T07:00:00Z",
        "updatedAt": "2026-09-04T07:00:00Z"
    })
}

pub fn response(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("solver stdout should be JSON")
}
