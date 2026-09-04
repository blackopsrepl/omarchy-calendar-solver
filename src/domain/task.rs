use solverforge::prelude::*;
use std::collections::BTreeMap;

#[planning_entity]
pub struct SolverTask {
    #[planning_id]
    pub id: String,
    pub duration_minutes: i64,
    pub priority_weight: i64,
    pub deadline_lateness_by_slot: BTreeMap<usize, i64>,
    pub cognitive_penalty_by_slot: BTreeMap<usize, i64>,
    pub start_minute_by_slot: BTreeMap<usize, i64>,
    pub recovery_minutes: i64,
    pub recovery_penalty: i64,
    pub high_cognitive_load: bool,
    pub predecessor_ids: Vec<String>,
    pub feasible_slot_ids: Vec<usize>,

    #[planning_variable(
        value_range_provider = "slots",
        allows_unassigned = true,
        construction_entity_order_key = "construction_entity_order"
    )]
    pub slot_id: Option<usize>,
}

pub fn construction_entity_order(
    _plan: &super::plan::SolverPlan,
    task: &SolverTask,
) -> i64 {
    -task.priority_weight * 1_000_000 + task.duration_minutes
}
