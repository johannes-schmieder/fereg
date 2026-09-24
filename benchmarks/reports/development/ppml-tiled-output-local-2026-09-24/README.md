# Tiled canonical PPML output: local development evidence, 2026-09-24

## Decision and scope

Adopt `canonicaltiles2` under [ADR 0173](../../../../docs/adr/0173-tiled-canonical-ppml-reconstruction.md).
It removes the full eliminated-group coefficient bank as a mandatory output
allocation for low two-categorical-FE canonical PPML. The existing grounded
component Schur solver, all-RHS solves, independent original-input residual
certificate, direct residual Gram and canonical rank/inference consumers remain.
Affordable full-bank paths remain preferred. Weights and group masses remain
immutable through bounded row reconstruction; no cache crosses a weight frame.
Memory allowances, work guards, samples, tolerances and scientific policy do not
change. This is local development evidence, not qualification.

Two fresh threaded wide cases improve 18% and 21%; every native pair improves.
The newly admitted serial case loses 3.3% in every pair, while retaining a large
stock advantage. That is an accepted tradeoff, not a zero-regression claim.
This bounded storage experiment is closed. Narrow/fast losses and reduction-
limited wide admission remain open; the broad <=0.90× stock goal is unfinished.

## Exact artifacts

All timings below use uninstrumented plugins. Diagnostic timings are separated
later. No timings were taken during compilation, tests, profiling, archiving or
other agent-owned fits. All completed headline monitors report empty overlap
and error lists. The unrelated GUI Stata process was left untouched.

| Artifact | Source | Plugin SHA-256 |
|---|---|---|
| Baseline `canonicalstream3` | Captured 91fa4c7600cfc39002dff573f40ea30682039305 plus canonical_products.rs; all 548 inputs match b6088330 | 49df141053ad0e99603fb33147012a2e1cd2cd8157ca8d2c1470b3f7590b5399 |
| Intermediate `canonicaltiles1` | b6088330f1c1c46ea6946d66b35fdab33cc6fdaf plus three source files; built/tested, no headline fits | af3c898d1a6b8491a8f102a301b3c55fabd206cde0df142a332c5c9293cd3edb |
| Accepted `canonicaltiles2` | Same base plus canonical_products.rs, prepared_scratch.rs and linear_session.rs; all 548 captured inputs match final source | e7c6d4bd81433f0a7067107bf3c167e02146845122c9c53e057673beccc2f529 |

The final artifact adds an explicit response-overlap bound for platforms where
indices occupy fewer bytes than doubles, clarifies comments, and formats the
changed scratch methods. Its arm64 arithmetic matches the intermediate. Both
complete source inventories, builds and recipes are archived. The safe core,
main-thread SPI boundary and transactional publication remain unchanged.

Comparable reference is the pinned, unmodified fixest 0.14.2 stock library,
archive SHA-256
17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9.
Its existing public comparator uses glm.tol=1e-10 and fixef.tol=1e-11, robust
inference and fitted means. Default-reference discrepancies remain separate.
Ordinary fixest weights are not used as a Stata frequency-weight oracle.
Native threads match each declaration; BLAS helper threads are fixed at one.
These are warm public fits: the established harness first fits a separate
<=7680-row warmup input. They are not cold first-command timings.

## Fresh distributions and losses

Seed **202609389** is consumed/frozen. Five rotating baseline/candidate/stock
repetitions per cell; all ten designs use robust inference and saved means.
Low storage unless the row says fast. Values are medians [minimum–maximum] in
milliseconds. Ratio is the ratio of medians; the last column is the full range
of matched candidate/baseline ratios. No scientific discrepancy is ranked.

