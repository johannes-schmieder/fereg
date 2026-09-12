# ADR 0044: sequential low-memory clustered score consumption

Status: accepted for local development, 2026-09-12.

## Decision

Low-memory OLS clustered inference may accumulate compensated group scores in
original sample order when the complete Cartesian group-score table and presence
flags fit within the displaced `N * sizeof(usize)` tuple-ordering budget.
The table replaces the order allocation; they never coexist. Larger or
unrepresentable Cartesian products retain the existing stable tuple-sort route.
Actual reservations remain fallible and capacity checked.

The accumulator is shared with the existing dense-score route. Scratch inference
still finishes occupied groups in lexicographic order through its original
per-group bread transformation, compensated covariance sums and independent
certificate. It does not select the dense parallel certificate policy.
Within each group, rows and floating-point operations retain their original
order. The final-sample codes, weights, reported coordinates, multiway subset
order, inclusion-exclusion, DoF, PSD handling and publication checks are unchanged.

There is no retained N-by-K score copy, per-worker group bank, new SPI access,
solver region or expanded memory admission. The score scratch files, row tile,
score reconstruction and number of subset passes remain. Reading each eligible
subset sequentially avoids repeatedly loading its scratch tiles in group order.
PPML's paired-independent reducer remains separate and unchanged.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/cluster-grouping-local-2026-09-12/README.md)
records thirteen complete-command pairs against `2243b4d`: six repeated primary
pairs, six protected controls and one separate traced pilot. Paired gains are
16.4% for FW one-way clustering and 48.2% for PW two-way clustering. Primary
native peaks and scratch peaks are unchanged. Every scientific export, saved
output and non-time solver diagnostic agrees exactly.

Nine distinct focused Rust tests cover existing/legacy covariance arithmetic,
allocation failures and the precise table-admission boundary. The independent
Stata full-dummy oracle passes both artifacts for unit/AW/FW/PW and full/deficient
rank. Universal static/native arm64 ABI checks pass. A reviewed rebuild is
byte-identical to the measured prototype.

The weak low-memory control improves but retains a substantial pre-existing
observed/reported memory-model discrepancy in both arms. This is not a qualified
memory pass. No protected control slows in the bounded comparisons, but their
single-pair timings are not repeatability claims. Additional subset-pass fusion,
score reconstruction, interactions and memory-model work remain open under #27.
