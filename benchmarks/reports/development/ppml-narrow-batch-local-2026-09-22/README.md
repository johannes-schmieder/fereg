# Narrow PPML checkpoints: bounded local adoption

Extend the existing certified current-weight Schur mechanism to 2–15 actual
checkpoint RHSs. Selected dense, weak, disconnected and unbalanced PPML cases
gain about 10–34%; a serial control gains 37%. Sparse confirmation loses about
1.4%. This fills a measured gap between the accepted single-residual and wide
routes, with no persistent cache or new memory policy.
[ADR 0107](../../../../docs/adr/0107-narrow-ppml-schur-checkpoints.md) records
the decision. Most comparable cases still lose to stock fixest.

## Source, mechanism and profile

Baseline `residual2` is the accepted plugin at `af38bce7`, SHA-256
`44090d1ab6db8a6a9afea8640c28e440de1eb4562010308bd22e3d604800c792`.
Candidate `batch1` is `af38bce7` plus the archived borrowed-map patch and new
licensed fixture. All 527 captured source/test inputs match the final source;
plugin SHA-256 is
`a191585c5a156809f8a572bbce37d948b2ede1e0ca8579a85f15e4cb32547d14`.
No candidate implementation changed after its build or first timing.

Only an already admitted two-FE categorical checkpoint-LSMR session with its
existing Schur owner, N>=16,384 and 1–64 actual RHSs can enter. For tiny PPML
batches, the probe now explicitly selects every available RHS; width three
no longer duplicates its second column. Larger batches and the separate OLS
entry retain their original selection. Immutable topology and admitted storage
are reused; current weights, group statistics and recurrence state are rebuilt
for every call. The fresh initial certificate, eight-step probe, .01 contraction
threshold, bounded full attempt and fresh canonical residual certificate stay
unchanged. Rejected candidates retain untouched-input LSMR/retry/MAP fallback.
Rank, score, reconstruction, objective and final-Hessian checks remain mandatory.

The separate degree-six diagnostic shows initial 12-RHS LSMR setup/solve/
certificate costing 36.70 ms and strict 11-RHS costing 45.90 ms. Certified
Schur costs 6.51 and 8.31 ms, including its probe and certificate. The sparse
12-/11-RHS probes reject at contractions .0836/.1468, adding 2.80/1.86 ms
before unchanged LSMR. These diagnostic clocks are excluded from headline
timings. There is no instrumented stock reference in any timing block.

Public runtime phase medians independently support the location. Degree-six
projection falls 95.26→64.58 ms and strict/final work 90.64→55.36 ms. Dense K4
projection falls 65.60→51.42 ms and strict/final 69.75→52.22 ms. Loading and
preparation remain about 19/16 ms respectively; publication stays about 9–10 ms.
`phase-medians.json` computes these over all three repetitions; the generic
audit's `runtime` field instead preserves its first successful repetition.

## Timing distributions and losses

Three alternating repetitions per cell, seed 202609221 for the screen and fresh
seed 202609228 for confirmation/controls. The original cases were declared
before baseline measurement. Disconnected and unbalanced confirmation cases
were added during that baseline, before implementation or candidate timing;
the extension receipt states this explicitly. No guard was fitted to these
seeds. The frozen 77 primary, 16 holdout and 111 native-control definitions and
reserved validation seeds remain unchanged.

All 129 paired-screen/confirmation/control attempts complete without timeout,
detected competing fit/build or monitor error. Eighteen earlier baseline calls
and six separate diagnostic calls also complete and remain archived. No
compilation, profiling, archiving, independent-oracle calculation or competing
fit ran alongside headline measurements. Public-command timers include native
loading, estimation, inference and publication; external import and evidence
export are excluded on both sides. The unchanged pinned stock fixest 0.14.2
artifact uses matching threads, regressors, samples, robust inference
HC0*N/(N−1), objective and no saved outputs. Its archive and library hashes are
recorded separately.

| PPML case, N / K / threads | Baseline / candidate / stock median, ms | Paired candidate/baseline min / median / max |
|---|---:|---:|
| Degree-six two FE, 61440 / 10 / 4 | 296 / 232 / 97 | .774 / .778 / .784 |
| Dense two FE, 61440 / 4 / 4 | 228 / 197 / 138 | .853 / .864 / .912 |
| Sparse two FE, 61440 / 10 / 4 | 335 / 337 / 1974 | .985 / .997 / 1.021 |
| Fresh degree-six, 61440 / 6 / 8 | 249 / 219 / 53 | .850 / .898 / .904 |
| Fresh dense, 49150 / 2 / 4 | 181 / 157 / 75 | .867 / .872 / .874 |
| Fresh sparse, 61440 / 12 / 4 | 364 / 369 / 2625 | 1.008 / 1.014 / 1.016 |
| Fresh weak degree-three, 30720 / 10 / 4 | 298 / 226 / 203 | .732 / .767 / .774 |
| Fresh disconnected degree-six, 30720 / 10 / 4 | 200 / 132 / 58 | .649 / .660 / .687 |
| Fresh unbalanced degree-six, 61440 / 7 / 8 | 234 / 176 / 55 | .715 / .761 / .782 |

