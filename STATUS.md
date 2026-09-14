# fereg 0.5.0 beta status

**Last updated:** 2026-09-14
**Branch:** main
**Mode:** prototype/development
**Release state:** public OLS and PPML beta surfaces; no qualified release,
tag, or Windows scientific qualification.

This is the sole mutable progress/handover record. Historical receipts remain
unchanged under benchmarks/reports/. This compact handover supersedes the
completed optimization/calibration investigation chronology formerly here.

## German PPML: report capacity adopted; batch retained scratch moments next

The [source-bound report](benchmarks/reports/development/german-ppml-report-capacity-local-2026-09-14/README.md)
bounds separation records/hashes by at most two generations for positive outcomes,
including initial recursive singleton removal and final replay. Zero-containing
samples retain N. The scan follows authoritative classification, allocates no
observation state and polls interruption. Actual reservations/appends obey the
same bound. Parent is `a1d0fd6` / `9d920101`; isolated plugin is `e1c72f56`.
All 177 inputs match the transfer; installed dist remains `4187efdf`.

Full N=91,045,290 probes reduce the low/pool1 plan from 156.87 to 68.74 GB,
an 88.13 GB modeled reservation saving. Pool4 needs 88.41 GB. Headroom and FE
bounds are unchanged. Both intentional one-byte-cap probes reject before fit
workspace allocation, with zero planner allocator growth. The prior normal
pool1 run returned r(909), against about 104.40 GB usable after headroom.

351 core PPML tests, 97 plugin tests on final rerun, two memory-contract tests,
universal static/ARM64 ABI checks and 26 licensed native fits pass. The first
plugin suite had one intermittent warmed-allocation event; isolated and whole
reruns passed unchanged. Baseline constrained-memory expectations were stale
for the prior low-projector adoption and are repaired; historical receipts stay.

Six paired German comparisons preserve all scientific outputs exactly, with
independent score/model-space checks and pinned reference parity. Native allocated
peaks fall 12.52→5.01 MB and 112.89→39.95 MB; RSS does not fall correspondingly.
Command medians are 1.172→1.246 s (+6.31%) and 12.281→12.307 s (+0.21%). Three
pairs do not establish systematic runtime changes. This is an admission/capacity
improvement, not a demonstrated speed gain. Fallbacks/retries are zero.

Pool1 takes 28.694 s on the larger subset; pool4 median is 12.307 s. The full
pool4 retry uses the unchanged physical-memory guard and exp(logrwage2), with
all extremes retained subject to ordinary sample rules. Attempt1 stopped before
estimation due to an incompatible diagnostic option. Attempt2 passed admission
and separation and reached the first retained finite-moment solve. A one-second
stack sample shows workers in per-row positional scratch reads; each Gram-column
partition rereads every row. It was interrupted with SIGINT after 723.36 process
seconds (65.07 GB maximum RSS, no swaps), before an estimate. The owned scratch is
preserved locally; its 12,798,182,016 bytes correspond to 88,876,264 rows by 18
slopes, but no authoritative full PPML row/result publication is claimed.

Next: batch retained scratch moment reads while preserving row/arithmetic order,
explicit worker-memory bounds, and positional-read validation; test both shared
OLS and PPML consumers before rerunning German PPML. Check partition-count bounds
when selected rank is below stored width. Raw evidence is under the local root
named in the report. Separate work and original untracked/STATUS edits remain.
No installed-binary replacement, cluster work or push.

