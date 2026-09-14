# P4/P6 generated finite-predictor tiles: adopted local checkpoint

Adopt [ADR 0068](../../../../docs/adr/0068-retained-generated-predictor-tiles.md)
against evidence HEAD `ef38dd5`, accepted native `fb4cc5b`/plugin `f4cb7dee`.
Candidate `e6978061` is bound to all 178 native inputs in
[built identities](built-identities.json), the [three-file patch](implementation.patch)
and [source archive](candidate-source.json). Core/plugin were forcibly rebuilt
for both macOS architectures; static and actual arm64 ABI/self-tests passed.
Main and installed dist were not built or replaced. These are local M2 Ultra/
Stata-MP development findings, not external-estimator or release qualification.

## Complete-command results

The original [screen protocol](protocol.md) and [follow-up](followup-protocol.md)
precede their respective measurements. Three seeds per primary, rotated order,
fresh processes, untimed small warm-ups, identical inputs and a 120-second cap
are retained. No builds, tests, oracles, generation or compression overlap timing.

| Generated target | N / numeric K / factor levels / workers | Command medians, seconds | Median paired reduction |
|---|---|---:|---:|
| FW, exposure, missing, robust | 122880 / 8 / 4×8 / 4 | 1.438 → 1.366 | 5.92% |
| Unit, missing, robust | 245760 / 8 / 8×8 / 16 | 4.029 → 3.691 | 8.39% |
| PW, offset, missing, two-way cluster | 122880 / 8 / 4×8 / 16 | 1.464 → 1.363 | 6.64% |

Generated widths are 40/72/40, including the structural constant. Every primary
pair improves. Percentages are medians of paired changes, not ratios of separate
medians. Median paired savings are 86/338/98 ms. The original FW screen remains
included; no unfavorable observation was replaced.

Strict/final phase medians fall 0.455→0.402 / 1.409→1.220 / 0.472→0.411 seconds.
Preparation stays about 0.076/0.182/0.075 seconds; projection and finite-work
variation remains in the full [summary](summary.csv) and
[paired statistics](adoption-summary.json). Nested phases must not be summed.
The existing public PPML runtime does not separately expose every input/outer
Stata pass; this report does not close P0's complete accounting item.

## Changed work and direct attribution

Fresh/strict retained solves accumulate generated predictors in 128-row tiles,
borrowing finalized ordinary or topology-backed factor codes. Each row executes
the original ordered coefficient FMAs, including zero indicators. The existing
numeric binding takes precedence. Eligibility requires at least 4096 rows,
eight generated columns and a categorical recipe, with no upper-width cap.
There is no new worker pool, SPI call, retained cache or observation-sized buffer.

A fixed 1024-byte stack bank per active tile replaces per-cell recipe resolution
with per-column/per-tile resolution. The existing predictor stays cache-local
while its selected columns are accumulated. Source-derived work still includes
N times selected-width FMAs and recipe-value construction; factor interactions
can visit more component codes than scalar early-exit evaluation. This is not a
measured DRAM-byte or reduced-FMA claim. No extra matrix is materialized.

The [baseline](baseline-diagnostic.json) and [candidate](candidate-diagnostic.json)
diagnostics use separate timing-only artifacts and the same original FW data.
All scientific and non-time solver fields match the corresponding untraced arm.
Three full-data accumulations on 122382 retained rows fall from 113.483 to
14.190 ms in total, saving 99.293 ms. Independent reconstruction/certification
stays 1.668→1.651 ms. Warm scopes remain separate. Instrumented command times are
excluded from speed claims, and the full command gain is not assigned exclusively
to this kernel. Diagnostic hooks are absent from adopted source.

Ordinary frozen PPML, RHS construction, one-shot OLS subtraction, reference
scales, rank, weight generations and final-Hessian policy are unchanged. Ordinary
OLS subtraction has different absent-indicator arithmetic and is deliberately
excluded from this predictor-only helper.

## Protected outcomes and memory

