#![allow(dead_code)]

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub timezone: String,
    pub availability: BTreeMap<String, Vec<AvailabilityWindow>>,
    pub horizon_days: u32,
    pub slot_minutes: u32,
    pub solve_seconds: u32,
    pub priority_low_weight: i64,
    pub priority_normal_weight: i64,
    pub priority_high_weight: i64,
    pub cognitive_enabled: bool,
    pub low_window_start: String,
    pub low_window_end: String,
    pub low_outside_penalty: i64,
    pub medium_window_start: String,
    pub medium_window_end: String,
    pub medium_outside_penalty: i64,
    pub high_window_start: String,
    pub high_window_end: String,
    pub high_outside_penalty: i64,
    pub high_streak_limit: u32,
    pub recovery_minutes: u32,
    pub excess_high_penalty: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AvailabilityWindow {
    pub start: String,
    pub end: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Event {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub start_at: String,
    pub end_at: String,
    pub timezone: String,
    pub all_day: bool,
    pub rrule: Option<String>,
    pub origin: EventOrigin,
    pub task_id: Option<String>,
    pub proposal_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EventOrigin {
    Manual,
    Planner,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub duration_minutes: u32,
    pub priority: Priority,
    pub cognitive_load: CognitiveLoad,
    pub earliest_at: Option<String>,
    pub deadline_kind: DeadlineKind,
    pub deadline_at: Option<String>,
    pub state: TaskState,
    pub linked_event_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Low,
    Normal,
    High,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "lowercase")]
pub enum CognitiveLoad {
    Low,
    Medium,
    High,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeadlineKind {
    None,
    Hard,
    Soft,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Inbox,
    Applied,
    MissingEvent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Dependency {
    pub from_task_id: String,
    pub to_task_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    pub hard: i64,
    pub medium: i64,
    pub soft: i64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticOutcome {
    Scheduled,
    NoHardFeasibleSlot,
    FeasibleButNotSelected,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalItem {
    pub task_id: String,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub scheduled: bool,
    pub cognitive_penalty: i64,
    pub fatigue_penalty: i64,
    pub explanation: String,
    pub diagnostics: ProposalDiagnostics,
    pub busy_blockers: Vec<String>,
    pub omitted_blocker_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalDiagnostics {
    pub outcome: DiagnosticOutcome,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    pub id: String,
    pub status: String,
    pub base_input_revision: u64,
    pub request_id: String,
    pub horizon_start: String,
    pub horizon_days: u32,
    pub timezone: String,
    pub score: Score,
    pub created_at: String,
    pub items: Vec<ProposalItem>,
    pub applicability_reasons: Vec<String>,
}