| Design | Baseline | Candidate | Stock | Candidate/stock | Paired C/B range |
|---|---:|---:|---:|---:|---:|
| Connected very wide, N22560/K142/T8 | 894 [890–908] | 735 [728–738] | 685 [678–699] | 1.073 | 0.8018–0.8270 |
| Larger connected very wide, N44880/K146/T8 | 1512 [1503–1518] | 1198 [1192–1224] | 1290 [1279–1304] | 0.929 | 0.7885–0.8137 |
| Serial very wide, N17280/K138/T1 | 1320 [1313–1328] | 1364 [1359–1367] | 2761 [2748–2768] | 0.494 | 1.0233–1.0373 |
| Already admitted wide, N28800/K104/T4 | 567 [565–569] | 570 [567–571] | 891 [879–893] | 0.640 | 1.0018–1.0106 |
| Disconnected, reduction-refused, N55200/K102/T8 | 1097 [1087–1106] | 1100 [1094–1104] | 954 [946–961] | 1.153 | 0.9892–1.0156 |
| Narrow, N22560/K2/T4 | 49 [48–51] | 48 [48–49] | 31 [31–32] | 1.548 | 0.9412–1.0208 |
| Fast mode, N22560/K10/T4 | 67 [66–71] | 65 [64–66] | 57 [55–57] | 1.140 | 0.9155–0.9851 |
| Two slope partitions, N22560/K10/T4 | 106 [103–106] | 104 [104–105] | 114 [111–116] | 0.912 | 0.9811–1.0097 |
| Three FE, unbalanced, N24480/K10/T4 | 92 [92–94] | 92 [91–95] | 95 [93–96] | 0.968 | 0.9681–1.0326 |
| Two FE, unbalanced, N24240/K28/T4 | 112 [111–113] | 111 [110–113] | 114 [113–116] | 0.974 | 0.9821–1.0180 |

Only two of these ten selected medians meet the full 10% lead; four additional
medians beat stock without that margin. Four still lose. This is deliberately
a remaining-gap screen, not a package-wide win rate. The serial slowdown,
already-admitted wide +3 ms, reduction-refused +3 ms, and all individual paired
losses remain in the receipts. Smaller excluded-path changes are not attributed
to the new tiled route.

Development seed202609347 (three rotations) recorded N21600/K140/T8 at
852 [845–854] → 696 [685–699] ms, stock660 [657–662]. Disconnected K100 remained
1059→1066 ms versus928; admitted K102 was553→554 versus854; narrow K2 was48→49
versus31. All 36 applications and 24 baseline pairs pass; the 12 direct stock
pairs pass. No thresholds or tile sizes were tuned to the fresh seed.

## Attribution and memory

Public native phase medians, milliseconds; strict solving includes its own
nested work, so do not add it to nested phase totals:

| Fresh case | FE projection B/C | Finite solve B/C | Strict final B/C | Inference B/C |
|---|---:|---:|---:|---:|
| N22560/K142/T8 |187.85 /155.69|97.53 /66.05|255.58 /158.67|97.97 /97.92|
| N44880/K146/T8 |358.02 /296.74|156.31 /93.91|469.42 /282.83|126.01 /125.73|
| N17280/K138/T1 |441.34 /508.34|146.63 /113.36|354.03 /363.56|160.14 /160.09|

Every target retains four outer iterations. Reported inner counts fall from
53/54/54 to41; a direct solve's count is not operation-equivalent to one MAP or
Krylov iteration. Threaded gains are concentrated in canonical projection,
finite products and strict solving. The serial loss is localized to +67 ms
projection and +10 ms strict work, partially offset by -33 ms finite work.
No isolated serial kernel profile was run, so this does not assign that loss
specifically to group recomputation versus direct certificate Gram arithmetic.

Tiled reconstruction replaces the full PW bank with an approximately 64 KiB
coefficient tile, a P-index lookup and bounded group IDs. It retains one extra
N-vector of current weights and P masses through writing. The response's
P-coefficient temporary is released before the tile bank. It performs no old
scratch reads and writes every output cell once. Groups spanning tiles may be
recomputed by exactly the certified pure recurrence.

All selected model and scratch sizes are unchanged. Newly admitted median
whole-action tracked peaks change by **+40,649 / -836,656 / -1,245,264 bytes**,
respectively. The already-admitted wide peak changes by -960 bytes; other fresh
control peaks are unchanged. Maximum fresh tracked/model ratio is 0.852472.
The archive contains every value and phase distribution. This is allocation
tracking within the existing model, not physical constrained-memory or RSS
qualification. The disconnected RHS/certificate-reduction refusal remains.

## Scientific and regression checks

