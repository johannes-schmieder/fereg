# ADR 0142: group anchors and contiguous concentrated PPML moments

Status: accepted for local development, 2026-09-23.

## Decision

The existing concentrated one-intercept-FE PPML engine binds each group's first
row once per direction evaluation. Its two centered-moment passes borrow that
anchor from the existing fixed group-block workspace. Each triangular Hessian
row is updated through contiguous slices with the original FMA arithmetic.
Observation order, group/block boundaries, compensated block merging, finite
standardization, solve and unfactored-Hessian residual check are unchanged.
This applies to admitted fast and low concentration, numeric and generated
columns, and all existing supported thread configurations. There is no new
solver, route guard, threshold or benchmark-specific decision.

Anchors are refreshed for every group on every evaluation. They are not cached
across mutable representative states. The owner retains capacity only. Means,
weighted moments and the Hessian still use the current means and prior weights.
In particular, an ordinary pre-step Hessian cannot be reused for a canonical
solve at newly accepted weights. Investigation found that exact-frame final-
Hessian reuse already exists; adding a cross-step product cache is not adopted.

The source-derived moment-access count falls from `4*N*K` to `(2*N+G)*K` per
direction, excluding unchanged predictor/profile evaluation. The number of
Hessian FMA updates remains `N*K*(K+1)/2`. Contiguous iteration permits better
code generation without changing these operations. The evidence measures both
changes together, not their separate machine-instruction contributions.

## Science and memory

Every required score, objective, projection, model-space, rank, finite solve,
reconstruction and final-Hessian check remains at its existing boundary and
tolerance. Canonical initial/strict products remain independent of the
concentrated direction. Samples, weight semantics, DoF, separation, inference,
Stata/Rust and main-thread SPI boundaries, and failure atomicity are unchanged.

Each fixed block of 64 groups adds K doubles and one vector descriptor:
`(8*K + size_of::<Vec<f64>>()) * ceil(G/64)` bytes. Actual storage uses active
finite width; conservative preflight uses the full candidate width. Reservations
are fallible, exact capacities enter the existing allocation witness, and all
storage is allocated before the warmed iteration window. There is no additional
N-by-K matrix. Optional admission refusal keeps its existing canonical route.

Measured tracked increments are 216–26,880 bytes in the sampled one-FE cases;
modeled increments are 240–27,648 bytes. Excluded multi-FE/slope peaks and all
scratch counts are unchanged. Fixed-process warmed allocation audits retain
zero events/growth and stable capacities. A separate optional implicit-state
budget refusal passes its oracle; it is not a direct anchor-refusal test or
physical-cap qualification. Historical allocation drift and constrained-fit
failures remain open.

## Evidence and limit

The [source-bound report](../../benchmarks/reports/development/ppml-concentrated-moments-local-2026-09-23/README.md)
records 391 Rust tests, independent full-dummy WLS/Poisson/Hessian oracles,
changing weights and shifted representatives, rank/domain boundaries, licensed
generated-term and allocation checks, stock rotations, fresh widths and OLS
controls. All 60 native PPML pairs have byte-identical scientific exports and
means, with unchanged outer/inner counts. Bitwise identity is evidence for this
implementation, not a new requirement for future algorithmic redesigns.

The 35 K24/K32/K40/K48/K80 pairs all improve, including fresh serial and T8 cases.
Narrow K10 has three 1-ms losses and two ties; excluded timing losses remain
visible. The change is retained for its consistent useful gains and small
complexity/storage cost. It does not complete the whole-command 0.90-times-stock
objective. The next substantial opportunities are canonical low multi-FE
construction, narrow public-command work and wide score/verification kernels.
