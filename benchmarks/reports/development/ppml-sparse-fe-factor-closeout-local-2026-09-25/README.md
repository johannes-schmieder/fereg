# Sparse FE-factor closeout — local 2026-09-25

Both isolated implementations are rejected. Production retains **higherdim1**
from **3e1d9120**, with all **565 captured computational inputs** matching the
closing worktree. The later planning commit 0b858a64 changes no runtime input.
[ADR 0204](../../../../docs/adr/0204-sparse-fe-factor-closeout.md) records the
decision. This is bounded development evidence, not qualification or completion
of the multi-FE <=0.90-stock objective.

## Source and measurement identities

| Artifact | Parent | Plugin SHA-256 | Role |
|---|---|---|---|
|higherdim1|d46b584e plus two overlays; matches 3e1d9120|92df56c44f5c40aabc4c297e3d2bbc863d528f0fe3f0ce395c7a7fb6723e50a2|Accepted baseline|
|sparseframecapture1|3e1d9120|1732f5fcf88403d74558c1491efb9735c66bd9e083bb16d9853df33135c463ff|Actual weighted-frame/native-boundary capture|
|sparsepublic1|3e1d9120|768f57bb5a8feeb7ee5b543158165cc6d01892fb0b980c700cbb770812f2b186|First public prototype|
|sparsecanonical1|0b858a64|950eaabbdaf32fdc898d1bcefb07606b886ba1ce5a4e78b34f65c352c4112ad9|Bounded correction|

Full source maps, builds, ABI self-test receipts and a fifth diagnostic artifact,
**sparseprofile1**, are in the archive. Diagnostic clocks never replace headline
clocks. The pinned unmodified fixest source archive SHA-256 is
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
The comparator uses `glm.tol=1e-10`, `fixef.tol=1e-11`, matching threads,
robust inference and fitted means. Ordinary reference weights are not used as
an FW inference oracle.

All headline screens reuse development seed **202609347**. No new validation seed
is consumed. No compilation, profiling, audits or archiving runs alongside their
timed calls. All sampled monitors are clean. The separate blocks below retain
their own baseline and reference distributions; they are not pooled.

## Actual weighted frames and kernel feasibility

The capture records original response, generated numeric controls, FE codes and
current solver-normalized weights at actual canonical and ordinary entries.
Capture I/O precedes its native phase timer. Two baseline/capture pairs have exact
coefficient, covariance and mean exports; all four independently audited states
pass. The large case supplies five full-size frames, including initial tolerance
1e-6, scheduled ordinary tolerances and strict 1e-12. The narrow case supplies two.

The diagnostic constructs the signed full bipartite FE Laplacian, grounds one
vertex per component, orders remaining vertices by static degree, diagonally
scales and factors with sparse LDL. Symbolic reach/fill persists; numeric factors
refresh for changed weights. Components and compatible RHSs run on the bounded
team. Fresh original-row moments check projection independently of the factor.

An independently assembled sparse explicit-design weighted LSMR oracle checks
**seven frames / 41 columns**. Maximum weighted relative error is **7.853e-14**;
T1/T4 residual binaries agree exactly. Large T4 symbolic setup costs **7.303 ms**,
with 169,957 factor nonzeros and **12,520,344 counted owned bytes**. Numeric
factor plus projection/fresh moments costs approximately **3.41–3.45 ms** per
frame. T1 costs approximately 12.98–13.25 ms. Narrow T4 symbolic setup is about
2.56 ms and the combined frame cost about 0.77 ms, with 3,186,376 owned bytes.

These kernel payloads exclude input-frame and other public owners. The clocks
exclude canonical finite rank, complete products, scratch/output writes and
strict finalization. They do not establish that independently projecting every
control is comparable to an ordinary joint direction. This limitation motivated
the public test and its correction.

## Complete public results

Case order throughout:

1. Disconnected two categorical FE, N61440/K6/T4, low memory.
2. Narrow two categorical FE, N24960/K2/T4, fast memory.
3. Two intercept-plus-slope partitions, N30720/K6/T8, low memory; excluded control.

The initial accepted-source screen has fereg/stock vectors, milliseconds:

| Case | fereg | Stock | Medians |
|---|---|---|---|
|1|109,105,106|101,100,102|106/101|
|2|36,37,37|34,33,35|37/34|
|3|72,70,71|81,81,82|71/81|

The first public integration installs sparse projection in the categorical owner
and disables the coefficient owner and fused canonical builder for two-category
fits. Full WLS then replaces ordinary joint directions. Its existing fresh
original-weight projection certificate remains mandatory.

