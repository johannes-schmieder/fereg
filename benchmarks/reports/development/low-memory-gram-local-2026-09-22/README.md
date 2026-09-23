# Low-memory OLS: faster compensated scratch certificates

Accept contiguous compensated row updates in the fresh scratch score Gram and
robust covariance certificate. Eligible cases improve by paired medians of
5.5–21.4%, without added allocation or changed numerical exports. The wide
public command improves 2088→1662 ms, with fresh K40 confirmation at
2680→2106 ms. Stock still wins most comparable cases. See
[ADR 0110](../../../../docs/adr/0110-compensated-scratch-certificate-rows.md).

## Source and comparable work

Baseline `qrnorm1` is the computational checkpoint committed at `38935943`,
plugin SHA-256
`f106400d4ed2561fcde5eedb367bcbe384e0c768aa563cc4df6840f09eb8846a`.
Candidate `gramrow2` is an isolated build of that commit with the archived
`prepared_unadjusted.rs`, `prepared_scores.rs` and `prepared_robust.rs`
overlays. All 529 captured inputs match the accepted working source. Its plugin
SHA-256 is
`812326ed980df16ff9b60df3e21359971fad4437a0fe25fb02c9645d8f501b6f`.
The archive binds each overlay, build, fixture and controller separately.

Stock fixest 0.14.2 is uninstrumented. Pinned archive SHA-256:
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`;
loaded library SHA-256:
`bece258f6c6fc3fc47accea45c7b004b0c495d46170d78fa2d6302e66de5fbf9`.
Native `memory(low)` is compared with stock's ordinary memory strategy, not
an equal memory limit. Fit/publication and requested prediction costs are
recorded separately; their sum is the command timer. Import and numerical
export are outside it. Thread requests, samples, regressors, inference and
requested outputs match except for the explicit DoF discrepancy below.
Weighted controls compare native artifacts only. Stata FW inference uses
independent frequency-aware regression/expansion oracles, not fixest weights.

All data generation, builds, fixtures, profiling, oracle computation and
archiving finish outside headline timing blocks. No detected competing fit,
compilation or other monitored workload overlaps those blocks. Frozen 77/16/111
definitions and reserved validation seeds remain unchanged. No stopped campaign,
SCC authentication attempt, qualification, push or release was performed.

## Dominant cost and implementation

The preceding current-source low-memory screen identified large practical
losses. A fresh separate trace on these exact artifacts confirms the target:
for N=122880, K=31, T8, score Gram certification costs 308 ms and the independent
robust covariance certificate costs 340 ms. Their scalar loops repeatedly exit
on each product/sum/correction check, despite independent matrix coordinates.
The candidate borrows contiguous rows of the existing sums and corrections,
performs the same Neumaier operations, and checks the combined finite-value
status before returning from that private row. The two phases fall to 112 and
110 ms. Production covariance remains 366 ms; score storage remains about
51–54 ms. Narrow K10 certificate phases improve 17.3→8.0 and 21.3→10.8 ms.
These diagnostic calls are excluded from headline medians.

Every coordinate retains its observation order and multiplication operands.
In particular, weighted Gram products remain `(weight * left) * right`; the
matrix is not replaced by a mirrored triangle. Both fresh traversals, inverse
and sandwich checks, weights, residuals, rank, DoF and tolerances remain.
Shape mismatch is rejected before mutation. A nonfinite product, sum or
correction anywhere in a row makes its certificate fail before publication,
even if later private coordinates have already been updated. No caller can
recover or publish that incomplete matrix.

The score Gram change reaches scratch-backed robust and clustered OLS; the
meat change reaches the scratch robust certificate. Existing score retention,
read-ahead and bounded row caches already reach these paths. Dense/strided
inference, production influence accumulation, PPML iteration state and the
low-memory finite solver are outside this change. There is no new cache,
buffer, thread team, admission guard or benchmark-specific dispatch.

## Alternating pairs, fresh cases and controls

Three rotated baseline/candidate/stock repetitions per row; weighted controls
have three alternating native pairs. Screen seed 202609236, fresh 202609237,
pilot 202609238 and controls 202609239 were predeclared. `K` is numeric width;
4×3 and 5×3 factors add 11 and 14 finite columns. All rows use low-memory robust
OLS unless marked otherwise. Ratios are candidate/baseline within repetitions.

| Case, N / K / threads | Baseline / candidate / stock medians, ms | Paired min / median / max |
|---|---:|---:|
| One FE, 61440 / 10 / 4 | 148 / 129 / 23 | .858 / .872 / .879 |
| One FE, 4×3 factors, 61440 / 8 / 4 | 466 / 391 / 68 | .820 / .841 / .844 |
| One FE wide, 122880 / 31 / 8 | 2088 / 1662 / 125 | .795 / .796 / .796 |
| Two FE degree-six, 61440 / 10 / 4 | 219 / 200 / 38 | .909 / .913 / .922 |
| Three FE dense, 61440 / 10 / 4 | 253 / 231 / 87 | .911 / .913 / .931 |
| Fresh one FE serial, 98304 / 14 / 1 | 432 / 377 / 103 | .861 / .875 / .884 |
| Fresh one FE wide, 98304 / 40 / 4 | 2680 / 2106 / 201 | .770 / .786 / .786 |
| Fresh unbalanced FE and 5×3 factors, 81920 / 10 / 4 | 949 / 777 / 143 | .798 / .820 / .824 |
| Fresh weak two FE, 61440 / 10 / 4 | 250 / 228 / 101 | .903 / .906 / .912 |
| Fresh disconnected sparse three FE, 61440 / 10 / 4 | 312 / 294 / 682, unranked | .904 / .939 / .942 |
| Fresh one FE + all outputs, 81920 / 8 / 1 | 174 / 154 / 43 | .864 / .897 / .922 |
| Excluded fast wide, 122880 / 31 / 8 | 86 / 87 / 124 | .965 / 1.000 / 1.024 |
| Excluded low IID two FE, 61440 / 10 / 4 | 148 / 148 / 31 | .993 / 1.000 / 1.000 |
| AW one FE, 61440 / 10 / 4 | 154 / 133 / — | .851 / .858 / .869 |
| PW clustered two FE, 61440 / 10 / 4 | 182 / 173 / — | .939 / .945 / .951 |
| FW one FE, 61440 / 10 / 4 | 153 / 132 / — | .856 / .859 / .863 |

All eligible native timing pairs improve. Both excluded controls have median
paired ratios of 1.00; the fast control includes a +2 ms individual loss and
its independent medians differ by +1 ms. Do not hide that distribution behind
the rounded paired median. Eleven of twelve stock-comparable rows still lose;
only the existing fast wide control wins. Three repetitions support this
bounded, large-effect decision, not a universal performance claim.

## Science, memory and visible failures

All 135 headline calls pass their application sentinels, with no timeout,
detected overlap or monitor error. All 48 native scientific pairs are
byte-identical, as are all three native saved-output binary pairs. Repeat
checks pass. Direct candidate/stock checks pass 36/39; the three failures are
the same disconnected sparse three-FE covariance comparison below. The ten
pilot calls and four separate diagnostic calls also pass application checks.
All five pilot cases pass independently constructed dummy-design coefficients,
full covariance, sample and RSS checks on both native artifacts.

Twenty-seven distinct focused Rust tests pass: the new scalar-bit/cancellation/
error pair, thirteen robust, two score-storage and ten clustered tests. The new
pair also passes optimized compilation. Widths 0–129, signed zeros, cancellation,
large/small magnitudes, malformed slices, invalid products, sum/correction
overflow, corrupt scores and allocation failure are covered. The actual-plugin
ABI harness passes its deliberate panic-containment injection. Three licensed
fixtures pass: `ols_low_one_fe`, `ols_cluster_low_memory` and
`estimate_ols_eclass_transaction`, including explicit dummy regression,
unit/AW/FW/PW inference, literal FW expansion, rescaling, collinearity,
thread/pool controls, residuals and transaction rollback.

The change adds no allocation or scratch ownership. Tracked native peaks are
identical except in parallel multi-FE cells, whose observed pair differences
range from −7,209,792 to +6,914,496 bytes. Those measurements remain in the
receipts; this is not a tighter whole-action memory guarantee or constrained-
budget result. Static allocation inventory drift, weighted constrained failures
and nonretaining CG instability remain unresolved.

Keep two development failures visible. The first correction-overflow test used
a decimal input that scalar rounding correctly kept finite. It was replaced
with an exact half-ULP overflow boundary; both scalar and row helpers reject it.
The first isolated build (`gramrow1`) received repeated `--overlay` flags,
which replaced earlier paths and caused an unresolved import. `gramrow2` uses
one flag with all three files. Original logs, the actual failed overlay hash
and the amendment are archived. Neither failure produced a timed candidate.

## Three-FE comparator boundary

The disconnected sparse case has matching samples, coefficients and RSS, but
its raw stock covariance fails (maximum absolute difference 1.72e−7). Native
residual DoF is 48061; stock reports K=14880, implying residual DoF 46560.
Multiplying the stock covariance by 46560/48061 in a separate diagnostic gives
agreement within 3.06e−13 absolute error. This localizes the discrepancy to the
reported correction counts. It does not independently adjudicate the full-size
FE rank or replace the original failed comparison. No headline covariance,
DoF setting, sample or tolerance was rewritten, and the stock timing stays
unranked. Earlier three-FE comparator discrepancies remain separate records.

## Evidence and next subsystem

[Evidence archive](evidence.zip) and [SHA-256 receipt](evidence.sha256) contain
the source identities/overlays, declarations, original failures, controllers,
numerical exports, comparisons, independent checks, test receipts and sanitized
phase traces. All 4,163 manifest entries verify. Archive size is 22,583,688 bytes;
SHA-256 `16906f6e946de2056782a3c29efb995ebfb7244f19ea011019f3737c33a21b20`.
Original and portable hashes are retained. Licensed logs, data and binaries
are excluded.

Next profile the low-memory finite fit in detail: it still consumes roughly
one second of the 1.66-second wide command, before the now faster robust
composer. Separate compensated moment construction, QR updates and reconstruction
before selecting a broader implementation. Production covariance is another
366 ms, but its accepted influence arithmetic must remain stable. Do not reopen
closed window/serial-slope tuning. Factor-base provenance, generated time-series,
multi-partition slopes, broader memory/topology coverage and SCC deferrals remain
explicitly open; this milestone does not complete the broad optimization goal.
