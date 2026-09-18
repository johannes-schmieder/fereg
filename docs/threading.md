# Threading contract

## 0.5.0 implementation

`ThreadTeam` installs each parallel phase into a bounded process-local Rayon
pool at the resolved count. `threads(0)` uses available processors; an explicit
positive value requests that team size. Ranges are contiguous, nonempty, and
returned in worker order. Native and plugin tests exercise 1, 2, 8, and 32
requested threads.

The cache owns at most one pool. A later install at the same count reuses its
registry and workers; a different count replaces it. Computation does not
depend on Rayon's ambient global pool. The cache retains no model, sample,
workspace, coefficient, or certificate state, so estimator ownership remains
per action and different thread counts remain safe across repeated Stata
calls. The current estimators reuse one resolved policy across parallel
preparation, matrix, score, reconstruction, and inference kernels.
Deterministic solver paths retain their registered reduction order. The
default fast path may choose a different certified reduction or solver route.
Retired scoped teams signal shutdown and join their worker handles after the
cache mutex is released. New and old teams can coexist during replacement.
The general Rust API avoids self-join when external asynchronous work drops the
last owner on one of its own workers; Stata actions use scoped work and release
their team owners on the calling thread.

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

Under ADR 0034, the pinned LSMR norm and dot kernels use one fixed vector
chunk per indexed reduction leaf. Worker stealing therefore cannot change
their addition tree. This is a kernel-level guarantee; other upstream
atomic and preconditioner reductions do not gain a general bitwise guarantee.

## Rayon decision

ADR 0002 recorded the earlier dependency-free construction. The 0.5.0 runtime
uses an exact-version Rayon dependency and a local pool while preserving the
same `ThreadTeam` abstraction. The crate and transitive dependency inventory is
captured in Cargo.lock and the release SBOM/license notices.

ADR 0020 fixes the PPML rule that retained projection, finite moments,
solve/reconstruction, and sandwich phases all run under the preflight-frozen
team count, including when an ambient pool has a different size. Qualification
checks the exact installed Rayon pool width with `rayon::current_num_threads()`;
the number of workers Rayon happened to schedule is diagnostic telemetry, not
a correctness gate.
