# Serial slope geometry experiments

Neither geometry experiment is retained. Both reduce reported setup time,
but whole-command results do not consistently improve the remaining serial
losses. The combined raw-basis cache and first-pivot norm reuse (`sgn1`) shows
a repeated unit-weight robust regression. Norm reuse alone (`snp1`) leaves
PW robust paired ratios above one in its screen and both confirmations.
The working computational source has been restored exactly to captured `rrc1`.

The new diagnostic separates the raw Gram, column normalization and independent
QR inside cached slope preparation. For N=245,760 and 49,152 groups:

| Diagnostic case | Raw Gram | Column normalization | QR |
|---|---:|---:|---:|
| AW K10, intercept plus slope | 1.50 ms | 3.99 ms | 7.62 ms |
| PW K6, three slopes only | 3.05 ms | 6.56 ms | 18.09 ms |

These are single instrumented traces. Per-group clock reads add overhead:
total setup rises from 13.79/27.73 ms in the uninstrumented calls to 16.40/31.88 ms.
They identify QR as the larger component; they are not headline timing evidence.
The two diagnostic native comparisons preserve exact numerical outputs.

`sgn1` caches up to four raw basis values within each Gram row while preserving
each entry's FMA order. It also reuses the first QR pivot's norm because swapping
that column does not change it and there are no prior directions to subtract.
`snp1` retains only that norm reuse. Later norms, rank cutoffs, twice-reorthogonalized
directions and all fresh residual certificates remain unchanged in both trials.
Their captured sources and patches remain archived after reversion.

| Serial case | Combined block 1 | Combined block 2 | Norm-only block 1 | Norm-only block 2 |
|---|---:|---:|---:|---:|
| AW K10 robust/full | 0.9876 | 1.0081 | 1.0253 | 0.9679 |
| AW K10 two-way cluster/full | 0.9859 | 0.9824 | 0.9897 | 0.9965 |
| PW K6 robust/full | 0.9908 | 1.0047 | 1.0136 | 1.0090 |
| PW K6 two-way cluster/full | 1.0041 | 0.9958 | 0.9839 | 0.9840 |
| PW K6 robust/saved | 0.9852 | 0.9848 | 1.0000 | 0.9948 |
| Unit K10 robust/full | 1.0136 | 1.0220 | 0.9863 | 1.0188 |
| FW K10 robust/full | 0.9960 | 1.0000 | 0.9841 | 0.9959 |

Values are median same-repeat candidate/prior ratios, not ratios of marginal
medians. [All timings](timings.md) include both twelve-cell three-pair screens
and all four frozen five-pair blocks. Each candidate remains unchanged between
its screen and confirmations. The prior plugin is `rrc1`; screens also include
pinned stock fixest except for FW, where ordinary fixest inference is not the
Stata frequency-weight comparator. N=245,760 and seed 104729 are unchanged.

Serial setup generally improves by about one millisecond. Some clustered/FW
whole-command cases improve modestly, but those gains do not justify retaining
the repeated regressions in other target cases. No weight/VCE-specific guard
is fitted to these timings. Rejecting the experiments does not resolve the
remaining reference losses or the optimization milestone.

All 490 screen/confirmation calls pass with clean isolation. All 212 native
pairs have exact numerical exports, samples and requested saved arrays, with
equal tracked allocation peaks. All 28 non-timing solver fields also match
in each pair, including routes, iterations and certificate diagnostics.
Independent output reconstruction and normal-equation checks pass. Each
candidate passes three focused release runs containing five Rust fixtures:
independent projection/certificate comparisons, rank and extreme-scale cases,
shuffled groups, exact reused norms, OLS-only admission and memory fallback.
The combined candidate additionally checks exact Gram entries. Both candidates
and the diagnostic plugin pass the actual ABI check.

The archive preserves 494 calls, four complete source identities, controllers,
filtered traces, patches, tests and scientific/isolation/solver audits. Integrity
verification covers 13,642 stored entries, 458 omitted local output arrays and
four full source reconstructions. Binaries and raw licensed logs are excluded.
The six earlier graph/window drafts remain in the captured source identities
and are separately pending Linux acceptance. No production kernel or automatic
solver rule is adopted here; no push, installation or release occurs.
