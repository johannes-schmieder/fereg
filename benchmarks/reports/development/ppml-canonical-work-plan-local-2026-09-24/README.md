# Multi-FE PPML: current canonical costs and next implementation plan

Local macOS arm64 development evidence, 2026-09-24. This is a diagnostic and
planning checkpoint, not a production optimization or a new stock timing claim.
Production remains `f6ed8db6`. The goal remains whole-public PPML at most 0.90
times comparable pinned stock fixest wherever reasonably achievable, scoped to
designs with at least two FE partitions.

## Current performance target

The following are the existing, source-bound three-rotation **fresh** results
for the production computational snapshot. They are copied with attribution from
the [initialization report](../ppml-multife-initialization-local-2026-09-24/README.md),
not remeasured in this diagnostic block. Robust inference and fitted means are
included; stock uses `glm.tol=1e-10`, `fixef.tol=1e-11`.

| Case | Production / stock ms | Ratio | Target ms | Additional saving needed |
|---|---:|---:|---:|---:|
| Narrow two FE N13200/K4/T1 | 49 / 47 | 1.043 | 42.3 | 6.7 ms / 13.7% |
| Disconnected two FE N22080/K14/T4 | 76 / 73 | 1.041 | 65.7 | 10.3 ms / 13.6% |
| Unbalanced three FE N24960/K10/T4 | 115 / 97 | 1.186 | 87.3 | 27.7 ms / 24.1% |
| Low two FE N18000/K28/T8 | 117 / 82 | 1.427 | 73.8 | 43.2 ms / 36.9% |
| Weak two FE N9600/K5/T1 | 42 / 838 | 0.050 | 754.2 | Already below target |
| Two slope partitions N17280/K8/T4 | 75 / 88 | 0.852 | 79.2 | Already below target |

These are case results, not universal family claims. The same production artifact
has a development slope loss (69 versus 66 ms). Historical wins, default-stock
accuracy failures and unresolved comparisons retain their original identities.
The previous report contains every repetition and losing pair. Seed 202609374
remains frozen; it cannot become a tuning seed.

## Exact diagnostic source and validation

Artifact `canonicalledger3` is commit
`f6ed8db6e5591a66a5c25c815df64cfec7f171bb` plus isolated scoped timing
instrumentation. It captures **546 computational inputs**, including the added
diagnostic timing module. Plugin SHA-256:
`22f911713d66784b9fc738c6061113547e4f0369aaa8488ea7809e9122955951`.
Production solver code was not changed. ABI/panic-containment build checks pass.

The six existing development seed347 cells each run once, after a warmup, with
the public command, robust inference and fitted means. All six applications and
independent sample, original-input stationarity, model-space and objective replay
checks pass. No stock fit, new independent Hessian oracle, Rust unit suite or
qualification campaign was run in this block. No application failure, timeout or
detected contention occurred. A first build invocation failed before compilation
because the system Python lacks the required tar extraction API; its receipt is
preserved. The successful build uses the existing development Python environment.

Scoped timestamps identify the timed plugin root separately from the warmup.
The summarizer subtracts the union of nested intervals; exclusive times reconcile
to the root. Instrumentation perturbs short calls. These measurements localize
costs and must not rank production against stock or predict additive speedups.

The [receipt archive](receipts-content-addressed.zip) contains 814 logical files,
724 unique payloads and 4,487,885 bytes, SHA-256
`a1356c85c5a30b45f99338ec5f4899bece132630ca72b345038472d144576fee`.
The [restore tool](restore_receipts.py) verifies source hashes and restores the
sanitized sources, build recipe, controllers, profiles, exports and audits.
Raw licensed logs, generated datasets and binaries are omitted with hashes.

## What the current profile establishes

All times below are milliseconds, from the diagnostic artifact only. Multiple
calls in a row are aggregated; parent and child rows overlap.

