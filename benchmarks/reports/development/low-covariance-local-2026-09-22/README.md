# Low-memory robust covariance: compensated influence rows

Accept reuse of the existing compensated row helper in production scratch
robust covariance. The wide public command improves 1046→945 ms, with fresh
K40 confirmation at 1305→1155 ms and unbalanced factors at 523→480 ms. Narrow
and three-FE effects are smaller or flat. The change adds no buffer or thread
team and preserves every numerical export in the native comparison.
See [ADR 0112](../../../../docs/adr/0112-scratch-influence-covariance-rows.md).

## Source and comparable work

Baseline `lowchol2` is the computational checkpoint committed at `fc2b0fc4`,
plugin SHA-256
`45e2c724d80ee38e45d935b5c0d658c45f3d9c144b8f888041fd11a4d0b3b1da`.
Candidate `covrow1` is an isolated build of that commit with the archived
`sandwich.rs` and `prepared_unadjusted.rs` overlays. All 529 captured inputs
match the accepted working source. Candidate plugin SHA-256:
`6f4ad854b739f5fd694f51f98f507dcc0e865d96ec4486564e6314dea0b07162`.
Diagnostic builds, controllers, source overlays, fixtures and reference
identities are separately hash-bound in the archive.

Stock fixest 0.14.2 is uninstrumented. Its pinned archive SHA-256 is
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`;
loaded library SHA-256 is
`bece258f6c6fc3fc47accea45c7b004b0c495d46170d78fa2d6302e66de5fbf9`.
Native `memory(low)` is compared with ordinary stock execution, not an equal
memory limit. Import/export are outside timing. Native loading, estimation
and publication are included in fit time; requested prediction cost is
separate and added to command time. Regressors, threads, outputs, samples and
inference match except for the explicit DoF discrepancy below. Weighted
controls compare native artifacts only; ordinary fixest weights are not used
as a Stata FW inference oracle.

No compilation, profiling, oracle computation, archiving or competing fits
run beside headline measurements. All 155 headline attempts pass without
timeout, detected overlap or monitor error. Frozen 77/16/111 definitions,
reserved seeds and reference pins are unchanged. No stopped empirical campaign,
SCC retry, qualification, push or release was performed.

## Profile and change

The current `lowchol2` trace puts about 360 ms of the wide command into
production robust covariance. An isolated per-row diagnostic splits that
into 187 ms for compensated bread multiplication, 165 ms for influence
outer-product accumulation and 12 ms for score loading. Its clocks perturb
absolute times and are excluded from headline results; all three diagnostic
pairs reproduce numerical exports exactly.

Preserve the serial influence calculation. The reducer still computes each
bread-times-score coordinate with the original compensated scalar loop, then
applies `(sqrt(HC1) * weight_root) * residual`. It still accumulates the upper
triangle of the resulting influence outer product in observation order.
It does not switch to a raw score meat followed by a sandwich product.

The outer triangle now borrows contiguous suffixes of the existing influence,
covariance and correction vectors and calls ADR 0110's compensated row helper.
The helper moves to the shared sandwich module, with the internal OLS re-export
retained. Existing score-Gram and independent covariance certificate calls
therefore use the same body as before. Each coordinate keeps its multiplication,
Neumaier operations and observation order. Slice shapes are checked before
mutation; every nonfinite product/sum/correction is remembered before returning
from a private row. The incomplete covariance cannot be published.

The candidate's diagnostic outer phase falls to 71 ms, while bread multiplication
remains 188 ms. Its uninstrumented-source trace reports 261 ms for total
production covariance, 168 ms for score-design construction/certification and
110 ms for the independent covariance certificate. At reported dimension 20,
outer accumulation changes 34.2→16.6 ms; at dimension 11, 10.1→7.1 ms.
The candidate diagnostic also has exact exports in all three pairs.
The new `--profile-serial-sandwich` harness flag only modifies isolated copies.

The change reaches robust OLS wherever score storage uses this scratch reducer.
Dense robust, IID and clustered calculations, PPML iteration state and all
sample/rank/DoF/weight/solver policies are unchanged. Independent certificates
remain mandatory. Four existing fallible workspaces are retained; there is no
new cache, allocation, worker team or reduction tree.

## Alternating distributions and losses

Three rotated baseline/candidate/stock repetitions per unweighted row and
three alternating native pairs per weighted row. Seeds 202609246 (screen),
202609247 (fresh), 202609248 (pilot), and 202609249 (weights) were predeclared.
Diagnostic seed 202609245 is separate. K is numeric width; 4×3 and 5×3 crosses
add 11 and 14 finite columns. Rows use low-memory robust inference unless
marked otherwise. Ratios compare paired repetitions, not independent medians.

| Case, N / K / threads | Baseline / candidate / stock medians, ms | Paired min / median / max |
|---|---:|---:|
| One FE, 61440 / 10 / 4 | 101 / 97 / 24 | 0.960 / 0.960 / 0.980 |
| One FE, 4×3 factors, 61440 / 8 / 4 | 305 / 289 / 69 | 0.928 / 0.948 / 1.020 |
| One FE wide, 122880 / 31 / 8 | 1046 / 945 / 128 | 0.897 / 0.903 / 0.906 |
| Two FE degree-six, 61440 / 10 / 4 | 172 / 170 / 36 | 0.982 / 0.988 / 1.179 |
| Three FE dense, 61440 / 10 / 4 | 208 / 207 / 89 | 0.957 / 1.000 / 1.005 |
| Fresh one FE serial, 98304 / 14 / 1 | 271 / 252 / 106 | 0.926 / 0.930 / 0.934 |
| Fresh one FE wide, 98304 / 40 / 4 | 1305 / 1155 / 208 | 0.879 / 0.884 / 0.918 |
| Fresh unbalanced FE and 5×3 factors, 81920 / 10 / 4 | 523 / 480 / 138 | 0.914 / 0.915 / 0.918 |
| Fresh weak two FE, 61440 / 10 / 4 | 205 / 198 / 97 | 0.933 / 0.966 / 0.980 |
| Fresh disconnected three FE, 61440 / 10 / 4 | 264 / 257 / 663, unranked | 0.927 / 0.996 / 1.011 |
| Fresh one FE + all outputs, 81920 / 8 / 1 | 121 / 116 / 44 | 0.950 / 0.959 / 0.976 |
| Excluded fast wide, 122880 / 31 / 8 | 85 / 87 / 125 | 1.012 / 1.012 / 1.082 |
| Excluded low IID two FE, 61440 / 10 / 4 | 121 / 120 / 33 | 0.976 / 1.000 / 1.008 |
| AW one FE, 61440 / 10 / 4 | 106 / 103 / — | 0.954 / 0.972 / 0.981 |
| Excluded PW clustered two FE, 61440 / 10 / 4 | 148 / 146 / — | 0.967 / 0.986 / 1.000 |
| FW one FE, 61440 / 10 / 4 | 104 / 100 / — | 0.935 / 0.962 / 0.971 |
| Five-pair two-FE confirmation, same data | 180 / 173 / — | 0.929 / 0.945 / 1.006 |
| Five-pair fast wide control confirmation | 86 / 86 / — | 0.989 / 1.000 / 1.023 |

Wide paired median gains are 9.7% and 11.6%; fresh unbalanced factors gain 8.5%,
and fresh serial K14 gains 7.0%. Three-FE results are essentially flat. Smaller
narrow gains are retained because the shared helper is already tested and adds
no storage or dispatch rule. Eleven of twelve stock-comparable rows still lose;
the fast control wins. The disconnected stock row remains unranked.

Do not discard the original two-FE candidate call at 204 versus 173 ms, the
factor +6 ms loss, or the fast-control 92 versus 85 ms call. Before further
measurement, a separate five-pair confirmation was declared for the exact same
two-FE data and an additional fast wide control at seed 202609246. Both warm
and full two-FE input binaries match the original hashes. No candidate or
routing edit followed the timing observations. Confirmation does not reproduce
the large two-FE loss (maximum +1 ms), and fast-control medians are both 86 ms,
with individual losses up to +2 ms. The original distributions remain visible.
No claim of universal per-call improvement follows.

## Science, memory and boundaries

All 58 headline native pairs and repeat checks have exact scientific exports;
all three saved-output binary pairs are identical. All ten pilot calls and
twelve diagnostic calls pass. Five pilot designs independently verify full
coefficients, covariance, retained sample and RSS on both native artifacts.
Direct candidate/stock checks pass 36/39. The three failures are the disconnected
three-FE covariance: native residual DoF is 48079 versus stock 46560, with
original maximum covariance difference 1.73e−7. Diagnostic stock covariance
rescaling by `46560/48079` agrees within 1.29e−13. This neither adjudicates
full-size rank independently nor changes the original failed comparison.

Thirty-one distinct Rust tests pass, including the four serial tests under
optimized compilation. New full-reducer comparisons cover widths 0–129,
vector tails, signed zeros and extreme scales against a separately written
scalar loop. Nonfinite bread, influence products and covariance sums fail
closed. Existing row-helper tests cover malformed slices, cancellation,
product/sum/correction overflow at several bad-coordinate positions; existing
robust/clustered corruption and reservation tests pass. The reducer still makes
exactly four workspace reservations, including rank-zero and injected failure
controls. Actual-plugin ABI/panic containment and three licensed fixtures pass:
`ols_low_one_fe`, `ols_cluster_low_memory`, and
`estimate_ols_eclass_transaction`. These include independent explicit dummies,
AW/FW/PW rules, literal frequency expansion, rank, pool/thread controls,
residuals and transaction rollback.

Tracked native peaks are identical outside parallel multi-FE variation, whose
observed pair differences span −1,597,472 to +504,616 bytes. There is no new
buffer or admission policy and no tighter whole-action memory claim. The
static allocation audit still fails source drift; no inventory was regenerated.
Previously documented formatting drift, weighted constrained-fit failures,
nonretaining CG span instability and SCC/Linux deferrals remain open.

## Next subsystem

Close this bounded covariance experiment. Compensated bread multiplication
and scratch moments still cost time, but narrow gains now approach noise and
stock's unconstrained memory strategy remains substantially faster here.
Return to the coverage inventory with a small current-source screen of
generated time-series terms and multi-partition slopes, including sample
changes and independent explicit-design oracles, before selecting another
optimization. Factor-base provenance and prior comparator discrepancies also
remain open. Do not reopen closed serial-slope/window experiments or infer
that historical timings apply to this artifact.

This is a local development milestone, not completion of the broad goal or
release qualification.

## Immutable evidence

[Receipts](receipts.zip) and [checksum](receipts.sha256) contain 5,166 verified
entries plus the hash manifest (24,224,575 compressed bytes). SHA-256:
`03cef9b62b12c4799cf7eb949a1268d49fe7340e8dcac08264bc0e0045c7ac17`. All archived hashes verify, and machine paths are sanitized.
Data, binaries and licensed Stata logs are excluded. Original log hashes,
fixture protocols, source/build identities, numerical exports, declarations,
confirmation amendments, individual losses and the failing static audit remain.
