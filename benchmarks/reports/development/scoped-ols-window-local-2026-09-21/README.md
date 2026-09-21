# Scoped serial OLS memory and narrow residual experiments

The scoped serial LSMR window reduces the unit ring K50/T1/full-output tracked
allocation peak from 645,481,767 to 523,613,047 bytes, a reduction of
121,868,720 bytes. Its paired whole-command ratio is 0.9948. All other measured
peaks are unchanged. Across thirteen cells, native paired runtime ratios are
0.9908–1.0171, with exact finite exports, samples and saved vectors in all
39 native pairs. All 78 calls complete without detected timing overlap.
This is a narrow memory benefit, not a broad runtime speedup or Linux acceptance.

The separate fourteen-cell residual traversal experiment completes 126 calls
with clean isolation and exact exports, samples and allocation peaks in all
42 native pairs. It shows no consistent useful gain; ratios range from
0.9783 to 1.0345, including excluded controls. The change was rejected and
reverted. The new owf1 candidate contains the scoped memory policy and the
preceding residual traversal. Its build, actual ABI check and eight focused
transport, memory, MAP, numeric and weighted full-fit fixtures pass.

Independent inference passes all 26 native window and 28 native residual
comparisons. All 162 within-arm repeats and 78 native saved-output checks
pass. The residual reference audit has thirteen passes and one discrepancy;
that default-fixest discrepancy remains recorded. The native output auditor
was also mistakenly invoked on fixest; its wrong-arm assertion is preserved
as a controller error, not a scientific reference failure.

The new optional oracle cache binds the independent solver source, complete
prior audit, weight/VCE and byte-identical original input. Every fresh export
is still compared, and every native fit retains its fresh runtime certificates.
Missing cache entries use a fresh independent solve. All fourteen residual
cells reuse the audited primary equations; the window cells use fresh solves.
Four controller fixtures cover completeness, cache hits/misses, changed actual
input, wrong source, incomplete records and conflicting entries. Two fresh
independent re-solves exactly reproduce the cached unit and weighted oracles.

Ten constrained-memory attempts preserve one successful unit ring fit,
seven r(909) rejections and two ninety-second weighted timeouts. At 1 GB
usable, nrt2 passes the unit fit with independent inference and outputs where
in1 rejects. Both weighted 1 GB calls reject in the joint automatic preflight;
both weighted 1.5 GB calls time out. Both 300 MB nrt2 requests reject.
The scoped policy does not resolve the weighted constrained case. These
concurrent-audit diagnostics do not rank runtime or prove transaction atomicity.

The archive also preserves the failed ow1 compilation, ow2's overly strict
memory-fixture assertion, a pre-existing artifact-name collision and a
controller-only visible-error patch assertion. None changed scientific
thresholds or overwrote existing artifacts. The corrected memory fixture
recognizes an unchanged preconditioner-construction peak exactly.

