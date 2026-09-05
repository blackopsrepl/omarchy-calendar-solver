use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

use chrono::{DateTime, Duration, Timelike, Utc};
use solverforge::{SolverEvent, SolverManager};
use uuid::Uuid;

use crate::domain::{score, SolverFixedTask, SolverPlan, SolverSlot, SolverTask};
use crate::error::AppError;
use crate::model::{
    CognitiveLoad, DeadlineKind, Dependency, DiagnosticOutcome, Priority, Proposal,
    ProposalDiagnostics, ProposalItem, Score, Settings, Task, TaskState,
};
use crate::protocol::Request;
use crate::recurrence::{expand_events, BusyInterval};
use crate::time::{build_horizon, fits_availability, is_available_at, PlanningHorizon};
use crate::validation::parse_timestamp;

#[derive(Clone, Debug)]
struct Slot {
    id: usize,
    start: DateTime<chrono_tz::Tz>,
    end: DateTime<chrono_tz::Tz>,
}

#[derive(Clone, Debug)]
struct Candidate {
    slot: Slot,
    soft_deadline_penalty: i64,
    cognitive_penalty: i64,
}

#[derive(Debug)]
struct PreparedProblem {
    horizon: PlanningHorizon,
    slots: Vec<Slot>,
    candidates: BTreeMap<String, Vec<Candidate>>,
    inbox_tasks: Vec<Task>,
    fixed_tasks: Vec<SolverFixedTask>,
    busy_blockers: BTreeMap<String, Vec<String>>,
}

static SOLVER_MANAGER: OnceLock<SolverManager<SolverPlan>> = OnceLock::new();

pub fn solve(request: &Request) -> Result<Proposal, AppError> {
    let problem = prepare(request)?;
    let plan = make_plan(request, &problem);
    let solved = if plan.tasks.is_empty() {
        plan
    } else {
        run_solver(plan, request.settings.solve_seconds)?
    };
    Ok(make_proposal(request, &problem, &solved))
}

fn prepare(request: &Request) -> Result<PreparedProblem, AppError> {
    let horizon = build_horizon(&request.settings, &request.now)?;
    let busy = expand_events(&request.events, &horizon)?;
    let slots = build_slots(&request.settings, &horizon);
    let mut inbox_tasks: Vec<_> = request
        .tasks
        .iter()
        .filter(|task| task.state == TaskState::Inbox)
        .cloned()
        .collect();
    inbox_tasks.sort_by_key(|task| {
        (
            -priority_weight(&request.settings, &task.priority),
            task.deadline_at.clone().unwrap_or_default(),
            task.id.clone(),
        )
    });
    let fixed_tasks = build_fixed_tasks(request, &horizon);
    let fixed_end_by_task_id: HashMap<_, _> = fixed_tasks
        .iter()
        .map(|task| (task.id.as_str(), task.end_minute))
        .collect();
    let mut candidates = BTreeMap::new();
    let mut busy_blockers = BTreeMap::new();
    for task in &inbox_tasks {
        let (task_candidates, blockers) = build_candidates(
            task,
            &request.settings,
            &horizon,
            &slots,
            &busy,
            &request.dependencies,
            &fixed_end_by_task_id,
        )?;
        candidates.insert(task.id.clone(), task_candidates);
        busy_blockers.insert(task.id.clone(), blockers);
    }
    Ok(PreparedProblem {
        horizon,
        slots,
        candidates,
        inbox_tasks,
        fixed_tasks,
        busy_blockers,
    })
}

fn build_slots(settings: &Settings, horizon: &PlanningHorizon) -> Vec<Slot> {
    let mut slots = Vec::new();
    let mut cursor = horizon.start.with_timezone(&Utc);
    let end = horizon.end.with_timezone(&Utc);
    while cursor < end {
        let local = cursor.with_timezone(&horizon.timezone);
        let local_minute = local.hour() * 60 + local.minute();
        if local_minute.is_multiple_of(settings.slot_minutes) && is_available_at(settings, local) {
            let local_end = local + Duration::minutes(i64::from(settings.slot_minutes));
            slots.push(Slot {
                id: slots.len(),
                start: local,
                end: local_end,
            });
        }
        cursor += Duration::minutes(1);
    }
    slots
}

