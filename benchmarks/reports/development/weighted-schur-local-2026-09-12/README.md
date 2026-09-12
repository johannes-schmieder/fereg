# P2: weighted admission to the existing OLS Schur engine

Adopt the weighted OLS admission in [ADR 0037](../../../../docs/adr/0037-weighted-ols-schur-admission.md)
for local development under [roadmap #27](https://github.com/johannes-schmieder/fereg/issues/27).
Baseline is `39d1a2a876b92dcc7f8032be792ee171cb85cfdd`. It includes the preceding
OLS ownership extension and committed PPML reconstruction repair. Separate
Veneto status/evidence and the working-tree dist plugin were preserved.

## Source and scope

[Identities](identities.json) bind isolated baseline/candidate source manifests,
the [implementation patch](implementation.patch) and universal macOS plugins.
Both pass the normal binary audit and actual arm64 ABI/self-test. The final
candidate plugin is also byte-identical to the plugin used by the initial
licensed fixtures. Two initial build attempts compiled from the wrong working
directory; they were excluded before comparisons and rebuilt from verified
isolated source directories. No installed/dist binary was used as a baseline.

Production changes are confined to two OLS boundaries: the existing probe now
accepts and validates supplied weights, and the OLS planner/wrapper no longer
rejects them categorically. Its statistics and operator use the full weighted
graph with the same maximum normalization as the existing full-width engine.
There is no new numerical engine, public option, worker team or SPI call.

| P2 path / cost | Before | Adopted / remaining |
|---|---|---|
| Simple one FE, weighted or unit | Direct initial weighted projection and certified norm reuse | Unchanged; one-FE paired control retained |
| Two intercept FE, wide fast OLS, unit | Guarded Schur probe and solve | Same arithmetic/results/decisions on protected controls |
| Two intercept FE, wide fast OLS, supplied weights | Legacy CMG/LSMR selection | Probe actual weighted operator; existing weighted Schur engine if admitted |
| Tiny weighted cuts | Legacy route | Probe rejects; legacy scientific result is exact, with disclosed probe cost |
| Constant weights | Weighted preparation/reference policy and legacy route | Weighted Schur is eligible; weight kind and inference remain intact |
| Narrow/low-memory/saved FE/slopes/three-plus FE | Existing route families | Same eligibility restrictions; width/low/saved controls checked |
| PPML sessions and changing weights | Existing retained-session routes | Unchanged; P1 PPML and P4 remain open |

The probe adds positive-weight validation/normalization scans, three weighted
group statistics, and eight three-RHS Schur applications. It releases its
workspace before the full solver or rejected-route setup. Accepted fits still
construct the full weighted group state and certify the observation residuals.
They avoid the legacy CMG block solves/polish and, where the existing cache is
consumed, a redundant within-norm scan. No hardware traffic counter or general
setup/fallback instrumentation was added; P0's broader combined counter item
remains open. Interacted/slope one-FE passes and constant-weight specialization
remain separate experiments rather than being inferred from simple one-FE
coverage.

## Command time and attribution

[Protocol](protocol.json) was recorded before timing: target at least 10%
median paired full-command gain; investigate protected slowdowns exceeding
both 5% and 10 ms. There are **39 completed pairs / 78 successful measured
calls**: nine primary pairs, 20 controls, four separate traced pairs and six
rechecks. All execute sequentially in fresh local Stata/MP 19 processes on the
M2 Ultra, after a small untimed warm-up. Primary order rotates over three seeds.
Native thread requests are distinct from Stata's eight-processor license cap.
These are local development findings, not qualification or a fixest comparison.

Seconds below are command medians; paired percentages need not equal ratios
of the displayed medians.

| Target | N / controls / native threads | Baseline | Candidate | Paired change |
|---|---|---:|---:|---:|
| AW conventional, strong two FE | 983,040 / 50 / 4 | 6.623 | 1.244 | -80.8% |
| FW conventional, strong two FE | 245,760 / 50 / 16 | 0.780 | 0.237 | -69.6% |
| PW two-way clustering, strong two FE | 245,760 / 50 / 16 | 1.042 | 0.490 | -53.0% |

The separate target traces select Schur with contractions 7.55e-6, 2.64e-7 and
5.98e-6. Probe cost is 45.1, 8.0 and 6.8 ms respectively; full Schur uses 13,
11 and 14 iterations, plus the eight charged probe iterations. Initial
absorption falls from 5.407 to 0.466 seconds for AW, 0.530 to 0.056 for FW,
and 0.598 to 0.065 for PW clustering. Raw moments, preparation and covariance
work are unchanged in source. Input medians remain about 0.314 seconds for AW
and 0.078–0.081 for the smaller targets. The original trace arrays include
warm-up records; the last fit record is the measured target.

No natural full-solve fallback occurs in the captured comparisons. A forced
one-step cap in the Rust tests exercises weighted continuation from original
values, shared iteration limits, setup charges and unchanged raw scales. Its
full-command cost is not measured. The 64-step attempt cap and release-before-
continuation memory contract remain in force.

## Memory and protected costs

| Target | Native peak bytes: baseline → candidate | Sampled RSS GiB: baseline → candidate |
|---|---:|---:|
| AW conventional | 1,156,377,366 → 1,300,143,874 | 1.658 → 1.743 |
| FW conventional | 272,579,888 → 270,899,970 | 0.491 → 0.428 |
| PW clustered | 390,665,541 → 330,967,164 | 0.653 → 0.573 |

The AW speedup trades 12.4% more live native allocation for full-width Schur
workspace. Its modeled budget remains 1,675,120,968 bytes. All modeled budgets
are unchanged and newly admitted weighted Schur peaks stay below their reported
models. Two unchanged legacy controls have observed/model discrepancies in both
arms: low memory is 16,031,158 versus 14,567,688 bytes (+10.0%), and small/narrow
is 1,335,991 versus 1,044,512 (+27.9%). These existing modeling gaps remain open
under P0; they are not represented as successful qualification gates. RSS is
10 ms process-tree sampling, includes Stata/allocator state, and is separate
from native live allocation. Some smaller admitted routes also increase native
peaks; the summary reports each case rather than claiming universal savings.

Unit-weight weak/dense/bottleneck/cycle/disconnected/unbalanced controls retain
byte-exact full scientific exports and identical solver decisions. Their largest
slowdown is 7 ms (disconnected, 0.084→0.091). Weighted disconnected, unbalanced
and 12-order dispersed-weight controls improve 68.6%, 54.9% and 62.8%.

The tiny-weight cut rejects Schur with contraction 0.0583, retains CMG and has
byte-exact b/V, fit and certificate exports in every repeat. Initial command
time rises 0.267→0.279 seconds. Three new paired seeds give median commands
0.264→0.277 and median paired change +3.8%; individual increases are 7, 16 and
10 ms. The 16 ms / 6.1% observation crosses the review envelope and remains
visible. Its separate same-input trace records a 2.7 ms rejected probe and
about 15 ms higher initial absorption including that probe. Remaining cost is
in the unchanged legacy work; the exact cache/allocator/scheduling cause is
not established. This is a disclosed weighted-weak regression, not a zero-cost
rejection claim or a universal bound on routing regret.

The first small/narrow comparison is 11→15 ms; three separate seeds give
14→13 ms medians and -13.3% median paired change. The route is ineligible and
native peak is unchanged. One FE costs 184→188 ms, low memory 137→138 and saved
outputs 63→64, with saved values exact. At actual RHS widths 15/16/64/65,
commands are 50→51 / 56→25 / 190→74 / 193→194 ms; the eligibility boundary is
unchanged and the explicit constant is counted.

Constant AW improves 395→95 ms; a matching unit-weight candidate is 90 ms.
That single comparison plus exact common-scale probe/inference tests supports
admitting constant weights without losing their semantics. It does not establish
enough additional benefit to adopt a separate constant-weight specialization.

## Scientific validation and replay

All 39 pairs pass exact sample/status/DoF/weight/cluster policies and the existing
full numerical comparison tolerances. Maximum full coefficient and covariance
differences are 2.67e-15 and 3.55e-14 respectively. Every native rank, FE, score,
bread, covariance and determinant certificate passes. No tolerance or oracle
was relaxed. [Validation](validation.json) records:

- 60 prepared-fit tests, plus three focused Schur tests covering unit/constant/
  moderate/12-order weights, independent MAP, 1/4/16-thread identity, forced
  continuation, capacity/budget bounds, invalid weights and weighted cuts.
- Four distinct licensed Stata fixtures: weighted Schur, wide Schur, transaction
  and memory modes. The weighted fixture is rerun after adding exact finite
  collinearity and FE-collinearity checks, making five fixture invocations.
- Weighted conventional/HC1/cluster dummy regressions at 1/4/16 threads,
  literal frequency expansion, common AW/PW rescaling and rank checks pass.
  Every invocation has explicit Stata rc=0 and its unique terminal sentinel.
- Universal binary audits and actual arm64 ABI/self-tests pass for both arms.
  No broad qualification, SCC or Windows test is implied.

[Summary](summary.csv), [comparisons](comparisons.json),
[numeric archive](numeric-evidence.zip) and its [checksum](archive.json) retain
all full b/V/diagnostic exports, sample/import hashes, saved values and traces.
The existing collector now accepts an explicit parity marker for numerical
route changes; its preparation defaults are unchanged. Replay with
`weighted_schur_compare.py --part primary`, `controls` and `profile`, using
`FEREG_LOCAL_BENCH_ROOT` for isolated trees and `STATA_MP_PATH` for licensed
Stata. Rechecks call `pair` with the recorded seeds/order and `recheck-` prefix.
Synthetic data, binaries, raw licensed logs and excluded build attempts remain
outside Git. No remote jobs were touched and no code was pushed.
