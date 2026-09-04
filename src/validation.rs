use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use chrono::{DateTime, FixedOffset};
use chrono_tz::Tz;

use crate::error::AppError;
use crate::model::{DeadlineKind, Event, Settings, Task, TaskState};
use crate::protocol::{Request, PROTOCOL_VERSION};

const WEEKDAYS: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

pub fn validate_request(request: &Request) -> Result<(), AppError> {
    if request.protocol_version != PROTOCOL_VERSION {
        return Err(AppError::protocol(
            "unsupported_protocol_version",
            "protocolVersion",
            format!("expected protocol version {PROTOCOL_VERSION}"),
        ));
    }
    if request.request_id.trim().is_empty() {
        return Err(AppError::validation(
            "required",
            "requestId",
            "requestId is required",
        ));
    }
    parse_timestamp(&request.now, "now")?;
    validate_settings(&request.settings)?;

    let mut event_ids = HashSet::new();
    for (index, event) in request.events.iter().enumerate() {
        validate_event(event, &format!("events[{index}]"))?;
        if !event_ids.insert(event.id.as_str()) {
            return Err(AppError::validation(
                "duplicate_id",
                format!("events[{index}].id"),
                "event id is duplicated",
            ));
        }
    }

    let mut task_ids = HashSet::new();
    for (index, task) in request.tasks.iter().enumerate() {
        validate_task(task, &format!("tasks[{index}]"))?;
        if !task_ids.insert(task.id.as_str()) {
            return Err(AppError::validation(
                "duplicate_id",
                format!("tasks[{index}].id"),
                "task id is duplicated",
            ));
        }
    }

    let mut dependencies = HashSet::new();
    let mut outgoing: HashMap<&str, Vec<&str>> = HashMap::new();
    for (index, dependency) in request.dependencies.iter().enumerate() {
        let path = format!("dependencies[{index}]");
        if dependency.from_task_id == dependency.to_task_id {
            return Err(AppError::validation(
                "self_dependency",
                path,
                "a task cannot depend on itself",
            ));
        }
        if !task_ids.contains(dependency.from_task_id.as_str())
            || !task_ids.contains(dependency.to_task_id.as_str())
        {
            return Err(AppError::validation(
                "missing_task",
                path,
                "dependency refers to a missing task",
            ));
        }
        let key = format!("{}\u{0}{}", dependency.from_task_id, dependency.to_task_id);
        if !dependencies.insert(key) {
            return Err(AppError::validation(
                "duplicate_dependency",
                path,
                "dependency is duplicated",
            ));
        }
        outgoing
            .entry(dependency.from_task_id.as_str())
            .or_default()
            .push(dependency.to_task_id.as_str());
    }
    let mut visiting = HashSet::new();
    let mut visited = HashSet::new();
    for task_id in &task_ids {
        if has_cycle(task_id, &outgoing, &mut visiting, &mut visited) {
            return Err(AppError::validation(
                "dependency_cycle",
                "dependencies",
                "dependencies must form a DAG",
            ));
        }
    }
    Ok(())
}

pub fn validate_settings(settings: &Settings) -> Result<(), AppError> {
    let timezone = Tz::from_str(&settings.timezone).map_err(|_| {
        AppError::validation(
            "invalid_timezone",
            "settings.timezone",
            "timezone must be an IANA name",
        )
    })?;
    let _ = timezone;
    range(settings.horizon_days, 1, 90, "settings.horizonDays")?;
    range(settings.slot_minutes, 5, 120, "settings.slotMinutes")?;
    range(settings.solve_seconds, 1, 120, "settings.solveSeconds")?;
    for (value, path) in [
        (settings.priority_low_weight, "settings.priorityLowWeight"),
        (
            settings.priority_normal_weight,
            "settings.priorityNormalWeight",
        ),
        (settings.priority_high_weight, "settings.priorityHighWeight"),
        (settings.low_outside_penalty, "settings.lowOutsidePenalty"),
        (
            settings.medium_outside_penalty,
            "settings.mediumOutsidePenalty",
        ),
        (settings.high_outside_penalty, "settings.highOutsidePenalty"),
        (settings.excess_high_penalty, "settings.excessHighPenalty"),
    ] {
        if value < 0 {
            return Err(AppError::validation(
                "negative_value",
                path,
                "value must be non-negative",
            ));
        }
    }
    if settings.high_streak_limit < 1 {
        return Err(AppError::validation(
            "out_of_range",
            "settings.highStreakLimit",
            "high streak limit must be at least one",
        ));
    }
    let clocks = [
        (&settings.low_window_start, "settings.lowWindowStart"),
        (&settings.low_window_end, "settings.lowWindowEnd"),
        (&settings.medium_window_start, "settings.mediumWindowStart"),
        (&settings.medium_window_end, "settings.mediumWindowEnd"),
        (&settings.high_window_start, "settings.highWindowStart"),
        (&settings.high_window_end, "settings.highWindowEnd"),
    ];
    for (clock, path) in clocks {
        parse_clock(clock, path)?;
    }
    validate_availability(settings)
}