fn build_candidates(
    task: &Task,
    settings: &Settings,
    horizon: &PlanningHorizon,
    slots: &[Slot],
    busy: &[BusyInterval],
    dependencies: &[Dependency],
    fixed_end_by_task_id: &HashMap<&str, i64>,
) -> Result<(Vec<Candidate>, Vec<String>), AppError> {
    let earliest = match task.earliest_at.as_deref() {
        Some(value) => Some(
            parse_timestamp(value, format!("task {} earliestAt", task.id))?
                .with_timezone(&horizon.timezone),
        ),
        None => None,
    };
    let deadline = match task.deadline_at.as_deref() {
        Some(value) => Some(
            parse_timestamp(value, format!("task {} deadlineAt", task.id))?
                .with_timezone(&horizon.timezone),
        ),
        None => None,
    };
    let duration = Duration::minutes(i64::from(task.duration_minutes));
    let predecessor_fixed_end = dependencies
        .iter()
        .filter(|dependency| dependency.to_task_id == task.id)
        .filter_map(|dependency| fixed_end_by_task_id.get(dependency.from_task_id.as_str()))
        .copied()
        .max();
    let mut candidates = Vec::new();
    let mut blockers = Vec::new();
    for slot in slots {
        let start = slot.start;
        let end = start + duration;
        if start < horizon.now || end > horizon.end {
            continue;
        }
        if predecessor_fixed_end.is_some_and(|value| start.timestamp().div_euclid(60) < value) {
            continue;
        }
        if earliest.is_some_and(|value| start < value) {
            continue;
        }
        if task.deadline_kind == DeadlineKind::Hard && deadline.is_some_and(|value| end > value) {
            continue;
        }
        if !fits_availability(settings, start, end) {
            continue;
        }
        let start_utc = start.with_timezone(&Utc);
        let end_utc = end.with_timezone(&Utc);
        let mut blocked = false;
        for event in busy {
            if event.overlaps(start_utc, end_utc) {
                blocked = true;
                if !blockers.contains(&event.event_id) {
                    blockers.push(event.event_id.clone());
                }
            }
        }
        if blocked {
            continue;
        }
        let soft_deadline_penalty = if task.deadline_kind == DeadlineKind::Soft {
            deadline
                .map(|value| (end - value).num_minutes().max(0))
                .unwrap_or_default()
        } else {
            0
        };
        candidates.push(Candidate {
            slot: slot.clone(),
            soft_deadline_penalty,
            cognitive_penalty: cognitive_penalty(settings, task, start, end),
        });
    }
    Ok((candidates, blockers))
}

fn cognitive_penalty(
    settings: &Settings,
    task: &Task,
    start: DateTime<chrono_tz::Tz>,
    end: DateTime<chrono_tz::Tz>,
) -> i64 {
    if !settings.cognitive_enabled {
        return 0;
    }
    let (window_start, window_end, penalty) = match task.cognitive_load {
        CognitiveLoad::Low => (
            &settings.low_window_start,
            &settings.low_window_end,
            settings.low_outside_penalty,
        ),
        CognitiveLoad::Medium => (
            &settings.medium_window_start,
            &settings.medium_window_end,
            settings.medium_outside_penalty,
        ),
        CognitiveLoad::High => (
            &settings.high_window_start,
            &settings.high_window_end,
            settings.high_outside_penalty,
        ),
    };
    let Ok(window_start) = crate::validation::parse_clock(window_start, "cognitive window") else {
        return 0;
    };
    let Ok(window_end) = crate::validation::parse_clock(window_end, "cognitive window") else {
        return 0;
    };
    if window_start == window_end || penalty == 0 {
        return 0;
    }
    let mut total = 0;
    let mut cursor = start;
    while cursor < end {
        let minute = cursor.hour() * 60 + cursor.minute();
        if !(window_start <= minute && minute < window_end) {
            total += penalty;
        }
        cursor += Duration::minutes(1);
    }
    total
}

fn build_fixed_tasks(request: &Request, horizon: &PlanningHorizon) -> Vec<SolverFixedTask> {
    let mut fixed = Vec::new();
    for task in &request.tasks {
        if task.state != TaskState::Applied {
            continue;
        }
        let Some(event_id) = task.linked_event_id.as_deref() else {
            continue;
        };
        let Some(event) = request.events.iter().find(|event| event.id == event_id) else {
            continue;
        };
        let Ok(start) = parse_timestamp(&event.start_at, "applied event start") else {
            continue;
        };
        let Ok(end) = parse_timestamp(&event.end_at, "applied event end") else {
            continue;
        };
        let start = start.with_timezone(&horizon.timezone);
        let end = end.with_timezone(&horizon.timezone);
        fixed.push(SolverFixedTask {
            id: task.id.clone(),
            start_minute: start.timestamp().div_euclid(60),
            end_minute: end.timestamp().div_euclid(60),
            high_cognitive_load: task.cognitive_load == CognitiveLoad::High,
        });
    }
    fixed
}

