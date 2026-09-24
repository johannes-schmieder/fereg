# Work-balanced robust PPML inference

Local macOS arm64 development evidence, 2026-09-24. Adopt `inferwork1` under
[ADR 0160](../../../../docs/adr/0160-work-balanced-ppml-robust-reduction.md).
This small shared improvement removes avoidable underutilization in wide robust
inference. It does not complete the per-case 0.90-times-stock objective. Further
optimization is now scoped by the user to designs with at least two FE partitions;
one-FE cases below were declared and executed before that scope change.

## Source and evidence identities

Baseline `onefecentereddense5` is numerical implementation `fb617b80`, unchanged
at `0abf50fa`, plugin SHA-256
`887001e476148c757b10362e09bc5ae7349b147d0129ed1ad00e5e3f6dcefbfc`.
Candidate is `0abf50fa` plus `ppml/vce.rs`, `ppml/memory.rs` and the new
`feppml_wide_inference_oracle.do` fixture. All **545 captured computational
inputs match** the adopted worktree. Candidate plugin SHA-256:
`685bb0102fca0e95f6645c63449b3c8ba1ae383de8ac7a7634dab13f4a58d894`.

Stock is the pinned unmodified fixest 0.14.2 archive, SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched application threads and serial
BLAS. Both clocks include robust inference and fitted means. Process startup,
generation/import, warmup and exports are outside both clocks. Low native mode
is not a physically memory-matched comparison. No default-reference failures
are relabeled by these strict-reference results.

The [content-addressed archive](receipts-content-addressed.zip) contains
**6,200 logical receipts**, 1,565 unique payloads, 11,485,722 bytes, SHA-256
`4b93fd77244f4e46a294909214bd9fbe4aa10c2e9ed087665e7ef769072cc79f`.
The [restore tool](restore_receipts.py) independently verified and restored all
logical files under `work-balanced-inference/`. Full artifact sources, build
recipes/logs, tests, public exports, audits, timing controllers and profiles
are included. Raw licensed logs and binary/data payloads are omitted with hashes;
local paths are sanitized. Exact pinned-source inspection hashes are retained.

## Whole-command distributions

Three alternating baseline/candidate/stock rotations, sequential and uncontended.
No compilation, profiling, archiving or competing fits overlap headline runs;
all monitors have empty overlap/error lists. All numbers are milliseconds in
repetition order, not sorted. Ratios use medians from the same block.
Development seed 202609347; predeclared fresh **202609373 is now consumed/frozen**.

### Development

| Case | Baseline | Candidate | Stock | Paired C−B | C/stock |
|---|---|---|---|---|---|
| One FE N30720/K32/T8 | 93, 94, 93 | 90, 89, 85 | 96, 95, 102 | -3, -5, -8 | 0.927 |
| One FE N16320/K64/T4 | 152, 151, 154 | 141, 144, 143 | 165, 167, 162 | -11, -7, -11 | 0.867 |
| One FE unbalanced N30720/K3/T1 | 50, 56, 54 | 60, 56, 52 | 25, 26, 25 | 10, 0, -2 | 2.240 |
| Two FE N16320/K12/T4 | 68, 71, 68 | 70, 68, 68 | 55, 53, 54 | 2, -3, 0 | 1.259 |
| Two FE slopes N16320/K6/T4 | 69, 71, 68 | 68, 68, 69 | 64, 65, 67 | -1, -3, 1 | 1.046 |

### Fresh

| Case | Baseline | Candidate | Stock | Paired C−B | C/stock |
|---|---|---|---|---|---|
| One FE N24960/K24/T4 | 68, 70, 67 | 72, 66, 69 | 55, 54, 58 | 4, -4, 2 | 1.255 |
| One FE N23040/K48/T8 | 113, 104, 122 | 105, 105, 103 | 152, 154, 138 | -8, 1, -19 | 0.691 |
| One FE N30720/K32/T1 | 169, 170, 171 | 162, 164, 162 | 282, 277, 277 | -7, -6, -9 | 0.585 |
| Two FE N20160/K20/T4 | 103, 106, 104 | 103, 100, 98 | 88, 86, 92 | 0, -6, -6 | 1.136 |
| Two FE slopes N20160/K16/T4 | 112, 107, 112 | 113, 109, 108 | 155, 148, 155 | 1, 2, -4 | 0.703 |
| One FE N24960/K24/T4 low | 69, 72, 70 | 70, 75, 69 | 57, 58, 64 | 1, 3, -1 | 1.207 |
| Two FE weak_slope_barbell N10080/K4/T1 | 42, 45, 38 | 37, 39, 43 | 721, 713, 711 | -5, -6, 5 | 0.055 |

### Ols

| Case | Baseline | Candidate | Stock | Paired C−B | C/stock |
|---|---|---|---|---|---|
| One FE N10080/K6/T1 | 10, 11, 13 | 13, 13, 12 | — | 3, 2, -1 | — |
| Two FE N10080/K8/T4 | 17, 18, 18 | 17, 17, 17 | — | 0, -1, -1 | — |

Development wide medians improve 4.3% and 5.9%; all six pairs win. Fresh wide
K48 improves 7.1%, but includes a +1-ms pair. Fresh two-FE K20 improves 3.8%,
with 0/-6/-6-ms pairs. Slopes improve at the median but lose two of three pairs;
the mechanism's contribution there is small. Narrow serial loses 2 ms at its
median; fresh K24 fast loses 1 ms and low ties. Serial K32 improves as a whole,
but its inference span increases slightly: do not attribute that whole gain to
parallel reduction. The OLS one-FE +2-ms median loss remains visible despite
exact exports and an unchanged route. No universal non-regression claim.

