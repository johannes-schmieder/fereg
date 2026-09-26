# Full-frame three-FE canonical construction — local development, 2026-09-25

Adopt **canonicalthree1** under [ADR 0210](../../../../docs/adr/0210-three-fe-canonical-frame-products.md).
This completes the bounded three-FE construction milestone. The multi-FE
whole-public-command <=0.90-stock objective remains unfinished. This report is
local development evidence, not source/platform qualification.

## Source and comparison identity

B is accepted **finiteblock1**, based on fb785e45 plus its single finite-block
Rust overlay (accepted computation 6f697056). C is **45145c5f** plus the eight
Rust overlays recorded in `candidate-overlay-identity.json`; all **566 current
computational inputs** match its captured artifact. Plugin hashes:

- B: `d50517fc00792d26d6e78e62f54266ee42263abb1381ff87aa7c6b6060bf2b20`
- C: `f7d78fd3726139c98ede968ad24337b3844f6402af92f1b20beb391cda8f3ad4`
- Baseline diagnostic canonicalframe1: `b05e5910893ddcf3952222219c6b5003e9642aa566254b4d44698e82b6212ece`
- Candidate diagnostic canonicalframe2: `a911a5c349e3e78d1ee88886fdd189755ca99636cf2a38e14c27974a6e506a55`