| Case | Baseline | sparsepublic1 | Stock | B/C/stock medians |
|---|---|---|---|---|
|1|106,107,105|156,155,154|98,99,97|106/155/98|
|2|37,37,36|43,43,43|34,34,37|37/43/34|
|3|71,72,74|72,71,76|81,82,78|72/72/81|

The first large pair retains four outer iterations and zero strict rejections,
but projection telemetry increases **24.25->57.67 ms** and finite solving
**7.50->13.93 ms**. A separate profile confirms T4 execution and numerical factor
cost near 1.5 ms per changed frame. The full-WLS organization loses the kernel's
advantage. No claim that iteration counts represent equal operations is made.

The correction retains ordinary joint directions and all fused canonical
consumers. A fit-owned symbolic factor serves the initial/strict canonical
coefficient proposal under each exact current-weight frame. The existing direct
residual Gram, original-input certificate and output reconstruction consume it;
there is no second N-by-K input bank in this correction.

| Case | Baseline | sparsecanonical1 | Stock | B/C/stock medians | C/stock |
|---|---|---|---|---|---:|
|1|106,109,107|108,108,108|94,97,99|107/108/97|1.113|
|2|37,38,38|40,39,43|31,32,32|38/40/32|1.250|
|3|75,71,71|73,80,70|80,85,81|71/73/81|0.901|

There are **three native paired wins and six losses**; all narrow pairs lose.
No call is discarded. All 27 applications finish and every scientific comparison
passes. The excluded slope loss remains visible; it is not attributed to a
changed slope algorithm. The correction is closed without a fresh seed or
another ordering/threshold/phase experiment.

Large-case phase medians B/C, milliseconds, from the uninstrumented correction
pilot: preparation **16.41/16.37**, projection **24.66/29.83**, finite solving
**7.59/7.55**, strict final solving **17.05/13.69**, publication **9.19/8.88**.
Thus strict work improves about 3.4 ms while projection/setup adds about 5.2 ms.
Phase medians are not an additive attribution of median public time. Both use
four outer iterations and zero strict rejections. The recorded inner counts
69/33 mix direct solves and iterative work and are not comparable operation units.

## Science, memory and limitations

Each public prototype passes three licensed fixtures: iterative canonical
explicit Poisson/Hessian oracle, weight/inference oracle and PPML scientific
contract. Six fixture executions pass with explicit return codes and sentinels.
Across baseline, capture, both pilots and the diagnostic profile, **80 public
states and 49 baseline-anchored pairs** pass independent/public export audits.
Both pilot direct-stock blocks pass all **eighteen pairs**. The seven-frame
projection oracle is additional evidence, not a replacement for the public
Poisson checks. Corrected native differences are bounded by **8.999e-17** in
coefficients, **5.930e-21** in covariance and **2.310e-14** in means.

Corrected tracked peak memory rises **6,675,495 bytes** in the large case and
**3,151,600 bytes** narrow; the excluded slope peak is unchanged. Scratch remains
2,949,120 / 0 / 1,474,560 bytes. Modeled selected bytes are unchanged because
complete new-owner admission is **not implemented**; this is an explicit missing
production gate, not evidence that the new factor fits the old plan. Kernel and
public peak measures cover different lifetimes and must not be equated.

No new Rust unit-test campaign, OLS timing block, physical cap, Linux run, SCC
retry, qualification, push or release is claimed. Runtime on main is unchanged,
so the earlier OLS losses are neither repaired nor re-adjudicated. Existing
constrained-fit failures, allocation-inventory drift and disconnected scientific
discrepancies remain open. An initial builder marker assertion fails before
compilation; its failed script/log and corrected builder are preserved. All
actual builds pass ABI containment, including the deliberate panic self-test.

## Preserved evidence

`receipts-content-addressed.zip` contains **5,137 logical receipts**, deduplicated
into **1,139 payloads**, **9,497,674 bytes**. SHA-256:
`ab6768510a28f99139a059c63a33ce2263e95066ccaf8f118256f49d0df67f9c`.
Restore and verify using `restore_receipts.py` and a new destination directory.
Raw licensed headers, machine paths, data and compiled binaries are excluded;
original/archived hashes and omitted-data/binary hashes preserve their identities.
Source, build recipes, declarations, result exports, audit outputs, diagnostic
protocols, probe implementation and failed attempts are retained. The preceding
interim planning report remains unchanged.
