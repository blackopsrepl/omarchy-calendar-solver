use std::process::{Command, Stdio};

use serde_json::{json, Value};

fn settings(start: &str, end: &str) -> Value {
    json!({
        "timezone": "Europe/Rome",
        "availability": {"friday": [{"start": start, "end": end}]},
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

fn task(id: &str, duration: u32, priority: &str, load: &str) -> Value {
    json!({
        "id": id,
        "title": id,
        "durationMinutes": duration,
        "priority": priority,
        "cognitiveLoad": load,
        "earliestAt": null,
        "deadlineKind": "none",
        "deadlineAt": null,
        "state": "inbox",
        "linkedEventId": null,
        "createdAt": "2026-09-04T07:00:00Z",
        "updatedAt": "2026-09-04T07:00:00Z"
    })
}

fn event(id: &str, start: &str, end: &str, origin: &str, task_id: Option<&str>) -> Value {
    json!({
        "id": id,
        "title": id,
        "description": null,
        "startAt": start,
        "endAt": end,
        "timezone": "Europe/Rome",
        "allDay": false,
        "rrule": null,
        "origin": origin,
        "taskId": task_id,
        "proposalId": null,
        "createdAt": "2026-09-04T07:00:00Z",
        "updatedAt": "2026-09-04T07:00:00Z"
    })
}

fn request(
    settings: Value,
    tasks: Vec<Value>,
    events: Vec<Value>,
    dependencies: Vec<Value>,
) -> Value {
    json!({
        "protocolVersion": 1,
        "requestId": "constraint-test",
        "baseInputRevision": 9,
        "now": "2026-09-04T08:00:00+02:00",
        "settings": settings,
        "events": events,
        "tasks": tasks,
        "dependencies": dependencies
    })
}

fn solve(input: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omarchy-calendar-solver"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("solver process should start");
    let input = serde_json::to_vec(&input).unwrap();
    use std::io::Write;
    child.stdin.take().unwrap().write_all(&input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "solver stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn items(response: &Value) -> &Vec<Value> {
    response["proposal"]["items"].as_array().unwrap()
}

#[test]
fn busy_events_are_excluded_and_reported() {
    let response = solve(request(
        settings("09:00", "12:00"),
        vec![task("work", 60, "normal", "medium")],
        vec![event(
            "meeting",
            "2026-09-04T09:00:00+02:00",
            "2026-09-04T10:00:00+02:00",
            "manual",
            None,
        )],
        vec![],
    ));
    assert_eq!(items(&response)[0]["startAt"], "2026-09-04T10:00:00+02:00");
}

#[test]
fn priority_weight_prefers_high_priority_when_capacity_is_limited() {
    let mut limited_settings = settings("09:00", "10:00");
    limited_settings["horizonDays"] = json!(1);
    let response = solve(request(
        limited_settings,
        vec![
            task("low", 60, "low", "low"),
            task("high", 60, "high", "high"),
        ],
        vec![],
        vec![],
    ));
    let high = items(&response)
        .iter()
        .find(|item| item["taskId"] == "high")
        .unwrap();
    let low = items(&response)
        .iter()
        .find(|item| item["taskId"] == "low")
        .unwrap();
    assert_eq!(high["scheduled"], true);
    assert_eq!(low["scheduled"], false);
}

#[test]
fn dependencies_schedule_predecessor_before_dependent() {
    let response = solve(request(
        settings("09:00", "12:00"),
        vec![
            task("dependent", 60, "normal", "low"),
            task("predecessor", 60, "normal", "low"),
        ],
        vec![],
        vec![json!({"fromTaskId": "predecessor", "toTaskId": "dependent"})],
    ));
    let predecessor = items(&response)
        .iter()
        .find(|item| item["taskId"] == "predecessor")
        .unwrap();
    let dependent = items(&response)
        .iter()
        .find(|item| item["taskId"] == "dependent")
        .unwrap();
    assert_eq!(predecessor["startAt"], "2026-09-04T09:00:00+02:00");
    assert_eq!(dependent["startAt"], "2026-09-04T10:00:00+02:00");
}

#[test]
fn hard_deadline_can_leave_a_task_unscheduled() {
    let mut hard_task = task("hard", 120, "high", "high");
    hard_task["deadlineKind"] = json!("hard");
    hard_task["deadlineAt"] = json!("2026-09-04T10:00:00+02:00");
    let response = solve(request(
        settings("09:00", "12:00"),
        vec![hard_task],
        vec![],
        vec![],
    ));
    assert_eq!(items(&response)[0]["scheduled"], false);
    assert_eq!(
        items(&response)[0]["diagnostics"]["outcome"],
        "no_hard_feasible_slot"
    );
}

#[test]
fn applied_high_load_task_contributes_recovery_penalty() {
    let mut applied = task("applied", 60, "high", "high");
    applied["state"] = json!("applied");
    applied["linkedEventId"] = json!("applied-event");
    let response = solve(request(
        settings("09:00", "12:00"),
        vec![applied, task("next", 60, "high", "high")],
        vec![event(
            "applied-event",
            "2026-09-04T09:00:00+02:00",
            "2026-09-04T10:00:00+02:00",
            "planner",
            Some("applied"),
        )],
        vec![],
    ));
    let next = items(&response)
        .iter()
        .find(|item| item["taskId"] == "next")
        .unwrap();
    assert_eq!(next["startAt"], "2026-09-04T10:00:00+02:00");
    assert_eq!(next["fatiguePenalty"], 60);
}
