use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use rrule::{RRule, Unvalidated};
use std::str::FromStr;

use crate::error::AppError;
use crate::model::Event;
use crate::time::PlanningHorizon;
use crate::validation::parse_timestamp;

pub const RECURRENCE_LIMIT: u16 = 65_535;

#[derive(Clone, Debug)]
pub struct BusyInterval {
    pub event_id: String,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl BusyInterval {
    pub fn overlaps(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> bool {
        self.start < end && start < self.end
    }
}

pub fn expand_events(
    events: &[Event],
    horizon: &PlanningHorizon,
) -> Result<Vec<BusyInterval>, AppError> {
    let mut intervals = Vec::new();
    for (index, event) in events.iter().enumerate() {
        let path = format!("events[{index}]");
        let timezone = event.timezone.parse::<Tz>().map_err(|_| {
            AppError::recurrence(
                "invalid_timezone",
                format!("{path}.timezone"),
                "event timezone must be an IANA name",
            )
        })?;
        let start =
            parse_timestamp(&event.start_at, format!("{path}.startAt"))?.with_timezone(&timezone);
        let end = parse_timestamp(&event.end_at, format!("{path}.endAt"))?.with_timezone(&timezone);
        let duration = end.with_timezone(&Utc) - start.with_timezone(&Utc);
        if event.rrule.is_none() {
            push_if_overlapping(
                &mut intervals,
                event,
                start.with_timezone(&Utc),
                end.with_timezone(&Utc),
                horizon,
            );
            continue;
        }

        let rule_text = event
            .rrule
            .as_deref()
            .unwrap_or_default()
            .trim()
            .strip_prefix("RRULE:")
            .unwrap_or_else(|| event.rrule.as_deref().unwrap_or_default().trim());
        let rule = RRule::<Unvalidated>::from_str(rule_text).map_err(|error| {
            AppError::recurrence("invalid_rrule", format!("{path}.rrule"), error.to_string())
        })?;
        let recurrence_timezone: rrule::Tz = timezone.into();
        let recurrence_start = start.with_timezone(&recurrence_timezone);
        let set = rule.build(recurrence_start).map_err(|error| {
            AppError::recurrence("invalid_rrule", format!("{path}.rrule"), error.to_string())
        })?;
        let filter_start = horizon
            .start
            .with_timezone(&recurrence_timezone)
            .checked_sub_signed(duration)
            .ok_or_else(|| AppError::Internal("recurrence range underflowed".to_string()))?;
        let filter_end = horizon.end.with_timezone(&recurrence_timezone);
        let result = set
            .after(filter_start)
            .before(filter_end)
            .all(RECURRENCE_LIMIT);
        if result.limited {
            return Err(AppError::recurrence(
                "recurrence_limit",
                format!("{path}.rrule"),
                format!("recurrence expansion exceeded {RECURRENCE_LIMIT} occurrences"),
            ));
        }
        for occurrence in result.dates {
            let occurrence_start = occurrence.with_timezone(&Utc);
            push_if_overlapping(
                &mut intervals,
                event,
                occurrence_start,
                occurrence_start + duration,
                horizon,
            );
        }
    }
    intervals.sort_by_key(|interval| (interval.start, interval.event_id.clone()));
    Ok(intervals)
}

fn push_if_overlapping(
    intervals: &mut Vec<BusyInterval>,
    event: &Event,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    horizon: &PlanningHorizon,
) {
    let horizon_start = horizon.start.with_timezone(&Utc);
    let horizon_end = horizon.end.with_timezone(&Utc);
    if start < horizon_end && horizon_start < end {
        intervals.push(BusyInterval {
            event_id: event.id.clone(),
            start,
            end,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EventOrigin, Settings};
    use crate::time::build_horizon;
    use std::collections::BTreeMap;

    fn settings() -> Settings {
        Settings {
            timezone: "Europe/Rome".to_string(),
            availability: BTreeMap::from([(
                "sunday".to_string(),
                vec![crate::model::AvailabilityWindow {
                    start: "00:00".to_string(),
                    end: "23:00".to_string(),
                }],
            )]),
            horizon_days: 14,
            slot_minutes: 15,
            solve_seconds: 1,
            priority_low_weight: 1,
            priority_normal_weight: 5,
            priority_high_weight: 25,
            cognitive_enabled: false,
            low_window_start: "00:00".to_string(),
            low_window_end: "00:00".to_string(),
            low_outside_penalty: 0,
            medium_window_start: "00:00".to_string(),
            medium_window_end: "00:00".to_string(),
            medium_outside_penalty: 0,
            high_window_start: "00:00".to_string(),
            high_window_end: "00:00".to_string(),
            high_outside_penalty: 0,
            high_streak_limit: 1,
            recovery_minutes: 30,
            excess_high_penalty: 60,
        }
    }

    #[test]
    fn recurring_event_is_expanded_in_event_timezone() {
        let event = Event {
            id: "busy".to_string(),
            title: "Daily busy".to_string(),
            description: None,
            start_at: "2026-03-28T09:00:00+01:00".to_string(),
            end_at: "2026-03-28T10:00:00+01:00".to_string(),
            timezone: "Europe/Rome".to_string(),
            all_day: false,
            rrule: Some("FREQ=DAILY;COUNT=4".to_string()),
            origin: EventOrigin::Manual,
            task_id: None,
            proposal_id: None,
            created_at: "2026-03-01T00:00:00Z".to_string(),
            updated_at: "2026-03-01T00:00:00Z".to_string(),
        };
        let horizon = build_horizon(&settings(), "2026-03-28T12:00:00Z").unwrap();
        let intervals = expand_events(&[event], &horizon).unwrap();
        assert_eq!(intervals.len(), 4);
        assert_eq!(intervals[2].start.to_rfc3339(), "2026-03-30T07:00:00+00:00");
    }
}