Roadmap: [adopted capacity progress](https://github.com/johannes-schmieder/fereg/issues/27#issuecomment-5671590033).
The detailed external payload was rejected by automatic approval review; a
minimal progress note and one narrow checkbox were approved. All 49 earlier
checkboxes are preserved and 12 open items remain.

## German PPML: smaller samples pass; low-memory categorical projection adopted

The [source-bound report](benchmarks/reports/development/german-ppml-low-map-local-2026-09-14/README.md)
adopts bounded categorical MAP projection in low memory, reusing existing guarded
current-weight machinery. Dense response fusion stays fast-only; low retains
scratch blocks. Projector capacity is capped by design width, and low preflight
charges group/dot and accelerator banks before allocation. Parent native is
`1e9581e` / `bc1e8909`; candidate universal plugin is `9d920101`. All 177 compiled
inputs match the source transfer. Installed dist remains `4187efdf`.

Using the selected `exp(logrwage2)` outcome, four German subsets retain 6,654,
65,503, 132,691 and 338,054 rows with exact pinned-ppmlhdfe masks/order. Slopes
agree within 1.1e-13 and covariance within 1.3e-12. Native means pass independent
objective/score/model-space audits; independent FWL covariance agrees within
5.8e-17. Reference reconstructed means do not certify model space; its reported
objectives remain the authority under ADR 0026. Rank/absorbed-DoF normalization
bookkeeping differs by one between commands, with equal total rank/residual DoF.

The 75,350-row low fit completes in 9.824 s; parent low was interrupted at
257.68 process seconds during generic weighted projection, without a fit.
Small low falls 1.439→0.931 s. At 378,124 rows, low costs 86.049 s, fast 17.914 s
and ppmlhdfe 82.414 s; low native peak is 599 MB versus fast 1,183 MB, with
48.68 MB scratch. These are single local diagnostic timings, with tracing and
a disclosed brief contention risk for the first 75,350-row comparison. They
are not universal speed claims. All completed native solver fallback counts
are zero. Phase costs, inner/outer counts and small-memory overhead are archived.

347 PPML-related core tests, 213 OLS tests, 97 plugin tests, universal static
audit and ARM64 ABI/self-test pass. Two licensed fixtures contain 26 native fits
with one/two/three FE, tiny weighted mobility links, pools 1/2/10000, FW/PW,
clustering/missingness and singleton/separation cascades. A 557 MB cap rejects
fast before workspace allocation while low succeeds. The first dummy-Poisson
oracle's loose stopping and the independent audit's insufficient scratch budget
are preserved with their resolved reruns and unchanged candidate tolerances.

Roadmap evidence: [#27 adopted low-memory follow-up](https://github.com/johannes-schmieder/fereg/issues/27#issuecomment-5671023540).

Next: inspect full German PPML memory admission and a fitting bounded pool,
then estimate and compare with pinned ppmlhdfe. The 8.37-billion wage in the
largest subset was removed by ordinary singleton pruning in both commands;
full-sample extremes remain untested. No extra sample exclusions, SCC work,
installed-plugin replacement or push occurred. Unrelated work is preserved.

## German OLS: full fit certified after QR and macOS memory repairs

The [source-bound report](benchmarks/reports/development/german-ols-qr-memory-local-2026-09-14/README.md)
adopts long-vector compensated QR reductions and repairs macOS availability
through Mach free/inactive statistics, against `0af4321`. Rank/reference-scale,
weight, covariance, certificate and memory-headroom policies remain unchanged.
The isolated universal plugin is `bc1e8909`; installed dist remains `4187efdf`.
Patch replay matches 177 compiled inputs and the added licensed fixture. There
are 71 passing OLS fit tests, 302 PPML core tests, 97 final plugin tests, two
licensed fixtures (8 OLS and 14 PPML native fits), universal static audit and
actual ARM64 ABI/self-test. No Windows/x86 scientific runtime claim is made.

The original full German OLS specification now publishes certified estimates:
91,045,290 input rows, 2,169,026 singleton drops and **88,876,264 retained**.
All 18 slopes agree with the completed owner reghdfe result within 2.569e-11;
covariance maximum difference is 5.679e-20 and relative RSS difference 7.286e-13.
Rank is 18, absorbed DoF 14,702,025 and residual DoF 74,174,221 in both results.
Finite inverse errors are about 1e-16 under the unchanged 5e-10 tolerance.

Command time is **1,708.530 s (28.48 min)** versus the saved reghdfe run's
6,699.551 s (111.66 min), a single unpaired local ratio of 3.92. Preparation
including singleton pruning is 28.717 s. Maximum process RSS is 78.48 GB,
native allocation peak 44.55 GB and scratch 13.51 GB, with zero swaps. Initial
absorption uses three CG fallbacks (1,136.138 s solve); final residual absorption
uses one (132.160 s). A one-second stack sample is disclosed. No isolated QR
performance gain or full-reference mask comparison is claimed.

The old macOS sysctl omitted inactive pages; it can also fail under a restricted
invocation. The final plan keeps its original headroom. CMG's existing conservative
bound is about 731 GB before the 36.42 GB execution reserve, so certified CG is
selected. Tightening that broad bound without weakening admission is a next P0
candidate; no CMG sizing or hierarchy changes are adopted. The native peak is
22.33% above the published base execution estimate, within the joint solver plan.

The QR-only full attempt's r(909), earlier r(459), rejected short-vector/Givens
arithmetic, first Mach layout-length failure and zero-test launches remain
preserved. The successful full run uses CG whereas the earlier r(459) used CMG;
independent power-of-two oracles isolate the QR bug, but the full runs do not
isolate its sole causality. German subsets and exact small masks remain controls.

Next: test German PPML on smaller subsets using the user-selected imputed
wage outcome `exp(logrwage2)`, retaining its observed extremes, before any full run. Universal
singleton pruning is already adopted for OLS/PPML and all memory modes; no German
PPML result is claimed. Broader #27 work and the separate action-memory/PPML repair
remain open. No cluster work, installed-binary replacement or push occurred.

Roadmap disposition: [#27 evidence and open work](https://github.com/johannes-schmieder/fereg/issues/27#issuecomment-5670448455).

## Universal singleton pruning: prepared PPML queue adopted locally

The [incremental source-bound report](benchmarks/reports/development/ppml-singleton-index-local-2026-09-14/README.md)
extends parent `73aef645edc2` to production prepared PPML, which previously used
its own repeated membership-pass loop. Every production OLS/PPML singleton
entry path now uses an amortized linear fixed-point algorithm, independently of
fast/low/auto memory, solver, covariance or supported weight policy. PPML reuses
its CSR and partition codes, maintains exact `u128` frequency counts, queues each
deletion once and emits sorted proposals. Separation-triggered cascades use the
same boundary. Concrete per-level/row/view costs are admitted before preparation;
no-drop samples skip queue and selection-mask allocation. Ledger/reason ordering,
interruption and transactional publication are unchanged.

All 302 PPML core tests and three focused licensed fixtures pass. The new fixture
includes 14 native fits, exact drop reasons and samples, rank/status, DoF, final
weight normalization and covariance, with independent dummy Poisson coefficients
and fitted means. 640 unit/frequency multiway cases match a pass oracle; cascade
scale doubling bounds membership visits by 2*D*N. All 177 compiled inputs match
the adopted main source; isolated universal plugin is `312c7546`. Installed dist
remains `4187efdf` and was not rebuilt.

In 36 local paired commands, the 36,864-row PPML cascade falls 2.123→0.298 s;
its singleton/FE-separation phase falls 2.004218→0.001000 s. Recorded slope,
variance, counts, rank, DoF, deviance and iteration/fallback fields match exactly.
Fallback counts are zero. The largest cascade's native allocation peak increases
by 46,176 bytes; other case peaks and all scratch/selected model values match.
Balanced fast/low command medians are 1.1%/1.8% slower despite slightly faster
separation; three pairs do not establish this small variation as systematic.
A one-pass singleton kernel retains 0.03 ms overhead. Initial balanced-counter
regression was rejected before adoption; all raw receipts remain local.

Fresh pinned-reghdfe OLS screens on 75,350, 151,029 and 378,124 input rows pass
in both memory modes, retaining 65,503, 132,691 and 338,054 rows with exact masks
and DoF. Maximum coefficient difference is 1.11e-11. The largest subset costs
57.512/56.901 s (low/fast) versus reghdfe's 31.613 s: a useful weak-graph control,
not a speed win. These use the parent plugin and are single diagnostic timings.

Full German OLS estimation, unresolved at this checkpoint, is now certified
by the QR/macOS follow-up above. The user selected `exp(logrwage2)` for German
PPML; no empirical PPML success is claimed. #27 priorities and the unrelated
action-memory/PPML repair work remain open. No cluster work, installed-binary
replacement or push occurred.

## AKM singleton pruning: indexed linear-time fix, full retained count verified

The [source-bound report](benchmarks/reports/development/singleton-index-local-2026-09-14/README.md)
fixes the diagnosed observation-zero scan in `fe/singleton.rs` against `681634d`.
Each level keeps an active-row XOR; queue entries become global level indices,
paying for the index within the previous allocation envelope. Work is O(D*N+L).
The original no-singleton exit, queue/deletion order, masks, weight multiplicity,
fallible allocations, worker/SPI and interruption/publication boundaries remain.
The adopted isolated universal plugin is `253bf9eb`; installed dist is unchanged.

All 33 focused shared core tests, 95 serial plugin tests and four licensed fixtures
pass. New checks compare 640 cases with the old scanner and an independent mask
oracle, cover one-to-five effects and frequencies, long cascades, all seven
reservation/capacity failures, fast/low OLS and cluster exclusions. Patch replay
matches all 176 archived compiled inputs. In 108 bounded pruning calls, doubling
singleton-heavy inputs gives approximately 2x time versus the scanner's 4x;
65,540-row cascading pruning falls 1.106749→0.001242 s with exactly equal live
allocation peaks. No-singleton performance/allocations remain unchanged.

At this checkpoint, the original full AKM command cleared native preparation
in 27.574 s, then failed closed with r(459), `prepared OLS bread inverse certificate did not pass`,
in the unchanged low-memory numerical path. No full estimates are produced.
A separately archived diagnostic plugin (`e9b2141e`) reports the unchanged
prepared sample and deliberately stops before solving: 91,045,290 original and
candidate rows, 2,169,026 singleton drops, **88,876,264 retained**, exactly matching
reghdfe. Preparation is 29.582 s; process maximum RSS is 48.16 GB. Its expected
r(1) and no-public-coefficients assertion pass. This diagnostic hook is not adopted.
The normal candidate smoke retains 65,503 rows; its 18 coefficient and V maxima
against saved reghdfe results are 1.77913e-14 and 1.37152e-17, with exact N/DoF.

The later QR/macOS follow-up above now certifies full OLS estimation under
unchanged tolerances. This earlier inverse-failure receipt remains unchanged.
All failed runs/builds,
original data/scripts and historical receipts remain. This fix leaves #27's seven
P0–P7 items and the isolated action-memory prototype open; it does not add solver
changes, agents, cluster work or a push. Unrelated STATUS additions and all 358
original untracked paths are preserved.

## P0 CMG/Schur memory lifetimes: tested prototype continuation, not adopted

The [source-bound report](benchmarks/reports/development/action-memory-cmg-schur-local-2026-09-14/README.md)
archives full patch `2e03030b` and five-file incremental patch `d02e385b` against
carrier `082e1625`, evidence `2739915` and accepted native `25bf610`. Borrowed CG
reports actual fixed-bank/matrix/coefficient capacities. CMG retains admitted
construction through setup failure and hierarchy rejection, observes lazy pool
growth, charges hierarchy/coefficient overlap during ordinary polishing, and
separates released-hierarchy final correction. Three-vector norm construction
and its retained output are included. OLS Schur now carries allocated probe,
full-attempt and CG-continuation/reference-norm peaks through the outer adapter.

All 86 distinct focused core tests and 95 serial plugin tests pass on the final
frozen source without compiler warnings; replay matches all 179 candidate inputs.
Six new tests cover oversized reservations/early exits, real failed nuisance
construction, rejected admission, one-to-four pool growth, full-width coefficient
reconstruction, retained versus released correction, and one-time outer merging.
Existing weighted independent-MAP, forced Schur continuation, weak-graph and
exact/one-byte-short admission checks pass. Initial edit assertion, incorrect
pool accessor and test-only adapter warnings remain documented. No scientific
threshold or fixture was loosened.

The implementation stays isolated; public reporting, all 178 accepted main inputs
and installed dist remain unchanged. No release build, licensed-Stata oracle,
new public cap or command timing ran, so no performance gain or whole-action
bound is claimed. Next cover warm retained LSMR, saved MAP conversion/failed CMG
coefficient attempts, specialized slopes, recovery/inference and final action
transport, then the original three-FE and weak-wide checks. Prepared PPML parent
accounting also remains open. Full pass/copy/build/physical-attempt reporting and
seven P0–P7 items remain; R1–R5 are excluded. Reviews, upstream assessment and SCC
follow P0–P7. All 97 unrelated STATUS additions and 358 original untracked paths
remain; no agent, cluster, upstream change or push occurred.

## P0 compact action-memory carrier: implemented prototype, not adopted

The [source-bound checkpoint](benchmarks/reports/development/action-memory-carrier-local-2026-09-14/README.md)
archives patch `082e1625` against evidence `d0c0e86`/accepted native `25bf610`.
It adds an eight-byte workspace-peak carrier with a distinct unmodeled state,
reuses generic/owned/direct-CG admission quantities, counts fixed borrowed-MAP
capacities, and preserves admitted versus rejected/fallback attempt envelopes.
Low-memory report merges propagate unknown coverage. Warm retained LSMR overlap,
CMG/Schur construction, specialized slopes, recovery/inference and whole-action
transport remain explicitly unmodeled; public selected-memory reporting is unchanged.

All 41 distinct focused core tests and 95 serial plugin unit tests pass on the
final frozen source; an isolated patch replay matches all 179 candidate inputs.
Tests cover closed-form residuals, exact/one-byte-short bounds, weak-chain routing,
capacity over-reservation, discarded LSMR fallback and cold/warm reuse. Review found
and guarded retained-operator overlap and deferred compact conversion for an unused
CG route. Formatter/archive-parser findings remain recorded. No release-plugin
build, licensed oracle or performance measurement ran; avoiding the earlier
weak-wide overhead remains a hypothesis. Main retains all 178 accepted native
inputs and installed dist. The prototype stays isolated, with its replayable
patch/evidence archived here rather than adopted as complete reporting.

Next finish CMG/Schur construction and retained solver/pool accounting, saved
coefficient/recovery and inference propagation, then aggregate and publish the
complete model. Verify the original three-FE joint cap and the earlier weak-wide
cost controls. P0 pass/copy/build/physical-attempt work and the seven active P0–P7
items remain open; R1–R5 are excluded. Reviews, upstream assessment and SCC follow
P0–P7. No agents, cluster work, upstream change or push occurred.

## P6 generated OLS residual tiles: rejected; P0 three-FE memory gap

The [source-bound report](benchmarks/reports/development/generated-ols-residual-tiles-local-2026-09-14/README.md)
rejects candidate `e97dee05` against accepted `25bf610`/plugin `e6978061`.
Shared generated recipe tiles preserve each prepared layout's absent-indicator
FMA semantics. All 16 core/95 plugin tests, eight independent licensed fits per
artifact and universal/static/arm64 ABI checks pass. All 25 untraced pairs retain
exact science/solver fields. Repeated AW/unit/PW command medians are
1.095→1.039 / 1.252→1.195 / 1.553→1.484 seconds; median paired gains are
5.18%/5.25%/4.44%. PW misses the predeclared 5% repeated-primary gate. Separate
original-input diagnostics locate 77.978 ms in fast raw-residual accumulation;
low construction stays about 4.2 ms. Largest control cost is low PW +48 ms/2.21%.

All paired native peaks are exact, but the original actual-three-FE FW OLS
control peaks at 50,265,168 bytes against a reported 22,983,168-byte selected model
on both artifacts. This is a baseline P0 memory-accounting gap, not an admitted
control. Other cases fit their models; PW sampled RSS rises. Both violations,
all 56 captures and the summary's failed admission assertion remain. No extra
repeats/public cap calls follow rejection. All 178 accepted source inputs are
restored; the prototype source/patch/artifacts remain isolated and archived.

Source [adjudication](benchmarks/reports/development/generated-ols-residual-tiles-local-2026-09-14/memory-gap-adjudication.md)
clarifies that the public value is a base execution model; adapters separately
admit incremental solver workspace against the usable cap. This confirms the
known P0 reporting gap, not an established breach of actual joint admission.
Next complete action/attempt memory reporting and verify the joint boundary on
the original three-FE inputs, including the older reporting prototype's known
weak-wide timing and coverage limitations. Do not inflate a multiplier to cover
the observed number.

Seven P0–P7 items remain. Next diagnose and repair that three-FE memory gap on its
original full/warm inputs before broader timing work. Then finish generated/shared
layouts, remaining PPML state/construction, P7 scheduling and P0 full accounting.
Outer Stata work is also material in the generated targets (about 0.44–0.53 s)
but is not yet attributed to a particular ado consumer. R1–R5 remain excluded;
independent reviews, upstream CMG assessment and the bounded SCC campaign follow
P0–P7. Installed dist, unrelated STATUS additions and original untracked files
remain. No agents, upstream changes, cluster work or push occurred.

## P4/P6 generated finite-predictor tiles: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/generated-predictor-tiles-local-2026-09-14/README.md)
and [ADR 0068](docs/adr/0068-retained-generated-predictor-tiles.md) adopt candidate
`e6978061` against evidence `ef38dd5`/accepted native `fb4cc5b`. Fresh/strict
retained generated predictors borrow final ordinary/topology factor codes into
128-row tiles under the existing team. Original coefficient-slot FMA order,
including zero indicators, remains exact. The fixed 1024-byte stack bank adds
no retained/N-sized allocation. Numeric precedence, ordinary frozen PPML, OLS
subtraction, rank, weight generations and independent reconstruction remain.

Nine primary pairs give FW/unit/PW command medians 1.438→1.366 / 4.029→3.691 /
1.464→1.363 seconds; median paired gains are 5.92%/8.39%/6.64%. Separate original-
input diagnostics locate 99.293 ms in three FW accumulations (113.483→14.190 ms),
with unchanged independent certification. All 23 successful untraced pairs have
exact science and non-time solver decisions. Fourteen successful controls cover
weak graphs, slopes, three FE, low memory, saved outputs and width/sample guards;
largest added cost is 6 ms on small numeric. Primary peaks are exact/admitted;
one weak-PW peak is lower by 1.380 MB, without an established cause or memory claim.

The original topology-backed/exact-duplicate control fails r(430) on both artifacts
in both warm/full fits because the final Hessian changes canonical basis. All
failures remain; explicit checks verify no coefficients are published. This is
failed positive public coverage, not a speed result, and no rank/tolerance rule
was changed. Sixteen distinct core and 95 serial plugin tests, seven independent
OLS/dummy-Poisson/Hessian/sandwich fits per artifact, universal/static/arm64 ABI
checks pass. Both artifacts admit the generated width-eight control at exactly
18,154,399 usable bytes and reject one byte below with r(909). All 54 captures,
formatting/Mata/summary-assumption failures, source and artifact identities remain.

Seven P0–P7 items remain. Next audit generated OLS raw-residual consumers and
shared slope/three-plus-FE layouts, including the unchanged canonical-basis
failure as a coverage limitation; then remaining PPML state/construction, P7
scheduling and P0 complete accounting. R1–R5 are excluded. Independent reviews,
upstream CMG assessment and the bounded SCC campaign follow P0–P7. Installed
dist, unrelated STATUS additions and original untracked files remain; no agents,
cluster work, upstream change or push occurred.

## P6 generated RHS tiles: standalone prototype rejected

The [source-bound screen](benchmarks/reports/development/generated-rhs-tiles-local-2026-09-14/README.md)
rejects shared generated RHS tiles against accepted `fb4cc5b`. Candidate `6bbc5ace`
uses borrowed ordinary/topology factor codes and a fixed 128-byte interaction mask
for both prepared layouts and low-memory blocks. All 18 focused core tests, 95
serial plugin tests, five independent dummy/Poisson/Hessian/sandwich fits per arm
and universal/static/actual arm64 ABI checks pass. Original fixture compile,
base-policy, rebased-axis, PPML omitted-slot and archive-interpreter failures remain.

Three untraced pairs preserve exact scientific exports and solver decisions:
fast AW OLS 1.118→1.129 s, low PW clustered OLS 2.224→2.231, FW PPML 1.522→1.481.
None meets the prospectively fixed 5% and 10 ms gain gate. Three separate diagnostic
pairs preserve original science; fast AW RHS saves only 6.311 ms (31.432→25.121),
and its entire baseline scope is 2.81% of command time. All three native peaks
are exact and admitted; RSS rises slightly. No extra seeds/graph timing/private
audit/public exact-cap calls follow rejection. All twelve captures and the patch
are archived; all 178 accepted inputs and installed dist are restored/unchanged.

Seven P0–P7 items remain. Next target repeated generated raw-residual/predictor
consumers, then shared slope/three-plus-FE layout, remaining PPML state, scheduling
and complete accounting. R1–R5 are excluded. Independent reviews, upstream CMG
assessment and the bounded SCC campaign follow P0–P7. No agents, cluster or push.

## P4 guarded PPML Schur checkpoints: adopted local checkpoint

The [final source-bound report](benchmarks/reports/development/ppml-schur-checkpoint-adoption-local-2026-09-14/README.md)
and [ADR 0067](docs/adr/0067-guarded-ppml-schur-checkpoints.md) adopt candidate
`f4cb7dee` against evidence `7996a5e`/accepted native `7739e8e`. Eligible admitted
wide two-FE checkpoints reuse the actual-weight probe and retained Schur graph;
a fallible transient copy preserves original-input LSMR/tighter retry/MAP fallback.
Ordinary joint LSMR, canonical raw norms/exact tolerances, rank, weights, covariance,
reconstruction and publication policies remain unchanged.

Final review added combined capped-Schur and double-LSMR-certificate rejection,
which exposed stale scalar energy in the later MAP recurrence. The final shared
entry clears all scalar banks and preserves both fallback diagnostics. The
original failed source/test and preliminary `cfeed644` measurements are archived;
that artifact was not adopted. The unchanged 2e-10 independent test is green, and
all comparisons/oracles were rerun on the corrected artifact without pooling.

Nine primary pairs give unit/FW/PW command medians 2.312→1.679 / 3.186→1.910 /
2.007→1.716 seconds; paired median reductions are 27.9%/40.1%/14.3%. Native peak
medians fall 22–30% within unchanged models. Candidate auto beats accepted explicit
MAP on all three FW inputs by 32.3–35.6%. Eleven protected controls preserve science;
maximum added time is 6 ms on three FE. Separate exact-science traces show changing-
weight probe rejection on weak/bottleneck graphs and both probes rejected on cycle/
tiny-weight bridges. All attempted work remains included; P0's physical attempt,
copy/build and complete reporting work remains open.

All 67 distinct focused core and 95 serial plugin tests, nine native and three
independent Poisson/Hessian/sandwich fits, universal static and actual arm64 ABI
checks pass. Public explicit LSMR admits both artifacts at exactly 2,429,462,126
usable bytes and rejects one byte below with r(909). A saved-two-FE/mean control
improves 2.472→1.860 seconds; all 122880 original-row outputs match at unchanged
tolerances (maximum absolute difference 3.28e-10). The final 52 main and six
supplemental captures retain both expected admission errors and source/artifact
identities; the preliminary 58 captures remain separate. Installed dist, 97 unrelated
STATUS additions and 358 original untracked paths remain. No agents, cluster,
upstream changes or push occurred.

P4's weighted-Schur comparison is resolved; seven active P0–P7 items remain. Next
continue generated/interacted block evaluation and shared slope/three-plus-FE
layouts, remaining PPML construction/state work, P7 scheduling and P0 accounting.
R1–R5 stay excluded; two independent reviews, upstream CMG assessment and the
bounded SCC campaign follow P0–P7.

## P4 weighted Schur route screen: checkpoint opportunity, no new route

The [source-bound existing-route screen](benchmarks/reports/development/ppml-weighted-schur-screen-local-2026-09-14/README.md)
compares accepted native `7739e8e`/plugin `2689faf0` under auto/MAP/CMG. PPML MAP
already calls the same weighted Schur recurrence as OLS after its pilot; no new
engine is needed. Nine independent two-FE/16-control Poisson/Hessian/sandwich
fits pass across unit/FW/PW and all three routes, including independently formed
correction factors. Nine untraced calls at N122880/K50 preserve exact discrete
science, registered b/V/objective tolerances and admitted native peaks.

Auto/MAP/CMG seconds are 2.342/2.978/4.142 dense unit, 3.289/2.878/7.203 strong FW
and 2.263/2.451/3.109 dense PW. MAP wins the strong FW screen by 12.5% and has
lower peaks, but loses both dense screens. Strict/final scopes are markedly
cheaper with MAP (0.594/0.549/0.483 s versus auto 0.933/1.610/0.985 s).
Nine separate trace calls preserve exact original-route science. Full LSMR
projection-attempt widths are 52/51/1 for unit and 52/51/51/1 for FW/PW: both
weighted strict 51-RHS first attempts miss the unchanged 1e-12 certificate at
about 1.16e-12 and the existing 1e-14 recurrence retry passes. All failed-attempt
costs remain. Original missing checkpoint trace flags and display corrections
are disclosed; aggregated solver observations are not literal attempt counts.

All 18 captures and replay scripts are archived. This is a single-seed baseline
screen, not an adopted optimization or completed P4 Schur item. Next implement a
bounded current-weight Schur checkpoint candidate using the existing recurrence,
retaining ordinary joint LSMR, original-input fallback, independent canonical
certificates and joint memory admission. Then repeat the original comparisons
and protected cases. Eight active P0–P7 items remain; R1–R5, independent reviews,
upstream and SCC retain their previously stated scope/order. No source/artifact
replacement, agents, cluster work or push occurred.

## P4 mean validity/reuse: standalone prototype rejected

The [source-bound report](benchmarks/reports/development/ppml-mean-reuse-local-2026-09-14/README.md)
rejects mean reuse against evidence `ebf038f`, accepted native `7739e8e`.
Candidate `8a85e77c` tracks paired eta/mean bounds validity through accepted/trial
swaps, invalidating before partial writes and strict-candidate scratch use.
All 58 focused core tests, 95 serial plugin tests, ten independent licensed
Poisson/Hessian/sandwich/generated/narrow fits per arm and universal/static/actual
arm64 ABI pass. Four untraced pairs and three separate diagnostic pairs preserve
exact science and solver decisions; all untraced native peaks/models are exact
and admitted. The actual workspace adds 80 fixed bytes, with no heap/vector.

Complete/FW/PW/narrow screens are 7.600→7.251 / 2.458→2.375 / 2.198→2.177 /
1.446→1.432 seconds. Complete/FW meet the time-only 2% and 10 ms gate, but direct
all-call diagnostics show only 11.724/3.613 ms saved in mean construction; narrow
saves 11.583 ms. The entire baseline mean scope is only 0.20%/0.18%/1.02% of
those command times. Most apparent wide-target gain is in unchanged finite work,
so the attribution gate fails. No extra seeds/holdout timings or private audit
follow. Narrow sampled RSS rises about 51 MiB despite its exact native peak.

The rejected patch, diagnostic identities and all 14 captures are archived;
all 178 accepted inputs and installed dist are restored/unchanged. Eight active
P0–P7 items remain. Next compare weighted Schur with actual joint LSMR/MAP/CMG
PPML workloads, then finish generated/slope/three-plus-FE layout, P7 and P0 work.
R1–R5 remain excluded. Independent reviews, upstream CMG assessment and the
bounded SCC campaign follow P0–P7. No agents, cluster work or push occurred.

## P4 complete-frame refresh: standalone prototype rejected

The [source-bound screen](benchmarks/reports/development/ppml-frame-refresh-local-2026-09-14/README.md)
rejects combined response/weight refresh against accepted `7739e8e`. Candidate
`ddb302c6` removed duplicate old-weight/response validation and applied the exact
weight-bit/all-scale identity rule to ordinary calls, retaining invalidation and
prior-response storage. Eleven core checks, 95 serial plugin tests, ten independent
Poisson/Hessian/sandwich/generated/narrow fits per arm and universal/static/native
arm64 ABI pass. All four complete-command pairs preserve exact science and solver
fields, with exact admitted native peaks.

Complete/FW/PW/narrow screens are 7.408→7.496 / 2.509→2.419 / 2.219→2.214 /
1.427→1.458 seconds (+1.19%/-3.59%/-0.23%/+2.17%). None meets the prospective
5% and 10 ms gain gate. Weight/response phases move only a few milliseconds;
most FW savings are in unchanged finite work. No extra seeds/holdout timings,
tracing or private allocation audit followed rejection. The distinct one-control
request was fixed before timing on unchanged original full/warm data.

The candidate patch and eight captures remain archived; all 178 accepted inputs
are restored. The predictor optimization remains adopted. Eight active P0–P7
items remain. Next examine mean reuse across accepted/trial swaps and strict
scratch reuse, including bounds/error invalidation, then the remaining P4/P6/P7
and P0 work. R1–R5 are excluded; the two independent reviews, upstream CMG
assessment and bounded SCC campaign remain later steps. Installed dist and
unrelated work are preserved; no agents, cluster work or push occurred.

## P4 retained numeric predictor tiles: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/ppml-predictor-tiles-local-2026-09-14/README.md)
and [ADR 0066](docs/adr/0066-retained-numeric-predictor-tiles.md) adopt bounded
numeric finite-predictor tiles against `e65d6f9`, isolated plugin `2689faf0`.
Fresh/strict retained solves reuse the existing numeric bindings and installed
team, preserving selected-column FMA order with a fixed stack bank and the
original reconstruction/certification. Ordinary frozen PPML, generic fallback,
reference scales, generation validity, low-memory storage and one-shot OLS remain.

Three-seed complete/FW/PW command medians are 7.785→7.467 / 2.585→2.436 /
2.376→2.202 seconds; median paired reductions are 5.72%/5.76%/6.33%. Separate
exact-science diagnostics locate 82/90 ms in the three weighted predictor
accumulations; unchanged reconstruction/certification remains about 3.4 ms.
All 25 untraced A/B pairs, six A/A pairs and three diagnostic pairs preserve
exact science and solver decisions. Untraced native peaks are exact and admitted;
weighted sampled RSS rises slightly. This is no memory-saving claim.

Original low-FW costs +142/+171/+3 ms remain, alongside later +151/+155 ms flags.
Six interleaved identical-binary A/A pairs range -132 to +185 ms. Six new A/B
pairs have median -24 ms/-2.62%; all nine A/B pairs have median +3 ms/+0.29%,
passing the prospectively fixed median cost gates. Adopt with the variability
and individual flags disclosed, without a low-FW speedup or zero-cost claim.
The low changed kernel saves about 2 ms, not enough to explain those swings.

Validation passes 20 distinct core checks, 95 serial plugin tests, four positive
independent Poisson/Hessian/sandwich/generated-factor fits per arm, and universal/
static/native arm64 ABI. Initial wrong test import, zero-match selector and two
archive collector failures are retained. All 178 candidate inputs match main;
the installed plugin and 97 unrelated STATUS additions/358 untracked paths remain.

Eight active P0–P7 items remain. Next reduce duplicated PPML response/weight
refresh passes while preserving exact frame identity and invalidation; then
weighted Schur comparisons, remaining generated/slope/three-FE layouts, P7 and
P0 reporting. The user excludes R1–R5 from this run. After P0–P7, two independent
agents review changes, followed by upstream CMG small-components assessment and
the bounded SCC benchmark campaign. No agents, cluster work or push occurred.

## P3/P6 low-memory OLS score retention: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/low-score-retention-local-2026-09-13/README.md)
and [ADR 0065](docs/adr/0065-low-memory-ols-score-retention.md) adopt original
projected-column scratch retention against `26d05b0`. Ordinary low robust and
clustered OLS hand a private owner from fit to inference, outside the certified
result. The copy borrows the existing I/O bank before QR and takes it afterward;
actual post-QR identities select columns for bounded row tiles. Independent raw
means and score/covariance certificates remain. Optional file/copy failures keep
the uncached path. IID, fast and shared PPML session policies are unchanged.

Thirteen untraced pairs and eight diagnostic/cap calls pass science, with exact
paired full b and maximum V difference 1.58e-15. Three same-input weak two-FE AW
repeats improve medians 0.198→0.148 seconds (25.25% paired median); score storage
falls 56.40→4.37 ms for 0.40 ms copy cost. Single slope/three-FE saved/rank-deficient
robust controls improve 15.2%/17.3%/11.0%. Wide AW 40.737→38.702 is 4.99546%, below
the declared 5% gain gate; PW 0.929→0.962 costs 33 ms/3.55% and sampled RSS rises
0.157→0.207 GiB. No untraced pair exceeds both 5% and 10 ms added cost. The original
rank-deficient saved control used IID; it is preserved and supplemented by robust
inference on the exact same data, with rank 49 selected from 51 generated columns.

The larger scratch footprint is accepted: logical peak becomes 16*N*G rather
than 8*N*max(G,2*R). The rank-deficient robust case rises 24.08→25.07 MB scratch
while native peak falls 40.59→38.20 MB. There is no general native/RSS saving claim
or new resident N-by-K copy. The weak public 9.41 MB execution base excludes
solver reservations. Current-model replay gives 75,268,168 usable bytes for LSMR
and 13,911,784 for CG; both original plugins pass both shared caps. At the smaller
cap both select CG and peak near 11.63 MB, with full science preserved and capped
versus unrestricted V difference at most 1.92e-13. Broader P0 reporting remains.

Validation passes 38 focused core checks, 95 serial plugin tests, 49 candidate
positive licensed fits, four baseline supplement fits and universal/static/native
arm64 ABI. Rank-selected/corrupt scratch, donor-buffer return after failed writes,
cleanup and independent dummy/sandwich/CGM checks are covered. The original
unsupported factor-by-continuous request fails closed on both plugins; a distinct
supported generated-interaction fixture passes both. Initial missing-import and
archive fixture-lookup errors are preserved separately. All 178 candidate inputs
match main and plugin `365bee73`; installed dist and unrelated work are unchanged.

This closes P3 low-memory duplicate residualization and P6 bounded column/scratch
handoff. Thirteen #27 items remain. Next examine remaining PPML construction,
weight/response/predictor passes and generation-valid reuse, then its weighted
Schur comparison, remaining P6/P7 and R1–R5. Complete #27 before the authorized
agents, upstream CMG assessment and small SCC campaign. No push or cluster work.

## P6 saved OLS final-target recovery: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/saved-schur-adoption-local-2026-09-13/README.md)
and [ADR 0064](docs/adr/0064-saved-ols-final-target-recovery.md) adopt the five-file
saved-Schur/recovery extension against accepted `bc5a6ae`. Public one-shot OLS
can recover requested categorical effects from its certified final target when
an accepted Schur solve omits initial coefficient products. Rejected probes keep
prior retention/routing; shared PPML defaults, weighted-operator/width guards,
continuation budgets and independent final 1e-14 output precision remain.
Intercept recovery attempts now enforce joint admission, including finishing
and SD/LSMR overlap, with the existing cheaper MAP fallback. Slopes/PPML retain
existing recovery policies; broader accounting remains open.

All 23 pairs pass full science and saved-output comparisons. Three-seed unit/AW/
PW command medians fall 3.012→0.809 / 83.397→68.675 / 3.641→2.676 seconds; median
paired gains are 73.5%/17.7%/26.8%. Native peak medians fall 54.9%/31.2%/30.9%.
Initial absorption supplies the main gains; final precision, recovery, complete
64-step attempts and CG continuations are included. Rejecting unit/PW seeds add
54/20 ms (1.80%/0.55%); rejecting AW adds 3.851 seconds/4.62%, mostly in unchanged
CG for an unestablished reason. No pair exceeds both 5% and 10 ms added cost.
The weighted cut rejects with exact science/peak and costs +10 ms/10.1%.
Earlier standalone admission +36/+43 ms flags remain historical and unexplained.

Disconnected/unbalanced/automatic-CMG saved controls improve 30–39% command time
but native peaks rise 19.97→36.54 / 19.93→26.06 / 17.50→32.87 MB. An isolated
allocator diagnostic reproduces exact candidate science and locates all these
peaks in final LSMR absorption; recovery peaks near 8.05 MB. At 35,148,872 usable
bytes, those three plus weak-d3 use the existing admitted final CG continuation
and peak at 15.2–17.3 MB. Independent full b/V and saved-output checks pass;
maximum cap/unrestricted output difference is 2.39e-12. The unrestricted memory
tradeoff is accepted with the proven bounded alternative, without a seed-specific
guard. Public execution-base reporting is still not a whole-action bound.

Validation passes 124 focused core checks, 95 serial plugin tests, 48 saved dummy
fits with eligible-Schur assertion, four weighted omission fits, exact/one-byte-
short recovery/CG/generic and original unit/PW initial public boundaries, plus
universal/static/native arm64 ABI. Initial unit/PW bounds are 827753800/831179336
usable bytes with admitted peaks. The oracle harness's 16-versus-17-coordinate
failure is preserved; independent grounded-dummy/SVD response influence extends
weak/disconnected/unbalanced oracles to the normalized intercept/full covariance.
Completed captures were reused by identity; independent LSQR/LSMR still agree.
No seed/input/tolerance was replaced. All 178 candidate inputs match main and
plugin `8bfa9fc2`; the diagnostic artifact is separate. The report archives 97
capture entries (58 command calls and 39 oracle CSVs), source and log identities.
Installed dist, 97 unrelated STATUS additions and 358 original untracked paths
remain intact. No agents, cluster, upstream update, qualification or push.

This closes only P6 final-target saved-FE recovery; 15 #27 items remain. Next
continue P3's low-memory scratch/duplicate projection audit, then generated and
sloped layouts, PPML pass/state/Schur work and R1–R5. Complete #27 before the
user-authorized two-agent investigation, upstream CMG review and small SCC run.

## P6 final-output precision: adopted on repaired memory admission

The [source-bound adoption](benchmarks/reports/development/ols-output-precision-adoption-local-2026-09-13/README.md)
and [ADR 0063](docs/adr/0063-ols-derived-output-final-precision.md) adopt the
three-file precision repair against `58bcf3a`. Isolated universal plugin
`14b396c7` binds 178 inputs. Requested residual/absorbed/combined-prediction/saved-FE
outputs use a separate final target at most 1e-14 in fast/low/projection-only fits,
independent of coefficient retention. Terminal CMG releases its hierarchy before
an admitted certified CG correction. Initial projection, reference scales, rank,
weights/sample/DoF and public tolerances remain unchanged.

All 109 focused core checks, 95 serial plugin tests, 48 saved dummy fits, four
weighted omission cases and universal/static/native arm64 ABI checks pass. Public
CG/generic exact and one-byte-short boundaries pass with admitted native peaks.
The original saved slope succeeds at 33268760 usable bytes with 28906404 observed,
using certified CG after memory-rejected final LSMR. One byte below rejects with
prior estimates/sample/order/output absence preserved; constrained saved values
match the previous unrestricted strict candidate within 1e-14. The original tiny
weighted cut passes all 30720 rows against the preserved high-precision/direct
oracles: maximum residual error 3.05e-13, full b/V and both saved FE pass.

All 19 paired commands pass candidate science, including the original thirteen
cases and six prescribed reversed follow-ups. Six baseline saved-output errors
remain independently adjudicated and excluded from accuracy-matched speed claims.
Repeated slope/weighted-cut/automatic-CMG added costs are 28/53/18 ms medians.
Unit/PW add 91/116 ms in their original pairs, largely final LSMR setup/solve.
Unrestricted slope native peak rises 28.87 to 38.54 MB; the constrained route
above fits. Other original peaks are exact except three FE +78933 bytes, low FW
+149 bytes and weak -1072016 bytes, without causal attribution. Base/scratch
models are unchanged and RSS moves both ways. This is a correctness adoption,
not a speed or general memory improvement. Earlier failed candidates/concurrent
plugin failure remain historical evidence, not fixes claimed by the serial run.

Next rebase the saved-Schur/final-target recovery prototype on this precision
baseline and repeat its original comparison. It remains unadopted; all 16 open
#27 items, broader memory reporting and specialized slope/recovery accounting
remain. The installed plugin, 97 unrelated STATUS additions and 358 original
untracked paths are preserved. No agents, SCC, upstream update, qualification
or push occurred.

## P0 generic owned-FE admission: adopted local repair

The [source-bound report](benchmarks/reports/development/generic-owned-fe-admission-local-2026-09-13/README.md)
adopts [ADR 0062](docs/adr/0062-generic-owned-fe-memory-admission.md) against
accepted numerical `f576bc5` at evidence HEAD `7ae569d`. The isolated universal
candidate `ad725932` binds 178 inputs. Owned-design construction and retained
codes/slopes/memberships/unit weights now enter joint admission. Generic MAP,
CG, accelerators and LSMR check their actual route, including explicit fallback
copies and separately admitted CG continuation after rejected LSMR. A cheap MAP
pilot need not reserve unused LSMR. Zero-control fits charge three overlapping
caller observation vectors. Numerical rules and the categorical model remain.

Four accepted-source red regressions are green. All 134 focused core checks,
95 serial plugin tests, 48 saved dummy fits, four weighted omission cases and
universal/static/native arm64 ABI checks pass. The public AW intercept/slope
fixture with zero finite controls matches independent dummy residuals. Its exact
cap leaves 806328 usable bytes and observes 730668 native bytes; one byte below
admission returns r(909) and preserves prior estimates/sample/order/output absence.
The superseded 708024-byte estimate failed the observed-peak check. Separate
allocator tracing localized the missing caller overlap; its diagnostic artifact,
failed estimate and initial build/collector corrections remain archived.

Ten fixed paired commands preserve exact scientific exports and OLS non-time
solver fields. Original slope auto/LSMR/CG changes are +2/+3/-1 ms; none crosses
the prospective 5% and 10 ms review threshold. Native peaks are exact in nine
pairs; weak saved OLS adds 530456 bytes (1.6%) for an untraced reason. Public
bases/scratch remain unchanged and RSS moves both ways. This is a correctness
repair, with no claimed speedup or general memory reduction.

Next rebase and validate the separate final-output precision repair, then resume
saved-Schur/recovery comparisons. Both prototypes remain unadopted. Specialized
slope MAP, recovery, whole-action reporting and opaque factor-storage bounds
remain separate P0 work; all 16 unresolved #27 items remain open. The installed
plugin, 97 unrelated STATUS additions and all 358 original untracked paths are
preserved. No agents, SCC, qualification, dependency update or push occurred.

## P0/P6 saved-output precision: science passes; generic admission prerequisite

The [source-bound investigation](benchmarks/reports/development/ols-saved-residual-precision-local-2026-09-13/README.md)
preserves four precision candidates and the separate
[saved-Schur failures/oracles](benchmarks/reports/development/saved-schur-residual-precision-local-2026-09-13/README.md).
Accepted numerical source remains `f576bc5`; neither the three-file precision
repair nor the archived five-file Schur/recovery optimization is adopted.

The final isolated precision artifact `76858524` binds 176 inputs. Requested
residual-derived outputs receive a separate final tolerance at most 1e-14 in
fast/low/projection-only fits. Terminal CMG releases its hierarchy before checked
CG correction. The original tiny weighted cut passes all 30,720 rows against an
80-digit grounded oracle and independent LSQR/LSMR: maximum residual error
3.05e-13, versus 1.76e-9 and 28 failing rows at the preceding 1e-12 target. Full
b/V, saved FE, rank and DoF pass at unchanged public tolerances.

All 97 focused core tests, 48 saved dummy fits, four weighted omission cases,
public exact/one-byte-short CG caps, universal build and native ABI checks pass.
The initial plugin suite passes 95/95; a final concurrent rerun passes 94/95 with
one unexpected PPML warm-allocation event. Ten fixed exact checks per accepted/
candidate binary and one complete serial 95-test suite per binary all pass.
The failed concurrent result and unresolved cause remain evidence; no test or
PPML production code was weakened. Timings start only after the serial gates.

Nineteen complete cost pairs include all thirteen planned inputs and six
prescribed reversed follow-ups. Candidate science passes throughout; independent
oracles adjudicate six original baseline output-error cases, excluded from
accuracy-matched speed claims. Median added costs are slope +27 ms/7.6%, weighted
cut +54 ms, and automatic CMG +19 ms. These are correctness costs, not speedups.
The slope final projection switches MAP→LSMR and native peak rises 28.87→38.54 MB
against an unchanged public 18.93 MB execution base. Full phases, peaks, scratch,
RSS, fallback/warning fields and all failed/superseded captures are archived.

Source inspection and four isolated accepted-source closed-form regressions
prove that generic owned-FE auto/MAP/CG/LSMR ignore the supplied joint workspace
budget. Each unrestricted residual matches the oracle, then incorrectly succeeds
at a one-byte budget. This pre-existing gap blocks precision adoption because
the new final slope LSMR workspace needs checked admission. The initial missing
test-fixture build failure is preserved; exact accepted fixture bytes produce
all four intended red assertions.

Next implement checked generic owned-design and solver/fallback admission, test
the original slope and constrained boundaries, then resume precision adoption
and the saved-Schur comparison on that repaired baseline. Do not drop slope
coverage or relax precision to evade the gap. P0 reporting, P6 and the other
#27 items remain open. All 97 unrelated STATUS additions, 358 original untracked
paths and the protected installed plugin remain preserved in this step. No
agents, SCC, qualification, dependency update or push.

## P6 prerequisite: omitted-column output publication repaired

The [source-bound repair](benchmarks/reports/development/ols-derived-output-omissions-local-2026-09-13/README.md)
adopts [ADR 0061](docs/adr/0061-ols-derived-output-omitted-columns.md) against
accepted `834703b`. OLS derived outputs now skip generated columns carrying the
existing FE-collinear or numerical-rank-deficient status and no coefficient.
Canonical slots, rank, zero public coordinates and all numerical fit/inference
and transaction policies remain. The isolated one-file candidate `fe066c02`
excludes the saved-Schur/recovery prototype and binds 176 inputs.

A closed-form regression fails at the original selector and passes in fast/low
modes after repair. The initial post-fix test's wrong wire-status assertion is
preserved and corrected to existing status 3. All 95 plugin tests, four weighted
independent omission/output oracle cases per artifact and 48 ordinary saved-output
fits pass. The exact original slope and rank-deficient saved warm/full requests
now succeed, retaining ranks 15/49 and independent native certificates. Universal
build/static and actual arm64 ABI checks pass. Public base-model gaps on those
functional requests remain P0 work.

Eight cost pairs retain exact full science/solver fields and native/base/scratch
peaks. PW adds 819 ms/21.6%, then reversed repeats change -70 and +1961 ms/44.5%.
The excess lies mainly in unchanged initial CG; its cause is not established.
Four prescribed follow-ups keep A/A separate: A/A +132/-9 ms; A/B +48 ms/1.35%
and +9 ms/0.25%. All flags remain disclosed. This is a correctness adoption with
timing uncertainty, not a speed or universal memory claim.

The [expanded saved-Schur screen](benchmarks/reports/development/saved-schur-controls-local-2026-09-13/README.md)
preserves the still-unadopted five-file prototype on `834703b`. Nine original
primary pairs pass: unit/AW/PW command medians 2.896→0.694 / 82.925→65.283 /
3.499→2.582 seconds, paired gains 76.1%/22.3%/25.8%, including rejected probes
and AW/PW continuation costs. Median native peaks fall 54.9%/31.2%/30.9%.
Fifteen core/five plugin checks, 48 independent saved fits and initial/recovery
exact/one-byte-short admission checks pass; full overlap audit and limitations
are archived. Five protected pairs have exact science and at most +5 ms.

Weak, disconnected and unbalanced saved comparisons fail baseline parity.
Grounded direct dummy/SVD and separate LSQR/LSMR oracles agree within 9e-13;
candidate residual/FE outputs all pass the original thresholds. The baseline
misses residual tolerance on 1/1/155 rows and first-FE tolerance on 216 unbalanced
rows. These three adjudicated comparisons remain failed parity evidence, outside
passing timing aggregates. The subsequent slope warm-up exposes the publication
bug repaired above; no failed input or seed was replaced.

Next rebase the saved-Schur/recovery prototype on this omission repair and finish
the original adoption comparison. The old screen's three-FE saved, deficient-rank
saved and explicit-CG saved pairs remain unexecuted; the fixed original slope
pair also needs comparison. Retain all baseline-error adjudications and timing
flags. P6 adoption/P0 whole-action reporting and other #27 items remain open.
Installed dist, 97 unrelated STATUS additions and all 358 original untracked paths
remain preserved. No agents, SCC, qualification, dependency update or push.

## P0 prerequisite: prepared direct-CG admission repaired

The [source-bound report](benchmarks/reports/development/prepared-cg-admission-local-2026-09-13/README.md)
adopts [ADR 0060](docs/adr/0060-prepared-direct-cg-memory-admission.md). Simple and
categorical direct-CG adapters now enforce the existing joint execution/CG
budget, including retained coefficient recurrence and the installed team. The
isolated candidate `fdc3e141` changes two files against accepted `dadd956` at
`de6988b`; it excludes the unadopted saved-Schur/recovery prototype. Arithmetic,
reference scales, weights/sample/rank/VCE/DoF and certificates remain unchanged.

The closed-form missing-admission test fails before the fix and passes afterward
across 36 dispatch/weight/topology/worker combinations. All 65 isolated prepared-
fit tests, 48 saved-OLS dummy fits and one/seven-worker weighted direct-CG oracle
checks pass. Public one-byte-short admission returns r(909) with prior results,
sample/order and requested-output absence preserved. Exact admission succeeds
at 296259 native bytes within 311496 usable. Both architecture builds and native
arm64 ABI checks pass. Eight fixed paired commands preserve exact scientific
exports and solver decisions; largest increase is PW +18 ms/0.5%. Explicit-CG
pairs are +12 ms/0.4% and +4 ms/1.9%. None triggers repeats. Native peaks are
exact except low FW -4608 bytes; public base/scratch models remain unchanged.
RSS moves both ways. This is a correctness repair, not a speed or memory gain.

A separate three-request diagnostic on the unadopted saved prototype records
actual initial joint checks: unit 827.75 MB and PW 831.18 MB, versus observed
378.71/581.41 MB and public bases 418.80/424.69 MB. PW rechecks CG continuation;
low FW checks CMG construction/retention. Recovery planning takes 3.334/3.250
microseconds on the full saved requests, so its direct scalar cost cannot
explain earlier 36–43 ms increases. Their cause remains unresolved; no flagged
observation is discarded. Full recovery remains 0.385/0.622 s in these separate
diagnostics, which are excluded from ordinary timing comparisons.

Next rebase the saved-Schur/recovery prototype on this repair, complete its
initial/final overlap admission review, and compare complete saved-output cost
against the repaired accepted baseline. P0 reporting/other-route accounting and
P6 adoption remain open; the five prior prototype files remain uncommitted.
Installed dist, 97 unrelated STATUS additions, original untracked paths and
separate Veneto work remain preserved. No agents, SCC, qualification or push.

## P0/P6 recovery admission tested; initial-peak diagnosis corrected

The [source-bound recovery report](benchmarks/reports/development/saved-recovery-admission-local-2026-09-13/README.md)
records an unadopted five-file extension to the saved-Schur prototype. Plugin
`247f889a` binds 176 inputs; both architecture builds/native arm64 ABI, 13 core
recovery tests, five plugin saved tests and all 48 independent saved-OLS oracle
fits pass. Each intercept-recovery route has a checked incremental reserve.
An over-budget CG/LSMR attempt can still use the existing admitted MAP fallback;
valid retained coefficients need no unused Schwarz reserve. PPML/slopes retain
their existing policies. One/four-worker exact-boundary and failed-product tests
pass. The public cap fixture rejects one byte below admission with r(909),
preserves prior estimates/sample/output identities, and succeeds at the exact
cap: 362557 native bytes versus 381696 usable. This is scoped boundary evidence.

The earlier score-cache-overlap diagnosis is corrected: ordinary robust/clustered
inference consumes and drops its score design before recovery. A separate exact
original-PW allocator diagnostic reaches its 581.41 MB whole peak beforehand;
recovery peaks at 140.70 MB and adds 13.78 MB above entry. Recovery takes 0.697 s,
with exact full science and saved values. This diagnostic is not pooled into
command comparisons. The public 424.69 MB base omits the initial solver reserve;
remaining initial-peak reporting and complete overlap accounting are P0 work.

Twelve incremental cost pairs include six controls/screens, two triggered seeds
and four bounded A/A/reversed-order follow-ups. All preserve exact science,
solver fields, native peaks, models and scratch. The original accepting unit
case slows +36 ms/5.3% and reversed +43 ms/6.1%. One rejecting seed adds 333 ms,
mostly in earlier CG, but its reversed pair is -7 ms. A/A moves +23/-11 ms.
The accepting-case cost signal remains unresolved; no admission speedup or
combined-prototype adoption is claimed. PW/low observed/model gaps remain.

The five changed source files remain uncommitted; accepted numerical baseline
is `dadd956`. Next resolve initial-solver peak reporting/admission overlap and
localize the accepting-unit cost before primary saved-Schur adoption comparisons.
Historical receipts, installed dist, unrelated Veneto work and original untracked
paths remain preserved. #27 is unfinished; no agents, SCC, qualification or push.

## P6 original PW screen resumed; recovery admission remains open

The [repaired-baseline screen](benchmarks/reports/development/saved-schur-probe-local-2026-09-13/repaired-baseline/README.md)
reapplies the archived two-file saved-Schur prototype against accepted `dadd956`.
Plugin `18427c19` binds 176 inputs, passes both architecture builds/native ABI,
the focused guard test and all 48 independent saved-output oracle fits. The
identical baseline plugin's 48-fit oracle receipt is retained from the CG repair.

The exact original PW missing/two-way-cluster/partial-saved pair now completes:
3.498→2.518 seconds (−28.0%), native peak 841.84→581.41 MB (−30.9%), input near
0.081 s in both. Full b/V/residuals are exact, saved FE maximum difference is
6.05e-10 within unchanged tolerances, and samples/status/DoF/certificates pass.
The probe accepts at 0.00794698, but its full Schur attempt reaches the 64-step
cap and continues with CG: candidate 138 = 8 probe + 64 Schur + 66 CG iterations,
versus baseline 66. Both fallback flags remain set. Reported command/peak includes
that discarded attempt and final recovery; this is one pair, not adoption.

Both peaks exceed the unchanged 424.69 MB model. This screen's proposed
score-cache-overlap diagnosis is corrected by the newer source/allocator audit
above: the ordinary inference cache is already dropped and the whole peak
precedes recovery. Recovery admission and constrained rollback now have focused
evidence, while initial-peak reporting and an incremental cost signal remain
open. The combined prototype remains unadopted; accepted numerical baseline
remains `dadd956`, installed dist and unrelated Veneto changes are preserved.

## P6 prerequisite: negative CG energy independently audited

The [source-bound repair](benchmarks/reports/development/cg-energy-audit-local-2026-09-13/README.md)
is adopted under [ADR 0059](docs/adr/0059-borrowed-cg-negative-energy-audit.md),
against accepted numerical `257f4ce` at evidence HEAD `d4d7e97`. Candidate plugin
`e831a657` binds 176 inputs; only borrowed symmetric CG changes. A previously
fatal small negative Hestenes subtraction now requires an independent scaled,
compensated check of current and pending weighted energy. Larger/inconsistent
steps still fail; original recurrence stopping, reference scales, FE and strict
reconstruction certificates remain. The calculation adds no heap workspace.

The exact original PW missing/two-way-cluster/partial-saved worker now succeeds
in 3.510 seconds. Separate tracing confirms RHS 51's two-ulp overshoot and zero
independent remaining energy. This is a correctness repair; the failed baseline
is not a timing denominator. The final 45-row closed-form fixture fails on the
isolated accepted core and passes with the repair. All 57 focused Rust tests,
48 saved-OLS dummy fits per artifact and both independent Poisson fixture arms
pass. Both architecture builds and native arm64 ABI checks pass. Initial fixture
expectation/overflow failures, a missing-oracle setup error and collector-field
corrections remain disclosed; no existing test/public tolerance was loosened.

All twelve protected pairs preserve exact full science, sample/status/DoF,
solver decisions, native peaks, models and scratch. Largest absolute increase
is saved unit 2.890→2.938 seconds (+1.7%); dense increases 5 ms/14.7%. No increase
crosses max(10 ms,5%); no extra seeds were triggered. The saved-unit fallback
flag and 59 iterations remain exact. RSS moves both ways. Existing low/saved
observed/model gaps remain; repaired PW reaches 841.84 versus 424.69 MB. These
are unresolved P0/P6 gaps, not memory qualification.

Next reapply the archived saved-Schur prototype against this repaired baseline
and resume the original PW screen, then prove overlapping final-recovery/
fallback admission and constrained budgets before adoption and primary repeats.
The prototype remains unadopted. Other #27 items remain open; finish those before
the two-agent investigation, upstream CMG review and authorized later small SCC
campaign. Installed dist, unrelated Veneto work and historical receipts remain.
No agents, SCC, qualification or push occurred.

## P6 saved two-FE Schur: archived prototype and baseline failure

The [source-bound probe](benchmarks/reports/development/saved-schur-probe-local-2026-09-13/README.md)
tests ordinary OLS permission to omit its initial coefficient batch only after
the existing weighted Schur probe accepts. Rejected probes retain coefficients;
other weighted-linear callers keep their default requirement. Baseline numerical
source is `257f4ce` at evidence HEAD `75ad177`. Prototype plugin `002a3089` binds
176 inputs, passes both architecture builds/native arm64 ABI, the focused core
guard test and 48 independent saved-output dummy-oracle fits per artifact.
Eligible weighted two-FE oracle fits demonstrate actual new Schur dispatch.

The original unit saved-both-channel screen falls 2.897→0.650 seconds, with
native peak 839.05→378.71 MB. Input time stays about 0.075 s; native falls
2.784→0.538 s and initial absorption supplies the savings. Full b/V and saved
values pass original tolerances; sample/status/DoF remain exact. This is one
pair, not adoption or a cumulative speed claim. Its baseline exceeds the public
418.80 MB model; its candidate fits, but complete recovery admission is unproven.

The original degree-two AW operator rejects Schur: contraction 0.0106146 exceeds
0.01. Both commands spend about 84 seconds, with exact b/V/fit/certificate/saved
exports and unchanged 3363.96 MB native peak, above the 1675.12 MB public model.
The candidate adds the charged eight-step probe. The harness initially assumed
every nominal strong graph must select Schur; that assertion failure and its
correction remain preserved. No original AW pair was replaced or rerun.

The original PW missing/two-way-cluster/partial-saved baseline fails r(459)
before the candidate runs. Exact noisy-worker replay identifies a CG Hestenes
negative-energy failure during absorption. Two direct replays pass; the cause
of that worker/direct difference remains unestablished. A separate accepted-
baseline scalar diagnostic (plugin `200370b2`) reproduces the error on RHS 51,
the explicit constant: previous 139583.428571428522, decrease
139583.428571428580, remaining −5.820766e−11 versus floor 3.099375e−11. This is
a two-ulp overshoot. The diagnostic changes no arithmetic or error policy and
passes both architecture builds/native ABI. All failures and setup mistakes
remain in the report; no failed command is counted as a pass.

The P6 prototype is archived and removed. Its prerequisite energy repair is
now adopted in ADR 0059 above. Resume the exact original PW screen against that
repaired baseline, resolve overlapping final-recovery/fallback memory admission
and constrained budgets, and complete the prescribed protected/repeated
comparisons before adopting the saved-Schur optimization.
P6 and the remaining #27 work stay open; no target/seed substitution, agents,
SCC, qualification or push. Installed dist and unrelated Veneto work remain.

## P6 selected-output mask: evaluated and rejected

The [isolated report](benchmarks/reports/development/selected-output-mask-local-2026-09-13/README.md)
rejects the standalone three-file recovery mask change against accepted numerical
`257f4ce` (evidence HEAD `25b1bb3`). Plugin `79ff38b2` binds 176 inputs, passes
both architecture builds/arm64 ABI, all 17 topology/recovery tests and 48
independent explicit-dummy saved-output fits per artifact. Unit/AW/FW/PW,
one/two FE, fast/low, partial/combined outputs and rollback pass.

The three prescribed screens are 0.207→0.207 / 0.329→0.315 / 0.409→0.412 seconds
for unit saved, AW low saved and PW two-FE clustered partial saved. None meets
the fixed command/native-peak threshold. Full b/V, sample/status/DoF/FE/cluster
and saved values are exact. Unit/AW native peaks are unchanged and admitted.
PW falls 398.11→394.59 MB for an untraced reason, but both exceed the unchanged
107.15 MB model; this is a P0 gap, not memory acceptance. PW sampled RSS rises.
The separate AW profile is 0.310→0.313 seconds with unchanged native peak and
no isolated recovery-allocation timer. No additional seeds or protected
performance campaign ran after rejection. The full patch and receipts are
archived and all 176 accepted build inputs restored; no production source
changes remain from either P5 or this mask experiment.

P6 saved outputs remains open. The next substantive comparison is fast two-FE
saved output without retaining all initial projection coefficients, using the
existing accepted-final-target recovery. Include final recovery/fallback cost
and resolve its overlapping memory accounting; one-FE Auto already omits that
initial coefficient batch. The remaining P0/P3/P4/P6/P7 and R1–R5 work precedes
the authorized agents, upstream CMG review and later small SCC campaign.
Installed dist and unrelated Veneto work remain unchanged. No push.

## P5 centered tiles: evaluated and rejected for native adoption

The [final QR/publication report](benchmarks/reports/development/centered-tiles-probe-local-2026-09-13/qr-public/README.md)
concludes the bounded centered one-FE evaluation against accepted numerical
`257f4ce` (evidence HEAD `436fe2a`). Stable current-frame means, streamed finite
moments, existing rank-revealing scratch QR, original-row residuals and centered
robust/clustered scores were implemented and tested in the archived prototype.
They are not adopted. The conditional PPML extension is declined because there
is no acceptable OLS winner; R3 may revisit a materially different mechanism.

The earlier actual-dispatch screens saved 40–43% native peak. Bounded parallel
Gram/FE certificates reduced initial overhead, but full-rank IID still missed
the prescribed memory/time tradeoff (0.573→0.631 seconds, +10.1%). Those stages
and their failures remain immutable in the same report tree.

The near-capacity QR repair was then built as plugin `c8817a0b`. Its first public
unit-IID candidate returned r(498): precommit required fast mode to report zero
scratch. A separate diagnostic confirmed that cause and previous e(b) rollback.
An experimental origin flag and exact final-axis/file-bound check repaired this
reporting boundary without changing ordinary fast/low checks. Final plugin
`840092dd` binds 182 inputs and passes both architecture builds/arm64 ABI.
Thirty-five centered core and 46 plugin tests pass (81 distinct). An unavailable
Cargo launcher and two report-comparison fixture failures remain disclosed.

All five prospective public QR controls now pass science but violate the command
cost envelope. Near unit/AW/FW/PW commands are 0.034→0.051 / 0.047→0.063 /
0.046→0.062 / 0.046→0.065 seconds (+34–50%). Wide deficient AW is 1.366→1.898
(+38.9%, +532 ms). Native peaks fall 30–41% and fit unchanged models; AW/wide
sampled RSS rises. Two separate profiles confirm excess fit cost; wide score
construction savings do not offset it. Full b/V and independent certificates
pass, with exact sample/rank/status/DoF/FE/cluster structure. Scratch capacities
are 2.23/40.11 MB; actual I/O traffic is unmeasured. Full overlapping admission
proof remains incomplete. Cost rejection stops adoption before broader controls,
repeat primary seeds or a PPML extension; no failing result is discarded.

The rejected route, report extension and prototype-only modules were removed
from main after the complete patch/source/artifact/numeric evidence was archived.
All 176 accepted baseline build inputs matched before isolating the independent
P6 selected-output candidate in three files: `fe/topology_slope_map.rs`,
`fe/mod.rs` and `ols/prepared_categorical_data.rs`. That unadopted candidate only
forwards the requested combined-output mask to the existing recovery finisher;
its earlier independent evidence remains in the scope/saved-output stage.
Next test and measure it separately, then continue remaining #27 P0/P3/P4/P6/P7
and R1–R5 items. P6 saved outputs remains open. Complete #27 before the two-agent
investigation, upstream CMG assessment and later small SCC campaign.

Installed dist, the 97 unrelated STATUS additions and all 358 original untracked
paths remain preserved. No agents, SCC, qualification or push occurred.

## P3 cluster-state reuse: evaluated; certificate prototype rejected

The [source-bound report](benchmarks/reports/development/cluster-certificate-local-2026-09-13/README.md)
rejects representative-tuple certification against accepted `257f4ce`. Dense OLS,
sparse OLS and sparse PPML first-screen commands fall 0.318→0.308 / 0.172→0.170 /
0.363→0.358 seconds: none meets the prospective joint 5%/10 ms threshold. No
additional primary seeds ran. All nine pairs (three screens, four controls and
two separate public profiles) preserve exact scientific exports and solver
fields. Largest protected slowdown is low AW saved-output OLS +3 ms/4.1%.
The sparse OLS profile saves 6.4 ms in preparation; that does not overturn the
command-screen rejection. All three primary native peaks are unchanged/admitted,
while sampled RSS rises. Two control peaks differ for untraced reasons.

The small-slope and weak OLS controls exceed their unchanged models on both
artifacts: 1,388,568 versus 1,382,472 bytes, and 27.49/27.91 versus 13.64 MB.
These are recorded P0 gaps, not memory acceptance passes. Three focused unit
tests, both independent OLS/Poisson interaction oracle arms, universal static
checks and native arm64 ABI checks pass. Initial compilation and collection/checker
failures remain disclosed. All 179 accepted inputs are restored, installed dist
is unchanged, and unrelated Veneto work is preserved.

The [consumer audit](benchmarks/reports/development/cluster-certificate-local-2026-09-13/consumer-audit.md)
completes the bounded cluster-state reuse evaluation: final codes/subset orders
already serve their production and certificate consumers; the identified extra
categorical factorization alternative is rejected. Low-memory QR overwrites
within scratch, so direct inference handoff would be incorrect. That broader
P3 item remains open. Next prototype stable one-FE centered tiles through the
existing moment/rank boundaries, preserving reference scales and QR fallback;
P0 and other P4–P7/R1–R5 work remain. No agents, SCC, qualification or push.

## P3/P4 exact wide covariance pivots: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/ppml-pivot-cache-local-2026-09-13/README.md)
adopts exact upper-row maximum reuse in PPML Jacobi eigensystems above 64
identified dimensions against `d9aa8c0`. The existing eigenvalue vector holds
temporary maxima within its modeled lifetime; only affected rows are rescanned.
Pivot/tie order, rotations, stopping, clipping/rank policy and independent
certificates remain unchanged. There is no new heap allocation, retained state,
worker team or SPI call. [ADR 0058](docs/adr/0058-exact-ppml-jacobi-pivot-cache.md)
records the boundary.

All 19 planned pairs pass: nine primary pairs over three seeds, nine protected
controls and one separate private audit. Unit K65/FW K129/PW K257 command medians
fall 1.637→1.597 / 3.148→2.264 / 14.784→4.072 seconds; median paired gains are
2.4%/28.0%/72.3%. Adoption rests on the repeated K129/K257 gains. Their inference
medians fall 0.810→0.230 / 8.609→0.827 and publication 0.347→0.055 / 3.239→0.320
seconds. Baseline instrumentation attributes most eigensystem time to exhaustive
pivot search; separate standalone whole-inspection timings are not pooled with
command evidence.

Every scientific CSV is byte-identical, with exact sample/rank/status/DoF,
cluster/separation/PSD reports and discrete solver fields. Native peaks are exact
in 18/19 pairs, including all primaries, and every peak is admitted. Three-FE
peak rises 24,715 bytes to 38.510 MB against a 58.377 MB model; its allocation
source is untraced. All modeled phases are exact. Sampled RSS moves both ways;
no memory saving is claimed. Largest protected slowdown is small K2 +2 ms/7.4%,
within the review envelope. The private audit falls 19.344→8.623 seconds while
retaining zero events/live/peak growth over 16 iterations, stable capacities and
fresh/warm equivalence. Retry/fallback counts are zero throughout.

Forty-nine distinct core tests, 22 PPML plugin tests, both independent dummy
Poisson/Hessian/sandwich fixture arms and the original wide profiled-Poisson/
full-CGM oracle pass. The latter retains b/V errors 3.97e-15/2.87e-16. Isolated
universal/static and actual arm64 ABI checks pass. A read-only identity-key
mistake and initial collector's interleaved-test-output undercount are disclosed;
neither required a fit/test rerun. Candidate plugin is
`c64ead0763d662a8f7bd1c74f1058426ea03f90c871ff387e76a6fe935b2e3da`, with all
176 inputs verified. Installed dist, 97 unrelated STATUS additions and 358
original untracked paths remain intact. No agents, SCC, qualification or push.

This closes the bounded covariance experiment, not the remaining P3 cluster-state
reuse/duplicate residualization or broader P4/P5/P6/P7 and R1–R5 work. Next audit
final-sample cluster ordering consumers and low-memory score reconstruction, then
the remaining construction/state and centered-tile work. Complete #27 before the
authorized two-agent investigation, upstream CMG review and later small SCC run.

## P4/P6/P7 wider retained moments: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/ppml-wide-moments-local-2026-09-13/README.md)
adopts fixed two-tile standardization above 64 active columns against accepted
numerical `9ec47b8` (evidence-only `374d257` does not change that source). Each
existing Gram-row partition uses at most 64 weighted left and 64 standardized
right values: 1024 stack payload bytes, no heap, retained state or N-by-K copy.
Every output keeps its exact observation/FMA order, selected mapping, weights
and reference scales. The up-to-64 cache, scratch/unscaled/single-column paths,
one-shot OLS, QR, rank and strict current-frame policies remain unchanged.
[ADR 0057](docs/adr/0057-wider-retained-dense-moment-tiles.md) records the boundary.

The complete original protocol now passes on repaired artifacts: 17 new pairs,
including original PW K257 warm-up/full requests, nine primary pairs, seven
controls and a separate private audit. Earlier incomplete probe pairs are not
pooled. Unit K65/FW K129/PW K257 command medians are 2.308→1.678 /
4.479→3.213 / 16.106→15.171 seconds; median paired gains are 29.2%/28.0%/5.4%.
Finite-solve medians fall 0.730→0.310 / 1.247→0.405 / 1.282→0.756 seconds,
with lower strict/final work. Every primary pair improves. K257 inference and
publication dominate its remaining time; their costs are retained in totals.

Every scientific CSV is byte-identical; samples, rank/status/DoF, cluster and
separation tables, solver choices and certificates agree. Native peaks/models
are exact in all 17 pairs and admitted; primary deltas remain 364.49/287.97/
253.09 MB. PW sampled RSS rises 0.334→0.381 GiB; no memory saving is claimed.
Largest absolute control increase is unchanged complete K50 +89 ms/1.2%; low
FW adds 30 ms/3.3%. No control crosses the review envelope. The private audit
falls 21.923→19.398 seconds and retains zero events/live/peak growth over 16
iterations, stable capacities and fresh/warm equivalence. Retry/fallback counts
are zero throughout. Public and private timing aggregates remain separate.

Sixty-two distinct Rust checks and both independent dummy-Poisson/Hessian/
sandwich oracle arms pass. One matching integration test was initially outside
`--lib` filters and explicitly completed after timing; no failing numerical test
or fit was rerun. Isolated universal/static and actual arm64 ABI checks pass.
Candidate plugin is `d1e8efc290851ace10f917ac3cb630415bfaa975a2da588dead4a85b87fa2abc`,
with all 176 source/ado/fixture inputs verified. Installed dist and the 97
unrelated STATUS additions/358 original untracked paths are preserved. No agents,
cluster, qualification or push occurred.

This closes the bounded wider-moment evaluation. P6's width exercise combines
these 63/64/65/128/129/257 finite-column tests with the existing weighted-Schur
15/16/64/65 actual-RHS controls; rank policy is unchanged. Broader P4/P6/P7,
P5 and R1–R5 remain open. Next inspect the now-dominant K257 covariance/
eigensystem work, then remaining construction/predictor/state and centered-tile
opportunities. Complete #27 before the authorized two-agent investigation,
upstream CMG review and later small SCC campaign.

## P4/P6 published covariance inspection: adopted local repair

The [source-bound report](benchmarks/reports/development/ppml-psd-validation-local-2026-09-13/README.md)
repairs final PPML PSD-report validation against accepted `fd6b970`. The validator
inspects the published covariance's own eigensystem, retaining the existing
bound on further repair and all report/rank/tolerance checks. It no longer
substitutes diagnostics from a newly re-clipped comparison matrix. Production
CGM, inference and all numerical session/moment code remain unchanged;
[ADR 0056](docs/adr/0056-published-ppml-covariance-validation.md) records the boundary.

The original PW K257/N7680 warm-up fails r(430) on baseline and passes candidate.
A closed-form 65-dimensional Householder spectrum independently reproduces the
invalid rejection; forged reports and indefinite matrices still fail. A NumPy
profiled-Poisson/full-CGM oracle matches the original sample, identified and
reported ranks, nested FE and cluster structure, with maximum b/V errors
3.97e-15 / 2.87e-16. The N30720 full request has exact scientific exports in both
arms. Its common +576-byte modeled-phase offset is recorded separately as a
pre-validator dynamic embedding input discrepancy; its allocation source was
not traced. Native peak is unchanged/admitted. The initial oracle's incorrect
identified-versus-reported rank assertion is preserved and corrected to the
existing normalized-constant contract, not a new rank policy.

Forty-seven core tests, 33 PPML plugin tests and three independent licensed
fixtures on both arms pass, with explicit r(0)/sentinels. Isolated universal
build/static audits and actual arm64 ABI checks pass. Eleven paired cost
comparisons preserve exact b/V, discrete science, native peaks and modeled
budgets, all admitted. FW K129/exposure/missing saves 337/340/345 ms across
three seeds: median command 4.740→4.400 seconds (7.2% paired), publication
0.68449→0.34734. Inference remains unchanged. The largest absolute control
increase is complete K50 +230 ms/3.1%, in unchanged finite/strict work;
saved-output +11.5% is +3 ms. No control crosses the review envelope. K129 and
AW OLS sampled RSS rise; no memory saving is claimed. All PPML retry/fallback
counts stay zero. Cold functional calls are separate from timing evidence.

Candidate plugin is `411ed113101399adda2c893d0b3e51fa16cbc96e230871ec568e8b346a689126`;
176 source/ado/fixture inputs are bound by the report. An excluded initial
wrong-Cargo-directory diagnostic build, baseline failures and both harness
corrections remain disclosed. Installed/dist and unrelated Veneto evidence are
preserved. No agents, cluster, qualification or push occurred.

A [supplemental control correction](benchmarks/reports/development/ppml-psd-validation-local-2026-09-13/control-correction/comparisons.json)
clarifies that `weak_d3` means degree three with two FEs, not three FEs. The
original pair and all 11-pair archives remain unchanged. An added explicit
low-mobility three-FE pair passes 0.083→0.082 seconds with exact science and
admitted peak (18.453→18.452 MB, not exactly equal). A first missing-g3 warm-up
setup failure and an overstrict collection peak-equality assertion are preserved.
There are now twelve successful cost pairs, with the extra control separate.

Next resume the original 17-pair wider-moment protocol against this repair,
including the unchanged PW K257 warm-up and full request. The wider prototype
remains absent/unadopted; broader #27 P4/P6/P7 and other open items remain open.
Complete #27 before the authorized two-agent investigation, upstream CMG review
and later small SCC campaign.

## P4/P6 bounded request transport: adopted local repair

The [source-bound report](benchmarks/reports/development/request-transport-local-2026-09-13/README.md)
adopts [ADR 0055](docs/adr/0055-bounded-local-request-transport.md) against accepted
`22b5106`. Above 4,000 metadata bytes, OLS/PPML retain the unchanged framed
request in one fixed caller-local macro and pass a bounded byte descriptor to
the same native call. Exact length/NUL/UTF-8/framing and existing handshakes
precede observation reads. A 16 MiB metadata cap, extra truncation-detection
byte and separate SPI terminator capacity keep the read bounded. The buffer
is released after parsing; all numerical core inputs remain unchanged.

The final isolated plugin `7b7c6664...` passes 94 plugin tests, 26 static tests,
universal static/native arm64 ABI checks and the ordinary independent OLS/PPML
oracles. A new long-name fixture passes OLS AW robust and PPML FW robust/PW
clustered independent calculations, malformed/over-cap reads and rollback.
The old binary's r(1003) is expected transport-failure evidence, not a science
pass. Final review found and fixed the initial candidate's SPI terminator bound
and truncating-reader test model; its failing reproducer and eight initial
pairs remain archived separately. A fixture registration error and Python
launcher failures are preserved; corrected runs pass without weaker checks.

Eight final pairs preserve exact b/V and discrete science/solver fields. The
largest slowdown is complete PPML 7.354→7.500 s (+2.0%); none crosses the
prospective max(10 ms, 5%) review envelope. Native peaks are exact on seven
pairs and 4,302 bytes lower on FW K129; all are admitted. Sampled FW/weak RSS
rises, so no general memory saving or command-speed gain is claimed.

The exact original PW K257 full fit now succeeds at N30720/seed104729. Its
N7680/seed104730 warm-up reaches native code but fails the existing covariance
PSD-report consistency check with r(430), on both transport candidates. This
is a newly exposed separate numerical validation issue; it is not silently
accepted, bypassed or replaced with a smaller passing target. Next localize
and repair that check independently, then resume the original wider-moment
protocol. The wider optimization remains absent/unadopted and broader #27
items remain open. Installed dist, all 97 unrelated Veneto additions and prior
untracked evidence remain intact; no agents, cluster, push or qualification.

## P4/P6 wider moment probe: request transport needs repair first

The [source-bound probe](benchmarks/reports/development/ppml-wide-moments-probe-local-2026-09-13/README.md)
archives an unadopted extension against accepted `22b5106`. Two fixed 64-value
tiles preserve exact Gram/RHS row/FMA order above 64 active columns with no new
heap or observation-sized state. Sixty-two isolated tests pass, including exact
scalar moments at 128/129/257 columns and 1/4/16 workers; both narrow independent
Poisson/sandwich oracle arms and universal static/native arm64 ABI checks pass.

First-screen unit K65 and FW K129 commands fall 2.269→1.636 and 4.726→3.458
seconds, with exact b/V and admitted unchanged native peaks. These single pairs
are promising but insufficient for adoption. The PW K257 target fails on the
unchanged baseline's warm-up with r(1003), before measurement. No repeats,
protected performance controls, private audit or candidate K257 public fit ran.

Noisy replay/trace localize the failure to framed request transport at the
plugin invocation. A separate 270-variable short call reaches native code;
1,024/4,096-byte dummy options reach native validation, while 8,192+ bytes fail
inside Stata with `options too long`. The exact universal limit is not claimed.
All failures and diagnostic log hashes are preserved, including an initial
invalid `_dup` probe. The numerical prototype is archived and reversed; all
152 accepted native hashes are restored. Installed dist and Veneto work remain.

Next repair bounded metadata/request transport independently, preserving one
substantive native call, main-thread SPI, canonical framing and publication
atomicity, then resume the full wider-tile protocol against that repaired
baseline. Do not replace the unavailable PW target with a smaller passing case.
Other #27 work remains open; agents, CMG assessment and SCC remain after #27.

## P4/P7 retained dense moments: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/ppml-moments-local-2026-09-13/README.md)
adopts [ADR 0054](docs/adr/0054-retained-dense-moment-row-standardization.md)
against accepted native `6ad8b20`. Scaled retained dense moments at up to 64
active columns standardize each observation's selected suffix once per existing
Gram-row partition. The fixed local cache preserves exact row/FMA order and
uses no heap allocation, retained partial bank or observation-sized state.
Wider, scratch, unscaled and single-column partitions retain their previous
loops. Rank, scales, certificates, generations and one-shot OLS policy remain.

Sixteen pairs / 32 commands include nine primaries across three seeds, six
controls and one separate private audit. Complete/FW/PW command medians fall
9.540→7.440 / 3.180→2.446 / 2.944→2.253 seconds, with median paired gains
22.0%/24.0%/24.4%. Every primary improves. Finite and strict solving fall while
projection/preparation stay close. All native peaks are exact and admitted;
sampled RSS moves both ways, including higher PW RSS. The largest protected
increase is K=65 +10 ms/1.4%. The private audit falls 7.817→5.429 seconds and
retains zero events/live/peak growth, stable capacities and fresh/warm equivalence.

Full b/V and discrete science/solver fields are exact. Sixty-two isolated Rust
tests cover exact scalar moments, 1/4/16 workers, 1/7/16/63/64/65 columns, extreme
scales, selected storage, nonfinite rejection, retained QR, strict rollback and
stale generations. Both independent dummy Poisson/sandwich oracle arms and
universal static/native arm64 ABI checks pass. All 152 native inputs match the
measured isolated artifact. A rejected scratch-fixture NaN, incomplete diagnostic
sampler and duplicate-VCE wide-control warm-up failure are preserved and explained.
Only the malformed wide control was rerun; its corrected pair passes.

Next evaluate bounded standardization reuse beyond 64 columns and in scratch
consumers, then remaining P4 construction/predictor/state work and other #27
packages. Broad P4/P7/P6 items remain open. Complete #27 before the authorized
two independent agents, upstream CMG assessment and small SCC campaign. Installed
dist, unrelated Veneto work and historical receipts remain intact; no agents,
SCC, push or qualification ran.

## P4 fresh raw reference norms: evaluated and rejected

The [source-bound report](benchmarks/reports/development/ppml-raw-norms-local-2026-09-13/README.md)
evaluates raw norm reuse against accepted `6ad8b20`. A frame compatibility guard
allows fresh one-FE construction to omit its duplicate raw norm scan and copy
original-input norms from the same successful projection. Raw means, reference
scaling, initial/final certificates and rank policy remain. The prototype is
archived and reversed; all 152 accepted native input hashes are restored.

Fourteen pairs / 28 commands include nine primaries over three seeds and five
controls. Complete/FW/PW median paired command changes are −0.9%/−1.8%/+1.4%:
none reaches the prospective 2% gain. Projection falls in all nine primary pairs,
by 16–129 ms, but unchanged finite/strict timing variation prevents adoption.
No extra seeds or private allocation audit were run after the repeated screen
failed. All native peaks are exact and admitted; RSS moves both ways. Largest
protected increase is weak two FE +6 ms, below the review envelope.

Full b/V and discrete scientific/solver fields are exact; independent certificates
pass. Thirty isolated Rust tests and both independent dummy Poisson/sandwich
oracle arms pass, including original-input hypot norms, changing/extreme weights,
warm-zero returns and output nonmutation. Universal static/native arm64 ABI checks
pass. No test failure or tolerance change occurred. Installed dist and unrelated
Veneto work remain intact; no agents, SCC, push or qualification ran.

Accepted native remains `6ad8b20`, including selected within-norm reuse. Next
localize the larger remaining finite/strict matrix, moment and predictor costs;
raw-reference reuse is rejected as a standalone step, with possible combined
layout work left as a hypothesis. Other #27 packages and R1–R5 remain open.
Complete #27 before the authorized agents, CMG assessment and small SCC campaign.

## P4 fresh-design within norms: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/ppml-fresh-norms-local-2026-09-13/README.md)
adopts [ADR 0053](docs/adr/0053-ppml-fresh-design-norm-reuse.md) against accepted
native `ccc1f33`. Fresh fast one-FE PPML construction selects the same successful
projection's control norms through existing generated-column metadata. It omits
per-value norm arithmetic during compaction and two K-sized scratch allocations.
Raw reference norms/means, initial certificates, final checks and rank policy
remain; multi-FE, low/generic/slope and alternative divisors keep their calculation.

Fifteen pairs / 30 commands include nine primaries over three seeds, five controls
and one separate private audit. FW/PW median paired command gains are 2.4%/5.1%,
with projection medians 0.4704→0.4444 and 0.3883→0.3437 seconds. The complete
unweighted target has no repeatable command gain (+0.02% paired), despite lower
projection cost. Unchanged finite/strict/preparation variation is preserved and
not wholly attributed to norm reuse. The largest protected increase is +1 ms;
no public pair exceeds the prospective regression envelope. The private audit
adds 65 ms/0.7%, retaining zero events/live/peak growth and stable capacities.
Native peaks are exact and admitted on all pairs; RSS moves both ways.

Full b/V and discrete scientific/solver fields are exact; independent certificates
pass. Twenty-nine isolated Rust tests and both independent dummy Poisson/sandwich
oracle arms pass, as do universal static/native arm64 ABI checks. The new test's
overstrict cross-projector diagnostic comparison fails identically on accepted
baseline; only that new comparison was corrected, preserving its fixture,
scientific envelope, global helper and runtime contracts. All 152 native inputs
match the measured isolated artifact. Installed dist and unrelated Veneto work
remain intact; no agents, SCC, push or qualification ran.

Next inspect the duplicate fresh raw-reference norm pass and remaining matrix/
moment/predictor work. Other P4 items, P0 accounting, P3/P5/P6/P7 and R1–R5 remain
open. Complete #27 before the authorized independent agents, upstream CMG review
and small SCC campaign.

## P4 fresh-input certificate removal: evaluated and rejected

The [source-bound report](benchmarks/reports/development/ppml-fresh-input-local-2026-09-13/README.md)
evaluates a standalone fresh one-FE entry against accepted `ccc1f33`. Fresh raw
responses/design blocks skip the initial certificate, project once and independently
certify against original reference norms. The first sweep consumes the unchanged
iteration budget; retained warm residuals keep iteration-zero checks. The prototype
is rejected and its exact source/patch/artifacts remain archived. All 152 restored
native hashes match accepted `ccc1f33`; no prototype remains in active source.

Sixteen pairs / 32 commands include nine primaries, five controls and two low-FW
rechecks. Complete/FW/PW command medians are 9.683→9.751 / 3.223→3.209 /
2.979→2.821 seconds, with median paired changes +0.7%/−0.8%/−4.7%.
The only command target exceeding the prospective gain threshold has a 126 ms
median reduction in unchanged finite solving versus about 4 ms in projection.
A material complete-command gain attributable to the changed work is unproven.
The low-FW control first costs 72 ms; rechecks save 96 ms and cost 29 ms.
Every observation remains, without treating changing direction as a root cause.

Full paired b/V and discrete scientific/solver decisions are exact. Native peaks
are exact and admitted, with no memory gain. Twenty-eight isolated candidate
tests and both independent full-dummy Poisson/sandwich oracles pass, including
parallel weighted group means, fresh/prior equality, warm-zero and iteration
budgets, current-frame norms, stale-rank and transaction/generation checks.
Universal static/native arm64 ABI checks pass. No private allocation audit ran
after rejection; no new allocation-free claim is made.

This completes the bounded P4 fresh-input/current-frame norm evaluation: same-call
fused norm reuse is adopted and standalone fresh-certificate removal is rejected.
Next evaluate fresh-design norm construction and remaining matrix/moment/predictor
passes, then the remaining #27 packages. The two independent agents, upstream CMG
assessment and small SCC campaign remain after #27. Installed dist, all 97 unrelated
Veneto lines and prior evidence remain intact; no push, agents, cluster or qualification.

## P4 same-call projection norms: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/ppml-norm-reuse-local-2026-09-13/README.md)
adopts [ADR 0052](docs/adr/0052-ppml-same-call-projection-norms.md) against repaired
`6b7d0fe`. Successful fused one-FE PPML projection copies its certified trailing
control norms into retained storage when frame/MAP weight divisors match.
Multi-FE handoffs and alternative divisors retain the stable current-frame scan.
No last-result accessor, norm-vector transfer or observation-sized allocation is
introduced. Raw reference scales, warm certificates and strict rank policy remain.

Fifteen pairs / 30 commands include nine primaries over three seeds, five controls
and one separate private audit. Complete/FW/PW command medians fall
10.317→9.689 / 3.380→3.231 / 3.035→2.933 seconds; median paired gains are
8.5%/4.5%/4.5%. All primary pairs improve. The large first-screen preparation
difference is preserved and not attributed to norm reuse. Projection/strict
phases fall; native peaks are exact and admitted throughout. Sampled RSS moves
both ways, including higher explicit-MAP RSS; no memory saving is claimed.
The largest protected command increase is explicit MAP +5 ms. The private audit
falls 8.661→8.027 seconds and retains zero allocation events and live/peak growth.

Full paired b/V and discrete scientific/solver fields are exact; independent
certificates pass. Twenty-three isolated candidate tests include changed-weight,
iteration-zero, capacity and failure checks; the stale-rank oracle now exercises
both matching and alternative divisors. Both independent dummy Poisson/sandwich
oracle arms and universal static/native arm64 ABI checks pass. All 152 native
input hashes match the measured isolated plugin a4d4a33a... . Initial scale-field
compilation error and every measurement are disclosed; no tolerances changed.

The subsequent fresh-certificate evaluation is rejected above. Fresh-design norm
construction, remaining P4 passes and other #27 packages remain open. The two independent agents, upstream CMG assessment and small SCC
campaign remain after #27. Installed dist, all 97 unrelated Veneto lines and
historical receipts remain intact; no push, agents, cluster or qualification.

## P4 current-weight within norms: correctness repair

The [source-bound repair and cost report](benchmarks/reports/development/ppml-current-norms-local-2026-09-13/README.md)
fixes stale within-column norms in fused PPML response/design projection against
`c6ad4c4`. The raw reference norms and projected values refreshed, but strict
rank/score checks could consume within norms from older weights. A two-row-group
closed-form oracle gives a current within/raw squared ratio about 5.34e-15 versus
2.67e-7 with the stale numerator, across the unchanged 1e-9 FE-collinearity cutoff.
The old retained strict solve accepts rank one while a fresh solve gives rank zero.
The finalized test requires the existing strict prior-basis rejection, unchanged
basis generation and no current output; explicit basis refresh permits rank zero.

The repair uses the existing stable current-frame norm scan after fused matrix
compaction, with retained scratch and no observation-sized allocation. This
covers fused one/two/three-FE categorical checkpoint consumers. Raw scales, warm
projection, finite/QR policies, generations and strict transaction rules remain.
This is a correctness repair, not a performance gain or a public-estimator
counterexample claim.

Eleven pairs / 22 commands preserve exact full b/V and discrete science; all
independent certificates pass. The first public screen costs +4.8% for complete
one FE, +5.0% for FW/exposure and +4.6% for PW/offset/clustering. Two additional
FW seeds confirm a +5.3% median paired cost across three pairs. Native peaks are
exact and admitted throughout; sampled RSS moves both ways. Protected increases
are weak +4 ms, low FW +12 ms (1.5%) and explicit MAP +2 ms. The separate private
audit costs +12.4% but retains zero allocation events and zero live/peak growth.

Twenty-one distinct tests pass from a candidate-only Cargo target, including
strict rollback, identical-frame reuse, generation invalidation and the new
reproducer. Both artifacts pass the independent full-dummy Poisson/Hessian/
sandwich oracle. A shared Cargo target reused the baseline test executable after
switching copied source trees; those test results are excluded. A forced release
recompilation produces the exact measured candidate bytes (313e85aa...), with
universal static and native arm64 ABI checks passing again. The workflow now
records source-snapshot target isolation. Other scaffold/test-expectation
failures and all timing observations are preserved in the report.

The subsequent same-call norm reuse is adopted above. Fresh-input certificate
removal, further P4 passes and the remaining #27 work are open. The two independent agents,
upstream CMG assessment and small SCC campaign remain after #27. Installed
dist, all 97 unrelated Veneto lines and prior evidence remain intact; no push,
agents, cluster work or qualification occurred.

## P2 low-memory fresh blocks: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/low-one-fe-local-2026-09-13/README.md)
adopts [ADR 0051](docs/adr/0051-low-memory-fresh-block-projection.md) against
accepted native `4e18c22`. Low-memory raw outcome/regressor blocks use the
existing fresh absorption entry. The projected outcome consumes its one-column
storage; certified within norms are reused only with matching frame/projector
weight normalization. Explicit alternative divisors retain their original norm
calculation. Raw reference scales, QR, warm residuals, retained solvers, saved
products, scratch layout and admission remain intact.

Seventeen pairs / 34 measured commands include nine primary comparisons across
three seeds, seven controls and one separate trace. Narrow AW command medians
fall 0.369→0.336 seconds (8.9% median paired gain) and native peak 95.62→87.75 MB,
exactly one 7,864,320-byte outcome column (8.2%). Unit and PW-cluster command
gains are 4.6% and 2.6%; all primary pairs improve. Candidate primary peaks fit
their unchanged models; the AW baseline exceeded its model. Existing low-FW
interaction (8.77 vs 6.70 MB) and weak (22.87 vs 9.41 MB) gaps remain P0 work.
Sampled primary RSS falls, but saved/weak controls rise; no universal RSS claim.

Full paired b/V, sample/status/DoF and solver decisions are exact. Independent
certificates and saved values pass. Thirty-two distinct Rust tests, both arms
of the explicit-dummy/frequency-expansion Stata oracle, universal static audits
and the native arm64 ABI/self-test pass. No protected command slows in the
selected single pairs. The trace locates the gain in native fitting; input,
preparation and IID covariance work remain close. The report preserves the
initial missing-PATH Rust launch and all measured observations.

This completes the remaining P2 fresh-block evaluation. Next inspect PPML
fresh-input/current-frame norm reuse and remaining repeated passes, then the
remaining #27 inference, centered-tile, output/generated-column, scheduling and
research items. The two independent agents, upstream CMG assessment and small
SCC campaign remain after #27. Installed dist, all 97 unrelated Veneto STATUS
lines and pre-existing evidence are preserved; no push or cluster work occurred.

## P2 constant-weight specialization: evaluated and rejected

The [source-bound screen](benchmarks/reports/development/constant-weights-local-2026-09-13/README.md)
compares an isolated one-file prototype against accepted `4e18c22`. It detected
constant weights during existing validation, omitted normalized weight arithmetic,
and preserved original slices/scales for solver handoffs and raw certificates.
No target met the prospectively required 5% and 10 ms command gain. Four target
pairs changed −2 ms/−0.8% (narrow AW), −13 ms/−2.1% (wide AW), +5 ms/+1.2%
(FW cluster) and +3 ms/+1.0% (weighted Schur). The wide native phase saved only
1.7 ms; most apparent command savings were outside the modified kernel.

All six pairs / twelve commands preserve exact full science and solver decisions.
Two focused tests, universal static/native arm64 ABI checks and all runtime
certificates pass. Primary native peaks are unchanged and admitted; sampled RSS
moves in both directions, and the weak control retains its old model discrepancy.
No extra timing seeds or independent full-estimation oracle campaign ran because
the prospective screen rejected adoption. The source, patch, plugin identities
and all observations are preserved. After verifying the archived 152-file source
manifest and exact patch, only the prototype was reversed; native source remains
accepted `4e18c22`. Constant specialization is evaluated-and-rejected, not claimed
impossible in other workloads or platforms.

The subsequent low-memory fresh-block adoption is recorded above; other #27 work remains.
The authorized two independent agents, upstream CMG assessment and small SCC
campaign remain after #27. Installed dist, all 97 unrelated Veneto STATUS lines
and pre-existing evidence remain intact; no push, agents, cluster jobs or
qualification occurred.

## P2 categorical one-FE initial projection: adopted local checkpoint

The [source-bound report](benchmarks/reports/development/categorical-one-fe-local-2026-09-13/README.md)
adopts [ADR 0050](docs/adr/0050-categorical-one-fe-initial-projection.md) against
accepted native `7663a87`. Fast automatic OLS with one intercept-only categorical
effect and no saved-FE retention now reuses the existing simple one-FE initial
projector and OLS norm scheduling. It omits only the redundant raw certificate;
final independent certification and original reference scales remain. Warm
residuals, explicit solvers, slopes, multi-FE, low memory and PPML keep their
existing paths. No observation-sized allocation or alternate projector is added.

Seventeen pairs / 34 measured commands include nine primary pairs, seven controls
and one separate trace. Command medians fall 0.846→0.693 seconds for unit IID,
0.310→0.274 for AW robust and 0.521→0.478 for PW two-way clustering: median
paired gains 18.0%, 11.3% and 8.6%. The trace locates savings in initial absorption
(0.06000→0.02004). Primary native peaks are effectively unchanged and admitted;
weighted sampled RSS rises 0.392→0.502 / 0.394→0.491 GiB without an established
cause. Largest protected command increase is slope +2 ms. Existing low-memory
and weak model underreports remain open; no resident-memory gain is claimed.

Full paired b/V, samples, status/DoF and solver decisions are exact. Four focused
Rust tests and both arms of the independent interaction-dummy Stata oracle pass,
including all OLS weight kinds, 1/7 workers, frequency expansion, rescaling,
collinearity, raw-scale projection and warm/retained paths. All 152 native input
hashes match the isolated measured plugin `ea63bae2...`; universal static/native
arm64 ABI checks pass. Initial scaffold/toolchain and oracle-code assertion
failures are preserved and explained in validation, without weakening science.

The expanded P0 reporting prototype was verified against its archived 152-file
manifest and exact thirteen-file patch, then removed from active native source.
It remains archived and unadopted; this is not closure of its accounting gaps.
Next evaluate constant-weight specialization and fresh low-memory one-FE blocks,
then remaining #27 packages. Only after #27: the authorized two independent
agents, upstream CMG assessment and new small SCC campaign. Installed dist, all
97 unrelated Veneto STATUS lines and pre-existing evidence remain intact.
No push, new cluster work or qualification occurred.

## P0/R1/P7 bounded LSMR batches: adopted local checkpoint

The [adoption report](benchmarks/reports/development/lsmr-batch-adoption-local-2026-09-13/README.md)
supersedes the held-prototype decision at `cd298d5` without altering its immutable
72-pair investigation. The private adapter windows independent RHSs only above
64 columns and 4× the installed team, sharing one weighted solver. Narrow batches
retain upstream transport. Shared categorical OLS/PPML memory estimates now charge
recurrence, dense construction, scatter, output overlap and Schwarz stack/pool
state; see [ADR 0049](docs/adr/0049-bounded-lsmr-batches-and-memory.md).

Repeated wide weak OLS native peak falls 403.31→351.32 MB (12.9%) with +1.5%
median paired command cost; wide weak PPML falls 182.18→169.43 MB (7.0%) with
+0.5% command cost. Adoption rests on memory savings, not speed. A prospective
ordinary-command follow-up adds 12 pairs / 24 calls: six identical-binary A/A
controls and six A/B comparisons, interleaved over three seeds in both orders.
A/B differences are −6/−8/−17/+3/−16/−23 ms, all within the original review
envelope; A/A ranges −19 to +18 ms. Earlier traced PPML increases of 21/36/38 ms
remain unexplained and preserved. No causal narrow speedup is claimed.

Full science and non-time solver/retry decisions are exact. The preceding 67
LSMR-filter and 77 memory-filter tests (134 distinct), three independent licensed
fixtures in both arms and nine source-bound allocation probes remain applicable.
The reviewed source differs only by a corrected test comment; its universal plugin
is byte-identical to the measured `c9db0dce...` candidate and static/native arm64 ABI
checks pass. The report binds all 152 final native inputs and seven changed files.
A discriminating 327680-byte admission test rejects the old over-budget LSMR
construction and selects certified CG. Three unconverged allocation probes remain
allocation evidence only. The ill-conditioned smooth 65-column oracle and earlier
low-FW/saved-AW weak certificate failures remain failures, not passing controls.

Next repair actual attempted/fallback peak reporting: public OLS selected-model
bytes still omit solver increments. Opaque factor/stack bounds and full construction
counters remain open; this is not universal memory-bound qualification. Continue
remaining #27 packages before the authorized two-agent investigations, upstream
CMG assessment and new small SCC campaign. Installed dist, the 97 unrelated Veneto
STATUS additions and all pre-existing local evidence remain intact. No SCC,
qualification or push occurred.

## P0 attempted-memory reporting: expanded prototype, not adopted

The [expanded source-bound report](benchmarks/reports/development/solver-memory-reporting-expanded-local-2026-09-13/README.md)
records the archived thirteen-file native prototype against adopted `7663a87`,
superseding the initial eight-file prototype's coverage statement without changing
that historical receipt. Its exact patch was later removed from active source
before the P2 categorical one-FE experiment above. CMG construction/rejection/error estimates now survive into
fit reports, including retained hierarchy/coefficient overlap with CG polishing.
Transient/retained borrowed LSMR records estimates without a supplied memory cap
and uses the installed team; generic intercept-only LSMR preserves failed attempts.
Fast/low inference score reconstruction now contributes its separate solver peak.
The public selected-memory scalar and observed/model ratio remain unchanged.

All 152 native inputs are bound to an isolated universal candidate plugin,
`8b6f3ffaa711581367de389c9341d47ef57d2810c76188048852b10083b5b4d5`.
Static/native arm64 ABI checks, plugin type-check, 128 distinct focused Rust
tests and six licensed oracle invocations pass. The CMG budget test rejects
one byte below its modeled retained-state-plus-polish overlap and accepts the
exact amount, preserving residuals/coefficients and final-report persistence.
The independent Stata dummy oracles cover weighted inference, CMG/LSMR,
1/7 workers, saved effects and low pools 1/5/10.

The fifteen original comparison pairs retain exact full science/non-time solver
fields. CMG low/wide command medians are 0.128→0.128 / 0.214→0.210 seconds;
weak PPML is 0.412→0.413. Wide weak OLS is 1.006→1.054 (+4.8% median paired),
with +49/+9/+51 ms individual changes and the last crossing max(10 ms, 5%).
Six prescribed follow-up pairs keep identical-binary A/A separate from A/B:
A/A differences −9/+20/+21 ms; A/B −4/+13/+52 ms. The +52 ms loss again crosses
the envelope. These are 21 pairs / 42 calls total, with no observation excluded.
A/A variation does not explain the recurring losses. Do not adopt or count this
reporting prototype as a gain; its missing coverage and timing cost remain open.

Next complete slope/signed-cover and generic CG/MAP/owned-design accounting,
saved-effect recovery attempt/overlap/admission, retained coefficient overlaps,
and construction/discarded-work counters. Localize the weak wide cost before
publishing a complete action model. Public underreports remain visible, including
54.93 MB observed versus 1.045 MB modeled on small weak OLS. Separate Veneto
changes, historical receipts and installed dist remain intact. No push, new
cluster work, broad qualification or independent-agent phase occurred.


## P0/R1 borrowed LSMR transport: adopted local checkpoint

The borrowed categorical LSMR adapter now delays its overwritten candidate
allocation until the upstream solve ends, releases RHS transport before output
materialization, and releases demeaned/unrequested coefficient banks before
independent certification. The original input and requested recovery coefficients
retain their scientific lifetimes; see
[ADR 0048](docs/adr/0048-borrowed-lsmr-transport-lifetimes.md).

The [report](benchmarks/reports/development/lsmr-transport-local-2026-09-12/README.md)
binds seventeen successful pairs / 34 measured commands against `15bd21d`.
Three-seed automatic weak-graph native peaks fall 30.37→27.31 MB at N30720/K10
and 469.17→403.31 MB at N122880/K65 (10.1%/14.0%). Command medians are
0.060→0.060 and 0.997→1.013 seconds; the wide median paired overhead is 0.9%.
This is a measured memory improvement, not a speed claim. Full successful
science and non-time solver decisions remain exact. Generic explicit fast
LSMR, strong Schur and complete IID control peaks remain unchanged. The PPML
checkpoint control adds 10 ms/3.9%, with exact science and retry diagnostics.

Sixty-two focused Rust tests and both arms of two explicit-dummy Stata oracles
pass, including retained batches, saved coefficients, raw-scale certificates
and constrained routing. Two other weak requests fail identical r(459) guards
on both artifacts: low FW cluster is uncertified; saved AW coefficient recovery
is not certified/aligned. All seven timed failed calls and the separate
untimed duplicate-solver warm-up failure are preserved. Those failures remain
unresolved and do not count as passing oracles.

The [P0 audit](benchmarks/reports/development/lsmr-transport-local-2026-09-12/memory-audit.md)
proves the OLS model's missing per-RHS/coefficient-space recurrence accounting
with six independent allocation tests and four pinned-within batch probes.
Reporting separately omits topology solver increments. Row permutation,
scatter, batch output overlap and Schwarz dense/setup/apply lifetimes also need
reconciliation; inspect the existing more detailed PPML model before sharing
it, including its unproven nested-concurrency bound. Public model excesses persist:
27.31 versus 13.39 MB and 403.31 versus 268.55 MB on these primary cases.
The small weak control exposes about 55 MB versus 1.045 MB. P0 is not closed.

Adopted native inputs match the measured universal plugin; static/native arm64
ABI checks pass. Installed dist, 97 unrelated Veneto STATUS additions and all
pre-existing untracked paths remain preserved. No SCC, qualification or push.
Next correct and validate joint memory accounting/reporting, then address the
remaining #27 work and preserved certificate failures. Finish #27 before the
authorized independent agents, upstream CMG assessment and new small SCC campaign.

## P3/P7 cluster-boundary setup: adopted local checkpoint

OLS ordered clustering and PPML paired-independent reductions now share a bounded
search for each sorted cluster's end. It preserves every chunk/boundary, empty
chunk, row/reduction order and allocation; see
[ADR 0047](docs/adr/0047-bounded-cluster-boundaries.md). It removes repeated scans
through giant clusters without splitting their numerical reductions.

The [report](benchmarks/reports/development/cluster-boundaries-local-2026-09-12/README.md)
binds twenty pairs / forty measured commands against `fdf4624`. Repeated skewed
OLS command medians fall 0.430→0.401 seconds (6.5% paired, 27–37 ms savings).
PPML inference falls 0.06969→0.03532, but total command medians only fall
2.914→2.893 (1.1% paired, variable 7–47 ms savings). Adoption rests on OLS's
repeatable complete-command gain. Primary native/scratch peaks are unchanged.

All science, samples, saved outputs and non-time solver decisions are exact.
The initial PPML two-way control slows 43 ms/12.5%, localized to unchanged solve
phases; three fresh rechecks improve 19/53/17 ms. The original result is retained,
and the source of timing variation is not established. Other controls stay within
the review envelope. Weak OLS retains a pre-existing memory-model discrepancy.
Twenty-five focused Rust tests and both strengthened 65,536-row Stata/Mata oracle
arms pass; the first OLS oracle was enlarged after review because it did not cross
a chunk boundary. Adopted native inputs match the measured plugin exactly;
universal static/native arm64 ABI audits pass. Installed dist, unrelated Veneto
work and historical evidence remain intact; no SCC, qualification or push ran.

Next address P0's observed/model memory discrepancies, then remaining P2/P3/P4
passes and P5 centered-tile work, carrying winners through P6/P7. R1–R5 remain
open. Finish #27 before the authorized independent agents, upstream CMG review
and new small SCC campaign.

## P3/P6 interaction coding: adopted local checkpoint

Categorical FE/cluster interactions now use one shared exact dense tuple mapping
when its full u32 table fits the displaced physical-row ordering bytes. Sparse
or overflowing products retain comparison sorting without partial allocation.
Original validation, lexical codes, sample/weight/singleton semantics and numerical
operations remain; see [ADR 0046](docs/adr/0046-bounded-interaction-coding.md).

The [report](benchmarks/reports/development/interaction-codes-local-2026-09-12/README.md)
binds 27 pairs / 54 measured commands against `9c2bcc5`: nine primary, fifteen
protected, one pilot and two public profiles. Repeated OLS command medians fall
0.439→0.393 seconds for interacted clusters (10.5% paired), and 0.370→0.345 for
interacted FE/clusters (6.8%, with 24–26 ms savings). Separate traces locate those
gains in preparation. PPML falls 0.383→0.370 (3.4%), but its recorded preparation
time barely changes; that smaller gain is not wholly attributed to this timer.

Full science, samples, saved values and non-time solver diagnostics are exact.
Primary native peaks are unchanged within their models; the largest protected
command increase is unchanged IID +4 ms/0.7%. Graph controls retain pre-existing
memory-accounting gaps, including weak OLS 30.6→30.9 MB against a 13.6 MB model;
this does not close P0. Forty-one distinct focused Rust tests and both independent
Stata/Mata OLS/Poisson oracle arms pass. The final comment-reviewed universal
plugin is byte-identical to the measured candidate; static/native arm64 ABI checks
pass. The first comment-layout rebuild mismatch is retained in validation.

Installed dist, unrelated Veneto changes/evidence and historical receipts remain
intact. No SCC, qualification or push occurred. Next evaluate the duplicate
cluster-boundary scans under P3/P7, then remaining score reconstruction and the
P0/P2/P4–P7/R1–R5 work. Finish #27 before the authorized two-agent investigation,
upstream CMG review and new small SCC campaign.

## P3 shared subset score passes: adopted local checkpoint

Scratch-backed OLS covariance can now feed all subset accumulators from one
original-row traversal when the entire compensated bank, range metadata and
prefix scratch fit one displaced physical-row ordering budget. Planning is
all-or-none; larger products retain the accepted per-subset route. Group/merge
order, independent per-group bread, weights, PSD, rank and publication contracts
remain unchanged; see [ADR 0045](docs/adr/0045-batched-low-memory-cluster-scores.md).

The [report](benchmarks/reports/development/cluster-subset-batch-local-2026-09-12/README.md)
binds 26 pairs / 52 measured commands against `c37686b`, including original and
revised pilots, six original primary pairs, nine controls, six fresh many-way
pairs, a separate profile and two memory rechecks. The original two-/three-way
10% target was missed (0.7%/5.9% paired gains). A separately specified follow-up
repeats five-way gains of 20.4% at N30720/K4 and 17.4% at N122880/K10 with AW,
with command medians 0.054→0.043 and 0.441→0.365 seconds. Adoption rests on those
many-way results; ordinary two-way gain remains unconvincing.

All full scientific exports, saved values and non-time solver diagnostics are
exact. Primary native/scratch peaks are unchanged. Worst protected command
slowdown is one-way +5 ms / 2.0%. Weak controls still exceed their pre-existing
memory models. A two-way native-peak increase changes direction in rechecks;
the source of variation remains unestablished, and no model-bound pass or
universal RSS reduction is claimed. Bank bytes and fewer score traversals are
source-derived counts, not measured physical I/O.

Thirteen distinct focused Rust tests and both arms of an independent full-dummy/
Mata covariance oracle pass, covering unit/AW/FW/PW, full/deficient rank, all
31 five-way masks, exact admission bounds and fallible reservations. Initial
oracle failures (DoF rank, omitted PSD repair, eigenvector orientation) and the
slower initial prototype are preserved. The reviewed universal plugin is
byte-identical to the measured revised artifact; static/native arm64 ABI checks
pass. Installed dist, unrelated Veneto changes and historical receipts remain
intact. No SCC, qualification or push occurred.

Next: remaining P3 interaction construction/order reuse, skew scheduling and
score residualization; P0 memory/counter reconciliation and P2/P4–P7/R1–R5 remain
open. This completes the bounded low-memory subset-sharing experiment, not #27.
Finish #27 before the authorized independent agents, CMG review and new small
SCC campaign.

## P3 low-memory cluster scores: adopted local checkpoint

Low-memory OLS now reads eligible cluster score subsets sequentially, using
compensated group scores only when their complete table and presence flags fit
inside the displaced N-row ordering budget. Larger/unrepresentable Cartesian
products retain tuple sorting. Original within-group row order, lexicographic
group order, per-group bread and independent covariance arithmetic remain;
see [ADR 0044](docs/adr/0044-sequential-low-memory-cluster-scores.md).

The [report](benchmarks/reports/development/cluster-grouping-local-2026-09-12/README.md)
binds thirteen pairs against `2243b4d`: six primary comparisons, six controls and
one separate pilot. Complete-command paired gains are 16.4% for low FW one-way
clustering (0.307→0.259 s) and 48.2% for low PW two-way clustering (1.870→0.964 s).
Primary native/scratch peaks are unchanged; full scientific exports, saved
outputs and non-time solver diagnostics agree exactly. Every bounded control
improves, including weak, high-cardinality fallback, small, fast, saved and no-FE
cases. Single-pair control gains are not repeatability claims.

The weak control retains a substantial pre-existing observed/reported model
discrepancy: 29933828→29827588 native bytes versus 13460424 modeled. Primary and
other control bounds hold. PW sampled RSS rises 0.169→0.192 GiB; no general RSS
saving is claimed. A sample-hash-verified cache replay explains fewer repeated
tile requests, but is explicitly source-derived rather than measured disk I/O.

Nine distinct focused Rust tests and both arms of the full-dummy Stata b/V oracle
pass, covering unit/AW/FW/PW, full/deficient rank, precise table bounds, overflow
fallback and allocation failures. Universal static/native arm64 ABI audits pass;
the reviewed rebuild is byte-identical to the measured prototype. Installed dist
and unrelated Veneto work/evidence remain intact; no SCC or qualification ran.

A separate sequential-histogram prototype is not adopted: its ordering kernel
improves, but nine command pairs show under-2% noisy gains with changes in
unchanged input work. Both experiments and all attempts are archived. Existing
simple cluster codes and covariance/certificate order sharing were already in
place. Next P3 work is combining subset consumers within bounded score storage,
interaction construction/order reuse, skew and remaining score reconstruction.
P0 model/counter reconciliation, P2/P4–P7 and R1–R5 remain open. Continue #27
before the authorized two-agent investigation, CMG review and small SCC campaign.

## P3 inference ownership: adopted local checkpoint

Ordinary fast robust/clustered OLS now packs score values only up to 32 MiB,
then uses the existing retained strided route when the finite solve permits it.
This also selects existing stable raw moments and fused residual construction.
Shared weighted-linear/PPML policy, IID, low memory, rank, memory admission and
public contracts remain unchanged; see
[ADR 0043](docs/adr/0043-ols-inference-strided-cache-policy.md).

The [report](benchmarks/reports/development/ols-inference-ownership-local-2026-09-12/README.md)
binds 29 pairs / 58 measured calls against native baseline `f4cebe7`. The final
comment-reviewed source rebuild is byte-identical to the prototype plugin.
Three-seed command medians fall 1.312→0.803 seconds for robust missing,
0.368→0.253 for PW robust and 0.539→0.409 for FW clustering; paired gains are
38.8%, 31.4% and 24.1%. Primary native peaks fall about 25–26%, and every observed
action peak fits the unchanged model. These gains include reference/residual
passes; strided score certification costs more and covariance time stays similar.

Samples/status/DoF, FE/cluster tables and solver decisions agree exactly. Full
b/V differences are at most 3.34e-16 / 7.20e-21, with unchanged independent
certificates/tolerances. Newly eligible weak, no-FE, K65 and interacted two-way
cluster cases pass. No protected control exceeds max(10 ms, 5%); complete IID
is +12 ms / 2.1%, small/narrow +1 ms. QR profiles improve 4.0% while retaining
about 0.12 seconds of score reconstruction in both arms; that work is not removed.

[Validation](benchmarks/reports/development/ols-inference-ownership-local-2026-09-12/adoption/validation.json)
records eighteen distinct focused Rust tests and an explicit-dummy full-covariance
Stata oracle on baseline/candidate across unit/AW/FW/PW, robust/clustered and full/
deficient rank, with r(0) sentinels. Universal static/native arm64 ABI checks pass.
The initial pilot diagnostic-assertion failure remains disclosed in immutable
preliminary evidence. No full profile, SCC, Windows or qualification ran;
installed dist and unrelated Veneto work remain preserved.

P3's packed/strided lifetime comparison is adopted. Next audit actual repeated
cluster-state/order construction and score consumers, then evaluate useful tiling,
skew scheduling and low-memory duplicate work. P0/P2/P4–P7, R1–R5 and the later
agent/CMG/new-SCC phases of the active goal remain open.

## P1 candidate-run and mixed-source loading: adopted local checkpoint

Against `5411b9b4d37aab1a856b12431db84ecfce1c6f22`, shared OLS/PPML input loading
batches numeric peers of strings and long retained candidate runs at N≥16,384.
Runs under 64 rows stay scalar, chunks stay bounded at 65,536, and strings keep
original encounter order. PPML offsets/exposures use the same reader. No new
observation-sized storage, excluded-cell reads, classifier, memory-admission,
solver/inference or publication change is introduced; see
[ADR 0042](docs/adr/0042-candidate-run-numeric-loading.md).

The [report](benchmarks/reports/development/candidate-input-local-2026-09-12/README.md)
binds isolated plugins and 29 completed pairs / 58 measured calls. Three-seed
command medians fall 0.615→0.445 seconds for restricted AW OLS, 0.528→0.430 for
FW OLS with string clustering, and 1.643→1.539 for restricted PW PPML with offset.
Paired gains are 27.6%, 18.9% and 6.3%. OLS input medians fall 0.390→0.217 and
0.210→0.113 seconds with stable fit/inference. PPML has no separate input timer;
its unchanged finite work also varies, so the whole gain is not attributed to
loading. Complete-data adjustment-only pairs show no reliable command gain.

All scientific exports, samples/status/DoF, certificates, iterations, retries,
fallback decisions and saved outputs match exactly. All measured action peaks
fit unchanged models; primary peak medians are unchanged. One AW pair uses 0.7%
more native peak. Worst graph-control slowdown is weak robust +8 ms / 3.7%;
complete numeric, fragmented restrictions, small/narrow, low-memory and saved
controls remain inside the predeclared review envelope. No RSS gain is claimed.

[Validation](benchmarks/reports/development/candidate-input-local-2026-09-12/validation.json)
records 43 focused plugin tests, three candidate licensed fixtures and the new
explicit-dummy/Poisson/missing-code oracle on baseline, with r(0) sentinels.
Universal static and native arm64 ABI audits pass. A PPML control input-cache
name collision stopped setup before estimation; all four controls passed with
distinct identities afterward. Initial unit expectation and collector corrections
are disclosed. Installed dist, unrelated Veneto work and historical evidence
remain untouched. No SCC, push or qualification ran.

P1 loading is adopted; adjustment-only speedup remains unsupported. Continue P3
inference ownership/lifetime comparisons and the remaining P0/P2/P4–P7 work.
The full active goal (including later independent agents, CMG assessment and the
new small SCC campaign) remains open; P1 completion does not complete #27.

## P1 PPML ado preparation: adopted local checkpoint

Against `8be388e280eb64af5de9830204a63bee8de3b33d`, remove three nested PPML
sort-preservation scopes and the optional ado completeness marker/markout/count.
Candidate and weight-expression handling, authoritative native row/domain rules,
pre-admission classification and all publication/rollback checks remain intact.
[ADR 0041](docs/adr/0041-ppml-ado-preparation.md) records the decision.

The [report](benchmarks/reports/development/ppml-ado-preparation-local-2026-09-12/README.md)
binds the same native plugin in both arms and 25 pairs / 50 measured calls.
Primary complete-command paired medians improve 2.4% (complete one FE), 1.1%
(FW/exposure/missing) and 0.4% (PW/offset/two-way cluster). Outer Stata medians
fall 0.271→0.101, 0.092→0.052 and 0.074→0.042 seconds. Full native work varies;
these are modest command gains, not native-kernel gains. All full scientific
exports and solver/retry/fallback decisions are exact. Primary native peaks
are unchanged and all action peaks fit their unchanged models.

The low-memory control initially slows 957→1085 ms. Six retained follow-ups
plus that original yield a −1.0% median paired change, with variation mostly in
unchanged native finite/strict work; no low-memory speedup or precise external
cause is claimed. Other graph controls slow by at most 6 ms. Four distinct
candidate licensed fixtures and the baseline native-preparation fixture pass
with r(0) sentinels; 17 transaction and five registration tests pass. Tests cover
independent Poisson science, all missing codes, FW/PW, string clusters, singleton
ordering, saved outputs, original row/sort metadata and injected failure exits.
The [validation](benchmarks/reports/development/ppml-ado-preparation-local-2026-09-12/validation.json)
retains the corrected initial weighted-varlist draft failure. Installed/dist and
separate Veneto work remain untouched. No SCC or qualification ran for this step.

Continue #27's remaining preparation/loading, pass, inference, centered-tile,
output and scheduling investigations; the five weak-route allocation events also
remain open. The active user goal now explicitly authorizes completing #27,
then two independent agent investigations and their evidenced optimizations,
then assessment of upstream CMG small-component changes, followed by a new small
SCC benchmark report (about ten tasks, roughly 3–5 hours each, smaller datasets
across graph families and primarily fixest/pyfixest/reghdfe comparisons). This
is later work, not a restart of either stopped historical campaign. No push is
authorized. P0–P7 and R1–R5 completion must be evidenced, not inferred from the
completed checkpoints.

## P4 stable reference passes and scratch: adopted local checkpoint

Against `d32f05e45519aaee5c5d44bc87caddb4f68482f3`, two isolated changes are
adopted: reuse the retained identity bank for frozen-solve QR scratch, then
schedule current-weight reference norms/means by column for fast PPML with one
categorical FE at N≥65,536, eight stored columns and multiple installed workers.
Each column preserves original row order, scaled norms and compensated means;
there is no observation-sized cache, stale-weight reuse or solver/rank-policy
change. See [ADR 0040](docs/adr/0040-ppml-stable-reference-pass-scheduling.md).

The [report](benchmarks/reports/development/ppml-reference-passes-local-2026-09-12/README.md)
and [identities](benchmarks/reports/development/ppml-reference-passes-local-2026-09-12/identities.json)
bind three plugins and 36 paired comparisons / 72 measured calls. Scheduling
versus the allocation-only baseline improves public primary command medians
11.800→10.179 seconds (complete one-FE robust, N983040/K50/16 threads),
3.741→3.342 (FW/exposure/missing, N245760/K50/four threads), and 3.546→3.075
(PW/offset/two-way cluster, N245760/K50/16 threads). Median paired gains are
13.7%, 10.7% and 14.8%. Existing projection and strict/final timers locate the
savings. Preparation/inference remain near baseline; primary native peaks are
identical across arms at about 2.523 GB, 633.5 MB and 645.3 MB. All
observed native action/profile peaks remain within unchanged models. RSS is
reported separately, including an FW median increase; no RSS gain is claimed.

All scientific exports, sample/status/DoF, actual/reported solver iterations,
strict retries, fallback decisions and saved outputs are exact. Weak/dense/
bottleneck/cycle/disconnected/unbalanced, small/narrow, low-memory, restricted,
saved/separated and row-threshold controls pass. Worst scheduling slowdown is
55→62 ms on disconnected (+12.7%, +7 ms), below the declared review threshold.
An allocation-only low-memory flag (878→963 ms) did not reproduce in two
independent-seed repeats; the original remains archived.

The scratch fix removes the observed one allocation per frozen solve. Real
16-iteration audits change 16→0 events for fast PW and low-memory FW one-FE
cases, with passing whole-iteration allocation certificates. Subsequent FW/PW
scheduling profiles also record zero. The weak-graph audit changes 21→5 events
and **still fails** that operational certificate, despite stable live/peak
memory and exact science. The failed broad assertion and narrowed adoption
claim are explicitly recorded; the five residual events remain open.

[Validation](benchmarks/reports/development/ppml-reference-passes-local-2026-09-12/validation.json)
records 148 distinct focused Rust tests, three licensed allocation-only fixtures,
a new N65536/K8 full-dummy Poisson/Hessian/sandwich oracle against the final
plugin, five registration checks and universal static/native arm64 ABI audits.
No full scientific profile, SCC, Windows or qualification work ran. Installed
dist and unrelated Veneto status/evidence remain preserved; nothing was pushed.

Next: localize the five weak-route allocation events, then measure remaining
raw RHS/stride and within-norm passes or P3 inference ownership. Current-frame
norm reuse, skipping fresh-input checks, broader generated-column/low-memory
coverage and P5 centered tiles remain hypotheses. Scheduling does not itself
remove a mathematical reference-statistic pass.

## P4 PPML one-FE projection: adopted local development checkpoint

Against `0889721a7c781618dd9f132642a85d065e34afad`, fast PPML with one categorical
intercept effect now enters the existing reusable categorical projector and
fused changing-response/retained-design projection. It certifies after the first
group sweep, preserves zero-iteration certification of useful warm candidates,
and allocates no accelerator matrices. Current-weight raw reference norms/means,
rank/QR policy, final-Hessian synchronization, independent certificates and
strict RAII rollback remain unchanged. No automatic family region, memory
admission or separate reconstruction behavior changes; see
[ADR 0039](docs/adr/0039-ppml-one-fe-retained-projection.md).

The [report](benchmarks/reports/development/ppml-one-fe-local-2026-09-12/README.md)
and [identities](benchmarks/reports/development/ppml-one-fe-local-2026-09-12/identities.json)
bind 23 paired comparisons / 46 measured calls: nine public primary pairs,
ten protected pairs, three separate private allocation profiles and one pilot.
Three-seed command medians fall 14.027→11.724 seconds for complete one-FE robust
PPML at N983040/K50/16 threads, 4.422→3.708 for N245760/K50/four-thread FW with
exposure/missing data, and 4.223→3.572 for N245760/K50/16-thread PW with offset and
two-way clustering. Median paired gains are 16.3%, 16.0% and 15.7%. Projection
medians fall 3.555→2.001, 1.201→0.730 and 1.203→0.681 seconds; strict/final
medians fall 5.046→4.281, 1.292→1.102 and 1.243→1.078. Preparation and inference
are largely unchanged; overlapping timers and independent medians do not sum.

Reported outer/inner counts, step halvings, strict retries and fallback decisions
are unchanged. Actual MAP iteration totals fall 5→3 and 6→4 because certified
warm solves can require zero projections; the runtime count charges at least one
per solve. All samples/statuses/DoF/weight/cluster/separation/receipt identities
are exact; b/V differ at roundoff scale (max 5.99e-17 / 1.28e-20), and independent
certificates and saved outputs pass. Unaffected weak/dense/bottleneck/cycle/
disconnected/unbalanced and low-memory routes retain exact science. Worst
protected slowdown is cycle 57→64 ms (+12.3%, +7 ms), below the predeclared
max(10 ms, 5%) review threshold; small/narrow and requested-output controls improve.

Primary native peaks rise 3.4–3.5% (84.16 MB large, 21.04 MB weighted) because
the existing group/RHS workspace now serves one FE. All observed action and
profiled phase peaks remain inside unchanged models. Real allocation audits in
both arms record 16 events in 16 iterations, without live/peak growth: retained
capacities and fresh/warm equivalence pass, but the whole-iteration
allocation-free certificate remains false. Event sizes/call sites remain open;
neither mock-probe tests nor passing science imply that operational certificate.

[Validation](benchmarks/reports/development/ppml-one-fe-local-2026-09-12/validation.json)
records 140 focused Rust tests, five licensed fixtures with r(0) sentinels and
five profile-registration checks. New independent weighted group-mean and
full-dummy Poisson/Hessian/sandwich oracles cover unit/FW/PW and robust/clustered
inference. Universal static audits and native arm64 ABI tests pass; no complete
scientific profile, SCC, Windows or qualification run occurred. The installed
plugin and unrelated Veneto status/evidence remain preserved.

The subsequent reference-pass/scratch checkpoint above supersedes this
checkpoint's allocator and scheduling resume point. Its original
[cost audit](benchmarks/reports/development/ppml-one-fe-local-2026-09-12/cost-audit.md)
remains source-bound evidence; current-frame norm reuse and the broader pass
experiments remain open.

## P1 PPML numeric ownership: adopted local development checkpoint

Against `b19a1cfac038dcc6fbf015cfbfaf9f51022dafb4`, production PPML now constructs
the admitted numeric row ledger and weight reports without a throwaway compact
copy of outcome/model sources. It consumes offset/exposure storage through stable
compaction within the candidate-capacity bound. Raw original-axis sources and raw
pweights remain available for separation rebuilds. Allocation-free preflight and
exact shape agreement, original-row errors, canonical weight/exposure semantics,
solver/session/reconstruction code and existing memory admission remain unchanged.
[ADR 0038](docs/adr/0038-ppml-numeric-selection-ownership.md) records this boundary.

The [report](benchmarks/reports/development/ppml-owned-preparation-local-2026-09-12/README.md)
and [identities](benchmarks/reports/development/ppml-owned-preparation-local-2026-09-12/identities.json)
bind isolated universal plugins and 23 completed pairs / 46 measured calls:
nine public primary pairs, ten protected pairs, three private audit/profile pairs,
and one separately reported exploratory pilot. Three-seed command medians fall
15.395→14.408 seconds for N983040/K50/16-thread complete one-FE robust PPML,
4.683→4.534 for N245760/K50/four-thread FW with exposure/missing data, and
4.591→4.452 for N245760/K50/16-thread PW with offset/two-way clustering. Median
paired gains are 6.4%, 4.5% and 3.3%. Existing preparation-phase medians fall
1.615→0.880, 0.351→0.227 and 0.358→0.221 seconds; this phase also includes IRLS
initialization. Inference and all iteration/solver/fallback decisions are exact
apart from timing. Every b/V/sample/status/DoF/weight/cluster/separation/certificate
and requested saved-output comparison passes exactly.

The removed complete-case source-copy payload is 408,944,640 bytes, derived from
source shape rather than hardware traffic. Overall native peak stays about
2.438 GB for the large complete case; weighted peaks fall about 2.21 MB because
the loaded adjustment no longer remains live beside its canonical offset. Later
session initialization dominates the preparation phase peak. Private allocation
audits pass, with all observed action peaks within unchanged models. Sampled RSS
and all phase/peak/fallback results are retained; no 409 MB fit-peak reduction is
claimed. Weak/dense/bottleneck/cycle/disconnected/unbalanced, small/narrow,
restricted, low-memory and saved-separation controls retain exact science; worst
slowdown is 1 ms. Low-memory FW improves 0.925→0.843 seconds.

[Validation](benchmarks/reports/development/ppml-owned-preparation-local-2026-09-12/validation.json)
records 136 focused Rust tests, four licensed Stata fixtures with explicit r(0)
sentinels, analytic Poisson/FW-expansion/PW-scaling oracles, and universal static
audits plus arm64 ABI/self-tests. One weighted extraction attempt was excluded
after a shared benchmark do-file edit corrupted Stata's incremental reading;
per-run worker copies/hashes now prevent that race, and both arms were rerun.
The installed/dist plugin, separate Veneto STATUS additions and historical
evidence are preserved. No SCC, push or qualification occurred.

P1 numeric ownership is adopted. The source audit identifies PPML's triple
sort-preservation wrappers and input markout/count as removal candidates, but
does not adopt ado changes. Batched adjustment/restricted-source loading,
post-admission parallel classification, generation-copy reduction and P0 counters
remain open. Next P4 work should isolate fresh one-FE RHS/norm/mean construction,
warm projection and strict/final-Hessian passes while preserving reference scales
and weight-generation validity. The large candidate's projection/finite/strict
scopes remain substantial; these scopes overlap and must not be summed. P3 OLS
inference remains a separate measured opportunity after the earlier Schur gains.

## P2 weighted OLS Schur: adopted local development checkpoint

Against baseline `39d1a2a876b92dcc7f8032be792ee171cb85cfdd`, the OLS planner now
admits supplied weights through the existing three-RHS probe and weighted Schur
engine. The probe uses actual normalized weighted degrees, statistics and
operator applications. It preserves the 0.01 contraction guard, eight charged
probe steps, 64-step full attempt, shared continuation budget, raw reference
scales and independent final certificates. The 16--64 actual RHS / 16,384-row
fast-mode envelope, saved-FE exclusion and explicit solver semantics remain.
[ADR 0037](docs/adr/0037-weighted-ols-schur-admission.md) records the development
admission. PPML session/reconstruction files and separate Veneto work are intact.

The [report](benchmarks/reports/development/weighted-schur-local-2026-09-12/README.md)
and [source/artifact identities](benchmarks/reports/development/weighted-schur-local-2026-09-12/identities.json)
bind isolated universal plugins and 39 completed pairs / 78 measured commands.
Three-seed command medians fall 6.623→1.244 seconds for N983040/K50/four-thread
AW two FE, 0.780→0.237 for N245760/K50/16-thread FW, and 1.042→0.490 for PW
two-way clustering. Median paired gains are 80.8%, 69.6% and 53.0%. Separate
traces reduce initial absorption 5.407→0.466, 0.530→0.056 and 0.598→0.065 seconds,
including probes of 45.1, 8.0 and 6.8 ms. No natural full-solve fallback occurs.

The large AW target's native peak rises 12.4%, from 1.156 to 1.300 GB within
its unchanged 1.675 GB model. FW and PW native peaks fall; all per-case native,
modeled and sampled RSS results are retained. Unchanged low and small/narrow
controls have observed/model discrepancies of +10.0% and +27.9% in both arms;
these remain P0 modeling work, not qualified memory evidence.

All numerical comparisons and original certificates pass, with maximum full
b/V differences 2.67e-15 / 3.55e-14. Unit-weight weak/dense/bottleneck/cycle/
disconnected/unbalanced controls retain exact science and solver decisions,
with at most 7 ms slowdown. Tiny weighted cuts reject Schur (0.0583 contraction)
and keep exact CMG science. Their repeats have median paired cost +3.8%; an
individual +16 ms / 6.1% case and the traced 2.7 ms rejected probe are disclosed.
The remaining traced cost lies mostly in unchanged legacy absorption; its
precise cause is unestablished. Small/narrow repeats are 14→13 ms medians after
an initial 11→15 ms observation. Saved outputs remain exact; width/low controls
preserve the existing eligibility boundaries.

[Validation](benchmarks/reports/development/weighted-schur-local-2026-09-12/validation.json)
records 60 prepared-fit and three additional Schur tests, four distinct licensed
Stata fixtures with a fifth invocation for added rank checks, binary audits and
actual arm64 ABI/self-tests. Weighted dummy, frequency-expansion, AW/PW rescaling,
finite/FE collinearity, 1/4/16-thread, memory-budget, forced-continuation and
transaction checks pass. Two builds from an incorrect working directory were
excluded before comparisons; correctly isolated artifacts were used throughout.
The installed/dist plugin is unchanged. No SCC, push or qualification occurred.

P2 weighted probe/admission, bounded continuation and width comparisons are
adopted. Simple weighted one-FE already uses direct projection; complex one-FE
pass reduction and separate constant-weight specialization remain open. Continue
with #27's P4 one-FE/pass work and remaining P1 PPML materialization, preserving
the allocation-free preflight and reconstruction repair; P3 inference remains
a measured bottleneck after the absorption gains. P0 modeling/counter gaps and
the weighted-cut routing cost remain explicit follow-up work.

## P0/P1 OLS preparation: adopted local development checkpoint

Roadmap [#27](https://github.com/johannes-schmieder/fereg/issues/27) supersedes
#26 and #16--25 for optimization priorities. The baseline for this work is
`e93fe437f1d8855e1c3aa47ad03026eb23a9942d`, including the committed PPML
reconstruction repair. Its separate evidence and the Veneto status edits below
are preserved; no cluster job or installed/dist plugin was changed.

All six OLS preparation branches now consume loaded source buffers through
one ownership-aware boundary: unweighted/AW/FW/PW; IID/robust/one- and multiway
cluster inference; simple, interacted, slope and saved absorbed terms. The
existing row classifier, original-row domain precedence, raw weight kinds and
values, cluster missingness before singleton deletion, true categorical
singleton fixed point, final coding and candidate-capacity bounds are retained.
Wide preparation reuses the requested worker team after SPI loading. PPML
preflight/materialization and all solver/inference/transaction rules remain
unchanged. [ADR 0036](docs/adr/0036-native-ols-input-preparation.md) and the
[coverage/cost report](benchmarks/reports/development/ols-owned-preparation-local-2026-09-12/README.md)
record this extension.

[Source/artifact identities](benchmarks/reports/development/ols-owned-preparation-local-2026-09-12/identities.json)
bind isolated universal baseline/candidate plugins. The
[41 paired comparisons](benchmarks/reports/development/ols-owned-preparation-local-2026-09-12/comparisons.json)
include 82 successful measured local calls. Three-seed command medians fall
from 1.870 to 1.294 seconds for unweighted robust sparse-missing N983040/K50,
1.197 to 0.641 for AW IID missing, 0.652 to 0.527 for FW clustering at N245760,
0.490 to 0.352 for complete PW robust, and 0.638 to 0.510 for an interacted FE
with two-way clustering. Separate traces reduce AW preparation from 0.598 to
0.042 seconds while fit/inference remains about 0.204 seconds.

All pairs have exact full b/V, sample, status, DoF, fit, cluster and certificate
results and solver decisions apart from timings. Native peaks agree in 35/41
pairs, including every primary pair; restricted preparation falls from 129.1
to 118.3 MB. Weak/dense/bottleneck/cycle/disconnected/unbalanced controls retain
science, with at most 7 ms slowdown. Small/narrow repeats remain 7 versus 9 ms;
this disclosed regression is within the predeclared 10 ms review envelope.
The original 1.973-second AW candidate input-loading outlier remains in the
primary data; three separate rechecks are 0.614--0.623 versus 1.172--1.194.

[Validation](benchmarks/reports/development/ols-owned-preparation-local-2026-09-12/validation.json):
160 core tests, 40 plugin tests, 29 independent Rust oracle/preparation tests,
six distinct licensed Stata fixtures plus a constrained-memory rerun, universal
binary audit and actual arm64 ABI/self-test pass. The 543870912-byte constraint
rejects fast with r(909) and admits low/auto with matching science. The Python
boundary screen remains 25/26 on both baseline and candidate: an unchanged
prepared-view dynamic-dispatch assertion fails in `ols/prepared_fit.rs`.
No test is weakened or reported as passing when it failed.

P1 OLS ownership/weights/clustering/complex terms/team reuse is adopted for local
development. P0's source-derived copy costs and representative routes are
recorded; hardware traffic and full setup/build counts remain unmeasured.
PPML post-admission ownership, adjustment loading/ado work and restricted/mixed
SPI batching remain open. Next priorities remain P2 weighted absorption and
P4 one-FE PPML passes, then measured inference ownership and stable centered
tiles under #27. No new automatic solver region, SCC, Windows, qualification,
push or release is implied.

## PPML reconstruction fix: completed development validation

The requested Veneto repair changes the retained weighted-linear session's
reconstruction check to an operation-count rounding bound and constructs
canonical PPML predictors directly from the working response and residual.
The original response-relative diagnostic and final publication tolerance
remain unchanged, as do all other scientific checks and the one-shot OLS path.
[Numerical contract](docs/ppml_reconstruction.md) records the bound and
independent rational oracle, including corrupted and nonfinite negative cases.

[Local evidence](benchmarks/reports/development/veneto-ppml-reconstruction-2026-09-12/local-tests.json) records
94 passing Rust tests. The rebuilt universal macOS plugin passes binary audit,
actual arm64 ABI/self-test, and four licensed fixtures: OLS and PPML smoke,
PPML scientific contract, and weighted inference. Strict Clippy remains blocked
by six pre-existing lints in unchanged OLS/FE files, with none reported in the
changed files.

The [repair report](benchmarks/reports/development/veneto-ppml-reconstruction-2026-09-12/README.md) records
successful full Veneto fits on all 3,159,720 observations at four/fourteen
threads: 133.324/101.878 seconds, both with 15 outer and 589 inner iterations.
Slopes differ from the preserved ppmlhdfe reference by at most 4.59e-12;
SE and full slope covariance comparisons pass. Both final score/Hessian checks
pass, with identical samples and slightly better reported objectives. Native
allocation peaks are 5.958/7.374 GiB; these single runs are diagnostic, not a
general scaling claim.

SCC job **7536374** completed with scheduler `failed=0`, `exit_status=0` and
application/job sentinels. It reused the Linux plugin built and smoke-tested
in job **7536367**, whose later controller path typo stopped before full-panel
estimation. Both attempts remain documented. The
[validation](benchmarks/reports/development/veneto-ppml-reconstruction-2026-09-12/scc-validation.json) and
[source manifest](benchmarks/reports/development/veneto-ppml-reconstruction-2026-09-12/source-manifest.json)
bind the results to snapshot
`659924a603a3b750a7478cad608d95022166b5a83956e066d9b9e29698420b96`;
all 484 source/fixture files match the final working tree. The source and
portable validation evidence form the requested development checkpoint. No
qualification or broad benchmark restart occurred.

## Incomplete OLS compaction: preceding implementation

The user-requested development checkpoint collects the adopted OLS sources,
fixtures, benchmark tooling and publishable numerical evidence. The
[checkpoint inventory](benchmarks/reports/development/adopted-ols-checkpoint-2026-09-12.json)
verifies that production and fixture bytes match the retained local validation.
It records the path-bearing captures preserved outside Git and the configurable
SCC directory settings. No estimator benchmark or qualification was rerun for
this packaging step. Historical source identities and receipts are unchanged.

Unweighted IID preparation now consumes the authoritative row classifier's
drop mask and compacts owned columns independently. Wide inputs classify
16,384-row blocks on the installed requested team, preserving original-row
error order. Buffer reuse stays within the prior candidate-axis capacity
bound. Complete inputs retain their shortcut; weighted, complex categorical,
robust/clustered and PPML preparation keep their existing paths. No solver,
sample, tolerance, inference or transaction rule changes.

The [compaction report](benchmarks/reports/development/fixest-compaction-local-2026-09-12/README.md)
records 53 completed local invocations: 51 successful estimates and two
explained all-singleton negative controls. For 983,040 rows, 50 controls and
about 0.1% missing, three-seed previous/current medians are 1.461/0.926 seconds
at four threads and 1.230/0.613 at sixteen, improvements of 36.6% and 50.2%.
The original four-thread input-time outlier remains in the summaries; an
extra pair does not reproduce it. Complete-data one FE remains essentially
unchanged at 0.585/0.580 seconds versus fixest's 0.472, leaving a 22.9% gap.
Weak, dense, bottleneck, cycle, disconnected and unbalanced holdouts differ
by at most nine milliseconds and preserve all numerical results.

All 20 final candidate/previous pairs have byte-identical full b/V exports,
exact samples/statuses/DoF/fit/certificate tables and solver diagnostics
except timings. All seven fixest comparisons pass. Native allocation peaks
agree in 18 pairs; the two heavily filtered retained-pair holdouts fall from
220,448,053 to 120,419,044 bytes. Both versions return Stata code 2001 when
the initial filtering recipe leaves only singletons; those original failure
receipts remain intact.

[Validation](benchmarks/reports/development/fixest-compaction-local-2026-09-12/validation.json)
records 37 core preparation tests, five sample tests, 40 plugin action tests,
eight licensed Stata fixtures, universal binary audit and actual arm64
ABI/self-test. New coverage includes cross-block error order, retained value
bits/capacity, singleton cascades, and independent sparse/dense/restricted
wide dummy regressions at 1/4/16 threads.
[Replayable source identities](benchmarks/reports/development/fixest-compaction-local-2026-09-12/identities.json)
bind the retained plugin SHA-256
`43377269d0bbfd7dc1bc2a964e117d9f282e123350820252289e3f6b76a92fae`.

Remaining complete-data one-FE component medians are 0.303 seconds input,
0.210 native and 0.069 outer Stata work. Further parity work must address
loading or native estimation passes. Ownership/column compaction for weighted
and robust/clustered paths is a separate next experiment; small/narrow gaps
also remain unresolved. No SCC, remote job, qualification, commit, push or
release action was taken.

## Native OLS input preparation: preceding implementation

The local follow-up moves full-candidate numeric missing detection from ado
into Rust, removes the unused OLS sort-preservation wrapper, and installs
wide IID preparation on the requested worker team for independent source
completeness checks. The C bridge only transports SPI values and the SDK
missing boundary. Restricted/string loading, authoritative sample rules,
solver routes, tolerances, all numerical formulas and transaction guarantees
remain unchanged. [ADR 0036](docs/adr/0036-native-ols-input-preparation.md)
records the development extension to ADR 0013's loading procedure.

The [input report](benchmarks/reports/development/fixest-input-local-2026-09-12/README.md)
records 134 completed local measurements, including 54 rotated primary calls
at three seeds. At 983,040 rows and 50 controls, previous/current/fixest
medians are 0.916/0.776/0.806 seconds for one FE at four native threads,
0.690/0.582/0.454 for one FE at sixteen, 0.878/0.763/0.841 for degree three
at sixteen, and 0.836/0.712/0.695 for degree six at sixteen. Current fereg
narrowly beats fixest on four-thread one FE, leads degree three by 9.3% at
sixteen threads, and trails degree six by 2.4%. One FE at sixteen threads
still trails by 28.2%. Small/narrow one-FE cases also retain gaps.

All 39 final candidate/previous pairs have byte-identical full b/V exports,
exact samples/statuses/DoF/fit/certificate tables and solver diagnostics
except timings. All 25 final fixest comparisons pass. Weak, bottleneck,
cycle, disconnected, unbalanced and dense holdouts retain their performance
in these bounded local checks. Modeled memory budgets agree; native peak
allocation is unchanged except an unexplained 819,240-byte decrease in one
narrow weak holdout, which is not claimed as a memory improvement.

[Validation](benchmarks/reports/development/fixest-input-local-2026-09-12/validation.json)
records 35 core preparation tests, 40 plugin action tests, eight licensed
Stata fixtures, universal binary audit and actual arm64 ABI/self-test. The
new fixture checks all 27 Stata missing codes in five storage types, weights,
clusters/strings, exclusions, singleton deletion, row/sort-state preservation
on direct failure exits, and independent wide IID dummy regressions at
1/4/16 threads. Existing TS, transaction, saved-output, memory-mode,
weighted/robust oracle and PPML smoke fixtures also pass.
[Source identities and replayable patches](benchmarks/reports/development/fixest-input-local-2026-09-12/identities.json)
bind the retained plugin SHA-256
`93412e7c7654deac201f5f8c1bdc5af62aece7ebf4347180df0acebe3d227470`.

An 8,192-row loading chunk gave no consistent benefit and is rejected;
65,536 rows remain. Current sixteen-thread one-FE component medians are
0.301 seconds input, 0.207 native and 0.073 outer Stata work. Further
one-FE gains need fewer native data/layout passes or a demonstrably useful
loading overlap, not just more workers for the now-small completeness check.
A separate three-seed sparse-missing probe improved from 2.846 to 1.174
seconds after removing the input traversal cliff, but still exposes costly
row-wise native classification/copying when a small fraction of rows drops.
That report identified owned column compaction as the next experiment;
the implementation and new evidence are recorded above.
No SCC, Linux/Windows science, qualification, commit, push or release action
was taken. Historical receipts remain unchanged.

## Wide-OLS scheduling: preceding implementation

The local follow-up retains three OLS scheduling changes: disjoint Schur
recurrence updates above seven workers, parallel three-column probe setup,
and fixed group partials for wide one-FE/Schur certificates. The shared
PPML recurrence crossover, existing weak-graph solver routes, all acceptance
maxima, tolerances, iteration/fallback rules and memory budgets are unchanged.
The diagnostic two-norm changes summation order; it does not control
acceptance on these routes. No new workspace is allocated.

The [scheduling report](benchmarks/reports/development/fixest-scheduling-local-2026-09-12/README.md)
records 140 completed local measured calls, including 54 rotated primary
comparisons at three seeds. At 983,040 rows and 50 controls, sixteen-thread
previous/current/fixest medians are 0.951/0.881/0.838 seconds on degree three
and 0.875/0.835/0.704 on degree six: 7.4% and 4.6% improvements, leaving gaps
of 5.1% and 18.6%. At four threads current fereg remains 32% and 24% faster
than fixest on these designs. One-FE medians are 0.947/0.687 seconds at 4/16
threads; the small four-thread slowdown did not recur consistently in three
additional paired checks. All weak/bottleneck/cycle/disconnected/dense
holdouts pass and retain their performance within the bounded local evidence.
The separate eight-thread crossover checks improve and beat fixest.

All 35 final candidate/prior pairs have byte-identical full b/V exports,
samples, statuses, DoF, fit/certificate tables, acceptance maxima, routing,
iterations and fallback. The largest diagnostic norm relative change is
7.3e-14. All 24 final fixest comparisons pass. Primary peak allocated bytes
are exactly unchanged; one unchanged narrow weak holdout has an unexplained
3.8 MB lower reported peak, which is not claimed as a memory improvement.
[Validation](benchmarks/reports/development/fixest-scheduling-local-2026-09-12/validation.json)
records 34 focused Rust FE tests, four licensed Stata fixtures (including the
weighted/robust explicit-dummy oracle at 1/4/16 threads), universal binary
audit and actual arm64 ABI/self-test. The retained plugin SHA-256 is
`20289553c62d5280ece81a07dc39abeecaa573430c34a280cfb3c7cc1b2df763`.
[Source identities and replayable patches](benchmarks/reports/development/fixest-scheduling-local-2026-09-12/identities.json)
bind the uncommitted source; this is development evidence, not qualification.

Independent one/four-variable Schur jobs and RHS-parallel scalar reductions
were slower and are removed. Retain the batched graph operator unless a
better layout wins a controlled comparison. The remaining roughly 0.45–0.46
seconds of input/outer Stata work matters especially for one FE; further
native data/normalization passes and staging/input work are the next local
targets. No SCC campaign was submitted or resumed, and no commit, push or
release action was taken. Historical receipts remain unchanged.

## Numeric passes: preceding implementation

The local follow-up retains tiled numeric RHS construction and independent
raw-residual updates, parallel initialization of already reserved storage,
and parallel wide-source finite checks within an installed worker pool.
It changes four OLS source files; the preceding Schur route, C bridge and
Stata transaction are unchanged. General validation before pool installation
stays serial. No tolerances, sample/weight rules, rank, DoF or covariance
formulas change, and no additional observation-scale allocation is introduced.

The [numeric-pass report](benchmarks/reports/development/fixest-passes-local-2026-09-12/README.md)
records 139 measured calls, including 54 rotated primary comparisons at three
seeds and 44 final holdout/profile/repeat calls. Relative to the retained
Schur baseline, complete-command medians fall another 5–10%. At 983,040 rows
and 50 controls, current/fixest medians are 0.951/0.798, 1.326/1.927 and
1.222/1.578 seconds for one FE and degree-three/six graphs at four threads.
At sixteen threads they are 0.704/0.470, 0.960/0.835 and 0.878/0.692.
The tested wide weak-graph holdouts retain their performance. A 6 ms narrow
holdout difference did not recur across three additional paired seeds.

All 35 final current/prior pairs have identical exported full b/V files,
fit/certificate tables and solver diagnostics except timings. All 22 final
candidate/fixest comparisons pass. The final source passes 7 RHS tests,
60 prepared-fit tests and four licensed Stata fixtures, including the
50-control weighted/robust explicit-dummy oracle, saved outputs, memory
modes and PPML smoke. The universal macOS build passes binary audit and the
actual arm64 ABI/self-test. This is development evidence, not qualification.
[Source identities, patches and validation](benchmarks/reports/development/fixest-passes-local-2026-09-12/identities.json)
bind the retained plugin SHA-256
`26aca86c83c1cc7426a8de6601b1d8ee632acc43e3e5aacc9f0c3ea3a3ed0a2b`.

The report retains a 1.337-second one-FE outlier, largely located in unchanged
input loading; it did not recur in two paired checks, but its cause is not
established. Two external RSS samples miss the native peak and are flagged;
no new RSS ranking is claimed. Native peak allocated bytes and modeled fast
budgets are exactly unchanged across the 18 primary pairs.

The main remaining cost is about 0.45 seconds of input/outer Stata work at
sixteen threads. Callback hoisting and C-side tiled copying did not establish
a useful improvement and were rejected. Next local investigations are smaller
wide-model scheduling, remaining native passes, and reuse of private Stata
staging storage without weakening sampling or transaction guarantees. No
SCC campaign was submitted or resumed, and no commit, push or release action
was taken. Historical reports remain unchanged.

## Guarded OLS Schur route: preceding implementation

Johannes approved ambitious local work to approach fixest while preserving
weak-graph strengths. [ADR 0035](docs/adr/0035-guarded-wide-ols-schur.md) admits a
guarded Schur route for fast, unit-weight, two-FE OLS at 16–64 actual RHSs and
at least 16,384 observations. An eight-step three-column probe selects the
full-width solve; a 64-step cap and ordinary certified CG continuation share
the iteration budget. Both workspaces must fit the joint memory budget.
Other route classes retain the previous planner. The one-FE initial projection
now reaches its exact direct path, and fresh certified norms move into OLS.
Parallel group preparation/reconstruction and deterministic OLS norm reductions
remove repeated work without weakening scientific checks.

The [September 12 report](benchmarks/reports/development/fixest-gap-local-2026-09-12/README.md)
preserves 148 completed measured invocations, including 54 rotated final
primary calls at three seeds, 25 final holdout/narrow calls and six final
profiles. At 983,040 rows and 50 controls, four-thread medians are 1.404/1.297
seconds for degree-three/six versus fixest's 1.916/1.548 and baseline fereg's
7.630/6.962. Sixteen-thread medians are 1.009/0.936 versus fixest's 0.835/0.686.
One-FE time falls to 1.034/0.769 at 4/16 threads; fixest remains faster at
0.795/0.447. The weak and dense holdouts improve; the difficult cycle rejects
Schur and incurs about 3 ms of probe work. Narrow controls retain exact full
b/V parity. These are local development measurements, not universal graph
parity or release evidence.

Independent full dummy-model solves adjudicate native fixest discrepancies
on bottleneck and disconnected graphs. The final candidate matches the
independent coefficients/RSS/DoF. A native fixest stopping discrepancy and a
separate connected-component DoF convention difference remain explicit
failed native-parity comparisons; neither is turned into an equal-accuracy
speed claim. The earlier [September 11 report](benchmarks/reports/development/fixest-gap-local-2026-09-11/README.md)
remains unchanged, including the prototype weakness explained by the corrected
recurrence/certificate scaling.

[Validation](benchmarks/reports/development/fixest-gap-local-2026-09-12/validation.json)
records 33 borrowed-FE and 58 prepared-OLS tests, seven licensed Stata fixtures,
and the normal universal macOS build with binary audit and actual arm64
ABI/self-test. That report's measured plugin SHA-256 is
`ffa5537a0da609888de55ad122973140f86f48700a3368fb2f3098a785d072ac`.
[Source manifests and replayable patches](benchmarks/reports/development/fixest-gap-local-2026-09-12/identities.json)
bind the uncommitted source snapshot to base `d44e36bfa5ad7`; the base SHA
alone does not identify these changes. The [local harness](benchmarks/local_fixest_gap/README.md)
contains the follow-up reproduction entry points. No SCC job was submitted,
resumed or changed for this investigation; the stopped systematic campaign
stays stopped. No commit, push or release action was taken.

The numeric-pass follow-up above addresses the next RHS and raw-residual
work. Coefficient-space Irons–Tuck and batch missing detection were slower
and were not retained in this preceding investigation.

## Systematic benchmark campaign: stopped and reported

Johannes stopped this campaign on September 11. Array **7522398** was cancelled
and its monitor deleted. The [final cancellation receipt](benchmarks/reports/development/systematic-production-2026-09-11/cancellation-confirmed.json)
confirms the array is absent from the scheduler: tasks 1--78 completed, task 79
was interrupted, and tasks 80--237 never started. No continuation or checkpoint
resubmission is authorized.

The [discussion report](benchmarks/reports/systematic-stopped-2026-09-11/discussion.pdf)
and [figure atlas](benchmarks/reports/systematic-stopped-2026-09-11/figure-atlas.pdf)
analyze the collected partial evidence. The atlas retains individual vector
panels assembled with LaTeX `subcaption`. The [coverage receipt](benchmarks/reports/systematic-stopped-2026-09-11/stopped-coverage.json)
records 826 accepted registered comparison groups, plus two off-grid pilot
groups; 100 OLS configurations have six valid warm repetitions. PPML has five
accepted pilot groups and no complete repetition grid. Finished calls from
cancelled task 79 remain in the raw observations but are excluded from strict
aggregates. The [evidence archive](benchmarks/reports/systematic-stopped-2026-09-11/evidence.zip)
contains numerical receipts, provenance, accounting and analysis CSVs. Collected
raw logs are retained separately in the ignored production receipt `work/`
directory and on SCC. The [delivery receipt](benchmarks/reports/development/systematic-production-2026-09-11/report-delivery.json)
binds the eight-page discussion, 45-page atlas and checked evidence archive;
all pages were visually reviewed and all 132 panel PDFs remain vector figures.

Fereg's clearest advantage is difficult sparse FE versus reghdfe. Its main
weaknesses are the gap to fixest on well-connected designs, limited gains beyond
8--14 threads in several designs, and high RSS on sparse three-FE and the largest
degree-three case. In the latter case the automatic CMG-to-LSMR/Schwarz switch
coincides with setup increasing from 0.21 to 12.01 seconds; this is an observed
profiling lead, not a causal comparison of forced solvers. Tiny coefficient
discrepancies on the largest degree-two design remain unresolved and excluded.
Production consumed 96.3 node-hours (2,695 reserved core-hours); setup, smoke and
pilot work are additional. The grid is incomplete and ordered by model/design,
so it supports no balanced overall ranking. Estimator code and frozen scientific
thresholds are unchanged.

The requested OLS/PPML comparison harness is implemented under
[`benchmarks/systematic/`](benchmarks/systematic/README.md), with 304 unique
configurations, six warm repetitions and one cold diagnostic. It compares
the requested Stata estimators and pinned R/Python fixest versions on eight
designs, up to 3,932,160 observations. Its figures are individual vector PDF
panels assembled with LaTeX `subcaption`, including time, RSS and paired ratios.
The candidate remains clean source revision `d44e36b`; estimator code is unchanged.

The [development checkpoint](benchmarks/reports/development/systematic-2026-09-10/checkpoint.json)
and adjacent compact application/scientific/accounting receipts record the SCC
work in run `20260910-systematic-d44e36b`. Setup job 7520628 passed on a 28-core
node. Probe 7520678 passed its application checks. Smoke job 7520690 completed
all eight OLS designs with passing independent coefficient/SE checks; PPML
passed those checks for one FE and dense two FE, then its independent oracle
failed to converge on other designs. Focused retest 7521074 did not resolve the
oracle failure. The unrelated R metadata-rounding correction passed its OLS
retest. Scientific differences in native comparator output remain recorded.

Johannes authorized further investigation on September 11 before production.
Diagnostic job 7521509 completed successfully and traced three
failing cases with and without column scaling;
the [submission receipt](benchmarks/reports/development/systematic-oracle-investigation-2026-09-11-submission.json)
binds its inputs, controller and 28-core/30-minute resource request.
the [diagnosis and traces](benchmarks/reports/development/systematic-oracle-2026-09-11/diagnosis.json)
show an inner LSMR normwise accuracy floor from unequal column norms. Column
equilibration reaches the unchanged outer score threshold in six/seven iterations
for all three cases. The independent oracle now applies that correction; estimator
settings, inputs and acceptance tolerances are unchanged. The diagnostic heartbeat
was deleted. Array 7522234 (tasks 1--8) validated the final harness on both models,
all eight designs, two seeds and four/28 requested cores, with four groups packed
per task and a one-hour limit. All 32 candidate fits passed independent coefficient
and SE checks and pinned reghdfe/ppmlhdfe comparisons; all eight tasks have accepted
accounting. The [final smoke acceptance](benchmarks/reports/development/systematic-final-smoke-2026-09-11/acceptance.json)
preserves native comparator discrepancies and three gglm initialization errors.
These errors do not establish failure of the larger target fits. Its isolated
controller is `controller-final` and its output is `validation-final`.

The production controller adds same-node/task and partial-group recovery guards;
all 32 comparisons replay with unchanged outcomes, and 14 local evidence/figure
tests pass. Numerical workers, data generation and oracle are unchanged from the
accepted smoke. The runtime is frozen with fingerprint `04b5e8cf00ae6947…` under
`controller-production`. [Pilot array 7522274](benchmarks/reports/development/systematic-pilot-2026-09-11/submission.json)
was accepted with tasks 1--5, two groups per task, 28 E5-2680v4 cores,
8 GiB/core and a 12-hour limit. All ten groups and 55 arm receipts are complete,
with accepted scheduler accounting and application completion markers. The
[pilot acceptance](benchmarks/reports/development/systematic-pilot-2026-09-11/acceptance.json)
records ten successful candidate fits, nine passing pinned-reference comparisons
and one ppmlhdfe command timeout. Native pyfixest iteration failures, gglm
initialization failures/timeouts and accuracy discrepancies remain recorded.
Peak estimator RSS was 6.87 GiB; scheduler maxvmem was 13.004 GiB. The longest
task took 5,054 seconds. The pilot heartbeat was deleted after terminal accounting.

[Production array 7522398](benchmarks/reports/development/systematic-production-2026-09-11/submission-1.json)
was submitted with tasks 1--237, at most 12 concurrent tasks, 28 E5-2680v4 cores,
3 GiB/core and a 12-hour cap. It packs 2,120 remaining comparison groups; eight
registered pilot groups are reused under the same frozen runtime identity.
Measured packing targets eight hours and checkpoints unstarted groups before
the walltime cap. Native failures remain terminal benchmark outcomes and do not
trigger estimator-option changes. That continuation plan was superseded by the
user's stop instruction. Only 28-core SCC nodes were used and task walltime was
capped at 12 hours. Local checks exercise evidence acceptance and figure
assembly, never estimator benchmarks.

The [first 78 terminal tasks](benchmarks/reports/development/systematic-production-2026-09-11/first-78-validation.json)
have accepted scheduler accounting and 818 validated complete comparison groups.
Task 51 checkpointed after 20,626 seconds: ten groups completed and six never
started. Its [preserved checkpoint](benchmarks/reports/development/systematic-production-2026-09-11/checkpoint-51.json)
and [prepared two-task plan](benchmarks/reports/development/systematic-production-2026-09-11/prepared-checkpoint-51-plan.json)
are preserved as historical evidence; the prepared plan was never submitted and
will not be resumed. The frozen controller still matches its inventory. Final
accounting contains 79 started tasks, with nonzero cancellation status on task
79. Never-started tasks have no accounting record.

## OLS performance and LSMR correctness

This checkpoint collects the within-norm and wide raw-moment scan optimizations,
guarded wide finite solve, low-memory LSMR preparation reuse, certified mixed
LSMR/CG reporting, and the published parallel-repeatability repair below.
Johannes authorized committing the accumulated work on main and pushing it.
The production and controller file hashes match the tested integration
snapshot; the immutable receipts retain their original source identities.
Prior sections describe the development state when each receipt was recorded.

The approved patch is published in the within fork as
[`1ac3522`](https://github.com/johannes-schmieder/within/commit/1ac3522a251c2622dbeab451415924f9ebe117b9),
a fast-forward of its existing `fix/weighted-categorical-assembly` branch.
Both fereg runtime dependencies, Cargo.lock and the reference lock now use
that exact commit under [ADR 0034](docs/adr/0034-lsmr-parallel-reduction-order.md).
The [published-pin integration receipt](benchmarks/reports/development/ols-lsmr-repeatability-pin-local-2026-09-10.json)
binds the actual Cargo checkout, source snapshot and rebuilt plugin.
Earlier work and evidence are preserved in this checkpoint. The installed
plugin has not been replaced.

The upstream implementation changes only two indexed LSMR norm/dot sums:
one existing 4096-element chunk per reduction leaf fixes their arithmetic
tree independently of worker stealing. Thresholds, per-chunk arithmetic,
public APIs, allocations, routing, iteration budgets and independent
certificates are unchanged. No local dependency override or copied solver
internals remain in fereg. The reviewed patch and prepublication receipt are
preserved under `benchmarks/patches/` and `benchmarks/reports/development/`.

The published-pin build passes all 11 selected captures with exact science
against the reviewed patch, unchanged exact self-repetition and complete
pinned reghdfe checks. This includes the four original N100,000 seven-thread
failures: balanced two FE, balanced three FE and mixed three FE in fast
memory, plus mixed three FE in low memory. The serial low-memory case and
six K29/pool-size/weight/one-FE controls also pass. Weighted automatic and
explicit-Schwarz saved-output fixtures and transaction rollback pass.

Focused validation of the published pin passes: 40 upstream LSMR tests,
57 prepared OLS tests, 31 shared PPML tests, 11 independent prepared-fit oracle
tests, 31 allocation-contract tests and the exact dependency-pin test.
Both macOS slices build and pass binary/native ABI audits; clippy, formatting
and diff checks pass. Allocation inventory v17 changes only the reviewed
Cargo.lock and core dependency wiring; all source allocation entries and
hard boundaries match v16. No unrelated dependency versions changed.

The prepublication receipt records the intentional last-bit changes against
the old dependency and the source-extracted kernel benchmark. Seven-thread
median paired ratios were 0.986 for the norm and 0.992 for the dot; serial
ratios were 1.011 and 1.013. Those component measurements were not rerun for
pin integration and are not command-speed evidence. Failed baseline captures
remain failed evidence. The frozen F0 exact-extraction policy is unchanged.
No RSS, F0, SCC, Windows or release qualification was run.

This integration is complete. The separate explicit-diagonal LSMR certificate
failure and remaining performance milestones stay open.

## Prior certified LSMR/CG report aggregation

The next tracker #26 correctness issue is fixed under
[ADR 0033](docs/adr/0033-certified-lsmr-cg-batch-reporting.md), with the
[source/artifact-bound receipt](benchmarks/reports/development/ols-mixed-report-local-2026-09-10.json).
The baseline is the September 9 retained-LSMR candidate below. Only
`ols/prepared_fit.rs` changes in production relative to that baseline; all
work remains uncommitted on main.

Low-memory automatic fits can combine a certified LSMR batch and a certified
CG continuation when their requested/effective LSMR preconditioners agree
and the CG report carries its fallback flag. The aggregate reports requested
`auto`, used `cg`, and fallback true in either batch order. Maximum iterations
and certificate norms, summed timing/retry fields and ordered warnings retain
their prior conventions. Failed certificates, incompatible requests or
preconditioners, unflagged CG and other mixed engines remain rejected.
Solver choice, numerical work, tolerances, budgets, memory lifetimes and the
public result wire are unchanged.

The original weighted automatic fixture now passes against Stata explicit
dummies, including covariance, fitted values, residuals, saved-FE normalization
and reconstruction at pool sizes 1/5/10. Its data, commands and numerical
assertions are unchanged; only the expected aggregate solver metadata and
terminal name were updated. The same baseline still fails with `r(459)`.
The N100,000 three-FE low-memory serial reproduction now passes complete pinned
reghdfe comparison and exact self-repetition; maximum coefficient difference
from the reference is below 2.3e-15. The seven-thread reproduction gets past
the report error but fails exact repetition on coefficients at about 2.5e-17.
That capture remains failed evidence, separate from the fixed report error.

All six previously successful controls retain exact scientific equality,
including low/fast memory, pool sizes, one/seven threads and probability
weights. Focused checks pass: 57 prepared OLS tests, 31 shared PPML tests,
11 independent prepared-fit oracle tests, 31 allocation-contract tests,
clippy, formatting and licensed transaction rollback. The new report test
fails before the fix and passes after it for all six batch orders; rejection
tests preserve invalid-input atomicity. Both macOS plugin slices build with
Rust 1.85.1 and pass binary/native ABI audits. Inventory v16 changes only the
reviewed production tokens; allocation sites and hard boundaries match v15.

This is a correctness fix with no performance claim. Peak RSS, F0, SCC,
Windows and release qualification were not run; nothing was pushed or installed.
The next correctness work is parallel exact-repeat behavior and the separate
explicit-diagonal LSMR certificate failures. Broader M1a setup lifetimes and
other performance milestones remain open.

## Prior low-memory LSMR preparation reuse

The bounded categorical projection portion of tracker #26 M1a is implemented
under [ADR 0032](docs/adr/0032-low-ols-lsmr-preparation-reuse.md), with the
[source/artifact-bound receipt](benchmarks/reports/development/ols-lsmr-reuse-local-2026-09-09.json).
The baseline is the prior wide raw-moment candidate below. Three production
files change relative to that baseline: the borrowed FE adapter, its private
module export and prepared OLS integration. All work remains uncommitted on main.

Eligible low-memory categorical fits lazily prepare one LSMR operator and
preconditioner for the response and all regressor projection batches. The
sample, finalized FE design, weights and solver configuration remain fixed.
Weighted preparation uses one fallibly reserved owned weight copy. Every batch
retains its original RHS scale, iteration budget and independent certificate.
The operator is released before QR/VCE/output recovery and before any CG
continuation; after a continuation, subsequent batches use the existing
one-shot route. Fast/generic/sloped/output paths and report-merging policy are
unchanged. This does not complete all of M1a.

Five alternating untraced pairs per target, N=30,000, three categorical FEs,
29 controls, one native thread, low memory:

| Pool size | Baseline median seconds | Candidate median seconds | Median paired reduction |
|---|---:|---:|---:|
| 10 | 0.459 | 0.445 | 3.6% |
| 2 | 0.519 | 0.460 | 11.5% |

Reductions use paired ratios, not ratios of displayed medians. Every pair
matches scientific fields exactly and passes the complete pinned reghdfe
comparison. Five-pair fast controls have median candidate/baseline ratios
1.043 for the same three-FE design and 1.025 for one FE; individual timings are
noisy. Separate traces show projection setup falling from 18.0 to 6.25 ms at
pool 10 and from 63.5 to 6.44 ms at pool 2. Successful operator constructions
fall from four/sixteen to one per projection; the separate final-residual
LSMR call remains one-shot. Independent operator-application counts and RSS
were not measured.

All nine diagnostic cases pass exact baseline/candidate and self-repetition
checks plus complete pinned references, including analytic/probability
weights, seven threads, K10 and pool-size controls. Focused checks pass: 30
borrowed FE tests, 55 prepared OLS tests, 31 shared PPML tests, 22 independent
Rust oracle tests, 31 allocation-contract tests, nine harness tests, clippy
and formatting. The explicit Schwarz-LSMR licensed fixture checks weighted
coefficients, covariance, residuals, saved FE normalization and reconstruction
against Stata explicit dummies at pool sizes 1/5/10; transaction rollback also
passes. Both macOS plugin slices build with Rust 1.85.1 and pass binary/native
ABI audits. Inventory v15 records the reviewed allocations and preserves the
hard one-shot/transaction boundaries.

The five older N100,000 multi-FE failures reproduce on both the source-bound
baseline and final candidate. Three seven-thread fast cases fail exact
repetition; two low-memory cases fail with inconsistent solver reports. A
fresh trace localizes the low-mode failure: the response certifies with LSMR,
a regressor batch fails the tighter LSMR certificate and continues to certified
CG, and the existing report merger rejects the mixed engines. No tolerance or
report-policy change is retained. The initial constant-tail hypothesis did
not explain that failure, and its speculative report-merging edit was removed
before the final rebuild and measurements.

The weighted output fixture's automatic and explicit-diagonal variants also
fail on both baseline and candidate; the tighter explicit-diagonal attempt
fails too. These are preserved as failed evidence, separate from the passing
Schwarz fixture. Report aggregation is addressed by the current work above; the parallel
exact-repeat and explicit-diagonal certificate failures remain open.
Remaining performance work includes fast/generic/output setup lifetimes,
redundant RHS transport and measured serial VCE work. No F0, SCC, Windows,
release qualification, push or installed-plugin replacement was performed.

## Prior wide raw-moment scans

Tracker #26's next selected optimization is implemented under
[ADR 0031](docs/adr/0031-wide-raw-moment-column-blocks.md), with the
[source/artifact-bound receipt](benchmarks/reports/development/ols-raw-moments-local-2026-09-09.json).
The baseline is the guarded wide-solve candidate below, including the earlier
within-norm optimization. Production changes only `ols/prepared_fit.rs` relative
to that baseline; the work remains uncommitted on main.

Wide raw-moment caches scan eight adjacent columns per block, sharing matrix
loads and weight reads. Each column retains the prior arithmetic and row order
for maxima, scaled norms and compensated means. Blocking requires enough
blocks for the worker team; raw matrices at most 64 columns retain their
existing fused algorithm. There is no new heap allocation. The five fixed-size
accumulator arrays use 320 bytes per active block; inventory v14 preserves
allocation sites and the hard estimation/transaction boundaries.

Five alternating pairs per target, N=100,000, one FE, fast memory:

| Controls | Native threads | Baseline median seconds | Candidate median seconds | Median paired reduction |
|---|---:|---:|---:|---:|
| 64 | 1 | 0.437 | 0.352 | 18.8% |
| 64 | 7 | 0.186 | 0.178 | 4.3% |
| 65 | 1 | 0.464 | 0.371 | 20.4% |
| 65 | 7 | 0.228 | 0.185 | 12.6% |

Reductions use the median of paired ratios, not the ratio of the displayed
medians. Every pair matches all scientific fields exactly and passes its
complete pinned reference comparison. Five-pair unchanged K50 controls have
median ratios 1.000 and 1.00694 at one/seven threads. Separate K65 traces put
raw moments at 0.157 to 0.0643 seconds with one thread and 0.0297 to 0.00967
seconds with seven threads. Their phase timings are not paired speed claims.

All 16 additional diagnostic scopes reproduce the source-bound baseline
exactly and pass complete pinned reghdfe comparisons, including low memory,
K128, no FE, analytic/frequency/probability weights, exact collinearity and
absorbed slopes. The independent K65 group-elimination/SVD oracle passes;
coefficient error is below 1.2e-15. Licensed fixtures pass wide weighted saved
outputs against explicit dummies, PPML against Stata Poisson, fast/low parity
and transaction rollback. Focused checks pass: 54 prepared-OLS tests, 335 PPML
tests, 11 independent prepared-fit oracle tests, 31 allocation-contract tests,
seven diagnostic-harness tests, clippy and formatting. Both macOS plugin
slices build with Rust 1.85.1 and pass the binary/native ABI audits.

This is focused local macOS development. Peak RSS and independent operator
counts were not measured; no F0 matrix, SCC, Windows or release qualification,
push or installed-plugin replacement was performed. Earlier multi-FE repeat
failures and the separately recorded repository-checker failures remain open.
Retained LSMR setup reuse against this improved baseline is addressed by the
current work above.

## Prior guarded wide OLS solve

Johannes approved extending the guarded finite solve above 64 candidates.
[ADR 0030](docs/adr/0030-wide-guarded-ols-finite-solve.md) and the
[source/artifact-bound development receipt](benchmarks/reports/development/ols-wide-cholesky-local-2026-09-09.json)
record the implemented change. The baseline is the prior norm optimization
below; its plugin is unchanged from the earlier width receipt.

Wide fast fits now use disjoint Gram-row partitions and attempt the existing
guarded Cholesky solve. Weak/rank-deficient pivots and failed inverse
certificates retain QR; fresh score, FE, bread and reconstruction checks keep
their existing bounds. Wide moment construction needs no partial bank. Fast
preflight charges an additional `32*K^2 + 128*K` bytes for the live Cholesky
workspace, independent of thread count. At-most-64 arithmetic and low-memory
QR remain unchanged.

Five alternating warm pairs per target, N=100,000, one FE, K=65, fast memory:

| Native threads | Baseline median seconds | Candidate median seconds | Median paired reduction |
|---|---:|---:|---:|
| 1 | 6.965 | 0.429 | 93.8% |
| 7 | 2.447 | 0.207 | 91.6% |

Reductions use paired ratios. Five-pair K64 controls show no material
regression and exact science. A separate one-thread trace puts the finite
solve at 6.708 seconds before and 0.0575 seconds after. FE/pilot iterations,
solver choices and fallback reporting are unchanged in every accepted
comparison; independent operator-call counts were not collected.

All 16 diagnostic scopes have complete pinned reghdfe comparisons. They cover
K64/K65 at N100,000 plus K128, no FE, low memory, analytic/frequency/probability
weights, exact collinearity and absorbed slopes at N10,000. K64 and low-mode
science remain exact. K65 paired coefficient differences are below 2.8e-15;
all changed numerical results pass the registered OLS envelopes and unchanged
absolute certificates. Six independent group-elimination/SVD oracles pass.
The development comparator records covariance determinant and pilot
contraction as numerical diagnostics while keeping policy decisions exact;
the original F0 evaluator and its `extraction_exact` policy are unchanged.

Final-source checks: 52 prepared-OLS, 21 finite-kernel, 49 memory, 31 PPML,
40 plugin-estimation and 22 independent Rust oracle tests pass, as do 31
allocation-contract tests, seven base-harness tests and seven final wide
comparator tests. Wide unit cases include weighted full rank, near deficiency,
exact deficiency, thread determinism, overflow and memory preflight. Licensed
fixtures verify wide weighted saved outputs against explicit-dummy regression,
wide PPML against Stata Poisson and fast/low results, and transaction rollback.
Both macOS plugin slices build with Rust 1.85.1; binary and native ABI audits
pass. Inventory v13 records three reviewed production files with unchanged
allocation-site inventories and hard transaction/one-shot boundaries.

Limits: local Apple Silicon/Stata 19 development only; no F0 screen, SCC,
qualification, Windows execution, release, push or installed-plugin
replacement. Peak RSS was not measured. The K128/weighted timings are unpaired
diagnostics. The next measured bottleneck is the existing wide raw-moment
scan: about 0.153 seconds of a 0.319-second prepared fit at one thread/K65.
That bottleneck is addressed by the current work above. Earlier multi-FE
exact-repeat failures remain open.

The broad repository checker still fails on five obsolete STATUS headings
already absent at `c975bc5` and an ignored captured source snapshot counted as
a second status document. [The bounded check](benchmarks/reports/development/ols-wide-checker-preexisting-2026-09-09.json)
records these pre-existing findings; the checker and historical snapshot were
left unchanged. Focused scientific, build and allocation checks pass.

## Prior arithmetic-preserving OLS performance

Local development result from clean `main` at
`c975bc5c6e2aa5ff4be159225447d31f0fe20de6`:
[source/artifact-bound receipt](benchmarks/reports/development/ols-arithmetic-local-2026-09-08.json).
The retained source overlay changes only `ols/prepared_fit.rs` in production.

Wide parallel within-norm scans now process eight adjacent columns with
worker-local stack accumulators. Each column retains the scalar scaled-sum
arithmetic and row order. Blocking requires enough blocks for the existing
worker team; serial and narrow parallel paths retain their prior scans.
There is no new heap allocation, solver policy, tolerance, protocol or public
syntax change. The one-FE wide norm phase fell from about 10.2 ms to 3.7 ms
in separate traced runs.

Five alternating baseline/candidate warm pairs per target, N=100,000, K=50,
fast memory and seven native threads:

| Target | Baseline median seconds | Candidate median seconds | Median paired improvement |
|---|---:|---:|---:|
| No FE | 0.167 | 0.157 | 6.5% |
| One FE | 0.141 | 0.128 | 9.2% |

The improvement column uses the median of paired ratios, not the ratio of
the two displayed medians. All target pairs match science exactly and have
complete pinned reghdfe comparisons plus an independent group-elimination/SVD
oracle. Coefficient differences from that oracle are below 5e-16. Five-pair
narrow guards and three serial controls show no median regression above 5%.
Earlier width experiments, regressing narrow dispatch, and builds with invalid
source/artifact binding are preserved as excluded or rejected evidence.

The separate 38-scope diagnostic has 26 accepted baseline scopes and 25
accepted candidate scopes; all 28 complete baseline/candidate captures match
exactly. Several multi-FE scopes fail exact self-repetition, including an
additional candidate scope with a prior baseline trace failure. Four original
reference attempts lack complete accepted records. These remain unavailable
evidence. A supplemental wide one-FE guard matches saved FE and residual
outputs exactly and has a complete pinned comparator; it does not replace the
original unavailable weak-two-FE saved-output comparison.

Final-source checks pass: 51 prepared-OLS tests, 22 independent oracle tests,
40 plugin estimation tests, 31 shared PPML tests, 31 allocation-contract tests,
six diagnostic-harness tests, formatting and clippy. Five licensed Stata
fixtures cover transaction rollback, fast/low memory, constrained preflight,
PPML auto/explicit and saved-output behavior. Both universal-plugin slices
were built in an isolated target directory; binary and actual ABI checks pass.
Allocation-source inventory v12 changes only the reviewed production tokens
and error literal in `prepared_fit.rs`; allocation sites and hard F0 seams are
unchanged. No scientific baseline/candidate differences were accepted.

Supplemental K=64/65 checks are recorded in
[the width receipt](benchmarks/reports/development/ols-width-local-2026-09-08.json).
The current production files and both plugin hashes match the original
source-bound artifacts. All six one-FE scope pairs (one/seven threads, fast
K64/K65 and low K65) match science exactly and pass the complete pinned
reference comparison. Three independent SVD oracles pass, with coefficient
differences below 2.5e-15; seven harness tests pass. The diagnostic runner now
accepts a separate manifest and generates its requested control width without
altering the original manifest or receipt.

The earlier width receipt established the old K64/K65 discontinuity and exact
science for the norm optimization. Its timings are sequential diagnostic
captures. The subsequent approved wide-solve implementation and new evidence
are recorded above; neither historical receipt was rewritten.

## PPML auto admitted for development

ADR 0029 now admits the implemented automatic policy:

- Exactly two or three categorical intercept FE dimensions select LSMR when
  fast execution and the complete ordinary/checkpoint workspace fit the
  generation-zero memory plan.
- Other designs, absorbed slopes, low-memory execution, allocation audit, or
  modeled specialized-workspace veto select MAP. Auto never selects CMG.
- A modeled veto occurs before solver construction and is not a fallback.
  Actual allocation/interruption/data/internal errors remain fatal.
- Requested, selected, concrete, effective, checkpoint, retry and fallback
  reporting is preserved. Auto uses the same certified engine as explicit LSMR.

Admission: benchmarks/reports/development/ppml-auto-development-admission-2026-09-08.json.
This is development acceptance, not formal qualification, a successful
reclassification of the failed calibration campaigns, or a claim that every
scope was rerun on the repaired source.

## Scientific adjudication

All21 hard actual-auto scopes on9 original datasets have independent
coefficient binding, model-space, objective, finite/per-FE score, rank/DoF,
covariance and default/tighter stability evidence.

- 18 scopes pass the unchanged numerical comparisons using complete pinned
 reference records. Earlier complete rows in failed campaigns are prospective
 scientific witnesses only, not accepted timings or campaign passes.
- Task17 has a fresh optimum-accuracy proof on the repaired Linux capture:
 coefficient radius4.882e-12 and covariance radius1.762e-12. All5 complete
 reference repetitions have17 coefficients and50 covariance cells proven
 outside their existing envelopes; other compared science passes.
- Tasks7/8 share the many-level N75000/K50 dataset with no complete reference
 row. Both pass independent optimum inclusion. The task7 coefficient and
 covariance bounds are6.345e-12 and1.793e-12. Reference error is not claimed.

The apparent four additional VCE discrepancies were truncated-record artifacts.
The new strict reference parser rejects incomplete matrices/records, invalid
stripes, unsuccessful fits and invalid objective pairs. It does not treat
missing evidence as a numerical mismatch. ADR0026 finite/domain/Poisson-identity
and no-worse objective rules apply without weakening coefficients
(rtol1e-7/atol1e-10) or VCE (rtol1e-6/atol1e-10).

All9 preserved SCC dataset hashes were reverified read-only; the reference
closure matches original setup. Original input/source and audit bindings are
in ppml-auto-adjudication-completion-2026-09-08/work/aggregate-bindings.json.
No projected reference state is used as an additional objective witness.

## Common checkpoint-cycle repair

The Linux K0 N75000 many-level two-FE tighter fit cycled: a certified warm
checkpoint could reconstruct the older canonical predictor, moving eta by
1.599e-10 against the requested1e-10 profile gate despite a passing score.

A fit-owned checkpoint target now tightens100x after a certified unchanged-basis
profile rejection, bounded below by1e-14 without loosening an already-tighter
target. The refined target also governs final-Hessian synchronization. The
next actual checkpoint counts the extra work as a retry. Existing RAII,
warm-state invalidation, two consecutive deviance-plus-score convergence,
canonical rank and exact final q_i*mu_i certificates remain mandatory.

The formerly300-second timeout now takes1.873seconds at both default and
tighter outer tolerance. Strict/final eta movements are8.42e-12/7.29e-16.
Default b/V/objective/rank/DoF are unchanged. Repaired difficult2FE and task17
default results are identical to baseline; sparse3FE b/V differences are
1.05e-16/3.69e-19, with unchanged objective/rank/DoF and passing certificates.
Different hosts prohibit matched performance claims for those guard timings.

## Source, jobs and artifacts

Baseline main commit:7c1fc09207864b41f851c0cb58c2dcbe80b1e8be.
Frozen dirty snapshot:da29adb7f6061a933dccbedbae923d96957361f3aa495f75780be8a05755362e.
The sole subsequent compiled-production overlay is
crates/fereg-core/src/ppml/prepared_irls.rs SHA
977522f1b878c2f96b5147f85fd927e739cb65b5111a9062f25d5f3ff50d3c93.
Documentation, adjudicator and fixture changes are separately identified.

- 7490248: actual-auto Linux smoke, task17, pass.
- 7490294/7490295: baseline actual-auto arrays;16 successful scope receipts.
- 7490361: independent direct-ring reaudits repair auditor failures4/7/8;
 original failures remain recorded.
- 7490499: trace-only diagnosis, no numerical policy change.
- 7490653: repaired K0 task3, successful fits and independent audit.
- 7490705: repaired guard scopes1/11/17; all scheduler/application/audit passes.
- 7490776.1–4: strict reference attempts all timeout1200seconds,
 failed0/exit1, application124, no fit sentinel. None accepted.

Every substantive job reserved28cores. No job is currently pending for this
goal and the completed campaign monitors were deleted.

Baseline Linux plugin:12f17d5873a0092e2ac6baee6b3ced2732504550c2983e5ada4a2d1a07f3450a.
Repaired Linux plugin:01af450c8fa6d841584661608fa45db41b0f9afb14cd7750eeaa9a3f51bc5124.
Repaired universal Mac plugin:47e578679f81ba9f0ffbb087c94864019427fa15c1f1ecac7ecc3bac5791caee.
The isolated Mac build used installed target libraries; only unavailable rustup
provisioning was omitted. Standard compilation, binary audit and actual ABI
loader tests passed. Formal exact-source qualification remains separate.

## Validation and handoff

- 745 repaired-core tests;18 focused strict-checkpoint tests;2 constrained-memory
 tests; formatting and core all-target Clippy passed.
- 71 focused auto Python tests,10 accuracy tests,8 optimum-adjudication tests,
 16 PPML help-contract tests,31 allocation-source-contract tests and10 build-
 closure tests passed. Existing independent oracle/memory evidence is retained.
- 12 licensed Mac auto-versus-explicit route pairs pass with exact b/V/objective/
 rank/DoF/sample/mu agreement. Low memory selects MAP; failed auto restores
 prior results. An additional3FE pair exactly matches saved effects and eta/d.
- Terminal Stata sentinel and explicit application returncode0 were checked.
- Git diff whitespace validation passes.

The full evidence chain is listed in the development admission receipt.
The admitted universal artifact is installed in dist and passes the same
fixture in a fresh Stata process, including its explicit terminal sentinel.
Activation receipt: ppml-auto-local-activation-2026-09-08.json.
Final whitespace-only cleanup of the independent coefficient-proof script
preserves its Python AST and nonwhitespace bytes exactly;10 accuracy tests were
rerun. The original executed source is preserved and historical proof hashes
are not rebound to the cleaned file.
Historical optimization evidence remains in its named immutable reports;
nothing is promoted across source revisions. The coherent development commit
includes this handover and the source-bound admission/activation records.

## Remaining performance work (not part of this admission)

- Sparse/mixed3FE ordinary joint LSMR conditioning and operator work remain
 high-leverage; do not restore the rejected retained all-pairs Schwarz policy.
- CMG keeps its bounded nuisance envelope and fresh certificates. Guarded
 weak-edge grouping, graph batches and certified warm starts are retained;
 unconditional grouping and ineffective response-start experiments are not.
- Explicit MAP is still slow on poorly connected graphs. Auto now avoids it
 within its admitted two/three-effect envelope.
- Easy one-FE wide-K weighted-design passes remain a useful optimization target.
- No further upstream API expansion, auto-policy refit or new campaign is implied.

## Release boundary

Development acceptance does not close the beta's formal exact-source
scientific, memory, performance or cross-platform release gates. Windows and
the formal aggregate remain deferred. No push, tag, release or broader
qualification is authorized by this completed development admission.
