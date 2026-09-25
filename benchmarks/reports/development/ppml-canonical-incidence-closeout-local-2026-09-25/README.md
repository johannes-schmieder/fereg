# Canonical PPML incidence experiment — 2026-09-25

Local Mac arm64 development evidence. [ADR 0200](../../../../docs/adr/0200-canonical-ppml-incidence-closeout.md)
rejects both prototypes. **Retain e3ef1757 / slopetopology1 runtime.** The corrected
incidence representation saves some large-system solver work, but principal low
public gains are small/mixed, narrow loses, and a routing test remains unresolved.
No new family reaches the 0.90-stock target in the corrected pilot.

## Source and computation

Both candidates are e3ef1757 plus the captured `ols/canonical_products.rs` overlay.
Baseline slopetopology1 was built from d25c6e06 plus ADR0199's overlays and matches
all 558 current accepted computational inputs. That exact source is restored.
Full snapshots, rejected patches, build/test recipes and artifact identities are
preserved in the archive.

| Artifact | Plugin SHA-256 |
|---|---|
|slopetopology1|1c598ce6ecbdea8e1a6e0ffcc9dfcd655c89165750a8d2b02d8189ec15b9ae1e|
|canonicalincidence1|36145555bd8a88da508d656e506db557665860a978ad867b2ea49a6049882797|
|canonicalincidence2|b92469cba5ff9057050038ef56e2f7829fef2f6cad3ecf42d3f3d563752e0635|

On the existing iterative canonical route, both replace dense grounded Schur
assembly with `diag(m_b/roots²) - C'C`, using exact current-weight group incidence.
Affordable direct factors keep their route. Positive prefix/suffix sums construct
diagonal scaling. Original-input projection/direct-Gram certificates and all later
rank, residual, score, reconstruction, strict, objective and final-Hessian checks
remain unchanged. No previous-weight authority or tolerance change is introduced.

The first candidate applies observation incidence using borrowed membership and
runs one CG recurrence across disconnected components. The one declared correction
packs aggregated non-anchor group-pair weights and local indices, restores
independent component/RHS stopping and visits only the relevant component rows.
All extra arrays are charged to the existing fallible phase budget. Neither
candidate adds an observation-by-regressor bank or persistent weighted cache.

## Public protocol and complete distributions

Both six-cell pilots were declared before execution, on the existing development
seed **202609347**, with three alternating baseline/candidate/stock rotations.
They are separate timing blocks and must not be pooled. The reference is the
unmodified pinned fixest **0.14.2** archive, SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
at glm.tol=1e-10 and fixef.tol=1e-11. Fits include robust inference and fitted means,
matched requested Rayon/OpenMP threads and BLAS1. Startup, import and the warm fit
precede the clock; public loading, estimation, checking, inference and outputs
remain inside it. These are not physical-memory-matched stock comparisons.

All headline process monitors pass. No compilation, profiling, auditing, archiving
or competing fit ran alongside those blocks. Monitor success does not rule out
all ambient operating-system variability. No fresh confirmation seed was consumed.

First prototype:

| Case | Baseline ms | Candidate ms | Stock ms | B/C/S medians | Wins/losses/ties |
|---|---|---|---|---|---|
|Disconnected categorical; N61440/K6/T4 low|112, 111, 110|115, 116, 116|95, 96, 96|111/116/96|0/3/0|
|Disconnected categorical; N61440/K6/T4 fast|101, 104, 102|107, 109, 129|98, 96, 103|102/109/98|0/3/0|
|Serial categorical; N38400/K10/T1 low|139, 140, 140|170, 173, 171|241, 241, 241|140/171/241|0/3/0|
|Narrow categorical; N24960/K2/T4 fast|42, 41, 41|41, 42, 42|29, 31, 31|41/42/31|1/2/0|
|Wide categorical; N23040/K36/T8 low|89, 98, 91|88, 89, 88|127, 126, 129|91/88/127|3/0/0|
|Two slope partitions; N30720/K6/T8 fast|74, 74, 76|74, 73, 74|78, 76, 77|74/74/77|2/0/1|

There are **six paired wins, eleven losses and one tie**. All nine principal
low/fast/serial pairs lose. Outer counts remain four, but reported inner work
changes 69→89, 56→76 and 63→81. The first low solver report's median aggregate
setup falls 6.058→3.699 ms while aggregate execution rises 23.466→31.565 ms.
Those native phase totals are not additive public-time attribution.

Corrected component-local prototype:

