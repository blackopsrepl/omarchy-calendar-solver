solverforge::planning_model! {
    root = "src/domain";

    mod slot;
    mod task;
    mod plan;

    pub use slot::SolverSlot;
    pub use slot::SolverFixedTask;
    pub use task::SolverTask;
    pub use plan::SolverPlan;
    pub use plan::score;
}