fn validate_availability(settings: &Settings) -> Result<(), AppError> {
    for (day, windows) in &settings.availability {
        if !WEEKDAYS.contains(&day.as_str()) {
            return Err(AppError::validation(
                "unknown_weekday",
                format!("settings.availability.{day}"),
                "weekday is not recognized",
            ));
        }
        if windows.is_empty() {
            return Err(AppError::validation(
                "empty_day",
                format!("settings.availability.{day}"),
                "configured days need a window",
            ));
        }
        for (index, window) in windows.iter().enumerate() {
            let start = parse_clock(
                &window.start,
                format!("settings.availability.{day}[{index}].start"),
            )?;
            let end = parse_clock(
                &window.end,
                format!("settings.availability.{day}[{index}].end"),
            )?;
            if end <= start {
                return Err(AppError::validation(
                    "invalid_window",
                    format!("settings.availability.{day}[{index}]"),
                    "window must end after it starts",
                ));
            }
        }
    }
    if settings.availability.values().map(Vec::len).sum::<usize>() == 0 {
        return Err(AppError::validation(
            "availability_required",
            "settings.availability",
            "at least one weekly availability window is required",
        ));
    }
    Ok(())
}

fn range(value: u32, min: u32, max: u32, path: &str) -> Result<(), AppError> {
    if !(min..=max).contains(&value) {
        return Err(AppError::validation(
            "out_of_range",
            path,
            format!("value must be between {min} and {max}"),
        ));
    }
    Ok(())
}

pub fn parse_timestamp(
    value: &str,
    path: impl Into<String>,
) -> Result<DateTime<FixedOffset>, AppError> {
    DateTime::parse_from_rfc3339(value)
        .map_err(|_| AppError::validation("invalid_timestamp", path, "timestamp must be RFC 3339"))
}

pub fn parse_clock(value: &str, path: impl Into<String>) -> Result<u32, AppError> {
    let Some((hours, minutes)) = value.split_once(':') else {
        return Err(AppError::validation(
            "invalid_clock",
            path,
            "clock must be HH:MM",
        ));
    };
    if hours.len() != 2 || minutes.len() != 2 {
        return Err(AppError::validation(
            "invalid_clock",
            path,
            "clock must be normalized to HH:MM",
        ));
    }
    let hours = hours.parse::<u32>().ok();
    let minutes = minutes.parse::<u32>().ok();
    match (hours, minutes) {
        (Some(hours), Some(minutes)) if hours < 24 && minutes < 60 => Ok(hours * 60 + minutes),
        _ => Err(AppError::validation(
            "invalid_clock",
            path,
            "clock is outside the day",
        )),
    }
}

fn validate_event(event: &Event, path: &str) -> Result<(), AppError> {
    required(&event.id, format!("{path}.id"))?;
    required(&event.title, format!("{path}.title"))?;
    let start = parse_timestamp(&event.start_at, format!("{path}.startAt"))?;
    let end = parse_timestamp(&event.end_at, format!("{path}.endAt"))?;
    if end <= start {
        return Err(AppError::validation(
            "invalid_interval",
            path,
            "event end must be after start",
        ));
    }
    Tz::from_str(&event.timezone).map_err(|_| {
        AppError::validation(
            "invalid_timezone",
            format!("{path}.timezone"),
            "event timezone must be an IANA name",
        )
    })?;
    if event
        .rrule
        .as_deref()
        .is_some_and(|rule| rule.trim().is_empty())
    {
        return Err(AppError::validation(
            "invalid_rrule",
            format!("{path}.rrule"),
            "rrule cannot be empty",
        ));
    }
    Ok(())
}

fn validate_task(task: &Task, path: &str) -> Result<(), AppError> {
    required(&task.id, format!("{path}.id"))?;
    required(&task.title, format!("{path}.title"))?;
    if task.duration_minutes == 0 {
        return Err(AppError::validation(
            "invalid_duration",
            format!("{path}.durationMinutes"),
            "duration must be greater than zero",
        ));
    }
    if let Some(earliest) = &task.earliest_at {
        parse_timestamp(earliest, format!("{path}.earliestAt"))?;
    }
    match (&task.deadline_kind, &task.deadline_at) {
        (DeadlineKind::None, Some(_)) => {
            return Err(AppError::validation(
                "contradictory_deadline",
                path,
                "deadlineAt requires a hard or soft deadline",
            ))
        }
        (DeadlineKind::Hard | DeadlineKind::Soft, None) => {
            return Err(AppError::validation(
                "missing_deadline",
                format!("{path}.deadlineAt"),
                "deadlineAt is required for this deadline kind",
            ))
        }
        (_, Some(deadline)) => {
            parse_timestamp(deadline, format!("{path}.deadlineAt"))?;
        }
        _ => {}
    }
    if task.state != TaskState::Inbox && task.linked_event_id.is_none() {
        return Err(AppError::validation(
            "missing_linked_event",
            format!("{path}.linkedEventId"),
            "applied tasks require a linked event",
        ));
    }
    Ok(())
}

fn required(value: &str, path: impl Into<String>) -> Result<(), AppError> {
    if value.trim().is_empty() {
        Err(AppError::validation("required", path, "value is required"))
    } else {
        Ok(())
    }
}

fn has_cycle<'a>(
    node: &'a str,
    outgoing: &HashMap<&'a str, Vec<&'a str>>,
    visiting: &mut HashSet<&'a str>,
    visited: &mut HashSet<&'a str>,
) -> bool {
    if visiting.contains(node) {
        return true;
    }
    if visited.contains(node) {
        return false;
    }
    visiting.insert(node);
    if let Some(next) = outgoing.get(node) {
        for child in next {
            if has_cycle(child, outgoing, visiting, visited) {
                return true;
            }
        }
    }
    visiting.remove(node);
    visited.insert(node);
    false
}