| Case | Baseline ms | Candidate ms | Stock ms | B/C/S medians | Wins/losses/ties |
|---|---|---|---|---|---|
|Disconnected categorical; N61440/K6/T4 low|111, 107, 107|103, 105, 109|98, 101, 96|107/105/98|2/1/0|
|Disconnected categorical; N61440/K6/T4 fast|100, 100, 99|94, 97, 95|102, 101, 101|100/95/101|3/0/0|
|Serial categorical; N38400/K10/T1 low|145, 144, 139|140, 143, 141|245, 244, 246|144/141/245|2/1/0|
|Narrow categorical; N24960/K2/T4 fast|37, 39, 41|43, 42, 41|35, 34, 31|39/42/34|0/2/1|
|Wide categorical; N23040/K36/T8 low|90, 89, 87|89, 89, 90|129, 129, 130|89/89/129|1/1/1|
|Two slope partitions; N30720/K6/T8 fast|72, 71, 72|72, 71, 75|82, 80, 83|72/72/82|0/1/2|

There are **eight wins, six losses and four ties**. Principal low gains are only
two ms at the median with one losing pair. Fast gains five ms in all three pairs;
serial gains three ms with one loss. Narrow loses three ms with two losing pairs.
Wide and slopes tie. Reported inner counts return to 69/56/63, matching baseline;
all six families retain outer and fallback counts. No new family reaches 0.90.
The narrow system also uses iterative construction; its cheaper dense operator
can lose to incidence traffic. The narrow regression is not an excluded-path
speed claim or evidence supporting a benchmark-fitted operator guard.

## Closing diagnostic, memory and scientific checks

A separate two-case, two-rotation trace uses the same production artifacts and
has eight completed public states. Median aggregate solver setup/execution in ms:

| Case | Setup B/C | Execution B/C | Certification B/C |
|---|---:|---:|---:|
|Large low|5.989 / 4.117|23.298 / 20.866|1.968 / 1.977|
|Large fast|5.377 / 3.667|18.753 / 16.505|6.177 / 6.550|

This confirms roughly four ms of solver savings, not removal of the main
whole-command deficit. The totals include the solver work reported by the public
protocol; they are not an independently timed stock build or a sum of disjoint
public lifecycle intervals. The earlier lifecycle ledger remains separately
source-bound in ADR0199's report.

Corrected tracked peaks fall exactly **2,122,544 bytes** in both large cases and
37,104 bytes serial, rise 247,936 bytes narrow, and are unchanged wide/slopes.
Scratch deltas are zero. Selected telemetry ranges from -112 to +48 bytes.
This does not repair the static inventory or demonstrate constrained physical
memory, warmed zero-allocation behavior, or cross-platform qualification.

Each candidate passes twelve distinct canonical Rust tests. The new independent
dense-elimination oracle covers repeated edges, disconnected components, changing
weights and common scales from 1e-200 to 1e200. Existing analytic nonzero-residual
WLS, T1/T4, rank, interruption/rollback and exact-budget/refusal tests pass.

One existing work-history routing test fails in each candidate and passes in the
accepted baseline executable. The closing unit trace confirms that the smaller
workspace now admits a cold iterative canonical solve (56 reported iterations),
so no fallback work hint exists. Later assertions in that test are not reached.
It was neither rewritten nor claimed green; production restoration removes the
behavior change. The full captured sources and executable hashes identify each
run.

Each candidate passes the same six licensed fixtures, with explicit terminal
sentinels and zero Stata/process return codes: iterative canonical, two-partition
slopes, three-partition slopes, scientific contract, weights/inference and
separation/outputs. Both actual-plugin ABI/self-test and intentional panic
containment builds pass. **All 116 public states, 76 baseline-anchored pairs and
36 direct candidate/stock pairs pass** the independent scientific audits across
the two pilots and closing diagnostic. Changed recurrences need numerical-envelope
agreement; bitwise equality is not asserted. No new OLS campaign was run.

## Decision and next work

Reject both implementations. No third packed-operator variant, crossover tuning,
fresh holdout or favorable-subset retention follows this bounded experiment.
The corrected representation is mathematically viable and reduces some memory,
but its limited public benefit and narrow loss do not justify integration.

Move to the complete narrow command lifecycle, with a measured savings budget
across preparation, execution, validation and publication. Keep initial/strict
canonical verification and exact final-Hessian authority. Do not return to another
isolated matrix-application experiment or erase the existing six-ms OLS slope
concern. Known scientific discrepancies, allocation-inventory drift, constrained
fit/CG failures and deferred platforms remain open. No push, release, SCC retry
or stopped campaign occurred. The multi-FE performance goal remains unfinished.

## Immutable receipts

`receipts-content-addressed.zip` contains **4,959 logical receipts**, **1,321
payloads**, **13,504,515 bytes**; SHA-256:
`3f3b68e3b8d0fb338d0e242db4ee8ae3147a076f0aa19191beacc45b818c8c95`.

`restore_receipts.py DESTINATION` restores and verifies each logical receipt.
Raw licensed logs, input data and binaries are omitted; input/binary hashes and
reproduction recipes remain. Paths are sanitized. The archive retains both source
snapshots/patches, failed assertions, declarations, controllers, monitors, timing
vectors, scientific audits and closing diagnostics. `summary.json` and
`verification.json` provide machine-readable results.
