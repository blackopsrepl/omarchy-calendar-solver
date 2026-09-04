mod support;

use serde_json::json;

#[test]
fn fixture_is_a_valid_first_run_request() {
    let output = support::invoke(&support::fixture());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response = support::response(&output);
    assert_eq!(response["ok"], true);
    assert_eq!(response["requestId"], "fixture-request");
}

#[test]
fn settings_ranges_and_timezone_are_reported_by_field() {
    let mut request = support::fixture();
    request["settings"]["horizonDays"] = json!(0);
    let output = support::invoke(&request);
    assert_eq!(output.status.code(), Some(2));
    let response = support::response(&output);
    assert_eq!(response["error"]["code"], "out_of_range");
    assert_eq!(response["error"]["fieldPath"], "settings.horizonDays");

    request = support::fixture();
    request["settings"]["timezone"] = json!("Mars/Olympus");
    let response = support::response(&support::invoke(&request));
    assert_eq!(response["error"]["code"], "invalid_timezone");
    assert_eq!(response["error"]["fieldPath"], "settings.timezone");
}

#[test]
fn empty_and_unknown_availability_are_rejected() {
    let mut request = support::fixture();
    request["settings"]["availability"] = json!({});
    let response = support::response(&support::invoke(&request));
    assert_eq!(response["error"]["code"], "availability_required");

    request = support::fixture();
    request["settings"]["availability"] = json!({
        "friday": [{"start": "09:00", "end": "17:00"}],
        "funday": [{"start": "09:00", "end": "17:00"}]
    });
    let response = support::response(&support::invoke(&request));
    assert_eq!(response["error"]["code"], "unknown_weekday");
}
