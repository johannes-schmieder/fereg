# Slope certificate traversal experiment

Reject the contiguous-slice rewrite of the cached slope certificate. Its
sixteen-cell screen has mixed whole-command results, and the diagnostic PW K6
fresh-check time increases while AW K10 improves. The experiment does not
establish a consistent gain over the accepted `scb1` cache-binding source.
The complete working computational source is restored exactly to `scb1`.

The candidate binds each residual row once and traverses bounded scratch
slices for scaling, QR projections and compensated raw moments. Every
division, FMA, multiplication, sum and within-coordinate observation order
is preserved. It changes no geometry, rank rule, tolerance, ownership,
memory admission or solver selection. Both serial and threaded cached-slope
calls use the rewritten loops; explicit/shared and PPML eligibility stays
unchanged.

The three-pair screen uses N=245,760, seed 104729 and the same pinned stock
fixest reference. It covers all weights, robust/two-way clustered inference,
full/saved/no outputs, K6/K10/K50 and T1/T8/T16. Ordinary fixest is excluded
for FW inference. [All timings](timings.md) retain every cell and repetition.

| Selected case | Paired candidate/prior |
|---|---:|
| AW K10 T1 robust/full | 1.0172 |
| AW K10 T1 two-way cluster/full | 0.9930 |
| PW K6 T1 robust/full | 1.0046 |
| PW K6 T1 two-way cluster/full | 1.0247 |
| FW K10 T1 robust/full | 0.9593 |
| Unit K10 T1 iid/no outputs | 0.9492 |
| PW K6 T8 two-way cluster/full | 1.0190 |
| AW K10 T16 robust/full | 1.0270 |
| AW K50 T16 robust/full | 0.9958 |

Ratios are medians of same-repeat ratios. They are screening results, not
confirmed regressions or final acceptance blocks. The inconsistent screen
and phase results do not justify further confirmation of this exact rewrite.
Prepared five-pair controllers were not executed. No width/weight/VCE guard
is fitted to the favorable cells.

Separate single instrumented traces measure the initial fresh-check stage:

| Case | Prior checks | Candidate checks |
|---|---:|---:|
| AW K10, 12 RHSs | 10.414 ms | 8.331 ms |
| PW K6, 7 RHSs | 8.834 ms | 9.988 ms |
| AW K50, 52 RHSs | 31.770 ms | 30.658 ms |

These diagnostics identify the mixed kernel response; they are not headline
timing evidence. Each diagnostic case includes the prior instrumented plugin,
the uninstrumented candidate and its instrumented copy, with exact outputs.

All 141 screen calls pass with clean isolation. All 48 native pairs have exact
numerical exports, samples, requested saved arrays and all 28 non-timing solver
fields, with equal tracked allocation peaks. Independent output reconstruction
and normal-equation checks pass. The nine diagnostic calls also pass. Five
existing focused Rust fixtures pass in three release invocations, including
independent projection/certificate comparisons, rank-deficient and extreme-
weight cases, shuffled groups, multiple slopes, narrow/wide batches, protected
eligibility and budget/fallback checks. Both new plugins pass the actual ABI
test.

The rejected candidate `sct1` has plugin hash
`3933141837aaa58bffad97a84547e118594591f10d2bafb52cc0af306b110ab9`.
The archive preserves 150 calls, four complete source identities, the rejected
source, controllers, filtered traces and scientific/isolation/solver audits.
Integrity verification covers 3,612 stored entries, 132 omitted local output
arrays and four complete source reconstructions. Binaries and raw licensed
logs are excluded. The six preceding graph/window drafts remain separately
pending Linux acceptance. This investigation changes no production kernel or
routing rule and does not close the remaining serial reference losses.
