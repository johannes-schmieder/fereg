# Canonical PPML cost attribution and closed Schur-scheduling probes

Local Mac arm64 development evidence, 2026-09-25. No qualification or release.
Decision: [ADR 0188](../../../../docs/adr/0188-canonical-ppml-cost-and-schur-closeout.md).

## Result and scope

Retain the production solver at `ad776bd53c361152aa3618c87dc26c4c9f2079d4`.
Both Schur-construction prototypes are rejected. The active row-panel version
saves approximately **1.20 ms** in its diagnostic matrix-construction phase but
ties the baseline's whole-command target median at **78 ms**. Its six-case block
has **seven paired wins, eight losses and three ties**. This does not justify
another scheduling layer or a fresh validation campaign. There is no accepted
runtime speedup, new routing guard, allocation allowance or scientific-policy
change from this report.

This bounded milestone follows the accepted coefficient solver, current-weight
geometry/coupling reuse, fused joint directions, parallel geometry/compact
products and profiled Poisson starts/trials. It does not reopen their rejected
paired lanes, retained-coefficient layouts, full reduced factors, residual-space
extrapolation or serial weighted-OLS tuning. The multi-FE 0.90-times-stock goal
remains unfinished.

## Source and comparator identities

The production baseline artifact is `slopecurve3`, based on `a3bd6dc6` plus its two
accepted PPML overlays; **all 555 captured inputs match restored production**.
The additional fixture introduced at `ad776bd5` remains separately bound by its
original report. The new prototypes and diagnostic builds start at `ad776bd5`.
`schurteam1` overlays the coefficient owner, joint owner and coupling module;
`schurrows1` overlays only `fe/slope_coefficients/coupling.rs`. Complete captured
source inputs, build logs, patches and artifact identity maps are archived.
Diagnostic builds additionally inject the captured span instrumentation and are
never used as stock-comparator headline artifacts.

| Artifact | Plugin SHA-256 |
|---|---|
|slopecurve3|ebfca50ca203293d14e3b710951276b14d4221ff550b610a3c171bb1a7e22fe8|
|canonstage1|bbbbe12a1be91944b2f1d155f81921960a1ff7a1f07f1ab0dbab46abdab1ccf1|
|schurteam1|cbd62994cc10c0dcedbe0d62dbdf991bd3a068780e18ff33694eec38a9d4e822|
|schurstage1|4b6e31a0d351a10dc72bf669af1b72acd523a0e9e57bc094a0b5526baa11140d|
|schurrows1|b655cdfa4882d4c07d64d4ba5cb50f04c083da4cd0b307c9960af1130e2a3d54|
|rowstage1|cb17d4480897d1b53f569b1ccd29a4a7cb06054fa9fcb6d88550872eeb5caf4b|

Comparable stock fixest is the pinned unmodified 0.14.2 library, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Use `glm.tol=1e-10`, `fixef.tol=1e-11`, matched thread counts, robust inference,
and fitted means; BLAS/OMP/vecLib threads are one. Default-reference accuracy
failures remain separate historical evidence. These headline cases are numeric,
unweighted, positive-outcome PPML. No frequency-weight inference claim follows.

## Current-source native attribution

Eight initial diagnostic fits cover two-slope N30720/K6/T8, large unbalanced
three-FE slopes N61440/K10/T8 in fast/low modes and narrow two-categorical-FE
N21120/K2/T4. Each uses two repetitions and the last full `plugin:estimate` span
in its process, excluding the preceding warm fit. The subsequent diagnostic
blocks rotate baseline/candidate builds. All diagnostic public states are audited.

Initial two-slope medians, ms:

| Native quantity | Calls | Inclusive time |
|---|---:|---:|
| Full plugin |1|72.581|
| Initial canonical solve |1|11.443|
| Strict canonical candidate |1|10.604|
| Combined canonical retained construction |2|21.635|
| Full retained initial design |1|7.656|
| Fused strict reprojection |1|5.635|
| Canonical rank work |2|6.756|
| Rank residual projection, inside rank work |2|2.362|
| Current-weight coupling preparation |5|5.955|
| Schur construction, inside coupling preparation |5|4.394|
| Current-weight geometry |5|3.447|
| Full FE certificate |7|3.944|
| Initial Poisson state and associated work |1|4.856|
| Core preparation |1|2.176|
| Plugin loading |1|2.275|

