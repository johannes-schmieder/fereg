# PPML checkpoint consistency: local development evidence

Parent is `258ab62` with isolated plugin `6181be78`; candidate is `156f3835`,
built as `258ab62-checkpoint-repair`. Compiled inputs and source patch are bound
here. Installed dist is unchanged. No full-data convergence or release claim.

## Repair and independent evidence

The old ordinary scheduler clamped a checkpoint-refined 1e-14 target back to
1e-12. The failing scheduler test records that exact regression. Preserve a
valid tighter previous target in scheduling, and carry rejected-checkpoint
refinement into ordinary iterations. All acceptance tolerances stay unchanged.

A 160-row disconnected two-FE Poisson model has paired identical covariates and
outcomes at 0.9/1.1 times a known mean. Their score contributions cancel at the
known predictor (beta 0.2). A perturbed cold component alongside a log-mean-30
component exposes the scheduling error. Parent needs seven iterations and two
strict rejections, with scaled predictor error 9.06e-9. Repair needs five and
one, with error 1.95e-12. The regression permits six iterations and requires
predictor error below 1e-10. Exploratory hotter/noiseless variants failed line
search or PCG and remain local failed diagnostics, not accepted repair evidence.

At the actual inner floor, three same-basis rejected strict profiles without
halving their discrepancy trigger one canonical rebuild. The same dense/scratch
design and admitted block are overwritten with original generated inputs;
existing fast response banks reset to the current response. The rebuilt attempt
and two more rejected strict corrections exhaust recovery with r(430). Fresh
certificates remain mandatory and no failed result is published. Progress and
basis changes retain their own handling. See ADR 0072.

All 360 PPML/separation, 215 OLS/session and 97 plugin tests pass. Focused tests
cover bounded rejection, successful composite recovery, failure atomicity,
caller-thread interruption, exact warm capacities/scratch identity, and fresh
b/bread/predictor parity in fast/low and pool controls. Universal static and
ARM64 actual-plugin ABI checks pass. Twenty-six licensed native fits preserve
weak weighted graphs, FW/PW, clusters/missingness, singleton/separation cascades,
fast/low and pool/memory boundaries.

## Paired complete commands and costs

Three alternating serial pairs per case, eight native threads. Parent already
has the memory-bound repair, isolating checkpoint changes. Private tracing is
on in both arms; candidate adds bounded worst-row diagnostics at checkpoints.
No concurrent task-owned build, oracle, data generation or fit ran during these
measurements. Read-only process checks immediately before and after found no
competing Stata or build job. These are local development controls, not a
qualification campaign or a full-data speed estimate.

| Case | Parent median | Candidate median |
|---|---:|---:|
| German 65,503 retained, low | 2.286 s | 2.224 s |
| Same sample, unrestricted fast | 3.070 s | 3.069 s |
| Narrow 512-row low | 0.090 s | 0.089 s |

These fits did not encounter the diagnosed loop; their small timing changes
are not attributed to recovery. Fast admits automatic joint LSMR here, unlike
the preceding constrained-auto memory comparison, which selected MAP. Do not
compare the 3.069 s unrestricted fast result to the 1.381 s constrained result
as if they had identical solver admission.

Low projection medians are 1.2320→1.1804 s, finite 0.3921→0.3916 and strict
0.3590→0.3580. Fast projection is 2.0610→2.0509, finite 0.0492→0.0496 and strict
0.6839→0.7028; the latter increase is disclosed. Complete per-phase times,
solver/iteration state, selected memory and all paired differences are retained.
Low observed native peak grows 772 bytes (40,000,836→40,001,608), consistent with
extra trace formatting; scratch is exactly unchanged at 9,432,432 bytes.
Fast peak varies across runs with concurrent workspace lifetimes; medians are
163.90→159.04 MB. Process RSS medians are 103.20→111.07 MB low,
291.67→284.38 MB fast, and 37.21→37.32 MB narrow. These RSS differences are
diagnostic. Solver fallback counts remain zero on these controls; injected
recovery tests require one visible fallback and bounded subsequent failure.

All paired samples/order, b, V, means, absorbed predictions, status, DoF and
drop counts are exact. Independent scores and fixed-beta model-space checks
pass for all three candidate cases. The pinned German reference subset passes
common slopes/V/sample/total-rank/residual-DoF comparison. The bounded full-output
comparison and streamed public-mean audit calibrate on this subset, including
agreement with the registered all-column oracle. This does not certify the
unrun full fit or the reference's reconstructed mean under ADR 0026.

## Remaining full-data check

Restart the 91,045,290-row untrimmed exp(logrwage2) model using the isolated
combined build and fast memory only after actual admission. Verify 88,876,264
retained rows. Preserve strict worst-row, selected-engine, iteration, phase,
retry, memory and process receipts. Full covariance/convergence/publication and
the sequential pinned reference comparison remain pending. The stopped earlier
low run is failed/incomplete evidence; no keepsingletons or external pruning
is used.