Stock is the pinned unmodified fixest library, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`. Default-reference accuracy failures remain
separate historical evidence. Public commands match threads, controls, robust
or two-way cluster inference, and fitted means. The small science screen also
requests complete saved outputs. All manifests, generated input fingerprints,
reference pin, captured source and monitor/controller identities are in the
restorable receipts. Existing frozen 77/16/111 definitions are unchanged.

Three development rotations use seed 202609347. Ten fresh cells and five rotations
were declared before the pilot, conditional on science/performance success,
using **202609440**. No estimator or guard change follows the fresh results;
**441 is unconsumed**. Arm order rotates per case/repetition. Data generation,
compilation, profiling, audits and archiving occur outside headline clocks.
The production PPML blocks have no detected overlap, monitor error, application
failure or timeout. A clean sampled monitor does not exclude all ambient OS load.

## Computational change and attribution

The old low three-FE canonical path built equivalent current-weight Schur geometry
for response1, controls10 and controls9 at both initial and strict construction.
The direct two-FE product builder declined three FE. Construction also repeated
reference norms/means, materialized columns before projection, refreshed retained
statistics and rescanned finite products.

C shares the existing centered three-FE Schur geometry across the complete
response/control frame, independently stops each RHS, reconstructs from originals,
and forms group certificates plus finite moments on those same reconstructed
rows. Existing rank/bread consumers receive fresh norms, means and Gram. Retained
controls are written once. This replaces repeated construction and scans, not
ordinary joint LSMR. There is no cross-weight cache and no new N-by-K bank.
Disconnected/component-scaled calls retain their old route. The q² <= 6N work
rule, complete memory admission, original-input certificates, rank, objective,
score, reconstruction, exact final Hessian and atomic publication remain.

Separate diagnostic builds, two repetitions each, dev347:

|Case / canonical boundary|Baseline ms|Candidate ms|
|---|---|---|
|Low three-FE factors N153600/K19/T8: initial|177.94,179.68|82.23,82.59|
|Same: strict|175.09,175.66|88.12,89.91|
|Two-FE numeric N983040/K6/T8: initial|250.92,260.08|241.33,245.82|
|Same: strict|241.02,258.99|222.44,231.59|

Three-FE full geometry preparations fall **six -> two** per fit. Baseline's six
setup reports total about 113 ms in the first diagnostic repetition (including
normalization/allocation around geometry); C's two separately instrumented
geometry preparations total 28.90 ms. These timers have different boundaries;
the full initial/strict construction comparison above is the complete boundary.
The candidate's first repetition has exclusive initial/strict costs respectively:
geometry 15.23/13.68 ms, RHS/solve 19.05/28.63, combined original-input certificate
and Gram 10.87/12.23, reference norms/means 5.60/5.35, retained scratch write 9.07/7.42,
finite rank 10.87/9.88. Additional residual projection, finite-score and bread
checks remain. These are measured costs, not lower bounds on required science.

Exclusive spans subtract the union of nested intervals. Do not add inclusive
parents to children. Diagnostic blocks are separate from headline rotations;
the two-FE diagnostic decrease is not attributed to this three-FE implementation.
All eight diagnostic applications complete with clean monitors and all four
cross-build coefficient/covariance/objective/mean comparisons pass.

## Whole-command results

All times are medians in milliseconds. K below is expanded finite width.
Complete ordered distributions follow in [distributions.md](distributions.md),
including every regression and the scientifically unranked sparse stock fits.

|Development case; N/K/T|B / C / stock|C improvement over B|C/stock median|
|---|---:|---:|---:|
|Unbalanced three-FE factors;153600/19/8;fast,cluster|698 /590 /705|15.5%|0.837|
|Matched numeric;fast,cluster|575 /472 /615|17.9%|0.767|
|Unbalanced three-FE factors;same N/K/T;low,cluster|707 /523 /699|26.0%|0.748|
|Matched numeric;low,cluster|652 /451 /624|30.8%|0.723|
|Unbalanced three-FE numeric;30720/6/4;fast,robust|83 /78 /82|6.0%|0.951|
|Two-FE numeric;983040/6/8;fast,robust|1227 /1238 /864|-0.9%|1.433|

Each of the first four rows clears0.90 in all three paired stock comparisons.
The small three-FE case misses the target; the million-row candidate has a 1329-ms
tail as well as a 1238-ms median. Other development controls have B/C medians:
simple slopes 70/70, fourFE124/123, narrow16/17, serial188/185,
sparse179/180 (stock unranked), disconnected245/248, wide225/224.
Across all 39 pairs: **22 wins,11 losses,6 ties**. Small timing changes on excluded
paths are reported, not attributed to the new builder.

|Fresh seed 440 case; N/K/T|B / C / stock|C/stock paired range|
|---|---:|---:|
|Fast unbalanced three-FE factors;184320/19/8;cluster|877 /719 /3148|0.226–0.233|
|Serial three FE;38400/8/1;robust|187 /186 /1275|0.144–0.146|
|Sparse three FE;38400/8/4;robust|185 /186 /14070|Unranked: stock means|
|Disconnected three FE;38400/6/4;cluster|188 /193 /8758|0.021–0.022|
|Wide three FE;15360/65/4;robust|225 /225 /1004|0.220–0.225|
|Large two FE;737280/6/8;robust|933 /932 /762|1.217–1.266|
|Low unbalanced three-FE factors;61440/19/4;cluster|337 /287 /406|0.703–0.751|
|Low three FE/four slopes;38400/6/8;robust|119 /120 /119|0.992–1.044|
|Fast unbalanced three-FE numeric;76800/12/4;robust|320 /304 /349|0.859–0.880|
|Fast two-FE factors;153600/19/8;robust|421 /418 /508|0.794–0.837|

Fresh hard-factor gains are18.0% fast and 14.8% low; fresh numeric gains 5.0%.
Seven of ten families clear 0.90 in every pair; sparse is unranked, large numeric
loses, and four slopes are near parity. Across 50 B/C pairs: **27 wins,14 losses,
9 ties**. Disconnected median regresses 2.7% (5 ms); wide ties. These losses and
scope limits prevent a universal non-regression claim. Fresh stock factors take
longer than the development design; do not pool their large stock ratios with
that block or attribute stock variation to fereg.

Production phase medians substantiate construction gains. Development hard fast
factor projection 279.5->228.2 ms, finite 52.4->40.0, strict 122.4->79.6; low factors
projection 216.0->127.5, strict 182.6->93.8. Low numeric projection 220.5->127.2,
strict 191.6->84.3. Fresh fast factors projection 334.3->266.8, strict 177.9->117.3;
fresh low factors 87.9->66.0 and 68.6->45.5. Preparation, inference and publication
remain broadly similar on these cases. Native phase fields are not an exhaustive
exclusive decomposition and must not be summed with the diagnostic spans.
Outer iterations and step halvings are unchanged. Reported inner totals rise
0..40: batching changes their aggregation and some recurrence paths; the
optimization is fewer repeated passes/builds, not an asserted iteration decrease.

## Science, controls and failures

- **45 distinct focused Rust tests pass**: 40 canonical-related plus two shared
  three-FE Schur tests and three strict/explicit-engine boundaries. New independent
  weighted-contrast tests cover T1/T4, changed weights, weight scales 1e±100,
  column scales 1e±30, nested/deficient/duplicate/FE-collinear columns, Gram,
  dense/scratch identity, exact/one-byte-short budgets and invalid/disconnected
  frame refusal. Session tests cover changed weights, fast/low, optional refusal,
  rank failure after physical write, dropped strict candidate, retry/commit,
  response-anchor and subsequent response-only handoff.
- The unchanged `initial_full_width_lsmr_preserves_canonical_multiway_rank_policy`
  test fails its report-flag assertion on C **and a freshly compiled, hash-verified
  finiteblock1 baseline in a separate Cargo target**. It expects MAP to imply
  fallback, but the accepted Schur accelerator can report MAP without a failed
  fallback. Preserve the failure; this report neither changes the test nor claims
  an all-green suite. The earlier reused-binary check is only preliminary evidence.
- **Five licensed fixtures pass** with explicit Stata return codes and terminal
  receipts: runtime validator, hidden transaction, categorical final oracle
  (72 fits), joint column products oracle and scientific contract. ABI/self-test
  passes; the expected injected panic is contained.
- Small science (N4800; generated three FE, fast/low, T1/T4, robust/cluster and
  means/all outputs) passes **12/12 public states,8/8 anchored comparisons,
  4/4 direct stock comparisons,12/12 independent Poisson/Hessian/sandwich arms**.
- Across small science, pilot and fresh: **279/279 applications/public states,
  93/93 B/C comparisons,178/186 anchored pairs,85/93 direct C/stock comparisons**
  pass. Eight sparse stock mean discrepancies (three pilot,five fresh) remain
  unranked. No accepted-result tolerance or sample is changed to rank those fits.
- OLS controls (slopes, low three FE, serial narrow two FE, low generated three FE)
  have **12/12 exact export pairs**. Clean retry B/C medians are67/66,77/76,14/14,
  213/207 ms. This is protection evidence, not proof that the new PPML route speeds
  OLS. Full distributions and exact-file checks are in the retry audit.
- The first OLS block stopped at a pre-fit foreign-process check after seven
  successful applications. Its controller did not persist the detected process
  identities; the subsequent unchanged monitor returned empty. Preserve the
  incomplete block and this uncertainty. The same four cells/seed/three rotations
  were repeated in a distinct directory; all 24 retry fits/monitors pass. Do not
  pool the interrupted clocks with the retry.

Preserved development failures also include the initial private-module compile
error, a new test's invalid one-iteration assumption, an optional fast-memory
forecast regression, and an unavailable ThreadConfig method name. The forecast
was fixed with shared complete shape/byte admission; the existing memory test
then passed unchanged. Only the new iteration assertion changed to test the
intended science/rollback contract. No runtime tuning followed fresh seed 440.

No new weak-connectivity campaign, prior-weight public matrix, Linux/Windows,
constrained-memory or full qualification is claimed. Static allocation inventory
drift, constrained weighted fits, nonretaining CG and prior unresolved scientific
comparisons remain open. No push, release, SCC retry or stopped campaign restart.

## Memory

Fast eligible three-FE calls separately reserve `16*N*min(pool,finite_columns)`
beside the existing complete plan. Low calls fit the previous allowance. Complete
preflight and actual cumulative capacity charging cover geometry, topology,
group/RHS coefficients and fixed worker buffers; scratch is charged even after
its lifetime ends. No extra resident N-by-K bank. Weighted geometry dies per call;
normalized weights become the response, and coefficients live through one write.

Across pilot/fresh, selected modeled deltas are **-48..+29,491,200 bytes**, tracked
peak deltas **-51,599,864..+3,802,640 bytes**; modeled/observed scratch is unchanged.
Small ±48-byte forecast differences are retained in receipts. Development fast
factors add 24.576 MB forecast while tracked peak falls 45.653 MB; low factors add
no forecast and tracked peak falls 32.392 MB. Fresh fast factors add 29.491 MB while
tracked peak falls 51.600 MB; fresh low factors fall 13.060 MB at unchanged scratch.
Do not interpret tracked allocation as RSS or a physical cap; no warmed-allocation,
RSS, physical-memory or platform qualification was performed.

## Remaining budget and next implementation

The production million-row two-FE case is **1238/864 ms**: reaching 0.90 requires
about **460 ms**, to777.6 ms. Fresh 737280 needs about **246 ms**, from 932 to 685.8.
Its unchanged dense two-FE product builder declines the large reduced geometry
(levels 163840/4096 at N983040). Baseline exclusive initial/strict construction is
roughly 250 ms each. Initial materialization ~45, reference means ~23, own norms ~18,
borrowed norms ~28, original-input certificate ~30, finite rank ~37 ms; strict raw
statistics ~46, norms ~37 combined, transport ~12, certificate ~20, finite rank ~47 ms.
These measured components identify removable organization costs, not permission
to remove their scientific invariants. The candidate diagnostic still exposes
those passes. Production preparation ~230 ms and publication ~122 ms also remain.

Next: build a coherent implicit two-FE canonical construction and cross-stage
ownership/pass design, using the existing operator where dense geometry is
unaffordable. Include loading, initialization, objective/score and publication
in its exclusive byte/pass ledger. Ordinary joint-solver replacement alone
cannot meet the budget. Keep small three-FE 78/82 and four-slope 120/119 losses
visible, addressing them through broad carryover rather than another threshold
cycle. Do not reopen the rejected standalone incidence, cache, Schur or borrowed
representative probes. Milestone 3 remains implementation work, not a closed goal.

## Receipt restoration

The content-addressed archive preserves source identities, declarations,
controllers, diagnostics, scientific exports/audits, distributions, test/build
failures, fixture terminal records and omitted-data fingerprints. Generated data,
binary observation outputs and plugins are hash-only; raw licensed logs and
private paths are excluded. Restore and verify using the adjacent scripts:

```bash
python3 restore_receipts.py /tmp/fereg-canonical-three-receipts
python3 verify_restored.py /tmp/fereg-canonical-three-receipts
```

See [archive-verification.json](archive-verification.json) and
[restored-verification.json](restored-verification.json) for the archive identity
and verified logical receipt count. Captured older reports remain immutable.