All 23 successful untraced pairs preserve byte-exact b/V, sample, status, DoF,
weight/cluster/separation structure, certificates, saved outputs and non-time
solver decisions. Fourteen successful controls cover weak/bottleneck/cycle/
disconnected/unbalanced and tiny-weight bridge graphs, actual three FE, a slope,
low FW clustering, saved/separated outputs, N4095, small numeric and widths7/8.
The largest added time is 6 ms (27.3%) on small numeric, below the predeclared
joint 5%-and-10-ms review gate; width7 adds 2 ms. Low FW improves 2.281→2.186
seconds in its single pair. Single controls are not repeated speed claims.

The topology-backed plus exact-duplicate control fails during warm-up on both
artifacts. Direct diagnostics of both original warm and full inputs reproduce
`r(430)`: final-Hessian canonical basis change. Independent
[failure-atomicity checks](topology-failure-atomicity-results.json) retain that
error and verify no e(b) is published. These are failed public coverage, not
successful fits or time observations. The code-level raw-level oracle separately
checks topology-backed and rebased factor identities. No seed, model, tolerance,
rank rule or input was changed to make the original control pass.

Primary native peaks are exactly 221.157/514.167/227.083 MB in both arms, within
unchanged 293.063/722.006/391.932 MB budgets. All successful peaks are admitted.
Weak PW has a 1.380 MB lower observed peak; the other successful pairs have exact
peaks. The cause of that difference is not established; it is not a memory-saving claim. Sampled
primary RSS medians are 0.1600→0.1598 / 0.3982→0.3969 / 0.1724→0.1722 GiB;
process RSS and native live allocation are distinct measures.

The [public admission supplement](boundary-results.json) runs the original
width-eight N8193 control in explicit fast mode. Both artifacts admit at
555025311 available bytes (18154399 usable after the existing headroom), with
exact science, and reject one byte below with r(909) and the required sentinel.
The two expected errors are preserved and excluded from performance claims.

## Validation, preserved failures and remaining scope

[Validation](adoption-summary.json) records 16 distinct focused core tests and
95 serial plugin tests. The new raw-level/FMA oracle covers both prepared
layouts, rebased/topology codes, vanished levels, widths above64, N4095/4096/4099,
workers1/4/16, reordered/repeated slots, signed zero and invalid inputs/overflow.
Reconstruction and strict caller rollback checks remain green.

Seven independent licensed fits per artifact cover AW robust OLS, low PW cluster
OLS, three-FE FW OLS, absorbed-slope OLS, and unit/FW/PW generated PPML. Explicit
dummy regressions/Poisson means and independently formed Hessian/sandwich matrices
check coefficients, covariance, sample, row order and registered corrections.
The [original fixture](generated_oracle.do) and
[weight supplement](generated_weights_oracle.do) retain their original tolerances.

A selective rustfmt-diff script initially split a moved block and produced
uncompilable source. The [failed patch](before-format-repair-implementation.patch)
and [test result](before-format-repair-test-results.json) remain. Complete-function
formatting repaired it before the tested build. A Mata block inside a Stata loop
initially caused r(1); the preserved fixture/results precede its conversion into
a separately defined function. No numerical input or tolerance changed. The
first summary incorrectly assumed every peak was identical and stopped on the
lower weak-PW peak; the corrected summary reports actual values and checks the
predeclared admission requirement. Failed scripts and log identities remain.

The [archive](archive.json) retains 54 command captures, including both failed
warm-ups and both expected admission failures, plus hashes of licensed diagnostic/
oracle logs. Raw licensed logs, data and plugins are not committed. Installed
dist, 97 unrelated STATUS additions and the original untracked files are preserved.

This adopts generated retained predictors. Seven P0–P7 items remain: broader
generated OLS consumers and slope/three-plus-FE layouts, remaining PPML state/
construction, scheduling and complete cost accounting. R1–R5 remain excluded.
Independent reviews, upstream CMG assessment and the bounded SCC campaign follow
P0–P7; no agents, cluster work, upstream change or push occurred here.
