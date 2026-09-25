# Current-weight finite blocks in joint PPML LSMR — 2026-09-25

Adopt **finiteblock1**, based on **fb785e45** plus one Rust overlay,
`crates/fereg-core/src/ppml/categorical_lsmr.rs`. All **565 captured computational
inputs match** the accepted source. Plugin SHA-256:
`d50517fc00792d26d6e78e62f54266ee42263abb1381ff87aa7c6b6060bf2b20`.
[ADR 0208](../../../../docs/adr/0208-joint-lsmr-finite-block.md) records the algorithm,
current-weight lifetime and refusal policy. Baseline **generatedscan1 / df0472d6**
plugin: `4443226a78caa96a264b9dd8e6d7e9a72b5551aec99e349bd7e22895ffd2b469`.
Artifacts and their input identities remain distinct; prior timing claims are not
transferred. This is bounded local development evidence, not qualification.

The original hard three-FE factor median improves **850 -> 726 ms**, against
**732 ms** stock. Fresh three-FE factors improve **1067 -> 867 ms**; fresh two-FE
factors improve **696 -> 432 ms**, against **535 ms** stock. The latter clears
0.90 in all five paired ratios (0.790–0.814). Numeric and wide-control cases have
roughly 2–4% median regressions. Retain the substantial generated-control gain
with its small memory cost; do not claim a universal improvement or closure of
the hard three-FE/large-N targets.

## Computational organization and attribution

Accepted fast two-/three-FE ordinary directions already use joint LSMR. The
rejected coefficient-engine switch (ADR 0207) is not revived. Keep its existing
FE core-pair/Schwarz preconditioner and change only the finite block: form the
standardized current-weight original-control Gram, factor it once per solve, and
reuse its forward/backward solve across recurrence attempts. Nonfinite or nearly
singular factors decline to the original diagonal without changing canonical
rank. Exact original-input certificates, recurrence convergence, outer checks and
canonical final-Hessian acceptance remain required. No tolerance or check schedule
changes; no term-name, seed, size or thread threshold selects the factor.

The bank is one K-squared array, admitted and allocated at owner construction;
actual allocation failure follows the existing fatal policy. Its numerical
contents refresh on every call. Disjoint Gram rows use the installed worker team
and each entry's original observation order. The operator, raw control matrix,
FE topology, finite/FE certificates and Stata transaction remain unchanged.

An isolated release test build replays 17 previously captured development frames
(large three-FE factors/numeric and million-row two-FE, plus warmups). It alternates
finite-diagonal/full-factor arms for three repetitions, with one owner per weight
sequence, T8. Both arms use identical controls, weights and targets. This is a
controlled native diagnostic, not the full outer driver's exact reset/line-search
history or a headline comparison. Both arms use the same test binary. All **102
solves** pass their original-input certificates. Construction is timed separately;
wall call times include preparation, solve and acceptance. No large explicit dense
WLS oracle is claimed for these frames.

|Full-size frozen sequence|Diagonal call medians ms|Full-block call medians ms|Iterations diagonal/full|
|---|---|---|---|
|Three-FE factors N153600/K19|75.04,125.67,110.30|90.24,55.81,52.03|19/17,31/14,30/13|
|Three-FE numeric N153600/K19|66.79,47.17,39.67|68.93,49.68,41.70|9/9,11/11,8/8|
|Two-FE numeric N983040/K6|60.06,85.03|63.66,92.60|6/6,12/12|

The first factor frame is slower despite fewer iterations: it needs a continuation
and another fresh certificate. Later factor frames avoid the baseline's extra
iterations/retries. Numeric calls retain their iteration counts and pay extra
Gram setup. Report setup, solve, checks, construction, retries and all individual
calls in replay-summary.json; iteration reduction alone is not acceptance.
The replay source/binary and frame hashes are preserved. Its first build omitted
a required CSV fixture and failed compilation; the corrected build included it.
The first public build attempt used an older Python lacking tarfile's filter
argument, failed before extraction, and was rerun with the established runtime.
These failed attempts remain evidence, not successful builds.

