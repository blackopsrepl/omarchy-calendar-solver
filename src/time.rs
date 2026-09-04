use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike, Weekday};
use chrono_tz::Tz;

use crate::error::AppError;
use crate::model::Settings;
use crate::validation::{parse_clock, parse_timestamp};

#[derive(Clone, Debug)]
pub struct PlanningHorizon {
    pub timezone: Tz,
    pub now: DateTime<Tz>,
    pub start: DateTime<Tz>,
    pub end: DateTime<Tz>,
}

pub fn build_horizon(settings: &Settings, now: &str) -> Result<PlanningHorizon, AppError> {
    let timezone = settings.timezone.parse::<Tz>().map_err(|_| {
        AppError::validation(
            "invalid_timezone",
            "settings.timezone",
            "timezone must be an IANA name",
        )
    })?;
    let now = parse_timestamp(now, "now")?.with_timezone(&timezone);
    let start_date = now.date_naive();
    let end_date = start_date
        .checked_add_signed(Duration::days(i64::from(settings.horizon_days)))
        .ok_or_else(|| AppError::Internal("planning horizon overflowed".to_string()))?;
    let start = at_local_midnight(timezone, start_date, "horizonStart")?;
    let end = at_local_midnight(timezone, end_date, "horizonEnd")?;
    Ok(PlanningHorizon {
        timezone,
        now,
        start,
        end,
    })
}

pub fn at_local_midnight(
    timezone: Tz,
    date: NaiveDate,
    path: impl Into<String>,
) -> Result<DateTime<Tz>, AppError> {
    let naive = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::Internal("invalid local midnight".to_string()))?;
    at_local(timezone, naive, path)
}

pub fn at_local(
    timezone: Tz,
    naive: NaiveDateTime,
    path: impl Into<String>,
) -> Result<DateTime<Tz>, AppError> {
    match timezone.from_local_datetime(&naive) {
        chrono::LocalResult::Single(value) => Ok(value),
        // Choosing the earlier instant makes repeated fall-back wall times
        // deterministic while still retaining the local timezone in output.
        chrono::LocalResult::Ambiguous(earlier, _) => Ok(earlier),
        chrono::LocalResult::None => Err(AppError::validation(
            "invalid_local_time",
            path,
            "local time does not exist in the configured timezone",
        )),
    }
}

pub fn weekday_name(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Mon => "monday",
        Weekday::Tue => "tuesday",
        Weekday::Wed => "wednesday",
        Weekday::Thu => "thursday",
        Weekday::Fri => "friday",
        Weekday::Sat => "saturday",
        Weekday::Sun => "sunday",
    }
}

pub fn clock_minutes(value: DateTime<Tz>) -> u32 {
    value.hour() * 60 + value.minute()
}

pub fn is_available_at(settings: &Settings, value: DateTime<Tz>) -> bool {
    let day = weekday_name(value.weekday());
    let Some(windows) = settings.availability.get(day) else {
        return false;
    };
    let minute = clock_minutes(value);
    windows.iter().any(|window| {
        let Ok(start) = parse_clock(&window.start, "availability.start") else {
            return false;
        };
        let Ok(end) = parse_clock(&window.end, "availability.end") else {
            return false;
        };
        start <= minute && minute < end
    })
}

pub fn fits_availability(settings: &Settings, start: DateTime<Tz>, end: DateTime<Tz>) -> bool {
    if end <= start || start.second() != 0 {
        return false;
    }
    let mut cursor = start;
    while cursor < end {
        if !is_available_at(settings, cursor) {
            return false;
        }
        cursor += Duration::minutes(1);
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AvailabilityWindow, Settings};
    use std::collections::BTreeMap;

    fn settings(timezone: &str) -> Settings {
        Settings {
            timezone: timezone.to_string(),
            availability: BTreeMap::from([(
                "sunday".to_string(),
                vec![AvailabilityWindow {
                    start: "01:00".to_string(),
                    end: "04:00".to_string(),
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
    fn horizon_uses_local_midnights() {
        let horizon = build_horizon(&settings("Europe/Rome"), "2026-03-28T23:00:00Z").unwrap();
        assert_eq!(horizon.start.to_rfc3339(), "2026-03-29T00:00:00+01:00");
        assert_eq!(horizon.end.to_rfc3339(), "2026-04-12T00:00:00+02:00");
    }

    #[test]
    fn nonexistent_local_time_is_rejected() {
        let timezone = "Europe/Rome".parse::<Tz>().unwrap();
        let local = NaiveDate::from_ymd_opt(2026, 3, 29)
            .unwrap()
            .and_hms_opt(2, 30, 0)
            .unwrap();
        assert!(at_local(timezone, local, "test").is_err());
    }
}
