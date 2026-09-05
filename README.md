# omarchy-calendar-solver

This repository contains the one-shot scheduling adapter used by Omarchy’s
native calendar planner. It reads one JSON request from stdin and writes one
JSON response to stdout. It has no persistence, network access, notifications,
daemon, UI, or background process.

The adapter uses the published `solverforge` crate pinned to `0.19.4` and keeps
wire DTOs separate from its SolverForge planning model. The request supplies
the current time, settings, events, tasks, and dependencies; the response is a
proposal that Omarchy may apply explicitly.

## Third-party software

This adapter incorporates [SolverForge](https://github.com/SolverForge/solverforge)
`0.19.4`, licensed under the Apache License, Version 2.0. The corresponding
license text is included in `LICENSES/SolverForge-Apache-2.0.txt` and is
installed with the package alongside this adapter's MIT license.

## Development

```sh
./bin/test
cargo run --locked < request.json
```

Human diagnostics are written to stderr. Exit status `0` means a valid response
was produced, `2` means the request was malformed or invalid, and `1` means an
internal or solver failure.

## SolverForge model

Each inbox task is a SolverForge planning entity. Its scalar planning variable
selects one of that task's prepared calendar slots through a SolverForge
candidate-values hook. SolverForge's native `first_fit_decreasing`
construction phase places dependency roots before dependents, and its native
local-search phase improves the complete assignment against the hard, medium,
and soft constraints in the planning model.

The behavior implemented here is intentionally local and deterministic around
the supplied `now`: IANA timezone conversion, local-day horizons, availability
slots, RFC 5545 busy recurrence expansion with a bounded occurrence count,
dependencies, fixed applied planner events, cognitive costs, and recovery
diagnostics.