| Development case | Native root | Canonical full-rank calls, inclusive | Moment formation | Residual construction | Residual reabsorption | Streamed joint reconstruction |
|---|---:|---:|---:|---:|---:|---:|
| Narrow two FE N12000/K3/T1 | 40.666 | 5.051 | 0.479 | 0.310 | 2.884 | — |
| Disconnected two FE N20160/K12/T4 | 58.641 | 6.037 | 2.006 | 0.369 | 1.803 | — |
| Unbalanced three FE N23040/K8/T4 | 88.056 | 10.497 | 1.571 | 0.281 | 6.626 | — |
| Low two FE N16320/K32/T8 | 118.410 | 17.539 | 9.817 | 2.672 | 2.366 | 10.796 |
| Low three FE N14400/K7/T4 | 80.466 | 11.489 | 1.266 | 0.711 | 8.157 | 2.141 |
| Two slope partitions N16320/K6/T4 | 57.477 | 3.680 | 0.702 | 0.195 | 1.269 | 2.308 |

Three-FE control projection remains the clearest solver target: the two selected-
norm absorption calls take 27.237 ms in fast unbalanced and 24.432 ms in low
mode. Their exclusive times are 25.126 and 22.197 ms. Separately, final residual
reabsorption takes 6.249 and 7.468 ms; the initial residual calls take only 0.377
and 0.689 ms. Low three FE also has a separate 6.617-ms response absorption.
This profile does not distinguish recurrence from all setup inside those spans.
Frozen-frame operation counts are required before choosing a replacement.

Wide low memory makes eleven two-FE PCG projection calls, totaling 22.689 ms
inclusive, 21.063 ms exclusive. Four streamed joint reconstructions take 10.796
ms. Initial retained design construction has 8.707 ms exclusive and changed-weight
reprojection 4.730 ms exclusive, alongside the 9.817-ms moment formation.
The independent row reconstruction in the rank solve is only 2.672 ms. Replacing
that small loop alone cannot close the roughly 37% fresh-case target.

The enclosing prepared-estimator exclusive bucket still contains 9–17 ms across
these cases; it includes operations not split by this instrumentation. It is not
an attribution to required verification. Likewise, the 20.364-ms exclusive joint
session span for the slope control contains checking and assembly that require
further subdivision before optimizing a slope loss.

## Bounded implementation plan

### 1. Redesign three-plus-FE canonical projection

Start with fast unbalanced three FE, retaining low three FE as a control. Capture
initial and strict batches from the current source: identical weights, original
responses/controls, topology, tolerance and output requirements. Count numeric
geometry builds, solved RHSs, per-column recurrence work, row passes, fresh
certificates and residual reconstruction. Compare with the pinned reference's
inner absorption separately from public timings.

Evaluate at most two coherent organizations: (a) coefficient-space block
Gauss–Seidel with safeguarded acceleration and current-weight group factors,
and (b) block-preconditioned coefficient Krylov with one partition eliminated.
Reuse existing proven slope/Schur primitives where they fit; do not wrap an
unchanged generic engine and assume its kernel advantage will survive the public
boundary. The rejected ordinary categorical wrappers in ADRs 0155/0157 remain
closed. This experiment concerns full-width canonical checkpoints and their
representation, with independent per-RHS stopping and current-call reuse.

Weighted geometry is prepared once per valid frame, shared across compatible
control/response batches, and revoked before any dependency changes. Prior-frame
coefficients can seed the solve but cannot certify it. Reconstruct from original
inputs, and retain a fresh complete original-weight certificate. Treat the final
residual as part of the same mathematical projection problem: test whether
retained coefficient products and independently checked residual moments can
avoid another complete solver setup. Do not simply skip reabsorption or rescale
its acceptance criterion. Any changed certificate procedure needs a successor
ADR, an explicit equivalence/error argument, and adversarial tests.

Engineering target: a 20–30% whole-command reduction on the current three-FE
loss, not merely a faster recurrence. This is a target to test, not a forecast.

### 2. Replace repeated low-memory materialization where justified