The new scoped target still has categorical losses: fresh two-FE K20 is 100 ms
against 88 ms stock (79.2-ms target); development K12 ties at 68 against 54
(48.6-ms target). The fresh wider slope and weak cases clear 0.90, whereas the
narrower development slope case is 68 against 65 (58.5-ms target). These claims
belong to this captured artifact and time block only.

## Attribution and computational organization

Separate baseline `inferledger1` and candidate `inferworkledger1` diagnostic
builds add scoped clocks. Their full source identities are in the archive;
candidate diagnostic plugin is
`c54affb1cd56d9304fc5980153041447a2b7eab3746de3a21acb0654c6d399a7`.
Each has two repetitions of four cases; all eight applications per build pass.
Nested clock summaries reconcile exclusive intervals to the plugin root. These
are attribution, not stock-comparator headline observations.

| Diagnostic phase | K32/T8 baseline → candidate ms | K64/T4 baseline → candidate ms |
|---|---|---|
| Two independent meats, rep 1 | 5.427 → 1.796 | 15.679 → 5.076 |
| Two independent meats, rep 2 | 6.004 → 1.684 | 15.637 → 5.045 |
| Entire inference, rep 1 | 6.701 → 3.098 | 22.534 → 12.357 |
| Entire inference, rep 2 | 7.344 → 2.973 | 22.939 → 11.912 |
| Production PSD, rep 1 | 0.723 → 0.745 | 4.856 → 5.233 |
| Independent publication PSD, rep 1 | 0.720 → 0.709 | 5.065 → 5.215 |

K32/N30720 changes two row blocks to 31; K64/N16320 changes one to 64. The two
sandwich products remain about 0.2 and 1.6 ms. The production and independent
publication eigensystem checks remain substantial at K64, about 10 ms together.
Narrow K3 and slope K6 retain their original row policy, with baseline meat spans
about 0.4 ms; inference cannot explain their larger public losses.

Pinned-source inspection (`reference-inspection.json`) finds that
`vcov_hetero_internal` computes score-times-bread and a crossproduct. C++
`matprod_XtX` partitions upper-triangle entries across OpenMP workers, scanning
contiguous observation columns. This establishes a concrete difference from
our old row-only task count. It is not a claim of measured stock phase times.
No instrumented reference timing is mixed into the stock column above.

The retained solution uses the existing row-oriented reducer with a work-based
block size. Both independent meats, separate compensated opposite-order merges,
current weights, sandwich, PSD and publication checks remain. The changed block
tree is identical across requested thread counts. No solver, rank, tolerance,
DoF, separation, sample or transactional-publication rule changes.

## Memory and science

The K32 partial bank grows 33,792→523,776 bytes and K64
66,560→4,259,840 bytes. Admission conservatively adds a checked bound for every
rank up to admitted width, on top of the prior robust inference allowance.
The K64 selected forecast grows 36,086,810→38,745,609 bytes; other screened
selected forecasts are unchanged because other phases dominate. Every measured
PPML tracked peak is unchanged. Phase inference forecasts rise, including on
narrow cases whose actual partition remains unchanged. This conservative
admission may change routing under tighter budgets; physical-cap qualification
and static allocation inventory repair are not claimed.

**104 distinct focused Rust tests pass**: 16 inference, 51 memory, seven PSD,
30 result/publication checks, on the exact captured candidate source. The new
independent scalar robust sandwich checks unit/FW, N8193/K24 and T1/T4 exact
covariance reproducibility. Rank-bound checks cover all smaller ranks under
nine widths through 4096, byte-cap transitions and overflow refusal.

**Five licensed fixtures pass**, with process status, Stata return code and
terminal sentinels: new wide explicit-dummy Poisson/Hessian/sandwich oracle,
centered dense oracle, one-FE dummy oracle, allocation/warm-fresh profile audit,
and multipartition slope oracle. The wide fixture covers 12 combinations of
unit/FW/PW, robust/clustered and T1/T4, N8192/K24. Its independent Mata sandwich
uses FW multiplicity and PW squared scores. Production ABI/panic checks pass.

All 120 headline applications pass; all 108 PPML states pass independent sample,
objective replay, score and model-space checks. All 36 native PPML pairs and
36 direct candidate-stock comparisons pass the existing envelopes. Native
coefficients, means and objective values remain exact. Covariance rounding
changes are bounded by the unchanged oracle/comparator criteria. All six OLS
pairs have exact scientific exports. The source-specific summary retains errors
and memory values per cell.

The initial diagnostic controller omitted a required archive argument and failed
at CLI validation before any fit; both the failed launch and corrected script
are preserved. There are no final application failures, timeouts or scientific
exclusions in this screen. Existing unresolved comparisons/failures are neither
rerun nor repaired here. Frozen 77/16/111 definitions remain intact.

## Next bounded target

Keep this improvement without threshold tuning. Focus further work on at least
two FE partitions. Rebuild the current categorical two-/three-FE cost inventory,
including serial/narrow, disconnected, unbalanced and low-memory cases. Prior
joint coefficient replacements are closed under ADRs 0155/0157; do not reopen
those wrappers without a materially different cost model. Profile common native
loading/preparation and the remaining ordinary categorical operator passes, then
choose the larger avoidable cost. Stronger independent checks remain mandatory.
No push, release, broad qualification, stopped campaign or SCC retry follows.
