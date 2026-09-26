# Superseded candidate: directionalscore1

**Do not adopt this candidate.** A later real T1 slope allocation audit failed
for directionalscore1 and passed for unchanged objectiveblocks1. Its extra
serial team submission violates the warmed-allocation requirement. The original
receipts remain immutable; later failure and repair receipts are supplemental.
The initial retention decision below is superseded by that evidence.

# Independent FE score directions — local development, 2026-09-26

**Accept directionalscore1**, with the weak-connectivity loss explicitly retained.
This changes shared PPML certificate execution, not the estimator, tolerance or
check schedule. Public large-fit medians improve 1.5–1.8% in the pilot and
1.4–2.6% across fresh large numeric, factor and three-FE slope cases. It adds no
numeric storage. This is progress toward the multi-FE stock target, not completion.
[ADR 0220](../../../../docs/adr/0220-owned-fe-score-directions.md) records the decision.

## Source and measurement

Baseline **objectiveblocks1** matches all 567 computational inputs of
**d15b907e**, plugin
`fe596205eef0faf28680d6682a778ad3f871e9530d57fd9b361ebf36cce84c04`.
Candidate is d15b907e plus the archived ppml/prepared_irls.rs overlay, plugin
`c42f0bab531d97c0404d50be85e21884f14732f18f252844f8ca1dd58197a7ed`.
All 567 final production inputs match this measured candidate.

Separate **scoreprofile1/scoreprofile2** diagnostic plugins:
`80c9de1a1c722cf642c6f0ec357a45bfdbf8a052db543980bc273041edaffee0` /
`13cb299b872646dacb902aa49204bcce1a720cd73b5843ac94f356a4b09f3fb4`.
All builds use distinct Cargo targets and pass the actual ABI panic-containment
self-test. Diagnostic clocks do not enter stock headline comparisons.

The unmodified pinned fixest source archive is
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Comparable stock uses glm.tol=1e-10 and fixef.tol=1e-11, matched threads,
inference and requested outputs, and single-threaded BLAS. All PPML timings
use public feppml and fitted means with robust or specified cluster inference.
Three pilot rotations use development seed202609347; five fresh rotations use
predeclared seed202609450. Variant order rotates. All attempts have clean process
monitors. No build, profile, audit or archive overlaps headline fits. Complete
ordered timing blocks appear in [distributions.md](distributions.md).

## Attribution and organization

Fresh baseline instrumentation attributes about 26–27 of 41–43 ms of large
outer-score work to serial FE accumulation. Independent scale sums cost 6.5–6.7
ms and already-parallel finite columns 7.3–7.4. These are full-fit totals over
five current-state checks, not per-iteration prices.

For N>=4096 and more than one worker, each FE direction now owns its two existing
scalar destinations and traverses its immutable level-membership index. Group
membership is stored in original row order, preserving that direction's exact
addition/multiplication sequence. Final certificate reduction stays in canonical
order. Small/T1 calls keep row scans. Each call freshly reads original y, q,
current mu and FE bases; all empty directions are reset. No previous-weight
result, projected/Newton product, new numeric bank or pool is introduced.

Paired diagnostic inclusive milliseconds (nested spans are not additive):

| Case | FE scores B/C | Entire score check B/C | Whole plugin B/C |
|---|---:|---:|---:|
| Million fast | 26.268 / 11.995 | 40.223 / 25.972 | 822.886 / 815.646 |
| Million low | 26.407 / 12.383 | 40.439 / 26.497 | 900.062 / 883.267 |
| Two slopes low | 1.337 / 1.045 | 2.700 / 2.455 | 69.638 / 67.619 |
| Three FE, two slopes | 3.108 / 1.740 | 5.663 / 4.312 | 146.935 / 146.546 |
| Factors | 7.271 / 1.666 | 13.345 / 7.793 | 488.363 / 483.218 |
| Four slopes low | 2.731 / 2.263 | 4.627 / 4.269 | 115.627 / 115.196 |

Large scale/finite work is essentially unchanged. The small four-slope saving
cannot close that design's larger canonical-solver gap. No scheduling constant
or source layout was tuned after these measurements.

## Public results and losses

Pilot B/C/stock medians in ms: million low **920/906/860**, fast
**856/841/865**, two slopes low **70/70/120**, three-FE two slopes
**151/146/231**, factors **564/562/703**, serial **540/540/938**.
There are **12 paired wins, four losses, two ties**. All six large fast/low
native pairs improve. Four of six comparable families meet 0.90 stock in every
pair; large fast (**0.971–0.988**) and low (**1.045–1.053**) do not. Median target
deficits remain **62.5 ms fast** and **132 ms low**.

Fresh B/C/stock medians: fast **651/638/763**, low **703/691/765**, T4 fast
**412/402/662**, factors **690/680/3166**, three-FE two slopes **192/187/288**,
four-slope low **113/112/120**, serial **542/540/937**, weak **119/121/768**,
disconnected low **92/91/98**. There are **35 wins, eight losses, two ties**.
All 45 stock comparisons pass science; six of nine families meet 0.90 in every
pair. Low large (**0.887–0.927**), four-slope low (**0.904–0.967**) and disconnected
low (**0.920–0.968**) still miss. Median deficits are **2.5, 4.0 and 2.8 ms**;
these small source/seed-specific gaps do not justify tuning a favorable cutoff.

