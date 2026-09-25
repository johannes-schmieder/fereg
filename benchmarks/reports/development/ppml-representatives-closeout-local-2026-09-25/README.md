# Canonical representative controls in joint PPML: closeout — 2026-09-25

Reject **representatives1** and the one bounded row-access correction
**representatives2**. Restore **finiteblock1 / 6f697056** computation; all **565
computational inputs match** after restoration. No runtime or test changes from
this experiment are retained. [ADR 0209](../../../../docs/adr/0209-joint-ppml-representatives-closeout.md)
records the decision. This is bounded local development evidence, not qualification.

The corrected candidate improves hard three-FE factors **705 -> 643 ms (8.8%)**
against **709 ms** comparable stock. Its three stock ratios are **0.922, 0.907,
0.915**, all above the 0.90 target. Matched numeric loses **578 -> 619 ms (7.1%)**;
wide controls lose **225 -> 243 ms (8.0%)**. Five candidate/baseline pairs win and
19 lose in the corrected eight-case block. Stop this organization without tuning
selectors or consuming fresh validation. **Seed202609440 remains unconsumed.**

## Source and mathematical organization

Both production candidates use base **50f67d6e** plus overlays to
`ppml/categorical_lsmr.rs`, `ppml/prepared_irls.rs` and `ols/linear_session.rs` in
the core. The artifact identities bind all 565 inputs. Plugin SHA-256:

- Baseline finiteblock1: `d50517fc00792d26d6e78e62f54266ee42263abb1381ff87aa7c6b6060bf2b20`.
- representatives1: `dc3df94f8f21bbc19d16c5f2c62d82dbe59815c7c8d44ded48dacc26c59ed477`.
- representatives2: `b768ac21d484345385cbc9169a1ff9983e910cdd7cf0f781e43ca84a18438c21`.
- Separate diagnostic capture: `497a370ebd1e3b7205c852031b76ad4e05e69fd7184fe084e57aa9c91bf30831`.

Keep the existing joint LSMR, FE core-pair/Schwarz preconditioner and full finite
Gram factor. Borrow the retained canonical controls R, selecting the frozen
active columns without a new N-by-K bank. For R = X - D A, the predictor
R beta + D gamma equals X beta + D(gamma - A beta): finite coefficients retain
their original units. Older-weight representatives supply coordinates only.
Every finite factor and residual/finite/FE certificate uses the current weights;
the independent certificate retains original X. No tolerance, check frequency,
sample, weight, rank, DoF, inference, separation or final-Hessian policy changes.

The session checks sample/topology and generated-column/kept-slot identity. Reset
speculative recurrence state before strict work can rewrite the physical bank,
including attempts whose accepted fit/basis later rolls back. Strict rollback
alone does not restore old physical projected rows. Borrowing leaves Rust-owned
buffers immutable during worker access, preserving the SPI and transaction boundary.

The first implementation resolves selected matrix indices within arithmetic
loops. The correction resolves row slices and column positions outside those
loops, retaining observation and FMA order. It changes no mathematical selector
or stopping threshold. Each candidate has its own source, binary, tests and clocks.

## Frozen-frame comparison

An isolated diagnostic public build captured 17 actual frames, including warmups,
from the hard factor, matched numeric and million-row designs. It records original
controls, selected canonical representatives, current Hessian weights, Newton
target, thread count and caller warm-reset state. The first attempt produced zero
frames because the measurement harness removed the diagnostic environment
variable. Its empty replay exited zero but is **not solve/certificate evidence**.
A separate diagnostic controller explicitly forwards that variable; the normal
production/stock controller is unchanged. Both attempts remain archived.

Each of two isolated release test builds runs three alternating original-control/
representative arms on the same captured frames: **102 certified solves each**.
The following table belongs only to the corrected diagnostic build. It compares
two coordinate systems inside that build, not the public accepted baseline binary.

|Full-size sequence|Original-coordinate call medians ms|Representative call medians ms|Iterations original/representative|
|---|---|---|---|
|Three-FE factors N153600/K19|90.10,56.58,85.58|52.14,45.74,42.56|17/6,14/7,15/6|
|Matched numeric N153600/K19|68.92,50.46,41.62|79.50,61.02,50.61|9/9,11/11,8/8|
|Two-FE numeric N983040/K6|64.39,63.74|65.81,64.69|6/6,6/6|