All 27 direct candidate/stock comparisons pass sample, beta, V, log-likelihood
and deviance checks, as do the baseline comparisons. There are two stock timing
wins and seven losses among these pairwise-comparable rows. These development
checks do not constitute the complete same-cell independent-oracle and pinned
ppmlhdfe qualification gate. The fresh disconnected case's pass does not
adjudicate the earlier, different large disconnected reference discrepancy.

| Native control, N / K / threads | Baseline / candidate median, ms | Paired min / median / max |
|---|---:|---:|
| Degree-six PPML serial, 61440 / 10 / 1 | 504 / 317 | .622 / .629 / .654 |
| Degree-six PPML PW/cluster, 30720 / 10 / 4 | 165 / 133 | .786 / .806 / .826 |
| Degree-six PPML low memory, 30720 / 10 / 4 | 356 / 345 | .932 / 1.000 / 1.006 |
| Degree-six PPML slope, 30720 / 10 / 4 | 1242 / 1234 | .980 / .994 / 1.002 |
| One-FE PPML, 61440 / 10 / 4 | 149 / 150 | .980 / 1.014 / 1.128 |
| Three-FE PPML, 61440 / 10 / 4 | 462 / 465 | 1.000 / 1.002 / 1.029 |
| Wide two-FE PPML, 61440 / 31 / 4 | 429 / 436 | .998 / 1.014 / 1.021 |
| Wide two-FE OLS, 61440 / 31 / 4 | 51 / 50 | .961 / .980 / 1.020 |

Medians of paired ratios need not equal ratios of medians. Excluded controls
show small or mixed variation, including a one-FE outlier; no excluded-path
gain is claimed. Sparse overhead is retained as the cost of a rejected probe.
The tiny-batch fixture also exposes a near-null third RHS rejecting the probe;
its certified fallback succeeds. Neither observation justifies another tuning
cycle or a seed-specific guard.

## Independent validation and memory

All 51 native pairs and within-artifact repeat checks pass. Eighteen pairs have
byte-identical scientific exports. Maximum beta/V/log-likelihood differences
are 2.10e−16/1.01e−17/2.92e−11; deviance and sample checks are identical. Changed
certificate residual values remain within the unchanged certificate policy.

The final source passes 391 distinct Rust tests in 406 executions: 17 checkpoint,
373 PPML and 16 Schur tests with overlap. Extended checkpoint tests cover actual
widths 1/2/3/8/15/16/17, T1/T4/T16, current supplied/scaled weights, poisoned
inactive storage, unchanged capacities and independent tighter MAP answers.
Forced finite-cap/certificate rejection checks untouched-input LSMR equivalence,
MAP fallback and fatal allocation/interruption propagation. Weighted weak cuts
reject before copying; invalid probe-column selections fail before mutation.
Existing exact/one-byte-short copy tests remain, and the modeled allowance check
now includes narrow widths. These are not constrained-fit qualification tests.

Actual-plugin ABI containment passes. Four licensed fixtures pass explicit
return-code and terminal-sentinel checks: `feppml_narrow_checkpoint_schur`,
`feppml_single_residual_schur`, `feppml_native_preparation`, and `ols_schur_wide`.
The new fixture runs at the N16,384 cutoff, finite K1/2/6/12, unit/FW/PW,
offsets, robust/clustered V, T1/T4 and saved fitted means. Twelve independent
dummy-Poisson fits and directly formed current-Hessian sandwiches check 24
native fits. Traces prove certified RHS2/3/4/7/8/13/14 admissions as well as
single-RHS admissions and rejected probes. Ordinary fixest weights are never
used as a Stata FW inference oracle.

No new persistent buffer or weighted cache is added. A transient candidate
replaces larger admitted LSMR banks and is dropped before fallback. Tracked
candidate-minus-baseline peaks range from −15,921,288 to +4,926,056 bytes;
the positive extreme is the fresh sparse case. Admission remains unchanged.
Screen degree-six tracked peaks are 94.57→86.20 MB and dense K4 50.38→34.75 MB.
Sampled process RSS medians are .192→.158 GiB and .134→.100 GiB respectively.
Fresh sparse RSS is .158→.160 GiB; low-memory control RSS rises .076→.093 GiB
despite identical tracked peaks. RSS sampling is nominally every 10 ms and
does not establish exact peak bounds. All per-run peaks and sample counts are
archived. This is neither exact peak neutrality nor constrained-memory proof.
The static allocation inventory still fails on documented drift and was not
regenerated.

## Milestone closeout

Accept this bounded change and close narrow-checkpoint tuning. The next useful
subsystem is the remaining three-FE upstream solve, which dominates its earlier
profile and is excluded from this two-FE mechanism. Generated/factor and broad
saved-output performance coverage remains incomplete. Earlier timing blocks
remain attached to their own artifacts; no historical aggregate win is
transferred here.

Weighted constrained-fit failures, nonretaining CG instability, unresolved
sparse three-FE/large disconnected reference comparisons, closed serial-slope
losses and deferred SCC/Linux confirmation remain open. No stopped campaign,
full qualification, push or release was run. `evidence.zip` retains portable
plans, captured controllers, hashes, all attempts/exports, diagnostic traces,
tests and fixture receipts, excluding data, binaries and raw licensed logs.
All 4,367 archived entries pass their hash and portable-path checks. An initial
archive attempt stopped on binary Finder folder metadata; its failure receipt
and partial-archive hash remain recorded, and the successful archive excludes
that unrelated metadata.
