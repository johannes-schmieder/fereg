# Threading contract

## 0.9.0 implementation

`ThreadTeam` creates an isolated Rayon pool for each native estimation action.
`threads(0)` uses available processors; an explicit positive value requests
that team size. Ranges are contiguous, nonempty, and returned in worker order.
Native and plugin tests exercise 1, 2, 8, and 32 requested threads.

There is no process-global Rayon pool. Per-action ownership avoids hidden state
across repeated Stata calls and permits different thread counts in one session.
The current estimator reuses its action-local pool across parallel preparation,
matrix, score, and reconstruction kernels. Deterministic solver paths retain
their registered reduction order. The default fast path may choose a different
certified reduction or solver route.

## SPI prohibition

No worker may call `SF_*`, access Stata memory, read a Stata macro/matrix, write
an output variable, or poll Stata directly. The main thread copies data into
Rust-owned arrays, workers compute, and the main thread copies certified output
back.

## Determinism

Default mode is numerically reproducible under the registered scientific
tolerances but does not promise bitwise identity across thread counts.
`deterministic` fixes chunk ordering and merge order. Tests require
same-architecture, same-thread-count bitwise reproducibility for deterministic
routes. Cross-thread and cross-architecture bitwise identity are not claimed;
scientific tolerances and exact sample/status/DoF gates still apply.

## Rayon decision

ADR 0002 recorded the earlier dependency-free construction. The 0.9.0 runtime
uses an exact-version Rayon dependency and a local pool while preserving the
same `ThreadTeam` abstraction. The crate and transitive dependency inventory is
captured in Cargo.lock and the release SBOM/license notices.
