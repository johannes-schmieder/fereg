# ADR 0008: certified solver routing and MAP-relative memory gate

**Status:** Accepted
**Date:** 2026-08-25
**Decision owner:** Johannes Schmieder

## Context

The deterministic optimized MAP implementation reached four sealed 10-million
observation paired cases at source
`df53822b3263d7425a9a2a42b16b58f7b306dc91`. Dense and sparse two-FE designs
at one and eight threads all exceeded 2x the pinned `reghdfe` comparator; the
formerly problematic sparse eight-thread case reached 2.40x. The next
reference arm had only begun, so this is a clean boundary between the MAP
baseline and solver-policy development.

The existing contract made fixed reduction order and cross-thread bitwise
identity properties of every public estimate. That constrained algorithm
selection without strengthening the econometric result. It also compared RSS
to `reghdfe`, even though the product decision permits spending more memory to
obtain materially faster certified estimates.

## Decision

1. The substantive OLS contract is unchanged. Exact sample and singleton
   handling, coefficient identities and omission status, DoF and cluster
   counts, VCE rules and small-sample corrections, stored results,
   postestimation, independent convergence/covariance certificates, and
   registered numerical parity remain mandatory.
2. Public estimation advances to action protocol 1.3 and OLS result wire 1.3;
   model wire 2.1 is unchanged. Minor-version-compatible diagnostic callers
   remain valid.
3. `solver(auto)` is the default and selects the fastest certified algorithm.
   Public explicit choices are `map`, `cg`, `it`, `squarem`, and `lsmr`.
   LSMR accepts `preconditioner(auto|off|diagonal|schwarz)`; `none` aliases
   `off` and `additive` aliases `schwarz`.
4. Automatic routing runs a bounded four-sweep pilot, accounts for setup cost,
   records its decision, and may make at most one certified fallback. The
   planned policy includes direct one-FE demeaning, MAP for well-connected
   designs, accelerated MAP candidates, and preconditioned LSMR for sparse or
   weakly connected designs. No route may publish without the same independent
   final certificates.
5. Default mode promises numerical reproducibility within the registered
   tolerances, but not bitwise identity across thread counts. `deterministic`
   fixes reduction order and requires same-architecture, same-thread-count
   bitwise reproducibility. It may be slower and defaults to deterministic MAP
   unless an explicitly selected solver has its own certified fixed-order
   implementation.
6. Runtime gates apply to default `solver(auto)` only: geometric-mean runtime
   ratio no greater than 0.40, at least 80 percent of cases at least 2x faster,
   every registered single-thread case at least 1.5x faster, and no unexplained
   case more than 25 percent slower than `reghdfe`.
7. Comparator-relative RSS gates are removed. For each matched case and thread
   count, default `solver(auto)` peak RSS may not exceed 1.30 times the frozen
   `df53822` fereg MAP baseline. Explicit solver modes have correctness and
   allocation-safety gates, not a release RSS threshold. Native allocation
   must remain within 25 percent of its registered model, repeated calls must
   not leak, and a constrained case must show `memory(low)` succeeding where
   `memory(fast)` fails preflight.
8. Solver selection begins with a short development matrix: dense two FE,
   sparse two FE, absorbed slope, clustered inference, and moderate K, each at
   one and eight threads where meaningful. Development runs use fewer
   repetitions. `gregress` is a diagnostic arm where its feature surface
   permits. The full frozen release matrix is rerun only after routing policy
   is frozen.

## Baseline evidence

The four immutable receipts and their checksum manifest are stored under
`build_evidence/local/benchmarks/map-baseline-df53822/`. They are historical
implementation evidence, not qualification of a later candidate SHA.

| Case | reghdfe warm median | fereg MAP warm median | Speedup |
|---|---:|---:|---:|
| dense two FE, 1 thread | 22.011 s | 9.123 s | 2.41x |
| dense two FE, 8 threads | 18.053 s | 7.150 s | 2.52x |
| sparse two FE, 1 thread | 31.801 s | 14.860 s | 2.14x |
| sparse two FE, 8 threads | 25.807 s | 10.751 s | 2.40x |

## Consequences

- Internal hexadecimal covariance/determinant anchors are diagnostic evidence,
  not public acceptance gates. Public covariance still must satisfy exact
  identity/status rules and the registered numerical/certificate tolerances.
- Faster algorithms may change the final floating-point bits or reduction
  order without changing the accepted estimator.
- MAP remains a public explicit solver and the frozen baseline for regression
  comparisons. It is no longer the only public or default solver.
- Release qualification remains exact-SHA, source-only until all feasible
  local, SCC, hosted-platform, and packaging gates pass. This ADR authorizes no
  development push, tag, or GitHub release.