The new independent WLS fixture has144 controls and paired observations with
identical explicit FE/control coordinates and reciprocal-weight errors. It
therefore has known nonzero residuals and known coefficients without either
reference solver. It covers changed weights, T1/T8, exact tiled/full output
identity, direct Gram agreement, complete session fits, and refusal one byte
below the minimum budget. The scratch preparation callback has a separate
poisoned-file failure test. Prior scale/rank/component/current-frame and strict
rollback tests remain.

53 distinct core tests passed during development: nine canonical tests and 44
row-related tests. The latter filter included existing adjacent row/Schur tests
and took 183.51 seconds. The final source replays all nine canonical tests and
the new prepared-tile failure test. Both plugin builds pass ABI/panic containment.
No broad Rust suite or Stata qualification profile is claimed.

Five licensed fixtures pass with explicit process/application success and
terminal sentinels: canonical PPML products; joint categorical allocation audit;
multipartition slope oracle; generated products with FW multiplicity/PW cluster
scores; and the three-FE OLS oracle. Raw licensed logs are excluded from the
archive; their hashes and selected protocol receipts are retained.

All 150 fresh public states,100 baseline comparisons and50 direct candidate/stock
pairs pass. Six further native/stock fits at N6000/K140/T1 and disconnected
N5760/K132/T1 pass independent explicit-design Poisson, Hessian, robust sandwich,
coefficients and fitted-mean oracles. Their independent FE ranks are 1024/1052,
residual DoF 4836/4576, with five oracle iterations. Seed202609390 is frozen.
The one-rotation oracle timings also show serial native losses,672→700 and
603→615 ms, while remaining below stock956/932; they are not confirmation
of timing distributions. Across development, fresh and oracle screens:
**192 public states,128 baseline pairs,64 direct stock pairs**, all passing.

All 24 OLS control applications and12 science/output pairs pass. Three rotations
at N16320 give one-FE/all-output17→16, two-FE29→29, low two-slope104→105, and
low three-FE62→61 ms. These control definitions and the 77/16/111 frozen definitions
are unchanged. Bitwise old recurrence equality is not a general acceptance rule.

## Current-source narrow/fast investigation preserved

Before implementing tiling, a four-case screen at b6088330 measured public warm
medians46/119/64/103 ms versus stock30/77/53/97 for connected N21120/K2/T4 low,
disconnected N42240/K10/T8 low, connected N21120/K10/T4 fast, and unbalanced
N23040/K26/T4 low. All 24 applications and public states plus 12 baseline/stock pairs pass.
The diagnostic builds below are **not** stock-comparator headline artifacts.

`jointboundary_profile` changes only two isolated ado files, keeping the
baseline plugin hash. `jointnative_profile2` instruments four isolated Rust
files at b6088330; plugin SHA-256
e90937a757433515b55de958fd03ec78f85d2c361425266a0c06cbe8ff095b8e.
Each diagnostic has 12 successful public calls with passing public-state audits.

| Case | Ado preparation | Plugin | Ado validator | Remaining ado components | Native core | All joint calls | Actual joint recurrence |
|---|---:|---:|---:|---:|---:|---:|---:|
| Narrow low |5|36|3|about3|34.62|9.16|1.21|
| Disconnected low |6|106|3|about4|100.33|31.06|10.19|
| Fast |7|50|3|about3|46.07|different LSMR engine|not instrumented here|
| Unbalanced low |9|88|3|about5|80.87|29.89|4.45|

These are medians of separate diagnostic scope sums, often nested; they are
not an additive accounting identity. Ado timers have1 ms resolution. All four
low joint invocations (including the strict witness) are counted in each total.
The narrow joint coupling, response norm, finite preparation and recurrence
sums are0.88/0.14/0.95/1.21 ms. Its two canonical preparation calls total8.77 ms,
with initial retained design3.16 ms and strict_candidate3.91 ms. Unbalanced
finite preparation alone totals14.23 ms. These figures rule out replacing only
the narrow recurrence as a consequential route to the27 ms target; they do
not prove that full joint reorganization or nonlinear concentration cannot help.

Three dedicated low-case captures preserve four actual IRLS frames each:
current weights, FE codes, response, controls, tolerance and returned solution.
Capture is disabled in timing runs. Binary hashes and deterministic recipes
are archived; local binary files remain under the diagnostic work root.
A frozen-frame comparison of alternative kernels has **not** yet run. The
captures establish a reproducible starting point, not a claimed speed result.