Next attack the wide two-FE loss as one storage/solve lifecycle problem. Compare
an admitted coefficient-backed view `X - D*gamma` with the current scratch-bank
representation. The view would borrow immutable originals and retain only
current-weight group coefficients, allowing canonical products, QR fallback and
inference to stream rows without repeatedly writing and rereading projected
control banks. It must include every candidate column before rank selection.

This generalizes the ownership principle of ADR 0139, not its one-FE closed-form
formula. Multi-FE coefficients require a certified iterative solve, gauges and
explicit generation authority. G-by-K storage can be large when G approaches N;
admit its full overlap before allocation, keep budget refusal transactional, and
preserve the affordable scratch route. Compare both representations using the
same weights and RHSs. Measure total scratch traffic, original-column scans,
resident peaks and final inference, not only projection time.

In parallel within this implementation (not concurrent with timings), organize
finite products and fresh residual moments as blocked work over borrowed inputs.
An optimized matrix-product kernel is worth testing only if it eliminates
measurable repeated normalization/traffic at the public boundary. ADR 0159's
rejected eight-row tile remains closed; do not resume tile-width tuning.

Engineering target: 35–40% whole-command reduction on the wide low-memory loss,
potentially requiring both the representation change and cheaper joint
reconstruction. A smaller coherent intermediate gain can be committed, but it
does not complete the target.

### 3. Close narrow/disconnected losses and remaining slope losses

After the two larger changes, rerun the narrow and disconnected anchors. Their
fresh targets require about 7 and 10 ms, so reducing only a sub-millisecond
kernel is insufficient. Split the remaining estimator and joint-session work
into outer score/objective evaluation, current-weight preparation, independent
checks, output construction and publication. Consolidate repeated immutable
input access and current-state row passes where validity is explicit. Preserve
both outer convergence passes and fresh score/objective authority.

If ordinary solver work still dominates, choose a materially different operator
or preconditioner organization based on its measured cost. Do not reopen the
rejected categorical wrappers or tune initializer sweep counts. Remeasure slope
losses as well as wins; improvements may carry through shared geometry and
streaming without a separate slope recurrence.

### 4. Expand coverage only around established gains

Predeclare fresh seeds and a compact validation matrix before timing successful
candidates. Cover two/three/four FE, balanced/unbalanced and dense/sparse/
disconnected/weak topology, narrow/wide/generated controls, multiple slopes,
changing weights, rank deficiency, T1/T4/T8, fast/low and budget refusal. Use
native independent FW replication and PW scaling oracles; ordinary fixest
weights are not an FW-inference oracle. Establish scientific comparability before
ranking any new weighted or clustered stock case. Include robust/clustered VCE,
means and selected full saved outputs, plus focused categorical/slope OLS and
one-FE regression controls.

Each implementation milestone needs frozen-input weighted-design checks,
independent Poisson/Hessian/sandwich evidence, generation/rollback boundaries,
and uncontended baseline/candidate/stock rotations. Use at least three paired
development rotations and five fresh confirmation rotations for an adoption
claim near 0.90; predeclare any follow-up when noise straddles the target. Report
all times, paired losses, tails, timeouts, refusals and scientific exclusions.
No universal win follows from an aggregate rate.

Limit each milestone to two materially different candidates. Stop a failed
organization after its bounded screen; do not fit thresholds to favorable seeds.
Accept smaller gains only when repeatable, broadly useful and cheap in complexity
and memory. Remove superseded machinery after replacement is established. Commit
accepted implementation and rejected evidence separately, with STATUS as the
only mutable resume point.

The immediate next experiment is item 1's frozen-frame three-FE canonical solver
comparison. Linux/physical-cap confirmation still awaits renewed SCC access.
The 77/16/111 definitions, static allocation-inventory failure, constrained-fit/
nonretaining-CG failures and previously unresolved scientific comparisons remain
unchanged. No stopped campaign, remote mutation, release or broad qualification
is authorized by this plan.
