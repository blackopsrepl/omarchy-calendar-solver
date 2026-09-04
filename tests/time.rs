mod support;

use serde_json::json;

#[test]
fn horizon_is_based_on_local_midnights_across_spring_forward() {
    let mut request = support::fixture();
    request["settings"]["timezone"] = json!("Europe/Rome");
    request["settings"]["horizonDays"] = json!(14);
    request["now"] = json!("2026-03-28T23:00:00Z");
    let response = support::response(&support::invoke(&request));
    assert_eq!(
        response["proposal"]["horizonStart"],
        "2026-03-29T00:00:00+01:00"
    );
}

#[test]
fn slot_alignment_uses_the_configured_local_clock() {
    let mut request = support::fixture();
    request["settings"]["timezone"] = json!("America/New_York");
    request["settings"]["slotMinutes"] = json!(45);
    request["settings"]["availability"] = json!({
        "friday": [{"start": "09:00", "end": "10:00"}]
    });
    request["tasks"] = json!([support::task("local-slot", 45)]);
    request["now"] = json!("2026-09-04T11:00:00Z");
    let response = support::response(&support::invoke(&request));
    let item = &response["proposal"]["items"][0];
    assert_eq!(item["scheduled"], true);
    assert_eq!(item["startAt"], "2026-09-04T09:00:00-04:00");
}

#[test]
fn recurring_busy_time_keeps_its_event_timezone_after_dst() {
    let mut request = support::fixture();
    request["settings"]["timezone"] = json!("Europe/Rome");
    request["settings"]["availability"] = json!({
        "monday": [{"start": "09:00", "end": "11:00"}]
    });
    request["now"] = json!("2026-10-25T00:00:00Z");
    request["events"] = json!([{
        "id": "dst-busy",
        "title": "DST busy",
        "description": null,
        "startAt": "2026-10-25T09:00:00+01:00",
        "endAt": "2026-10-25T10:00:00+01:00",
        "timezone": "Europe/Rome",
        "allDay": false,
        "rrule": "FREQ=DAILY;COUNT=2",
        "origin": "manual",
        "taskId": null,
        "proposalId": null,
        "createdAt": "2026-10-01T00:00:00Z",
        "updatedAt": "2026-10-01T00:00:00Z"
    }]);
    request["tasks"] = json!([support::task("after-dst", 60)]);
    let response = support::response(&support::invoke(&request));
    let item = &response["proposal"]["items"][0];
    assert_eq!(item["startAt"], "2026-10-26T10:00:00+01:00");
    assert_eq!(item["busyBlockers"][0], "dst-busy");
}
