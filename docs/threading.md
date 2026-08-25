# Threading contract

## 0.9.0 implementation

`ThreadTeam` creates an isolated Rayon pool for each native estimation action.
`threads(0)` uses available processors; an explicit positive value requests
that team size. Ranges are contiguous, nonempty, and returned in worker order.
Native and plugin tests exercise 1, 2, 8, and 32 requested threads.

There is no process-global Rayon pool. Per-action ownership avoids hidden state
across repeated Stata calls and permits different thread counts in one session.
The current estimator reuses its action-local pool across parallel preparation,
matrix, score, and reconstruction kernels. MAP group projections retain their
registered reduction order.

## SPI prohibition

No worker may call `SF_*`, access Stata memory, read a Stata macro/matrix, write
an output variable, or poll Stata directly. The main thread copies data into
Rust-owned arrays, workers compute, and the main thread copies certified output
back.

## Determinism

`deterministic` fixes chunk ordering and serial merge order. Tests require
bitwise-identical scientific results across registered thread counts on one
architecture. Cross-architecture bitwise identity is not claimed; scientific
tolerances and exact identity/status gates still apply.

## Rayon decision

ADR 0002 recorded the earlier dependency-free construction. The 0.9.0 runtime
uses an exact-version Rayon dependency and a local pool while preserving the
same `ThreadTeam` abstraction. The crate and transitive dependency inventory is
captured in Cargo.lock and the release SBOM/license notices.