Weak PPML loses two ms at its fresh median. One predeclared identical-input
five-pair repeat gives B **121,118,119,121,117**, C **122,124,123,118,119**:
medians **119/122**, four losses and one win. The diagnostic families do not
attribute this small regression; do not assume a cache or solver explanation.
Retain it as a measured affected-path trade-off for the broader gains and zero
extra memory. Do not claim universal non-regression or add a graph-specific guard.

Protected OLS B/C medians: **66/66, 75/76, 14/14, 207/213, 163/162, 17/18**.
A single predeclared repeat of the two consistently slower controls preserves
full/warm input hashes and both original blocks:

| Follow-up | B ordered ms | C ordered ms | B/C medians |
|---|---|---|---|
| Three-FE factor OLS | 207;213;208;212;209 | 216;210;207;208;210 | 209/210 |
| One-FE OLS | 17;17;18;18;17 | 17;16;18;16;17 | 17/17 |

Factor repeat has three paired wins, two losses and paired median C-B=-1 ms,
despite its one-ms ratio-of-medians loss; its 9-ms outlier remains. The initial
consistent excluded-path losses do not reproduce. No more repeat or tuning follows.

## Science, memory and failure visibility

**58 distinct prepared-IRLS tests pass**, including a new exact integer-design
oracle for FE moments/norms. It covers dominant groups, disconnected components,
empty levels, duplicate/zero slopes, slope-only partitions, changed means and
weights, 2^-400..2^400 common scales, T1/T4/T8, and stable workspace addresses and
capacities. Existing tests compare all fields and moments across numeric/generated
widths, boundary sizes, foreign teams and changing/extreme states.

**Eleven licensed fixtures** pass with explicit rc=0 terminal sentinels, including
independent categorical/three-partition/higher-dimensional oracles, weight and
inference policies, transactions and real T4 categorical/slope 16-iteration
audits with zero allocation events and growth. **250 public-state audits** and
**15 explicit Poisson/Hessian/sandwich comparisons** pass. The additional science
cell uses N4800/T4 unbalanced three-FE slopes in low mode, exercising this path
inside the explicit-design oracle rather than relying only on small scalar cases.
All **85 native PPML pairs** and **28 OLS pairs** have exact scientific exports.
Anchored comparisons pass **152/153**; direct C/stock **67/68**. The older small
three-FE saved-output discrepancy remains unranked. Fresh seed449's large mean
discrepancies are not reinterpreted as repaired by seed450's successful comparisons.

The sole failing Rust test is the inherited refined-checkpoint count assertion,
0 versus 1. Its current candidate failure and the previous source-bound baseline
full-suite receipt are preserved; all 567 baseline inputs match d15b907e. No
assertion was relaxed or removed. No application failure, timeout, incomplete
pair, interrupted fit or detected contention occurred in this experiment.

Observed heap and scratch peaks are identical in every pilot/fresh native pair.
Modeled bounds vary **-48..+48 bytes**, with no new allowance, owner or numeric
bank. Existing immutable topology and score buffers are reused; all fresh-state
mathematical checks remain. This is not physical-cap or platform qualification.
Static allocation drift, wire-minor/checkpoint-count failures, constrained-fit and
nonretaining-CG failures, SCC deferral and stopped campaigns remain open. Frozen
77/16/111 definitions and pins are unchanged. No push or release.

## Next subsystem

Move to **canonical multi-slope absorption and RHS scheduling**, beginning with
low-memory four-slope fits and applicable multi-partition OLS. Current diagnostic
canonical preparation is about **45 ms**; about **25–27 ms** remains in the
absorb_in_generation envelope after its currently instrumented descendants.
That name is not evidence of a generic-MAP fallback: source explicitly delegates
to the admitted coefficient solver. Weighted group geometry and ordinary joint
IRLS already have reusable owners and fresh certificates.

Instrument SlopeCoefficientWorkspace::solve, Worker::solve, operator sweeps,
acceleration/checking, packing/reconstruction and current-weight certificates.
Record actual RHS windows, active workers, individual column iterations and
geometry generations. The current entry distributes whole columns across
workers, so narrow low-memory windows may limit available parallelism; this is
a source-based hypothesis, not measured attribution. Compare a bounded RHS
schedule or disjoint within-column coefficient work if that is the dominant cost.
Keep full original-input acceptance and independently validated weighted geometry;
do not merely remove guards, reuse changing-weight products, or revive a rejected
residual-space extrapolation probe. Further score-scale micro-tuning is lower
priority than resolving this remaining solver organization.

## Receipts

Work root: `target/ppml-score-construction-20260926`. The content-addressed archive
retains full source snapshots, patch, declarations, controller/monitor code,
ordered distributions, all attempts, scientific audits, failed/passed tests and
separate diagnostic spans. Generated DTA and binary row arrays are hashed and
omitted; raw licensed logs are excluded. Use restore_receipts.py with a new
directory. The adjacent verification and privacy files bind the restored archive;
older timing claims do not transfer to this source.

The archive contains **11,419 restored and hash/privacy-verified logical receipts**.
SHA-256: `55692ae2fb9f7fdfd18bdbf73818f38250f939d3e1b774733c2bf7ad492465d9`.