fn make_plan(request: &Request, problem: &PreparedProblem) -> SolverPlan {
    let predecessor_map = predecessor_map(&request.dependencies);
    let dependency_depths = dependency_depths(&problem.inbox_tasks, &predecessor_map);
    let tasks = problem
        .inbox_tasks
        .iter()
        .map(|task| {
            let candidates = problem
                .candidates
                .get(&task.id)
                .cloned()
                .unwrap_or_default();
            SolverTask {
                id: task.id.clone(),
                duration_minutes: i64::from(task.duration_minutes),
                priority_weight: priority_weight(&request.settings, &task.priority),
                deadline_lateness_by_slot: candidates
                    .iter()
                    .map(|candidate| (candidate.slot.id, candidate.soft_deadline_penalty))
                    .collect(),
                cognitive_penalty_by_slot: candidates
                    .iter()
                    .map(|candidate| (candidate.slot.id, candidate.cognitive_penalty))
                    .collect(),
                start_minute_by_slot: candidates
                    .iter()
                    .map(|candidate| {
                        (
                            candidate.slot.id,
                            candidate.slot.start.timestamp().div_euclid(60),
                        )
                    })
                    .collect(),
                recovery_minutes: i64::from(request.settings.recovery_minutes),
                recovery_penalty: request.settings.excess_high_penalty,
                high_cognitive_load: task.cognitive_load == CognitiveLoad::High,
                predecessor_ids: predecessor_map.get(&task.id).cloned().unwrap_or_default(),
                dependency_depth: dependency_depths.get(&task.id).copied().unwrap_or_default(),
                feasible_slot_ids: candidates
                    .iter()
                    .map(|candidate| candidate.slot.id)
                    .collect(),
                slot_id: None,
            }
        })
        .collect();
    SolverPlan {
        slots: problem
            .slots
            .iter()
            .map(|slot| SolverSlot {
                id: slot.id,
                start_minute: slot.start.timestamp().div_euclid(60),
                end_minute: slot.end.timestamp().div_euclid(60),
            })
            .collect(),
        tasks,
        fixed_tasks: problem.fixed_tasks.clone(),
        score: None,
        solve_seconds: u64::from(request.settings.solve_seconds),
    }
}

fn dependency_depths(
    tasks: &[Task],
    predecessor_map: &HashMap<String, Vec<String>>,
) -> HashMap<String, i64> {
    fn depth(
        task_id: &str,
        predecessor_map: &HashMap<String, Vec<String>>,
        memo: &mut HashMap<String, i64>,
        visiting: &mut HashSet<String>,
    ) -> i64 {
        if let Some(value) = memo.get(task_id) {
            return *value;
        }
        if !visiting.insert(task_id.to_string()) {
            return 0;
        }
        let value = predecessor_map
            .get(task_id)
            .into_iter()
            .flatten()
            .map(|predecessor| depth(predecessor, predecessor_map, memo, visiting) + 1)
            .max()
            .unwrap_or(0);
        visiting.remove(task_id);
        memo.insert(task_id.to_string(), value);
        value
    }

    let mut memo = HashMap::new();
    let mut visiting = HashSet::new();
    for task in tasks {
        depth(&task.id, predecessor_map, &mut memo, &mut visiting);
    }
    memo
}

fn predecessor_map(dependencies: &[crate::model::Dependency]) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for dependency in dependencies {
        map.entry(dependency.to_task_id.clone())
            .or_default()
            .push(dependency.from_task_id.clone());
    }
    for values in map.values_mut() {
        values.sort();
    }
    map
}

fn priority_weight(settings: &Settings, priority: &Priority) -> i64 {
    match priority {
        Priority::Low => settings.priority_low_weight,
        Priority::Normal => settings.priority_normal_weight,
        Priority::High => settings.priority_high_weight,
    }
}

fn run_solver(plan: SolverPlan, _solve_seconds: u32) -> Result<SolverPlan, AppError> {
    let manager = SOLVER_MANAGER.get_or_init(SolverManager::new);
    let (job_id, mut receiver) = manager
        .solve(plan)
        .map_err(|error| AppError::Internal(format!("solver could not start: {error}")))?;
    let mut completed = None;
    while let Some(event) = receiver.blocking_recv() {
        match event {
            SolverEvent::Completed { solution, .. } => {
                completed = Some(solution);
                break;
            }
            SolverEvent::Failed { error, .. } => {
                let _ = manager.delete(job_id);
                return Err(AppError::Internal(format!("solver failed: {error}")));
            }
            _ => {}
        }
    }
    let _ = manager.delete(job_id);
    completed
        .ok_or_else(|| AppError::Internal("solver ended without a completed solution".to_string()))
}