## Failed attempts and preserved open work

No completed fit, oracle or test in this milestone failed scientific acceptance.
Controller failures remain visible: initial isolated native instrumentation hit
an UnboundLocalError before compilation; the first frame-capture fixture used
a mismatched sentinel despite process/application rc0; a successful corrected
capture followed. The first timing monitor lacked sandbox process visibility
and refused before timing; the authorized monitored retry succeeded. The fresh
manifest initially used incomplete graph row counts and the generator refused
before any fits; only those counts were rounded to whole240-row dimensions,
with unchanged seed/artifacts/settings. The initial OLS driver selected the
PPML adapter and refused before fits; the existing OLS adapter corrected it.
An archive recipe initially mixed absolute/relative paths before archive
creation; the corrected recipe and both logs are retained. Intermediate unused
declarations and all corrected attempts are preserved, not overwritten.

The pre-existing canonical-LSMR reporting assertion, static allocation source
drift, weighted constrained/nonretaining-CG span problems, seed378 comparator
issue, SCC authentication and Linux/thread/constrained-memory confirmation remain
open. No stopped campaign, remote mutation, SCC retry or release occurred.

## Evidence-backed next milestones

The target remains whole-public time <=0.90× scientifically comparable pinned
stock for PPML with at least two FE partitions. Do not tune on seeds389/390.

1. **Redesign two-FE Poisson directions as a complete lifecycle.** Prototype
   analytic elimination of one categorical partition at each Poisson trial,
   with joint finite/remaining-FE Newton steps. Reuse immutable topology and
   current-frame factors; previous predictors are guesses, never certificates.
   Use anchored/centered current-weight products and bounded storage, preserving
   canonical initial rank, independent line-search/score/model-space acceptance
   and final Hessian. Compare identical frozen weighted subproblems before
   public integration. Keep the existing fast categorical history boundary;
   do not repeat ADR0155's rejected family switch. The narrow recurrence's
   measured1.21 ms ceiling makes another isolated CG accelerator insufficient.
   Target a material20–40% whole-command gain, accounting for initial/strict
   costs as well as ordinary directions. A <=27 ms narrow result cannot be
   inferred from a faster inner recurrence alone.
2. **Remove remaining wide reduction duplication.** RHS and certificate workers
   still own full QW reductions. Partition/tile their coordinate ownership to
   admit disconnected wide cases inside the same allowance. For newly admitted
   serial wide work, compare the direct certified Gram schedule with the
   existing serial moment kernel on frozen inputs; projection is the localized
   loss. Do not add a thread/seed exclusion without an independent cost argument
   and fresh validation. N22560/K142 still needs735→616.5 ms to achieve the
   current stock target; larger N44880/K146 needs1198→1161 ms.
3. **Extend successful mechanisms across moderate width, slopes and three-plus
   FE only when whole-command evidence supports it.** The current fresh slope
   ratio is0.912, three-FE ratio0.968 and unbalanced two-FE ratio0.974. These
   differ from historical source-bound results. Preserve categorical/OLS,
   weights, generated terms, rank deficiency and memory controls as each
   mechanism expands; avoid broad reruns of already competitive families.

Each milestone starts with a small baseline and phase ceiling, selects at most
a few coherent candidates, uses independent oracles and fresh predeclared
stock/baseline/candidate rotations, reports every loss, and ends in a local
commit or a bounded rejection. The objective is a practical10% lead where
achievable, not a universal claim obtained by changing scientific work.

## Receipt restoration

`receipts-content-addressed.zip` contains sanitized exact-artifact inputs,
controllers, declarations, exported scientific/runtime states, monitors,
verification summaries and failure receipts. `receipt-index.json` maps logical
paths to payloads; `archive-hashes.json` retains original and archived hashes.
Data/plugin binaries and raw licensed logs are excluded and hash-manifested.
Use `restore_receipts.py` to verify and restore. Archive identity and verification
counts are recorded in the adjacent SHA/verification files.

Verified archive: **10,383 logical receipts / 2,378 payloads / 28,417,811 bytes**,
SHA-256 `6a3aebdc60810692f4c58198f774a4e8a62bf95148b9e9d84e70545c0ce9e3c1`.
