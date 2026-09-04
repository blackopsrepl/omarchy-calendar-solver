mod support;

use serde_json::json;

#[test]
fn proposal_contains_diagnostics_for_each_inbox_task() {
    let mut request = support::fixture();
    request["settings"]["horizonDays"] = json!(1);
    request["settings"]["availability"] = json!({
        "friday": [{"start": "09:00", "end": "10:00"}]
    });
    request["tasks"] = json!([support::task("fits", 60), support::task("does-not-fit", 60)]);
    let response = support::response(&support::invoke(&request));
    let items = response["proposal"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    for item in items {
        assert!(item.get("taskId").is_some());
        assert!(item.get("scheduled").is_some());
        assert!(item.get("explanation").is_some());
        assert!(item.get("diagnostics").is_some());
        assert!(item.get("busyBlockers").is_some());
        assert!(item.get("omittedBlockerCount").is_some());
    }
    assert_eq!(
        items[1]["diagnostics"]["outcome"],
        "feasible_but_not_selected"
    );
}

#[test]
fn derived_proposals_echo_the_input_revision_without_mutating_inputs() {
    let mut request = support::fixture();
    request["baseInputRevision"] = json!(42);
    request["tasks"] = json!([support::task("revision", 15)]);
    let response = support::response(&support::invoke(&request));
    assert_eq!(response["proposal"]["baseInputRevision"], 42);
    assert_eq!(response["proposal"]["status"], "ready");
    assert_eq!(request["tasks"][0]["state"], "inbox");
}

#[test]
fn recurrence_expansion_has_a_bounded_failure_mode() {
    let mut request = support::fixture();
    request["events"] = json!([{
        "id": "unbounded",
        "title": "Unbounded",
        "description": null,
        "startAt": "2026-09-04T09:00:00+02:00",
        "endAt": "2026-09-04T09:15:00+02:00",
        "timezone": "Europe/Rome",
        "allDay": false,
        "rrule": "FREQ=MINUTELY;COUNT=70000",
        "origin": "manual",
        "taskId": null,
        "proposalId": null,
        "createdAt": "2026-09-01T00:00:00Z",
        "updatedAt": "2026-09-01T00:00:00Z"
    }]);
    let output = support::invoke(&request);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        support::response(&output)["error"]["code"],
        "recurrence_limit"
    );
}
