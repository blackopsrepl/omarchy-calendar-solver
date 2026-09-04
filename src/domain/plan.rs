use solverforge::prelude::*;
use solverforge::stream::ConstraintFactory;
use solverforge::SolverConfig;

use super::{SolverFixedTask, SolverSlot, SolverTask};

#[planning_solution(
    constraints = "constraints",
    config = "solver_config_for_plan",
    solver_toml = "../../planner-solver.toml"
)]
pub struct SolverPlan {
    #[problem_fact_collection]
    pub slots: Vec<SolverSlot>,
    #[planning_entity_collection]
    pub tasks: Vec<SolverTask>,
    #[problem_fact_collection]
    pub fixed_tasks: Vec<SolverFixedTask>,
    #[planning_score]
    pub score: Option<HardMediumSoftScore>,
    pub solve_seconds: u64,
}

fn solver_config_for_plan(plan: &SolverPlan, config: SolverConfig) -> SolverConfig {
    config.with_termination_seconds(plan.solve_seconds)
}

fn start_minute(task: &SolverTask) -> Option<i64> {
    task.slot_id
        .and_then(|slot| task.start_minute_by_slot.get(&slot).copied())
}

fn constraints() -> impl ConstraintSet<SolverPlan, HardMediumSoftScore> {
    let factory = ConstraintFactory::<SolverPlan, HardMediumSoftScore>::new();
    (
        factory
            .clone()
            .for_each(SolverPlan::tasks())
            .unassigned()
            .penalize(|task: &SolverTask| HardMediumSoftScore::of_medium(task.priority_weight))
            .named("Prioritize scheduled inbox tasks"),
        factory
            .clone()
            .for_each(SolverPlan::tasks())
            .join((
                SolverPlan::slots(),
                solverforge::stream::joiner::equal_bi(
                    |task: &SolverTask| task.slot_id,
                    |slot: &SolverSlot| Some(slot.id),
                ),
            ))
            .filter(|task: &SolverTask, slot: &SolverSlot| {
                !task.feasible_slot_ids.contains(&slot.id)
            })
            .penalize(HardMediumSoftScore::of_hard(1))
            .named("Only feasible slots"),
        factory
            .clone()
            .for_each(SolverPlan::tasks())
            .join(solverforge::stream::joiner::equal(|_: &SolverTask| 0_u8))
            .filter(|left: &SolverTask, right: &SolverTask| {
                if left.id == right.id {
                    return false;
                }
                let (Some(left_start), Some(right_start)) = (start_minute(left), start_minute(right))
                else {
                    return false;
                };
                left_start < right_start + right.duration_minutes
                    && right_start < left_start + left.duration_minutes
            })
            .penalize(HardMediumSoftScore::of_hard(1))
            .named("No task overlap"),
        factory
            .clone()
            .for_each(SolverPlan::tasks())
            .join(solverforge::stream::joiner::equal(|_: &SolverTask| 0_u8))
            .filter(|left: &SolverTask, right: &SolverTask| {
                if left.id == right.id || !left.predecessor_ids.contains(&right.id) {
                    return false;
                }
                match (start_minute(left), start_minute(right)) {
                    (Some(left_start), Some(right_start)) => {
                        left_start < right_start + right.duration_minutes
                    }
                    (Some(_), None) => true,
                    _ => false,
                }
            })
            .penalize(HardMediumSoftScore::of_hard(1))
            .named("Task dependencies"),
        factory
            .clone()
            .for_each(SolverPlan::tasks())
            .filter(|task: &SolverTask| task.slot_id.is_some())
            .penalize(|task: &SolverTask| {
                HardMediumSoftScore::of_medium(
                    task.slot_id
                        .and_then(|slot| task.deadline_lateness_by_slot.get(&slot).copied())
                        .unwrap_or_default(),
                )
            })
            .named("Soft deadlines"),
        factory
            .clone()
            .for_each(SolverPlan::tasks())
            .filter(|task: &SolverTask| task.slot_id.is_some())
            .penalize(|task: &SolverTask| {
                HardMediumSoftScore::of_soft(
                    task.slot_id
                        .and_then(|slot| task.cognitive_penalty_by_slot.get(&slot).copied())
                        .unwrap_or_default(),
                )
            })
            .named("Cognitive timing"),
        factory
            .clone()
            .for_each(SolverPlan::tasks())
            .join(solverforge::stream::joiner::equal(|_: &SolverTask| 0_u8))
            .filter(|left: &SolverTask, right: &SolverTask| {
                if left.id >= right.id || !left.high_cognitive_load || !right.high_cognitive_load {
                    return false;
                }
                let (Some(left_start), Some(right_start)) = (start_minute(left), start_minute(right))
                else {
                    return false;
                };
                let gap = if left_start >= right_start {
                    left_start - (right_start + right.duration_minutes)
                } else {
                    right_start - (left_start + left.duration_minutes)
                };
                gap >= 0 && gap < left.recovery_minutes
            })
            .penalize(|left: &SolverTask, _right: &SolverTask| {
                HardMediumSoftScore::of_soft(left.recovery_penalty)
            })
            .named("High cognitive-load recovery"),
        factory
            .clone()
            .for_each(SolverPlan::tasks())
            .join((
                factory.clone().for_each(SolverPlan::fixed_tasks()),
                |_task: &SolverTask, _fixed: &SolverFixedTask| true,
            ))
            .filter(|task: &SolverTask, fixed: &SolverFixedTask| {
                if !task.high_cognitive_load || !fixed.high_cognitive_load {
                    return false;
                }
                let Some(task_start) = start_minute(task) else {
                    return false;
                };
                let task_end = task_start + task.duration_minutes;
                let gap = if task_start >= fixed.start_minute {
                    task_start - fixed.end_minute
                } else {
                    fixed.start_minute - task_end
                };
                gap >= 0 && gap < task.recovery_minutes
            })
            .penalize(|task: &SolverTask, _fixed: &SolverFixedTask| {
                HardMediumSoftScore::of_soft(task.recovery_penalty)
            })
            .named("Applied high cognitive-load recovery"),
    )
}

pub fn score(plan: &SolverPlan) -> HardMediumSoftScore {
    constraints().evaluate_all(plan)
}
