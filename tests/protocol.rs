use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

fn settings() -> Value {
    json!({
        "timezone": "Europe/Rome",
        "availability": {"friday": [{"start": "09:00", "end": "17:00"}]},
        "horizonDays": 14,
        "slotMinutes": 15,
        "solveSeconds": 1,
        "priorityLowWeight": 1,
        "priorityNormalWeight": 5,
        "priorityHighWeight": 25,
        "cognitiveEnabled": false,
        "lowWindowStart": "00:00",
        "lowWindowEnd": "00:00",
        "lowOutsidePenalty": 0,
        "mediumWindowStart": "00:00",
        "mediumWindowEnd": "00:00",
        "mediumOutsidePenalty": 0,
        "highWindowStart": "00:00",
        "highWindowEnd": "00:00",
        "highOutsidePenalty": 0,
        "highStreakLimit": 1,
        "recoveryMinutes": 30,
        "excessHighPenalty": 60
    })
}

fn request() -> Value {
    json!({
        "protocolVersion": 1,
        "requestId": "protocol-test",
        "baseInputRevision": 4,
        "now": "2026-09-04T08:00:00+02:00",
        "settings": settings(),
        "events": [],
        "tasks": [],
        "dependencies": []
    })
}

fn invoke(input: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omarchy-calendar-solver"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("solver process should start");
    use std::io::Write;
    child
        .stdin
        .take()
        .expect("solver stdin should be available")
        .write_all(input)
        .expect("request should be written");
    child
        .wait_with_output()
        .expect("solver process should exit")
}

fn response(output: &std::process::Output) -> Value {
    assert_eq!(output.stdout.split(|byte| *byte == b'\n').count(), 2);
    serde_json::from_slice(&output.stdout).expect("stdout should contain one JSON response")
}

#[test]
fn valid_request_has_one_response_and_echoes_request_id() {
    let output = invoke(serde_json::to_string(&request()).unwrap().as_bytes());
    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "valid requests should not log to stderr"
    );
    let response = response(&output);
    assert_eq!(response["ok"], true);
    assert_eq!(response["requestId"], "protocol-test");
}

#[test]
fn newline_terminated_request_does_not_wait_for_stdin_eof() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omarchy-calendar-solver"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("solver process should start");
    let mut stdin = child
        .stdin
        .take()
        .expect("solver stdin should be available");
    use std::io::Write;
    writeln!(stdin, "{}", serde_json::to_string(&request()).unwrap()).unwrap();

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            panic!("solver waited for stdin EOF after receiving a request line");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.stderr.is_empty());
    assert_eq!(response(&output)["requestId"], "protocol-test");
}

#[test]
fn malformed_json_uses_protocol_exit_code() {
    let output = invoke(br#"{"protocolVersion":1,"requestId":"bad""#);
    assert_eq!(output.status.code(), Some(2));
    let response = response(&output);
    assert_eq!(response["ok"], false);
    assert_eq!(response["requestId"], "");
    assert_eq!(response["error"]["code"], "invalid_json");
}

#[test]
fn unknown_envelope_fields_are_rejected_but_request_id_is_echoed() {
    let mut request = request();
    request["unexpected"] = json!(true);
    let output = invoke(serde_json::to_string(&request).unwrap().as_bytes());
    assert_eq!(output.status.code(), Some(2));
    let response = response(&output);
    assert_eq!(response["requestId"], "protocol-test");
}

#[test]
fn unsupported_protocol_is_rejected_cleanly() {
    let mut request = request();
    request["protocolVersion"] = json!(99);
    let output = invoke(serde_json::to_string(&request).unwrap().as_bytes());
    assert_eq!(output.status.code(), Some(2));
    let response = response(&output);
    assert_eq!(response["requestId"], "protocol-test");
    assert_eq!(response["error"]["code"], "unsupported_protocol_version");
}

#[test]
fn oversized_input_is_rejected_before_parsing() {
    let input = vec![b'x'; 4 * 1024 * 1024 + 1];
    let output = invoke(&input);
    assert_eq!(output.status.code(), Some(2));
    let response = response(&output);
    assert_eq!(response["error"]["code"], "input_too_large");
}
