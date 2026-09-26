# Implicit two-FE canonical construction — local development, 2026-09-25

Adopt **implicitframe1** under [ADR 0211](../../../../docs/adr/0211-implicit-canonical-frame-products.md).
The bounded large-N construction milestone improves selected public PPML fits
18–23%, with a diagnostic weak-graph improvement of 17–33%. The multi-FE
whole-command <=0.90-stock objective remains unfinished. This is local
development evidence, not qualification or a universal non-regression claim.

## Identity and scope

B is accepted **canonicalthree1**, computation at **de5883cd**, plugin SHA
`f7d78fd3726139c98ede968ad24337b3844f6402af92f1b20beb391cda8f3ad4`.
C is de5883cd plus two captured Rust overlays, `ols/canonical_products.rs` and
`ols/canonical_products/three.rs` (the latter is test-only), plugin SHA
`0f00d18b41c5a7588d9e1bd9448a8490120bd30737ae63ce85300c3a46692766`.
All **566 computational inputs** match the production artifact. The separate
additive repeated-pair development generator does not change frozen definitions.

Stock is the pinned, unmodified fixest library, archive SHA
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`. Threads, finite terms, robust or common
cluster inference, samples and requested outputs are matched and audited.
Default-reference accuracy failures remain separate evidence. Manifests,
controllers, fingerprints, source snapshots and reference pins are archived.
The frozen 77 primary,16 holdout,111 native-control definitions are unchanged.

Development uses seed 202609347, three rotating B/C/stock repetitions per family.
Eight fresh cells and five rotations use **202609441**, declared before the pilot
conditional on success. No production source or guard change follows those
results. Two complete fresh blocks are repeated solely for process contention,
with the original attempts preserved. Do not pool separate blocks or transfer
claims to another source. Build, audit, profile and archive work occurs outside
headline clocks. Sampled monitors cannot exclude all ambient OS load.

## Computational organization

The previous dense two-FE geometry does not fit the admitted allowance at large
reduced shapes. It declines full-frame certified products and falls back to
repeated observation-space construction. C instead prepares current-weight
packed group-pair incidence and applies the exact grounded, diagonally scaled
Schur operator. It shares geometry across independently stopping component/RHS
CG solves, then reconstructs original-input residuals and forms the full FE
certificate plus finite Gram together. Canonical rank/bread receives the same
certified means, norms and products. Retained dense/scratch controls are written
once. No ordinary IRLS solver or cross-weight cache is added.

This is materially different from rejected ADR 0200's operator-only replacement
inside already admitted dense iterative construction. Every affordable dense
route keeps its previous arithmetic. The full-frame surrounding work is the
reason to adopt incidence here. Dense and implicit routes share one recurrence;
no overlapping standalone iterative implementation is retained.

The pair-assembly work preflight and complete phase/capacity budget remain.
Numerical/work refusal retains the existing certified route; fatal errors
propagate. Original-input projection, finite solve, rank, score, model space,
objective, reconstruction and exact final q*mu Hessian acceptance are unchanged.
Strict transactions, samples, weight semantics, VCE, DoF, separation and failure
atomicity remain. No tolerance is relaxed.

## Whole-command results

All values below are median milliseconds; K is expanded finite width.
[distributions.md](distributions.md) contains every ordered selected distribution,
paired ratios, losses and unranked stock comparisons. Raw interrupted and
contended records remain in the receipts; they are not merged into these blocks.

|Development case; N/K/T; robust unless noted|B / C / stock|C/stock paired range|
|---|---:|---:|
|Two FE numeric;983040/6/8;fast|1248 /1013 /878|1.137–1.162|
|Two FE numeric;491520/6/8;fast|628 /506 /505|0.996–1.022|
|Two FE numeric;245760/6/4;fast|316 /318 /386|0.824–0.833|
|Two FE numeric;983040/6/8;low|1362 /1102 /865|1.257–1.294|
|Two FE numeric;491520/19/8;fast|1035 /1037 /1180|0.872–0.884|
|Unbalanced three-FE factors;153600/19/8;fast,cluster|613 /601 /714|0.830–0.842|
|Same;low,cluster|528 /536 /702|0.749–0.782|
|Two slope partitions;30720/6/8;low|75 /70 /81|0.852–0.878|
|Disconnected three FE;38400/6/4;fast,cluster|248 /245 /10197|0.024–0.024|
|Two FE factors;153600/19/8;fast|422 /425 /518|0.819–0.826|

Large narrow two-FE gains are 18.8%,19.4%,19.1%. Seven of ten development
families meet 0.90 in every comparable pair. Across 30 B/C pairs:20 wins,10 losses,
no ties. Protected-path clock changes are not attributed to implicit geometry.
The low three-FE control regresses8 ms; small and wide dense controls regress2 ms.

|Fresh seed 441 case; N/K/T|B / C / stock|C/stock paired range|
|---|---:|---:|
|Two FE numeric;737280/6/8;fast|932 /762 /759|1.000–1.008|
|Two FE numeric;368640/6/4;fast|596 /462 /661|Unranked: stock means|
|Two FE numeric;737280/6/8;low, clean retry|1029 /836 /765|1.069–1.109|
|Two FE factors;491520/19/8;fast, clean retry|1360 /1364 /1383|Unranked: stock coefficient/means|
|Serial two FE numeric;245760/6/1;fast|553 /556 /932|0.593–0.597|
|Unbalanced three-FE factors;184320/19/8;fast,cluster|762 /760 /3232|0.234–0.236|
|Disconnected three FE;38400/6/4;fast,cluster|187 /186 /13130|Unranked: stock means|
|Unbalanced three FE/four slopes;38400/6/8;low|123 /125 /119|0.983–1.076|

Fresh eligible numeric gains are 18.2%,22.5%,18.8%. Of eight fresh families,
**two meet 0.90 in every pair, three are unranked, three still miss the target**.
Across 40 B/C pairs:23 wins,13 losses,4 ties. Serial regresses3 ms, wide factors 4 ms,
four slopes 2 ms. A scientifically unresolved stock result cannot become a win,
even when its clock is much larger. No thresholds are tuned on seed 441.

A separate adaptive **one-rotation diagnostic**, unchanged weak_d3 generator,
confirms newly admitted weak graphs: N 61440/K6/T4 fast 162/135/812 ms and
N 122880/K10/T8 low 411/277/1340. All public science and direct stock checks pass.
Production trace confirms implicit initial/strict CG counts36/72 and 35/66,
respectively. These are not replicated headline claims.

## Validation and visible failures

The final focused Rust filter has **44 distinct passes and one known baseline
failure**. The unchanged multiway-rank test fails its MAP/fallback report flag
in this candidate; ADR0210 separately rebuilt the accepted baseline and proved
the same failure. No new assertion is ignored and no all-green suite is claimed.
An initial second failure was the routing test whose formerly refused topology
became newly admissible. Its degree/Q changes6/160→4/158, with N and every
assertion retained, restore the intended cold-refusal/work-history boundary.
Both attempts and the independent new-path tests are archived.

Independent operator tests use explicit weighted elimination, repeated pairs,
disconnected components, changed weights and common scales1e-200..1e200.
Full-frame analytic weighted contrasts cover nonzero residuals, an FE-collinear
column, changed weights, scales1e±100, column scales1e±20, T1/T4, dense/scratch
identity, exact/one-byte-short workspace, unfinished refusal and retained dense
routing. Five licensed fixtures pass explicit return codes and terminal
sentinels: runtime validator, hidden transaction, categorical final oracle
(72 fits), joint column-product oracle and public scientific contract.

Small public science has18 independent explicit-design Poisson/Hessian/sandwich
arm passes, including six on a newly declared repeated-pair topology that trace
confirms actually selects implicit construction. The first small four-cell
screen only reached affordable dense geometry; it remains a control, not proof
of implicit coverage. Two stock full-output residual comparisons fail at roughly
1.9e-8 maximum error (ratios4.35 and 4.67 of their coordinate envelopes). Their
coefficients, covariance, objectives and means pass. These discrepancies remain
separate from the headline means-only comparisons.

Fresh stock failures repeat on every rotation of their affected families:
N 368640 numeric means differ up to5.45e-7 (six coordinates;1.254× envelope);
large factor means up to6.40e-7 (six;1.427×) and one coefficient by 2.73e-9
(1.641×); disconnected means up to1.45e-6 (two;1.810×). All B/C comparisons
pass, including those cases. Samples agree. These are unresolved comparison
failures, not permission to relax envelopes or adjust comparator settings.

Four protected OLS definitions complete24 clean applications and 12 exactly
matching scientific export pairs. B/C medians are 66/66,77/75,14/14,210/208 ms.
The final low-factor pair regresses207→216 ms despite a lower median; it remains
in the distribution. No universal OLS timing non-regression is claimed.

## Isolation and preserved attempts

The first pilot stops at preflight without fits (foreign PID 58595). Pilot2
completes its first five families; its next family's third candidate run overlaps
PID 58987 and the following preflight stops the controller. Controls completes
three families before its next stock run overlaps PID 59363 and preflight stops.
Controls-rest completes the remaining two families cleanly. The development
summary uses only complete uncontended blocks: pilot2 cases0–4, controls0–2,
controls-rest0–1. No partial original block is pooled into a retry.

Fresh completes all 120 applications, but post-run inspection finds two transient
overlaps: stock case 2/repetition 0/PID 61659 and candidate case 3/repetition 4/PID 61851.
Both complete unchanged families are rerun (30 applications) with clean monitors.
All original timings and monitor process identities are retained. No unrelated
process is killed. Science checks remain valid and visible on completed attempts;
contended clocks are excluded. No application failures or timeouts occur in the
completed production records.

## Memory, residual cost and receipts

The existing allowance is unchanged. The complete phase plan charges at most N
incidence values, N indices, P three-index component descriptors and Q diagonal
slots, plus the existing RHS/Krylov banks. Geometry dies before certification;
no N-by-K bank is added. Exact/one-byte-short admission tests pass. Across selected
development/fresh pairs, modeled selected bytes differ by -48..+112 bytes,
tracked peak deltas range **-31,327,184..+424,088 bytes**, and modeled/observed scratch
is unchanged. The million-row fast and low peaks fall about 31.3 MB; fresh737280
falls23.5 MB. The largest rise is a disconnected control's first pair; its later
pairs and other controls vary too. This is tracked allocator evidence, not RSS.
The existing fast million-row modeled selected bound remains about 26.8 GB despite
tracked peak about 599 MB; this conservative planner discrepancy is not repaired
or recast as physical-memory qualification.

Separate diagnostic builds **canonicalframe2** and **implicitprofile1** use the
same current source computations with isolated timers. Their plugin SHAs are
`a911a5c349e3e78d1ee88886fdd189755ca99636cf2a38e14c27974a6e506a55` and
`4afde96eb554b99ec68f90907dea0db263f3b5f4e9c9eab9865d0e198cfef4a5`.
All eight diagnostic applications and four cross-build science comparisons pass
with clean monitors. Exclusive costs subtract the union of nested intervals;
never add an inclusive parent to its children. These are not stock timing claims.

|N 983040/K6/T8 canonical boundary|B ms, two repetitions|C ms, two repetitions|
|---|---|---|
|Fast initial|240.29,247.75|109.76,109.69|
|Fast strict|222.63,229.56|113.36,111.07|
|Low initial|255.81,256.36|138.53,137.37|
|Low strict|292.24,286.52|141.47,142.51|

For the first fast C fit, exclusive costs across both canonical calls include
setup40.48 ms, coefficient solve43.39, combined original-input FE certificate and
Gram21.63, finite rank24.66, separate finite-score checks13.62, bread/intercept
finalization20.18, original reference statistics3.85 and retained dense writes8.03.
Low retains roughly 66 ms finite-rank work and 23–24 ms scratch writing, versus
25/8 ms fast. The scientific checks are measured work, not irreducible lower
bounds. Much of B's repeated reference, normalization and materialization work
is gone; ordinary IRLS has not changed.

Production million-row preparation remains232/250 ms (fast/low), and publication
124/129 ms. Their field definitions include stages outside the instrumented
construction; they cannot be added to overlapping diagnostic parents. Exclusive
plugin loading is41 ms, plugin publication24 ms, initialize53–59 ms and the
unpartitioned estimator body219–227 ms. Source inspection places separation/model
preparation, final FE metadata, sample/output ownership and composite result
replay in that last boundary; this trace does not assign its entire time to any
one of them. Ordinary fast joint LSMR itself is75–77 ms, separately reconstructed
joint acceptance28 ms, and original PPML score checks about 40 ms. Final robust
inference is only 6–12 ms in production. Eliminating a small covariance kernel or
tuning the implicit matvec alone cannot close the remaining public gap.

**Next coherent subsystem:** prepared-sample and final-result ownership/replay.
Use the existing preparation/publication timers and split the219–227 ms boundary
at the named ownership transitions. Share immutable row/outcome/weight owners
where lifetime and generation permit; remove repeated copying/normalization and
combine independent original-input replay passes without replacing them by solver
recurrences. Any change needs a successor scientific decision and output,
separation, weights, memory and failure-atomicity tests. In low mode, also target
repeated finite-rank scratch scans (about 41 ms more than fast per fit). Keep the
current independently certified projection and exact final Hessian.

The clean current million-row fast and low cases still need about 223/324 ms to
reach0.90 stock; the fresh comparable737280 fast/low cases need 79/148 ms. Thus the
next change must remove substantial cross-stage work. The unresolved wide-factor
stock discrepancy precludes a target ratio there. Four-slope125/119 ms still
needs about 18 ms for0.90; do not reopen rejected threshold probes or infer its
costs from this categorical profile.

Across all production attempts, **274 public states pass and all 91 completed B/C
comparisons pass**. Including the separate diagnostic block, these become 282 and 95.
Of186 completed baseline-anchored pairs,164 pass;22 retain the two small full-output
and 20 fresh/retry stock discrepancies. Another36 planned pairs are incomplete
because controllers stopped. Direct production C/stock checks pass 71/91 completed
pairs, with 18 additional planned pairs incomplete. They check means, not the
extra full-output residual channels; this explains the two-pair difference.
All 18 independent small oracles pass. Counts do not treat missing work as success.

The content-addressed receipt archive retains every controller, attempt outcome,
scientific check, source/artifact identity, test result, monitor, recipe and
input fingerprint. Generated DTA, binary outputs and large saved-output CSVs are
hashed and omitted; raw licensed logs are replaced by protocol lines. Sources and
controlled paths are sanitized as documented by original/archive hash mappings.
All **10,792 logical receipts** were restored, hash-verified and privacy-checked.
Archive SHA: `114b4581aadbeea2af20a3b20ac1798fcb50e25e052c8bad4d5dfbc8762b3296`.
Use `python3 restore_receipts.py DESTINATION` from this report directory to
restore logical receipts; `archive-verification.json` and the adjacent SHA file
record the immutable archive identity.
The static allocation inventory's documented source drift remains a failure;
weighted constrained-fit and explicit/insufficient-budget nonretaining-CG issues
remain open. No SCC retry, Linux/Windows/RSS/physical-cap qualification, push,
release or stopped-campaign restart is performed.