**Nested spans are inclusive and must not be added.** Recursive parallel geometry
child sums are worker activity, not wall time. The original aggregate PPML
Preparation clock ends **before** the first canonical WLS. The previous handover's
rough 31-ms preparation-plus-strict budget is therefore not a measurement of
31 ms of canonical construction. This report corrects that attribution.

The two canonical construction calls total approximately **67.5 ms** in the
large fast case and **85.4 ms** in low mode; neither forms the explicit Schur
matrix. Narrow categorical canonical construction is approximately **6.5 ms**.
Original-column norm/mean visits, residual projection and materialization remain
visible costs. The profile alone does not establish which can be eliminated.
Original-row certificates, rank/bread, reconstruction and final q*mu authority
are required purposes; their current pass organization is an implementation.
No new attribution of fixest's inner solver is claimed by this scheduling trial.

## Prototype 1: full-square worker scratch

`schurteam1` divides eliminated groups among existing worker observation banks,
forms partial matrices and reduces them in fixed worker order. It adds no bank
or memory-plan term. Its independent dense C'C test covers empty groups, two/
three partitions, changing numeric blocks, multiple channels, T1/2/4/7 and short
scratch refusal. Parallel reduction can change roundoff.

The representative case has **5,120 eliminated groups, two channels and 256
remaining coordinates**. Its square has 65,536 elements, exceeding each N=30,720
worker bank. Consequently the proposed route **does not activate on the main
target**. A preliminary assumption of a smaller reduced dimension was wrong.
The source-bound result is coverage evidence, not evidence that an active
parallel kernel is ineffective. The following diagnostic confirms essentially
unchanged Schur time, 4.385/4.492 ms. No buffer-size threshold was tuned.

Seed **202609347**, three predeclared baseline/candidate/stock rotations:

| Case | Baseline / candidate / stock median ms | Candidate range ms | C/B | C/stock | Paired wins/losses/ties |
|---|---:|---:|---:|---:|---:|
|Large unbalanced three-FE slopes, fast|189 / 187 / 225|185–188|0.989|0.831|2/1/0|
|Same, low|202 / 203 / 233|202–207|1.005|0.871|1/1/1|
|Two slope partitions, T8|78 / 79 / 79|78–80|1.013|1.000|1/2/0|
|Smaller three-FE slopes, T4|89 / 88 / 117|88–90|0.989|0.752|2/1/0|
|Two slope partitions, T1|127 / 126 / 320|124–129|0.992|0.394|1/1/1|
|Narrow two-FE categorical|37 / 36 / 27|35–37|0.973|1.333|2/1/0|

The full block has nine wins, seven losses and two ties. The target's paired
baseline/candidate times are 78/79, 80/78 and 78/80 ms. No fresh seed was consumed.

## Prototype 2: disjoint Schur output rows

`schurrows1` partitions the already-owned dense Schur matrix into disjoint row
panels. Each panel visits groups/bases in the original order and updates only
its own upper-triangle entries; the established symmetry copy follows the join.
There is no partial matrix, new bank, cross-worker reduction or admission change.
Serial execution retains the original sparse row accumulator. Current-weight
refilling, curvature refusal and original-observation certificates remain intact.

The new unit oracle explicitly materializes C and computes its dense product
independently. It also requires **exact equality with serial matrix entries**
at T1/2/4/7, on two/three partitions, three numeric frames, empty declared groups,
unequal channels and both N=192/4096. This includes matrices larger than N and
incomplete final row panels. All existing focused slope and IRLS boundaries are
rerun before timing.

Same predeclared development inputs, independent three-rotation block:

