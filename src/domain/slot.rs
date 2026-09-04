use solverforge::prelude::*;

#[problem_fact]
pub struct SolverSlot {
    #[planning_id]
    pub id: usize,
    pub start_minute: i64,
    pub end_minute: i64,
}

#[problem_fact]
pub struct SolverFixedTask {
    #[planning_id]
    pub id: String,
    pub start_minute: i64,
    pub end_minute: i64,
    pub high_cognitive_load: bool,
}