fn make_proposal(request: &Request, problem: &PreparedProblem, plan: &SolverPlan) -> Proposal {
    let plan_score = score(plan);
    let assignments: HashMap<_, _> = plan
        .tasks
        .iter()
        .filter_map(|task| {
            let slot = task.slot_id?;
            let candidate = problem
                .candidates
                .get(&task.id)?
                .iter()
                .find(|candidate| candidate.slot.id == slot)?;
            let start = candidate.slot.start.timestamp().div_euclid(60);
            Some((task.id.as_str(), (start, start + task.duration_minutes)))
        })
        .collect();
    let items: Vec<ProposalItem> = problem
        .inbox_tasks
        .iter()
        .map(|task| {
            let solver_task = plan.tasks.iter().find(|candidate| candidate.id == task.id);
            let candidate = solver_task
                .and_then(|solver_task| solver_task.slot_id)
                .and_then(|slot| problem.candidates.get(&task.id)?.iter().find(|candidate| candidate.slot.id == slot));
            let scheduled = candidate.is_some();
            let (start_at, end_at) = candidate
                .map(|candidate| (Some(candidate.slot.start.to_rfc3339()), Some((candidate.slot.start + Duration::minutes(i64::from(task.duration_minutes))).to_rfc3339())))
                .unwrap_or((None, None));
            let fatigue_penalty = solver_task
                .and_then(|solver_task| {
                    let slot = solver_task.slot_id?;
                    let candidate = problem
                        .candidates
                        .get(&task.id)?
                        .iter()
                        .find(|candidate| candidate.slot.id == slot)?;
                    Some(fatigue_penalty(
                        task,
                        Some(candidate.slot.start.timestamp().div_euclid(60)),
                        &assignments,
                        problem,
                        request.settings.recovery_minutes,
                        request.settings.excess_high_penalty,
                    ))
                })
                .unwrap_or_default();
            let (outcome, explanation) = if scheduled {
                (DiagnosticOutcome::Scheduled, "Fits the configured availability without hard conflicts.".to_string())
            } else if problem.candidates.get(&task.id).is_none_or(Vec::is_empty) {
                (DiagnosticOutcome::NoHardFeasibleSlot, "No slot satisfies the hard timing, availability, or busy-time constraints.".to_string())
            } else {
                (DiagnosticOutcome::FeasibleButNotSelected, "A feasible slot exists, but the proposal leaves this task unscheduled to protect higher-priority constraints.".to_string())
            };
            ProposalItem {
                task_id: task.id.clone(),
                start_at,
                end_at,
                scheduled,
                cognitive_penalty: candidate.map(|candidate| candidate.cognitive_penalty).unwrap_or_default(),
                fatigue_penalty,
                explanation,
                diagnostics: ProposalDiagnostics { outcome },
                busy_blockers: problem.busy_blockers.get(&task.id).cloned().unwrap_or_default(),
                omitted_blocker_count: 0,
            }
        })
        .collect();
    let mut applicability_reasons = Vec::new();
    if items.is_empty() {
        applicability_reasons.push("No inbox tasks are ready to schedule.".to_string());
    }
    if items.iter().any(|item| !item.scheduled) {
        applicability_reasons.push(
            "Some tasks remain in the inbox because no hard-feasible assignment was selected."
                .to_string(),
        );
    }
    Proposal {
        id: Uuid::new_v4().to_string(),
        status: "ready".to_string(),
        base_input_revision: request.base_input_revision,
        request_id: request.request_id.clone(),
        horizon_start: problem.horizon.start.to_rfc3339(),
        horizon_days: request.settings.horizon_days,
        timezone: request.settings.timezone.clone(),
        score: Score {
            hard: plan_score.hard(),
            medium: plan_score.medium(),
            soft: plan_score.soft(),
        },
        created_at: problem.horizon.now.to_rfc3339(),
        items,
        applicability_reasons,
    }
}

fn fatigue_penalty(
    task: &Task,
    start_minute: Option<i64>,
    assignments: &HashMap<&str, (i64, i64)>,
    problem: &PreparedProblem,
    recovery_minutes: u32,
    penalty: i64,
) -> i64 {
    if task.cognitive_load != CognitiveLoad::High || start_minute.is_none() {
        return 0;
    }
    let start = start_minute.unwrap();
    let end = start + i64::from(task.duration_minutes);
    let mut total = 0;
    for (other_id, (other_start, other_end)) in assignments {
        if *other_id == task.id.as_str() {
            continue;
        }
        let gap = if start >= *other_start {
            start - *other_end
        } else {
            *other_start - end
        };
        if gap >= 0 && gap < i64::from(recovery_minutes) {
            total += penalty;
        }
    }
    for fixed in &problem.fixed_tasks {
        let gap = if start >= fixed.start_minute {
            start - fixed.end_minute
        } else {
            fixed.start_minute - end
        };
        if fixed.high_cognitive_load && gap >= 0 && gap < i64::from(recovery_minutes) {
            total += penalty;
        }
    }
    total
}