| Case | Baseline / candidate / stock median ms | Candidate range ms | C/B | C/stock | Paired wins/losses/ties |
|---|---:|---:|---:|---:|---:|
|Large unbalanced three-FE slopes, fast|185 / 187 / 232|186–189|1.011|0.806|1/2/0|
|Same, low|202 / 204 / 231|201–205|1.010|0.883|1/1/1|
|Two slope partitions, T8|78 / 78 / 79|77–78|1.000|0.987|2/1/0|
|Smaller three-FE slopes, T4|88 / 89 / 115|87–90|1.011|0.774|1/1/1|
|Two slope partitions, T1|127 / 124 / 319|124–128|0.976|0.389|2/1/0|
|Narrow two-FE categorical|36 / 37 / 31|36–38|1.028|1.194|0/2/1|

Target pairs are 77/78, 78/77 and 80/78 ms. No target-median improvement is
established. The unchanged serial control improves 3 ms while unchanged compact
and categorical medians generally lose 1–2 ms, showing why a small isolated
kernel change cannot be inferred from aggregate wins.

A final separate paired diagnostic confirms actual N30720/p256/G5120/C2/T8
activation on all five weight frames. Schur construction falls **4.343→3.144 ms**,
coupling preparation **5.904→4.689 ms**, and the plugin span **71.951→70.890 ms**.
Canonical construction remains **20.554→20.632 ms**. The large compact control
has no Schur calls. These are two-repetition instrumented medians, not headline
speed claims. The 1.2-ms saving is too small to justify this trial's duplicated
sparse traversal after its public-command tie. No panel-size tuning, extra
prototype or fresh performance campaign follows.

## Science, memory and limitations

All **140 public states**, **84 baseline comparisons**, **36 direct candidate/
stock comparisons** and **ten licensed fixture executions** pass. There are 108
headline PPML fits and 32 separately instrumented fits. Every progress record
has a passing application state and no monitored overlap/error. All **36**
headline baseline/candidate pairs have byte-identical coefficient, covariance
and mean exports. This is observed evidence, not a universal bitwise contract.

The final row candidate has **123 distinct focused Rust passes** and the known
baseline failure `refined_checkpoint_preserves_accuracy_on_a_small_dynamic_range_model`
(expected strict-rejection count one, actual zero). The same assertion also
fails in prototype 1; its baseline reproduction is preserved in the preceding
accepted reports. No tolerance or assertion was changed. The new matrix oracle
passes. Five licensed fixtures run against each candidate: independent weighted
multipartition Poisson/Hessian and three-FE slope oracles, canonical-product
oracle, one-FE categorical oracle and explicit three-FE OLS oracle. All include
explicit terminal sentinels and zero process/Stata return codes.

Selected-model admission, scratch and tracked allocation peaks have **zero delta
in every headline pair**. Unit workspace capacities are stable. No new real
warmed-allocation fixture or physical-cap/RSS qualification was run, since both
candidates were rejected. No OLS headline benchmark or broad qualification was
started. Frozen 77/16/111 definitions, static allocation-inventory drift,
constrained-fit/nonretaining-CG issues, deferred platforms and stopped campaigns
remain unchanged. No SCC retry, push or release occurred.

The production baseline in the final development block is 78/79 ms versus stock
for threaded two-slope PPML and 36/31 ms for narrow categorical PPML; the gaps to
0.90 stock are **6.9 and 8.1 ms** on those inputs. These are different sizes/seeds
from the preceding fresh block's 20.3/10.3-ms gaps. Do not transfer the earlier
fresh block's eight-of-ten success count to these cases or the rejected builds.

## Receipts and next boundary

The verified privacy-filtered archive contains **7,583 logical
receipts / 1,451 payloads / 10,753,119 bytes**, SHA-256
`10790d75e60a624cd7e7c30c128c117958465790d3ed04c086a34e2a6e07b57e`. It was restored and its scientific verification receipt
compared exactly. Data/binaries and raw licensed logs are omitted with hashes;
source snapshots, declarations, schedules, timings, public exports, audits,
phase protocols, test protocols and failed/unretained patches are preserved.
Use `restore_receipts.py` with a new destination directory. Work root:
`target/ppml-canonical-cost-20260925`.

The next implementation boundary is canonical design ownership and pass
organization shared by initial rank setup and strict final construction. The
current resume plan lives only in [STATUS.md](../../../../STATUS.md). This report
closes Schur scheduling; it does not claim the broader speed objective is met.
