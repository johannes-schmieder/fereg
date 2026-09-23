# Current-call PPML certificate geometry

Local Mac development evidence, 2026-09-23. Accepted under
[ADR 0121](../../../../docs/adr/0121-ppml-call-scoped-certificate-geometry.md).
Five successful slope cases improve 11.3–20.4% by paired medians. This is an
incremental native improvement; strict stock still wins those five cases.
The newly sampled disconnected case exposes a native convergence failure on
both baseline and candidate and is not ranked.

## Implementation and exact identity

Candidate `ppmlgeom1` is `5cb0434a` plus the archived five core overlays. All
529 computational inputs match the retained source. Plugin SHA-256:
`895b2ae2ce57094a1290255d6e6f02ae15f7e374441e340caf5052d490003875`.
Baseline `ppmlproj1` matches `1de0b1bb` and the computational source at
`5cb0434a`, plugin
`f42ed127513640759dd4b0135c665d39e49061b40627390574bbb81fe48b17c2`.
Identities bind every computational input; the archive contains every source
file that differs from each artifact's recorded base, including diagnostic
transformations. No later revision inherits these performance claims.

The reusable MAP certificate previously rebuilt identical current-weight QR
geometry at every check. The candidate copies the original rank-aware basis
at the first check of each absorption invocation. Later checks in that call
use the identical values and ranks. Validity lives on the call's stack and
always starts false, including after early success or error and on response-
only or low-memory block calls. No weighted product is authoritative across
calls, even when two weight slices happen to agree.

Residual norms, raw compensated moments, residual span projections, QR rank
rules, arithmetic order, MAP recurrence and certificate schedule are unchanged.
Final scores, objectives, Hessian synchronization, inference, saved outputs and
failure atomicity remain mandatory. Admission is limited to publication PPML,
resolved MAP, multiple effects and at least one slope. There is no N/thread/
seed guard. Mu, other engines, one-effect and categorical specializations keep
their execution. The rejected extrapolation route in ADR 0120 is not restored.

The optional bank is `8*N*sum(channels) + sizeof(usize)*sum(levels)` payload
bytes beside the existing maximum-group scratch. It follows the already
admitted projection bank and cannot displace that optimization. Checked size,
budget or allocation refusal retains uncached certification. Actual capacities
must fit; basis/rank capacities enter the stable session fingerprint. The bank
is charged during preparation and every live linear phase, excluded from
solver slack, and released before inference/recovery/publication. Fresh memory
telemetry re-admits both optional banks. Fixed descriptors/runtime headroom and
the incomplete whole-action allocation proof remain documented limitations.

## Comparable timings and distributions

Positive outcomes, numeric controls, robust inference and saved means are
matched. Native times include SPI loading, preparation, estimation and
publication; stock includes fitting and response prediction. External file
loading and scientific export are outside both command timers. Threads match
per row; BLAS is serial. Stock is the pinned unmodified fixest library using
previously adjudicated public `glm.tol=1e-10, fixef.tol=1e-11`. Earlier default-
stock scientific discrepancies remain unresolved historical evidence.

The representative rows use seed 202609300, N30720/K6/T4 and three rotating,
alternating repetitions. Fresh rows use seed 202609301 with predeclared width,
thread, memory and topology controls. No build, profile, archive, audit or
competing fit accompanies these timing blocks. Milliseconds are median
[min,max]; ratios are the distribution of paired candidate/baseline values.

| Successful slope case | Baseline | Candidate | Strict stock | Paired ratio median [min,max] | Added tracked heap |
| --- | ---: | ---: | ---: | ---: | ---: |
| Degree-six two FE | 905 [887,910] | 723 [720,725] | 120 [118,120] | .7956 [.7945,.8174] | 1,025,072 B |
| Unbalanced three FE | 594 [591,599] | 513 [505,517] | 117 [113,119] | .8636 [.8431,.8748] | 1,233,912 B |
| Fresh N61440/K10/T8 | 1280 [1270,1306] | 1137 [1132,1148] | 228 [226,229] | .8844 [.8706,.9039] | 2,466,328 B |
| Fresh two FE, T1 | 1478 [1477,1479] | 1289 [1284,1290] | 308 [305,310] | .8715 [.8693,.8728] | 1,025,072 B |
| Fresh three FE, low | 859 [842,888] | 762 [756,806] | 119 [119,120] | .8871 [.8514,.9572] | 1,233,912 B |

Added measured heap includes the bank and the 48-byte increase in the boxed
owner on this target. Peak differences are observations, not whole-action
memory proofs. Low-mode improvement varies across pairs: its weakest pair
improves only 4.3%. No threshold or chunk tuning follows this screen.

Excluded-path controls remain visible:

| Control | Baseline | Candidate | Strict stock | Paired ratio median [min,max] |
| --- | ---: | ---: | ---: | ---: |
| Categorical three-FE PPML | 196 [195,203] | 188 [188,189] | 234 [234,236] | .9641 [.9261,.9643] |
| Two-partition slope OLS | 69 [67,70] | 68 [67,68] | Not run | .9855 [.9714,1.0000] |
| Categorical three-FE OLS | 43 [42,44] | 43 [43,44] | Not run | 1.0233 [.9773,1.0238] |

These paths do not use the new bank; their small changes are not attributed to
it. The OLS categorical paired median loses 2.3% even though both medians are
43 ms. Control tracked heaps are unchanged. Seeds 202609302/303 identify these
controls. Single-run N2400/K4 independent pilots (seed 202609299) give
87→77 and 111→102 ms, versus strict stock 22/27 ms; they are science pilots,
not repeated timing evidence.

