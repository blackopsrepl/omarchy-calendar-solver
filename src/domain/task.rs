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
    /// Dependency depth used by SolverForge's native decreasing construction
    /// order. A lower depth is placed before its dependents.
    pub dependency_depth: i64,
    pub feasible_slot_ids: Vec<usize>,

    #[planning_variable(
        value_range_provider = "slots",
        allows_unassigned = true,
        candidate_values = "candidate_slots",
        construction_entity_order_key = "construction_entity_order"
    )]
    pub slot_id: Option<usize>,
}

/// Orders dependency roots before their dependents for SolverForge's
/// `first_fit_decreasing` construction heuristic. This is an entity-order
/// hook, not a second scheduling algorithm; SolverForge still selects every
/// value and evaluates every constraint.
pub fn construction_entity_order(
    _plan: &super::plan::SolverPlan,
    task: &SolverTask,
) -> i64 {
    -task.dependency_depth
}

/// Restricts SolverForge's scalar value range to the slots prepared for this
/// task. The solver still owns construction and local search; this hook only
/// describes each entity's legal planning values.
pub fn candidate_slots(
    plan: &super::plan::SolverPlan,
    entity_index: usize,
    _variable_index: usize,
) -> &[usize] {
    plan.tasks
        .get(entity_index)
        .map(|task| task.feasible_slot_ids.as_slice())
        .unwrap_or(&[])
}
