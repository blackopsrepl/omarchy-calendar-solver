# omarchy-calendar-solver

This repository contains the one-shot scheduling adapter used by Omarchy’s
native calendar planner. It reads one JSON request from stdin and writes one
JSON response to stdout. It has no persistence, network access, notifications,
daemon, UI, or background process.

The adapter uses the published `solverforge` crate pinned to `0.19.4` and keeps
wire DTOs separate from its SolverForge planning model. The request supplies
the current time, settings, events, tasks, and dependencies; the response is a
proposal that Omarchy may apply explicitly.

## Development

```sh
./bin/test
cargo run --locked < request.json
```

Human diagnostics are written to stderr. Exit status `0` means a valid response
was produced, `2` means the request was malformed or invalid, and `1` means an
internal or solver failure.

## Reference audit

The inspected `solverforge-calendar` reference was pinned at commit
`d37ed0a726c4c69f78b7b917ec73459ed955b758`. That checkout contains the older
TUI/SQLite/Google application, but does not contain the planner source paths or
`planner-solver.toml` named by the integration brief. This adapter therefore
records the missing reference as an integration gap; its checked-in
`planner-solver.toml` currently provides the documented `first_fit`
construction phase and must be replaced with the exact reference phase file
when that source becomes available.

The behavior implemented here is intentionally local and deterministic around
the supplied `now`: IANA timezone conversion, local-day horizons, availability
slots, RFC 5545 busy recurrence expansion with a bounded occurrence count,
dependencies, fixed applied planner events, cognitive costs, and recovery
diagnostics.