## Visible disconnected failure and bounded stop

The fourth fresh row is disconnected three-FE PPML with two slope partitions.
Baseline returns `r(459)` after 52.244 seconds. Candidate returns the same
application error after 43.583 seconds; a second candidate attempt returns it
after 43.898 seconds before the stop completes. Stock's first run succeeds
after 85.987 seconds. Its independent public-state audit passes score,
model-space feasibility and objective replay. Native failed elapsed times are
not compared as estimation performance.

A separate untimed noisy candidate diagnostic identifies:
`reusable MAP exhausted its iteration budget without certification`.
It reproduces the error and verifies that the requested mean and `e(cmd)` are
absent. This is a failure-atomicity reproduction, not a passing estimation.
It neither repairs the failure nor establishes an independent optimum for
this large disconnected case.

The remaining repetitions were stopped after the failed first native pair.
The controller had already completed the second candidate and started a second
stock attempt; that stock attempt is explicitly interrupted, and four declared
arms are unattempted. The incomplete screen, cancellation receipt, original
plans and every completed result remain archived. An initial exact-path
supervisor assertion failed before signaling anything; the corrected selector
stopped only this task's controller tree. Separate categorical/OLS controls
then ran to completion. No failed or missing call is silently dropped.

The PPML audit now records missing baseline/candidate arms as incomplete
comparisons instead of raising a missing-key exception. It does not infer
scientific success from a matching error or an absent result.

## Science and focused checks

There are 76 completed screen calls: 73 successes and three native application
failures, plus the interrupted stock attempt. No completed call has a timeout,
overlap or monitor error. All 26 successful native pairs, including pilots,
have exact scientific exports and non-clock solver fields. All 20 native mean
pairs and all 44 successful repeat comparisons are exact. All 18 comparable
headline stock comparisons and their fresh public states pass. The failed
case is separate from these counts.

Both independent small pilots pass explicit-design Poisson Newton/LSMR,
current-Hessian control residualization, robust sandwich, means, rank and
structural DoF checks for baseline, candidate and strict stock. Reference
agreement is not their sole oracle. Sixty-four distinct Rust tests pass:
changing-weight/RHS reuse, local numerical-rank changes, zero/duplicate/slope-
only channels, empty levels, extreme channel units, iteration exhaustion,
early collinear and overflow boundaries, independent slope/FE oracles,
strict checkpoint rollback and final-Hessian boundaries. Budget tests include
one-byte-short refusal, exact admission, other-bank/solver overlap and fresh
telemetry reconstruction. Ten harness tests pass.

Three licensed fixtures pass with explicit return codes and terminal sentinels:
PPML saved slopes, PPML weight/inference and OLS intercept-slope parity. The
known hidden PPML minor-version assertion remains unchanged and was not rerun.
Both new production/diagnostic artifact builds pass actual-plugin ABI/panic
containment. The first unit compile missed two zero capacity fields in the
categorical owner; the corrected compile and tests pass. No scientific rule
changed to resolve it. This is development evidence, not qualification.

## Profiles and next work

Separate diagnostic builds use seed 202609304, excluding warmups. The first
four calls captured always-on fine laps only because the controller strips
ambient diagnostic variables. A second four-call block explicitly enables
trace. Both blocks pass public-state and exact native-output checks; neither
is headline performance evidence. Group clocks and trace add overhead.

| Traced case | Projection calls | Certificate calls | Absorbs | Certificate seconds baseline→candidate | Complete absorb seconds |
| --- | ---: | ---: | ---: | ---: | ---: |
| Two FE | 1320→1320 | 130→130 | 20→20 | .516→.310 | .962→.758 |
| Three FE | 1120→1120 | 72→72 | 16→16 | .241→.169 | .570→.482 |

Weighted basis normalization plus QR falls from .219 to .034 seconds and
.090 to .020 seconds. Fresh raw moments remain about .133/.135 and .091/.091
seconds; fresh residual projection costs also remain comparable. Projection
itself takes .422/.424 and .309/.293 seconds. The cache removes repeated
geometry work without reducing certificate or sweep counts.

Close this geometry milestone. The next bounded solver investigation should
reduce and diagnose the disconnected slope-MAP convergence failure, then
consider a certified remedy with independent model-space checks. Do not repeat
its long timing failures, relax tolerances, substitute unresolved nonretaining
CG, or tune this cache to its seed. Batching fresh raw moments is a separate,
smaller performance opportunity. ADR 0120 stays closed; a later accelerator
would need a newly declared experiment justified by the now-lower certificate
cost.

Weighted/clustered, weakly connected and separation-heavy slope throughput,
individual saved-FE timing, Linux and constrained-memory confirmation remain
outside these timing claims. Frozen 77/16/111 definitions, reserved validation
seeds, static allocation-inventory drift, factor-base provenance, constrained
fit/CG issues and SCC deferrals remain unchanged. No stopped historical
campaign, push, release or qualification is resumed. Seeds 202609299–304 are
used; 202609305 onward remain unused here.

The immutable archive includes declarations, controllers, source/artifact and
input hashes, outcomes, scientific exports, missing/interrupted-arm records,
independent audits, source overlays, build errors and sanitized protocols.
Data, binaries and raw licensed logs are excluded.

`receipts.zip` verifies 2,581 hashed entries, 22,174,657 bytes, SHA-256
`741ea9e35ebd3dea38e461001aceda830a11b93d96cd4f85c3f38d932b1a3d87`.