| Experiment | Index | Graph | Solver | Weight | K | T | Request | Before ms | nrt2 ms | Paired ratio | Peak change, bytes |
|---|---:|---|---|---|---:|---:|---|---:|---:|---:|---:|
| ols-window | 0 | strong_d2 | auto | none | 50 | 1 | cluster2_cross_all | 3643 | 3639 | 1.0039 | 0 to 0 |
| ols-window | 1 | strong_d2 | auto | aw | 50 | 1 | cluster2_cross_all | 3890 | 3872 | 0.9951 | 0 to 0 |
| ols-window | 2 | bottleneck_ring | auto | none | 50 | 1 | cluster2_cross_all | 5738 | 5705 | 0.9948 | -121868720 to -121868720 |
| ols-window | 3 | bottleneck_ring | auto | aw | 50 | 1 | cluster2_cross_all | 5057 | 5119 | 1.0170 | 0 to 0 |
| ols-window | 4 | cycle | auto | none | 50 | 1 | cluster2_cross_all | 2583 | 2582 | 0.9958 | 0 to 0 |
| ols-window | 5 | cycle | auto | aw | 50 | 1 | cluster2_cross_all | 3162 | 3138 | 0.9908 | 0 to 0 |
| ols-window | 6 | bottleneck_ring | auto | none | 6 | 1 | cluster2_cross_all | 855 | 863 | 1.0105 | 0 to 0 |
| ols-window | 7 | bottleneck_ring | auto | aw | 6 | 1 | cluster2_cross_all | 873 | 878 | 1.0046 | 0 to 0 |
| ols-window | 8 | bottleneck_ring | auto | none | 50 | 1 | robust_none | 5491 | 5470 | 0.9947 | 0 to 0 |
| ols-window | 9 | bottleneck_ring | auto | aw | 50 | 16 | cluster2_cross_all | 1071 | 1069 | 1.0075 | 0 to 0 |
| ols-window | 10 | bottleneck_ring | auto | pw | 50 | 1 | cluster2_cross_all | 5093 | 5104 | 1.0037 | 0 to 0 |
| ols-window | 11 | bottleneck_ring | auto | fw | 50 | 1 | cluster2_cross_all | 5113 | 5088 | 0.9951 | 0 to 0 |
| ols-window | 12 | bottleneck_ring | lsmr | none | 50 | 1 | cluster2_cross_all | 4073 | 4075 | 0.9988 | 0 to 0 |
| narrow-residual | 0 | two_fe_dense | auto | none | 5 | 16 | iid | 211 | 208 | 0.9810 | 0 to 0 |
| narrow-residual | 1 | two_fe_dense | auto | aw | 5 | 16 | iid | 266 | 266 | 1.0114 | 0 to 0 |
| narrow-residual | 2 | two_fe_dense | auto | none | 6 | 16 | iid | 203 | 203 | 1.0050 | 0 to 0 |
| narrow-residual | 3 | two_fe_dense | auto | aw | 6 | 16 | iid | 241 | 244 | 1.0083 | 0 to 0 |
| narrow-residual | 4 | shuffled_dense | auto | none | 5 | 16 | iid | 230 | 229 | 0.9956 | 0 to 0 |
| narrow-residual | 5 | shuffled_dense | auto | aw | 6 | 16 | iid | 265 | 265 | 0.9886 | 0 to 0 |
| narrow-residual | 6 | strong_d6 | auto | none | 6 | 16 | iid | 51 | 51 | 1.0000 | 0 to 0 |
| narrow-residual | 7 | strong_d6 | auto | aw | 6 | 16 | iid | 61 | 62 | 1.0164 | 0 to 0 |
| narrow-residual | 8 | unbalanced | auto | none | 6 | 16 | iid | 58 | 59 | 1.0345 | 0 to 0 |
| narrow-residual | 9 | unbalanced | auto | aw | 6 | 16 | iid | 71 | 73 | 1.0145 | 0 to 0 |
| narrow-residual | 10 | two_fe_dense | auto | none | 13 | 16 | iid | 322 | 316 | 0.9783 | 0 to 0 |
| narrow-residual | 11 | shuffled_dense | auto | aw | 6 | 16 | iid_outputs | 415 | 433 | 1.0073 | 0 to 0 |
| narrow-residual | 12 | bottleneck_ring | auto | aw | 50 | 16 | iid | 848 | 854 | 1.0071 | 0 to 0 |
| narrow-residual | 13 | shuffled_dense | auto | none | 1 | 16 | iid | 198 | 198 | 1.0000 | 0 to 0 |

`verify.py` reconstructs all five successful source snapshots and the failed
ow1 source, checks archived hashes and binds both cache users to the original
independent oracle receipt. Large output arrays remain local with hashes;
raw licensed logs and compiled binaries are omitted. The new candidate still
requires final protected inventories and Linux confirmation. No installation,
push, release or production acceptance of the cycle routing experiment occurs.