Measured public native phase medians (B/C milliseconds):

|Case|Preparation|FE projection|Finite|Strict final|Inference|Publication|
|---|---:|---:|---:|---:|---:|---:|
|Pilot hard three-FE factors|80.6/79.6|413.0/289.6|57.6/54.5|124.2/131.6|13.8/15.4|20.2/20.3|
|Pilot numeric K19|73.4/74.5|219.6/228.8|56.3/56.8|118.3/119.0|14.0/14.6|20.6/20.3|
|Pilot million-row two-FE|236.3/238.9|249.4/265.7|104.9/105.6|262.4/264.8|6.7/6.5|122.6/122.8|
|Fresh three-FE factors|94.0/93.9|511.6/323.5|81.8/72.8|177.0/175.7|16.7/15.8|24.2/23.9|
|Fresh two-FE factors|60.5/60.6|365.5/115.7|52.4/38.4|51.2/52.0|4.9/4.7|20.4/20.7|

These phase fields exclude some public loading/control costs and are not an
exhaustive sum of the public timer. The factor gains arise predominantly in the
ordinary projection phase. The remaining strict/preparation/publication work is
not attributed to the preconditioner or declared scientifically unavoidable.

## Matched public distributions

Pinned unmodified stock fixest archive SHA-256:
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Use `glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads, robust or two-way-cluster
inference, and means/full outputs as declared. Imports and scientific exports
are outside the command clock. Default-reference accuracy failures remain
separate. Local Apple Silicon/Stata MP 19; no build, audit, profile, archive or
competing fit ran alongside headline clocks. Every controller completed and all
sampled monitors were clean; this does not exclude all ambient system noise.

Tables show **median [individual repetitions] in milliseconds**. B is
generatedscan1; C is finiteblock1. † denotes a failed stock mean comparison:
that stock comparison is unranked, regardless of its descriptive clock. Low,
slopes and four-FE controls do not enter the changed owner, so their timing
variation is not attributed to the new finite factor.

### Development seed202609347, three rotations

|Case; N/finite K/T; mode and VCE|B|C|Stock|C/stock|
|---|---|---|---|---:|
|Three-FE factors,unbalanced;153600/19/8;fast cluster|850 [867,850,832]|726 [734,726,725]|732 [732,730,735]|0.992|
|Three-FE numeric,unbalanced;153600/19/8;fast cluster|577 [583,577,574]|588 [599,581,588]|646 [646,635,674]|0.910|
|Three-FE numeric,unbalanced;30720/6/4;fast robust|84 [84,85,84]|87 [89,87,85]|83 [83,84,79]|1.048|
|Two-FE numeric;983040/6/8;fast robust|1242 [1241,1245,1242]|1268 [1272,1268,1258]|907 [895,907,943]|1.398|
|Three-FE factors,unbalanced;153600/19/8;low cluster|734 [727,737,734]|727 [736,727,723]|733 [734,733,729]|0.992|
|Two slope partitions;30720/6/8;low robust|71 [71,71,72]|72 [72,72,73]|81 [81,81,81]|0.889|
|Four-FE numeric;30720/6/4;fast robust|124 [124,128,123]|125 [126,125,125]|315 [315,317,315]|0.397|
|Three-FE narrow;1920/2/1;fast robust|17 [17,15,17]|16 [16,16,15]|39 [39,39,41]|0.410|
|Three-FE serial;38400/8/1;fast robust|186 [190,186,185]|189 [187,193,189]|1326 [1322,1328,1326]|0.143|
|Three-FE sparse†;38400/8/4;fast robust|181 [184,178,181]|183 [181,185,183]|11161 [11195,11161,11145]|unranked|
|Three-FE disconnected;38400/6/4;fast cluster|257 [250,257,262]|257 [253,257,257]|10099 [10091,10127,10099]|0.025|
|Three-FE wide;15360/65/4;fast robust|226 [223,230,226]|233 [233,233,232]|1010 [1015,1010,1007]|0.231|

### Frozen fresh seed202609439, five rotations

|Case; N/finite K/T; mode and VCE|B|C|Stock|C/stock|
|---|---|---|---|---:|
|Three-FE factors,unbalanced;184320/19/8;fast cluster|1067 [1061,1067,1067,1073,1040]|867 [865,859,867,871,868]|3355 [3355,3353,3360,3362,3354]|0.258|
|Three-FE serial;38400/8/1;fast robust|188 [188,188,187,189,186]|192 [191,192,192,196,195]|1284 [1283,1279,1288,1284,1295]|0.150|
|Three-FE sparse†;38400/8/4;fast robust|239 [239,237,237,239,245]|245 [239,245,241,246,248]|12260 [12260,12230,12264,12258,12272]|unranked|
|Three-FE disconnected;38400/6/4;fast cluster|214 [194,313,203,215,214]|199 [199,200,199,199,199]|7113 [7113,7073,7120,7068,7152]|0.028|
|Three-FE wide;15360/65/4;fast robust|230 [230,231,233,230,226]|238 [239,239,237,237,238]|1164 [1164,1195,1166,1157,1153]|0.204|
|Two-FE numeric;737280/6/8;fast robust|967 [990,967,967,956,973]|985 [974,974,987,988,985]|813 [812,813,815,813,805]|1.212|
|Three-FE factors,unbalanced;61440/19/4;low cluster|397 [390,397,397,399,397]|398 [387,393,401,402,398]|425 [425,426,423,428,423]|0.936|
|Three-FE/four slopes;38400/6/8;low robust|130 [125,126,130,133,131]|128 [129,133,126,128,126]|120 [120,121,119,120,118]|1.067|
|Three-FE numeric,unbalanced;76800/12/4;fast robust|235 [239,235,233,234,235]|239 [244,238,239,240,238]|367 [366,368,368,366,367]|0.651|
|Two-FE factors;153600/19/8;fast robust|696 [687,713,695,701,696]|432 [426,432,434,432,430]|535 [539,531,533,535,539]|0.807|

The hard factor case improves 14.6%; its stock ratios are 1.003,0.995,0.986,
so it does not clear the 0.90 target. The fresh factor medians improve 18.7%
(three FE) and 37.9% (two FE), and all ten B/C pairs improve. The fresh three-FE
stock slowdown cannot close the original hard case. Six of ten fresh families
clear 0.90 in every pair; sparse is unranked, and large numeric, low factors and
multiple slopes miss the target. The pilot simple-slope median clears 0.90, but
one repetition is 0.9012. Keep the original and fresh large-N losses visible:
1268/907 and 985/813 ms C/stock. No guard or source change follows seed439.

## Science, boundaries and preserved discrepancies

- **33 distinct Rust tests pass:** 29 categorical LSMR tests, including two new
  independently assembled weighted-Gram/refusal checks, plus four canonical,
  weighted-truth, strict-rollback and explicit solver-report checks. Changing
  weights, zero rank, topology identity, threads and memory forecast boundaries
  remain covered. This is a focused selection, not the entire Rust suite.
- **Five licensed fixtures pass**, with process/Stata return codes and terminal
  sentinels: runtime validator, hidden transaction, categorical final oracle,
  joint-column products oracle and scientific contract. The 72-fit categorical
  oracle includes MAP/LSMR/auto, two/three FE, unit/FW/PW, T1/T4, robust/cluster,
  offsets and reconstruction, with explicit Poisson and independent q*mu
  Hessian/sandwich checks. No new Mata compilation or result-wire change.
- **264/264 public states** pass over small science, pilot and fresh. All
  **86/86 pilot/fresh baseline/candidate comparisons** pass. Across the three
  screens, baseline-anchored pairs pass **164/176**, direct candidate/stock
  **80/88**, and independent explicit-design checks **4/6**. These denominators
  differ; this is not an all-green reference claim.
- Both small N4800 generated-control candidate and stock Poisson/Hessian/sandwich
  oracles pass (robust means and clustered full outputs). Baseline again fails
  one near-zero coefficient in each: maximum absolute error 2.749545e-10,
  registered tolerance ratio 1.34971. Candidate maximum coefficient error is
  1.319307e-12. Baseline-anchored pairs remain failed/unranked there, not waived.
- Three pilot and five fresh sparse stock mean comparisons fail. Candidate and
  baseline agree in those cases; no stock speed ranking or tolerance change is
  made. Exact errors and failed coordinates are retained in both audits. Prior
  separation/DoF/output and other unresolved comparisons are not adjudicated.

All 12 OLS pairwise scientific exports are **exact**. These four-family controls
are not new independent OLS oracles or frozen-matrix qualification:

|OLS case; three pairs|Baseline ms|Candidate ms|
|---|---|---|
|Two slope partitions|70 [70,70,69]|72 [72,72,71]|
|Low three FE|84 [89,82,84]|84 [88,84,82]|
|Narrow serial two FE|10 [10,10,12]|11 [11,10,11]|
|Low three-FE generated controls|223 [221,223,232]|223 [232,222,223]|

The 2-ms slope and 1-ms narrow median losses remain visible; no blanket OLS timing
non-regression claim. Frozen77/16/111 definitions and OLS implementation are
unchanged. Existing checkpoint-count test drift, constrained weighted fits and
nonretaining-CG failures were not repaired or reclassified. No stopped campaign,
SCC authentication retry, push, release or platform qualification occurred.

## Memory and next boundary

The new bank costs 8*K^2 bytes (2,888 at K19; 33,800 at K65). Complete admission
uses the maximum possible finite width, not only the realized active rank.
Across pilot/fresh, modeled selected differences range -48 to +34,848 bytes;
tracked peak differences range -22,128,420 to +184,608, reflecting changing
recurrence/preconditioner lifetimes as well as the new bank. The two-FE factor
peak is 22.1 MB lower in every pair. Observed scratch differences are zero.
These are model/tracked-allocation measurements, not RSS or physical-cap proof.
Static allocation inventory drift remains a failure; no warmed-allocation
campaign or Linux/Windows/constrained-memory confirmation is claimed.

The next coherent candidate is canonical FE-equivalent controls in the same joint
LSMR operator, with original-input certification and explicit warm-start/basis
invalidation. It may remove remaining finite/FE coupling; that benefit is still
a hypothesis. Aim to remove the roughly **67 ms** still needed on the hard fast
factor case, without losing FE preconditioning on sparse/disconnected designs.
If it fails the identical-frame/public checks, stop that experiment and move to
canonical weighted-frame construction. Low factors also need about 67 ms; the
million-row case needs about 452 ms across preparation, projection, strict work
and publication. Its two ordinary calls cannot supply that whole budget.

## Immutable evidence

The archive contains **9,394 logical receipts**, 2,510 payloads, 12,220,036 bytes.
SHA-256: `7166868c576da7c141149f62f2d6183890feec4a2e18146f22977307fe28f2c6`.
582 binary/large generated files are hash-recorded but omitted; raw licensed logs
are replaced by sanitized protocol evidence. Captured controllers, declarations,
all attempts, audits, source snapshots, build/test logs, replay and artifact
identities remain. Run `python3 restore_receipts.py <new-directory>` followed by
`python3 verify_restored.py <directory>` to verify/restore the logical records.
All restored hashes and privacy checks passed. Known public shared-reference
paths and synthetic redaction-test source literals are explicitly distinguished
from private run metadata. All seeds through **202609439** are now frozen;
**202609440** is unconsumed.