Call clocks include current setup, solve and original-input acceptance. Owner
construction is separate. Capture packs selected representatives for replay;
packing is outside these native clocks. Public production borrows the original
strided bank. Neither replay reproduces a separate baseline driver's entire
outer IRLS/line-search history. All 102 paired status, iteration, retry, factor
and certificate fields are identical between the two diagnostic builds; their
clock distributions remain separate. Full call/setup/solve/check arrays, source,
executable and frame hashes are retained. No large explicit dense WLS oracle is
claimed for these large frames.

## Matched public distributions

Development seed **202609347**, three rotations per block, Apple Silicon/Stata MP19.
Pinned unmodified stock archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`;
`glm.tol=1e-10`, `fixef.tol=1e-11`. Match threads, robust/two-way-cluster inference
and means or declared full outputs. Imports and scientific exports are outside
the command clock. No compilation, profile, audit, archive or competing fit ran
alongside headline clocks. All sampled monitors were clean, without claiming
absence of all ambient OS load. Tables show **median [individual] milliseconds**.
† Stock mean comparisons fail and remain unranked; their clocks are descriptive.

### First implementation: representatives1

|Case; N/K/T; mode and VCE|Baseline|Candidate|Stock|
|---|---|---|---|
|Three-FE factors,unbalanced;153600/19/8;fast cluster|692 [733,692,689]|699 [681,702,699]|710 [710,709,716]|
|Matched numeric;153600/19/8;fast cluster|594 [569,594,608]|661 [649,661,695]|629 [628,629,632]|
|Three-FE numeric,unbalanced;30720/6/4;fast robust|83 [82,83,86]|83 [83,82,83]|83 [83,81,83]|
|Two-FE numeric;983040/6/8;fast robust|1248 [1280,1248,1245]|1267 [1336,1246,1267]|877 [873,877,890]|
|Three-FE factors,unbalanced;153600/19/8;low cluster|714 [714,714,723]|713 [710,757,713]|714 [713,714,715]|
|Two slope partitions;30720/6/8;low robust|71 [71,70,71]|71 [70,74,71]|83 [83,81,84]|
|Four-FE numeric;30720/6/4;fast robust|126 [125,127,126]|125 [126,123,125]|322 [322,323,317]|
|Three-FE narrow;1920/2/1;fast robust|15 [15,14,15]|14 [14,16,14]|41 [41,41,40]|
|Three-FE serial;38400/8/1;fast robust|187 [187,185,187]|195 [194,198,195]|1315 [1315,1314,1323]|
|Three-FE sparse†;38400/8/4;fast robust|180 [180,182,179]|184 [183,186,184]|11202 [11193,11202,11240]|
|Three-FE disconnected;38400/6/4;fast cluster|249 [253,249,246]|247 [248,247,246]|10136 [10220,10135,10136]|
|Three-FE wide;15360/65/4;fast robust|226 [226,226,227]|265 [265,264,270]|1021 [1024,1021,1008]|

The first hard-factor median is flat/slower. Numeric loses 11.3% and wide loses
17.3%. Across all 36 pairs there are 13 wins, 21 losses and two ties. Low, slopes
and four-FE controls do not use the changed owner; their variation is not attributed
to the candidate. Do not pool this block with the correction or inherit its clocks.

### Row-access correction: representatives2

|Case; N/K/T; mode and VCE|Baseline|Candidate|Stock|
|---|---|---|---|
|Three-FE factors,unbalanced;153600/19/8;fast cluster|705 [709,705,703]|643 [654,643,637]|709 [709,709,696]|
|Matched numeric;153600/19/8;fast cluster|578 [575,578,599]|619 [619,623,603]|632 [625,632,634]|
|Three-FE numeric,unbalanced;30720/6/4;fast robust|82 [82,82,81]|84 [84,87,82]|84 [85,81,84]|
|Two-FE numeric;983040/6/8;fast robust|1241 [1241,1238,1246]|1251 [1243,1349,1251]|867 [867,866,880]|
|Three-FE serial;38400/8/1;fast robust|186 [184,186,191]|188 [188,188,188]|1315 [1315,1334,1314]|
|Three-FE sparse†;38400/8/4;fast robust|179 [178,179,179]|182 [181,182,182]|11208 [11251,11208,11180]|
|Three-FE disconnected;38400/6/4;fast cluster|246 [246,247,245]|248 [245,248,255]|10154 [10154,10291,10146]|
|Three-FE wide;15360/65/4;fast robust|225 [225,226,224]|243 [245,241,243]|1017 [1017,1017,1020]|

The hard-factor public inner count falls **71 -> 47**; outer count remains four.
Projection falls **284.7 -> 236.7 ms**, finite work **56.2 -> 45.9**, strict/final
**117.8 -> 116.6**. Matched numeric retains 55 inner iterations but projection
rises **226.5 -> 256.7 ms**. Wide inner work falls 66->63 while projection rises
**66.2 -> 80.4 ms**. Million-row inner work stays 28; public time rises
**1241 -> 1251 ms** against 867 stock. These native phase fields do not exhaust
public time. The measured iteration benefit is real, but the per-pass organization
still costs too much on numeric/wide controls. This evidence does not prove that
all FE-equivalent coordinate organizations are slow or verification is irreducible.

## Science, boundaries and memory

- **36 distinct focused Rust tests pass on each candidate:** 30 categorical LSMR
  tests and six canonical/strict/rollback/solver-identity tests. The new independent
  test constructs paired observations with opposite weighted nonzero errors,
  known finite/FE truth, selected/reordered columns and unused NaN columns. It
  covers two/three FE, T1/T4, changing weights, warm/reset boundaries, adjoint
  identity and invalid dimensions. Experimental tests are archived with their
  candidate implementations, not left dependent on removed production code.
- **Five licensed fixtures pass per candidate (ten executions)** with explicit
  Stata return codes and terminal sentinels: runtime validator, transaction,
  categorical final oracle, joint-column products oracle and scientific contract.
  They include the 72-fit categorical explicit Poisson/q*mu Hessian/sandwich
  boundary, weights, solver families, rank, reconstruction and rollback.
- Both small N4800 generated-control screens pass: **12/12 independent Poisson/
  Hessian/sandwich arm checks**, including robust means and clustered full outputs.
  All eight baseline-anchored pairs and four direct candidate/stock pairs pass.
- Across small science and both headline pilots, **192/192 applications/public
  states**, **64/64 baseline/candidate pairs**, **122/128 anchored pairs**, and
  **58/64 direct candidate/stock pairs** pass. Six sparse stock mean comparisons
  fail their registered envelope and remain unranked. The two separate diagnostic
  capture screens add six passing public states, without headline timing claims.
- One initial Rust build failed a borrow check before the strict-reset placement
  was corrected. The zero-frame capture/empty replay is also preserved. All
  measured public applications completed; no application failures/timeouts were
  silently dropped. Existing scientific discrepancies remain unresolved.

The borrowed view adds no heap bank and does not change memory admission.
First-block modeled selected deltas are -112..+48 bytes, tracked peaks
-61,440..+61,440. Corrected modeled deltas are 0..+112 bytes, tracked peaks
-2,516,776..+1,946,432. Scratch is unchanged. These telemetry differences do not
establish RSS, warmed-allocation or physical-cap behavior. Static allocation
inventory drift, constrained-fit and nonretaining-CG failures remain open.

After rejection, all 565 accepted computational inputs are hash-identical to
finiteblock1. No OLS timings, new restored-source test execution, fresh held-out
campaign or platform qualification is claimed. OLS source and frozen77/16/111
remain unchanged. No push/release, SCC retry or stopped campaign restart occurred.

## Decision and next implementation

Do not add a benchmark-fitted selector or another selected-view tuning round.
Keep full finite-block conditioning from ADR0208, remove this experiment's
runtime/test changes and preserve its source. Move to canonical current-weight
construction: compare a justified three-FE direct-product extension with removing
repeated normalization/materialization/product passes around the existing
projector. Any benefit must include independent original-input acceptance and
rank/bread/inference handoff. Initial versus strict work needs an exclusive trace,
not the sum of inclusive timers. Existing rejected incidence, isolated Schur
scheduling and frame-cache probes do not become new candidates without materially
changed organization. The million-row gap still needs a cross-stage pass budget.
The broad multi-FE 0.90-times-stock goal remains active and unfinished.

## Immutable receipts

The adjacent content-addressed archive preserves both candidate source trees,
diagnostic sources/binary hashes, declarations, controllers, all attempts,
scientific audits, distributions, test logs and restoration proof. Binary data
and large generated outputs are hash-recorded and remain local; raw licensed
logs are replaced by sanitized protocol evidence. Restore with
`python3 restore_receipts.py <new-directory>` and verify with
`python3 verify_restored.py <directory>`. Exact counts and archive hash are in
`archive-verification.json`; the archive is verified before the closeout commit.

Archive: **8,612 logical receipts**, 1,923 payloads, 9,896,042 bytes; 494 large/binary
files hash-recorded and omitted. SHA-256:
`d1a00b298869a481fc89f12a9f201063d4d34bb134ad755efa37756803d3691b`.
All restored receipt hashes and privacy checks passed.
