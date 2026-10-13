# Validation status

Johannes accepted the integrated runtime on 2026-10-07. That is architectural
acceptance, not a performance pass: the final timing campaign closed incomplete
and historical timing was mixed. The Mac is the qualification host. Linux and
Windows evidence comes from bounded build, load and mapped-fixture checks, which
do not establish full platform scientific or performance qualification. GitHub
testing is paused. Native distribution still needs the
[licensing decision](licensing.md).

This page keeps accepted evidence, open failures and standing caveats. The full
narrative ledger, with every run identifier, archive and receipt digest,
campaign parameter and failed attempt, is in Git at commit `140dda27c42a`:
`git show 140dda27c42a:docs/VALIDATION.md`. Ignored raw captures it mentions
were deleted; hashes and summaries are the remaining record.

## Mac OLS: CMG request versus auto on sparse AKM, 2026-10-13

Johannes requested a simple four-worker comparison on 16 million rows. The
frozen current Mac plugin at source `863318969195360ff06b668983fbdfafb63a3550`
ran in Stata/MP 19 on Apple M2 Ultra (24 physical cores, 192 GiB RAM), with
four requested and effective native workers, Stata processors set to four,
`memory(fast)`, `deterministic`, `tolerance(1e-8)`, default iteration budget,
unweighted OLS and unadjusted covariance. The command was
`fereg y x1-x16, absorb(g1 g2) solver(auto|cmg)` with those common options.
No saved FE or prediction outputs were requested.

The input uses the [campaign sparse AKM topology](../bench/campaign.py) and
[maintained outcome/control generator](../bench/generate.py): 3.2 million
workers, five periods each, 160,000 firms, initial firm `worker_index % firms`,
and 2% independent random destination mobility at each transition. The seed
is 739391; NumPy 2.3.5 and pandas 2.2.3 generated 16 double controls and the
OLS outcome without the campaign's unused auxiliary columns. Actual firm
changes number 256,355 (2.0027734375% of transitions). The DTA round trip is
exact. Both options retain all 16 million rows, rank 16, absorbed DoF
3,353,066 and residual DoF 12,646,918, with no singleton removals.

The fixed panel used one full-size warm-up per option and three measured
pairs in alternating order, with a 1,800-second total process limit and no
continuation. It completed in 633.82 seconds. One Stata process held the same
data throughout. The independent native command clock includes frontend,
native loading, estimation, inference and publication; data generation, DTA
loading, result export and checks are outside timing. A separate 10,000-row
preflight passed before the full panel.

| Observation | Auto seconds | CMG request seconds | Order |
| --- | ---: | ---: | --- |
| Warm-up (excluded) | 15.953673458 | 145.567799458 | auto, cmg |
| Pair 1 | 15.745327708 | 140.126737708 | auto, cmg |
| Pair 2 | 15.520258583 | 140.647376667 | cmg, auto |
| Pair 3 | 15.765887875 | 142.607612458 | auto, cmg |
| Median | 15.745327708 | 140.647376667 | |

The median CMG-request time is 8.93264 times auto's, an additional
124.90205 seconds. The paired ratios are 8.89958, 9.06218 and 9.04533. Auto
reports the MAP-owned family (code 2), 115 main iterations and 14 final
residual iterations, without fallback. **Every CMG request reports main
solver code 4 (CG), fallback=true and one warning**, with 117 main iterations;
the final residual is already certified (CMG code 7, zero iterations).
This measures the public CMG request including fallback, not a successful
CMG-only solve. The diagnostic does not retain the warning text, so it does
not establish the underlying fallback cause. Median native-only time is
13.92084 seconds for auto and 138.82115 seconds for the CMG request; median
input-loading time is 1.21052 and 1.21755 seconds, respectively.

All eight calls pass the native rank, score, bread, covariance and overall
certificates. Auto/CMG maximum coefficient difference is `2.78961e-13`,
maximum covariance-entry difference `2.17615e-20`, and relative RSS difference
`1.02143e-12`, passing the existing campaign comparison tolerances. Within
each option all four calls have exactly equal coefficients, covariance,
fit scalars and certificate matrices. This is agreement and repeatability
evidence, not a new independent large-design oracle.

No competing builds or tests were launched by this benchmark. Desktop
activity remained, including Spotlight indexing at launch. These are
descriptive observations with three measured pairs, not quiet-host release
qualification or evidence for changing automatic routing. No production
code, artifact inputs or scientific rules changed for this comparison.

SHA-256 identities (raw data, logs and task copies removed after recording):

| Item | SHA-256 |
| --- | --- |
| Frozen plugin | `85dbf448ab1974122a47684ac9b9f17964b451109b267df409963200cdfbe9cb` |
| Numeric DTA input | `c0950ef8151bc8966cc0cb377fb4394b0c269b6cc7f25738836319c20ccb4835` |
| Frontend manifest | `370ced2082667158c5f3700e1c430e8592e1b55806d820b9e8cad109f57c6048` |
| Command-clock plugin | `31dabb8cb6e6081eb268fc48a4a0c12d9b4d64d815504e062c2b1a30cb0077bf` |
| Input-generation wrapper | `c312876aac556fe107839617a765c0eb7dd199da1f0263aa4d01575d29e70054` |
| Stata comparison do-file | `eb6840c16e6f204377868465923cf7246e939daf6d389d549f13085f8fba9533` |
| Bounded process launcher | `9fd3f01bd8c1f48c5892fd34c33655b6e34f7cda1f56f4fc4ecf9e9aed5dbb6f` |
| Result-analysis script | `a8372e26cd0d477a6eae7b21358e2fd5088b9eb6e1da51f70e454efa27ecda50` |
| Full Stata log | `d9a8c4731abedc96a0193c4485c193a4291f1040545e9c96a69f4efadee8a8ae` |
| All run matrices/scalars/times | `e71841a99f85b3f26ec8e0fe3061ea91537fdce7d1767da4f65bf3270216a440` |

The frontend manifest hashes sorted records of each file's SHA-256 and
path relative to the frozen `stata` directory. The retained current source,
generator recipe, individual observations and hashes identify the comparison;
ignored captures are not retained evidence. The documentation-only change
passes local reference checks and `git diff --check`, reusing the existing
plugin validation without rebuilding or rerunning the runtime suite.

## H9 adopted: science passes, Mac timing inconclusive, 2026-10-13

Johannes authorized adoption of the H9f strict-only design despite its failed
5% discovery-improvement gate. After reviewing the Mac and SCC timings, he
confirmed “OK, let's adopt it” on 2026-10-13, accepting integration with the
Mac timing limitation recorded below. This does not establish quiet Mac timing
qualification or a release-speed claim. The runtime now retains accepted
canonical geometry only for automatic two-FE PPML strict checkpoints within an
existing Fast reservation, with no saved-FE channels. The operator, RHS,
certification and final accepted-mean Hessian remain current; all tolerances,
memory allowances and protected model decisions remain unchanged. Ordinary
frames, explicit solver requests, Low mode and Hessian refresh keep their
existing solves. Independent source review clears the acceptance/rollback,
identity, memory and deterministic per-RHS cold-retry safeguards.

The parent is `2a7328f9f47910c51114ed8f527cc144571902cc`. Fresh universal Mac
baseline/candidate plugins have SHA-256 `cc1d1ba5c8cb7394` / `d0053c508a61984d`;
the [retained evidence](../bench/results/h9-20261013-integration.json) contains
full identities, all structured checks, task harness sources and every timing
observation. The [case-specific numerical scope](../tests/replay/scopes/2026-10-13-h9-strict-macos.json)
was frozen from input identities and baseline presence before candidate replay.

The independent diagnostic consolidation subsequently entered main as
`25015664d506f31e23960dbdf1ae0b74b53ed6b8`. All eight H9 source hashes, its staged
changes apart from the coordinated inventory, and its measured plugins remain
unchanged. The combined complexity inventory and full runtime suite pass
against that new parent: 1,765 Rust test invocations in 64 groups and 207 Python
tests, with the same 13 optional Python skips and Mac GNU ELF audit skip. The
measurements here retain their original artifact identities; they do not
establish timing of the combined diagnostic/H9 artifact.

The combined universal Mac plugin, SHA-256 `85dbf448ab1974122`, passes binary
audit and native ARM64 load/ABI/selftest checks. Its 73-case replay (219 calls)
is scientifically exact against the earlier H9 candidate and across repeats,
with all independent references passing. Six further actual-reuse witness calls
at one and sixteen workers also match that candidate exactly and pass the
known-optimum/dummy-covariance checks. Four Stata/MP 19 fixtures pass for
installation, the single runtime, public PPML scope and hidden PPML transactions.
These composition checks have independent review and separate artifact bindings
in the retained evidence. They do not resolve the Mac timing limitation below.

| Check | Result |
| --- | --- |
| Full runtime suite | Pass: 1,759 Rust test invocations; 207 Python tests (200 in the main suite), 13 existing skips; formatting, Clippy, allocation audits, portable/static, complexity, mock-SPI and numerical guards. Mac skips the GNU ELF audit. |
| Native Mac build | Both universal builds pass binary audit and native ARM64 load/ABI checks. No new Linux or Windows qualification claim. |
| Native replay | All 73 cases pass, including independent references, fixed-worker repeats and scientific/protected-decision checks at 2/4/8/16 workers: 803 calls. |
| Licensed Stata | All 17 PPML science, transaction, allocation and validator fixtures, plus private/public PPML Break rollback checks, pass. |
| Actual reuse | Isolated full-fit Rust witness and 15 native calls at 1/2/4/8/16 workers pass; all ten candidate/repeat fits consume certified accepted geometry. |
| Timing science | All 20 standard baseline/candidate pairs are exact, including binary output columns. All 320 later timing calls reproduce those fingerprints. |
| Mac nonregression | 19/20 cells meet the confidence bound. OLS K=20 at 16 workers is inconclusive; background syncing, indexing and backups resumed. No clean Mac qualification or release-speed claim. |

The native paired-score witness has analytically known coefficients and means,
plus independent explicit-dummy covariance. Maximum coefficient error is
`1.53e-16`, maximum scaled mean error `1.36e-15`; baseline/candidate coefficients
are exact and the largest absolute covariance difference is `8.47e-21`.
Protected sample, missingness, rank/omission, separation, outer/strict, fallback
and acceptance decisions remain exact. Fixed-worker scientific repeats are
exact; cross-worker science and protected decisions agree. Reuse consumes
generation-1 geometry at generation 5 with 27 iterations, 36 operator actions
and 27 inverse actions. The 132,120-byte cache and 232-byte ledger fit within
338,752 bytes of reported workspace and the existing 1,048,576-byte reservation.
Neither standard PPML timing case attempts reuse; the separate witness and
earlier SCC experiment establish actual reuse coverage.

The frozen timing panel uses one warm-up plus seven alternating pairs per cell,
finishing all 320 calls in 650.64 of 2,400 seconds. The inconclusive cell has
baseline/candidate medians 0.392249/0.401017 seconds and paired-median ratio
1.020701. Its 95% bootstrap interval is [1.014167, 1.079446], exceeding the 1.05
upper allowance. Retain every observation; no continuation was run. Host
sampling records renewed FileProvider/Spotlight activity and later backups.
The earlier SCC improvement failure remains unchanged and is not converted
into a performance pass by the adoption instruction.

Setup failures remain recorded: a parent build invoked from the wrong working
directory was discarded before fits; the licensed driver rejected a filename
before Stata ran; a bounded input reader and redundant `separation(none)` wire
helper were corrected, with one baseline request rejected before estimation.
After six valid native witness calls, an overly strict trace reader rejected
only the order of concurrent RHS diagnostics. Independent review confirmed all
fields and science exact. The repaired reader sorts only contiguous RHS blocks,
preserving attempt boundaries and multiplicity; it verifies the six preserved
outputs/events and runs only the nine originally unrun calls. No estimator,
input, tolerance or fit budget changed in that repair.

## H9 strict-only fits: partial performance gain, 2026-10-13

The authorized [strict-only complete-fit test](fable_performance.md#h9f-authorized-strict-only-complete-fit-prototype)
finds useful savings but fails its predeclared discovery gate. Dense 4M total
native time falls by 9.26% at four workers and 3.41% at sixteen workers; both
sixteen-worker pairs miss the required 5% improvement. All scientific,
repeatability, provenance, reuse and nonregression gates pass. This experiment
left production unchanged. Johannes later explicitly authorized the
[strict-only integration](fable_performance.md#h9g-authorized-production-integration)
despite the failed improvement gate; ordinary-frame reuse and A2 remain outside
that authorization.

| Frozen input | Workers | Baseline median s | Candidate median s | Median paired reduction |
| --- | ---: | ---: | ---: | ---: |
| Dense 1M | 1 | 9.890 | 9.464 | 4.31% |
| Dense 1M | 4 | 4.569 | 4.321 | 5.41% |
| Dense 1M | 16 | 3.046 | 3.019 | 0.90% |
| Dense 4M | 1 | 46.387 | 42.522 | 8.33% |
| Dense 4M | 4 | 20.924 | 18.987 | 9.26% |
| Dense 4M | 16 | 13.046 | 12.600 | 3.41% |

These are complete native fits, including producer retention and discarded
work. Each cell has two alternating baseline/candidate pairs. Dense 4M's
four-worker ratios are 0.907086/0.907753 and its sixteen-worker ratios are
0.970733/0.961038. Percentages use the median paired ratio, not the ratio of
arm medians. All twelve cells meet the max(5%, 1 ms) nonregression allowance;
the largest median increase is 2.76% for AKM at sixteen workers. The small
SCC panel is descriptive and does not establish a Mac release speed claim.

The strict phase explains the gain: dense 4M median strict time falls from
5.109 to 3.189 seconds at four workers, and from 2.497 to 2.118 seconds at
sixteen. Corresponding complete command medians fall from 22.525 to 20.589
and 14.651 to 14.223 seconds. Earlier cache-retention costs remain inside
whole-fit timing. A successful strict solve alone would not pass this gate.

All 48 target fits and 48 initialization fits finish, as do the two smoke fits
and their two initializations: 100 native calls, with no fit retries. All 24
paired scientific comparisons, fixed-worker same-arm repeats and cross-worker
checks pass. Published coefficients match exactly; the largest absolute
covariance difference is `8.47e-22`. Sample, rank/omission, separation,
missingness, outer/strict decisions and existing fallback routes remain exact.
AKM and sparse comparisons are exact on the unaffected scope. Native
certificates pass at unchanged tolerances. The benchmark request adds no
saved-FE or fitted-mean export; known-optimum, accepted-Hessian and independent
oracle checks supply separate witnesses for those mathematical obligations.

All twelve dense candidate fits consume accepted generation-1 geometry at the
generation-5 checkpoint. Normalized weight ratios remain 22.07/36.84, with
four/eight PCG iterations per RHS and 68/136 total inverse actions. Maximum
true current relative residual is `3.72e-15`, below the unchanged `1e-14` inner
target. Retained caches are 18.0/72.0 MB, and inclusive current workspace is
42.7/169.3 MB within the existing 160/640 MB reservations (decimal MB). No
memory budget or Fast/Low selection changes. Dense 4M observed estimator
process RSS rises by about 121–123 MB; this observation is distinct from
modeled solver workspace and scheduler virtual-memory accounting. The small
smoke exercises a bounded cold retry and still publishes identical results.

Rust 1.85.1 checks pass on Mac and SCC: 29 canonical tests, the isolated full-fit
reuse witness, 435 PPML tests, 19 session tests and ten independent oracle tests.
The PPML/session groups each retain one pre-existing ignored test. Linux native
build/load/ABI and input/output guards pass. Independent Astra review approves
the repaired prototype and harness, then independently reproduces the final
failed-gate decision. The task-only patch reproduces all eight tested source
hashes exactly; the experiment itself did not integrate it into the runtime.

Build jobs `8051255`/`8051343` have `failed=0`, `exit_status=0`. Smoke wrapper
`8051353` exits 1 after successful fits because the first reader used the
incomplete auxiliary trace file. Complete stderr traces already existed in
`process.log`; hash-bound reanalysis passes unchanged comparisons. A reviewed
reader repair preserves those fits and runs only the original unrun targets.
No numerical rule, solver artifact, timing gate or fit budget changes. There
are 47 passing analyzer tests after the repair. Target job `8051451` runs on
isolated `scc-wc4` with 28 physical cores, `failed=0`, `exit_status=0`, 971
scheduler seconds and `maxvmem=6.771G`. Its panel uses 958.773 of 2,400 allowed
seconds; the longest launcher uses 56.74 of 120 seconds.

The [compact evidence](../bench/results/h9-20261013-strict.json) retains every
record losslessly, all paired observations, hashes, schemas, executable harness
sources, accounting summaries and the failed attempt. Its embedded decoder
reconstructs the complete records and reproduces the frozen analysis. The
[task-only patch](../bench/results/h9-20261013-strict.patch) is against `86eaf5ef`.
At the end of this experiment production identity matched that revision and
`3d06d0466587` across 179 runtime modules and 58 artifact inputs. Production
integration follows Johannes's later explicit instruction, not a pass of the
failed discovery gate. No ordinary extension, resampling or retuning follows.

## H9 stale-preconditioner quality: promising absorption savings, 2026-10-13

Johannes authorized the [bounded quality test](fable_performance.md#h9e-authorized-quality-test-beyond-the-inventory-drift-bound)
after the inventory below. The actual generation-1 preconditioners work on
generation-5 strict systems despite normalized weight ratios of 22.07 and
36.84. All six cells pass, and every measured pair is faster with reuse.
These are canonical absorption observations, not complete-fit or Mac release
performance claims. Production remains identical to `3d06d0466587` across
179 runtime modules and 58 artifact inputs.

| Frozen input | Workers | Cold median s | Reuse median s | Median paired time reduction |
| --- | ---: | ---: | ---: | ---: |
| Dense 1M | 1 | 1.662 | 1.184 | 28.8% |
| Dense 1M | 4 | 0.567 | 0.362 | 34.8% |
| Dense 1M | 16 | 0.269 | 0.213 | 20.7% |
| Dense 4M | 1 | 8.567 | 5.084 | 40.7% |
| Dense 4M | 4 | 3.060 | 1.687 | 45.0% |
| Dense 4M | 16 | 1.090 | 0.844 | 23.2% |

Each cell has one excluded warm-up pair and three alternating measured pairs.
The percentage is one minus the median paired ratio, which can differ from
the ratio of separately reported arm medians. Dense 1M at sixteen workers is
the noisiest cell: its three ratios are 0.823, 0.659 and 0.793. These small
panels are descriptive; no confidence-bound qualification is claimed.

Both captures and replays ran on isolated `scc-zj4` allocations of 28 physical
cores, using the frozen 1/4/16-worker masks. The cold one-worker replay matches
each captured geometry, representation and reconstructed-output signature
exactly. Higher-worker candidates are compared with their own cold arm, and
every repeated same-arm signature is exact at fixed worker count. All 24
candidate comparisons pass: maximum column error is `7.50e-15` relative to
`max(1, cold reconstructed column norm)`, and maximum normalized Gram entry error is
`4.45e-16`. Native and additional original-input certificates pass at the
unchanged `1e-12` request; no recurrence or historical matrix certifies a result.

Dense 1M uses the old direct factor and takes four PCG iterations per RHS.
Dense 4M uses one fixed symmetric Gauss–Seidel inverse action and takes eight,
versus six cold explicit-CG iterations. Across 17 RHSs, the candidates perform
68/136 preconditioner actions and 85/153 current-operator applications,
including one diagnostic true-residual application per RHS. Maximum true
relative residuals are `2.30e-15` and `3.72e-15`, below the unchanged inner
target of `1e-14`. Avoiding assembly therefore outweighs the added solve work
in these observations; a reduction in iteration count is not the explanation.

Timing includes retention copying, current geometry, solving, the native
certificate and complete `write_dense` reconstruction. Extra independent
screening and comparisons are outside arm timings but inside the panel's
220.540-second deadline usage. Capture-only hashing and file output are disabled
in replay. Modeled solver workspace rises from 12.1/48.5 MB to 42.7/169.3 MB,
including retained caches of 18.0/72.0 MB, within the captured 160/640 MB
allowances. These are decimal MB and include small diagnostic report storage.
The replay harness's scheduler `maxvmem=4.317G` includes fixtures and preparation;
it is not an estimator memory claim. Full estimator lifetimes, rollback and
memory-limited fallback still require integration checks.

All eight native estimation calls finish: two smoke fits, two target captures
and four launcher initializations. Smoke coefficient/covariance tables, sample
hashes and all 26 non-timing metadata fields match. Provenance verifies accepted
producer and committed strict consumer, sample/topology/basis identities,
coordinates and hashes. All 48 absorption calls finish without decline,
timeout, retry or retuning. Rust 1.85.1 builds and 26 focused canonical tests
pass on Mac and SCC; 26 provenance-reader witnesses, native Linux ABI/load and
input/output guard checks pass. Independent Astra reviews clear the design and
the final observations.

SCC jobs `8050485`, `8050486` and `8050487` have `failed=0`, `exit_status=0`,
with 785/80/228 scheduler seconds. Initial build `8050478` exits 101 before
compilation because its wrapper used the wrong working directory; held
dependents `8050479`/`8050480` were canceled without execution. The narrow
directory repair changes no solver source or fit budget. All failed evidence
is retained by hash and summary.

The [compact evidence](../bench/results/h9-20261013-quality.json) retains every
pair, RHS work count, scientific observation, capture/artifact hash, frozen
plan and analysis programs. The [task-only patch](../bench/results/h9-20261013-quality.patch)
reproduces the experiment against the baseline. Patch applicability, local
references and `git diff --check` pass. Existing production runtime evidence
is reused; no new full runtime suite or platform qualification is claimed.
Large captures and task build copies are deleted at closure.

This result supports a strict-only complete-fit prototype with a newly frozen
validation and cost budget. It does not supply a new weight-ratio cutoff or
pass the earlier full-fit 5% discovery gate. At sixteen workers the observed
dense 4M absorption saving is about 0.25 seconds; it must not be presented as
a 23% whole-fit gain. Ordinary-frame reuse, A2 and production integration
remain outside this completed diagnostic.

## H9 preconditioner inventory: stopped at drift gate, 2026-10-13

Johannes approved the [H9 design](fable_performance.md#7-h9-reuse-preconditioners-at-exact-current-weights)
and narrow SPEC section 13 exception for historical preconditioners. The first
evidence gate fails: all twelve frozen PPML cells exceed the fixed cache-build
weight-ratio bound of 2. No runtime prototype or subsequent fit discovery was
started. Production identity remains exact against execution baseline
`3d06d0466587001a732efc2e3c4c175c89e4c485` across 179 runtime modules and 58
artifact inputs; scientific tolerances are unchanged.

The actual accepted canonical producer is from generation 1, rather than the
last ordinary IRLS frame. Intermediate ordinary frames update weights without
rebuilding canonical geometry. The table shows the first strict attempt;
ratios are rounded across the 1/4/16-worker observations.

| Frozen PPML input | Producer representation | Producer → strict weight generation | Normalized cache-build ratio |
| --- | --- | --- | ---: |
| AKM 1M | Implicit | 1 → 5 | 14.84236 |
| Dense 1M | Direct Cholesky | 1 → 5 | 22.06557 |
| Dense 4M | Explicit iterative matrix | 1 → 5 | 36.84239 |
| Sparse 1M | Implicit | 1 → 8 | 239.75427 |

All compared source codes, generated columns, components, anchors, free
coordinates, sample/topology generations and basis identities match exactly.
The drift failure alone blocks the dense consumers; implicit producers are
also excluded by this design. Sparse canonical construction declines at
generations 8 and 11, while the enclosing strict fallback frames commit
successfully. The first decline clears simulated optional cache state, leaving
no accepted producer for the second strict canonical attempt. There is no
observed strict rollback. Overall, none of the fifteen target strict canonical
attempts is eligible before memory admission. Cache payload lower bounds are
retained, but complete overlap admission, inverse-action quality and PCG
convergence remain unmeasured.

SCC build job `8050148` and inventory job `8050149` both have `failed=0` and
`exit_status=0` (488 and 239 scheduler seconds). Both Linux plugins pass native
ABI/load and input/output guard checks; all 21 focused canonical tests pass.
Two 1,200-row baseline/diagnostic checks agree exactly in coefficients,
covariance, sample hash and selected scalar metadata. Twelve target fits finish
in 228.416 seconds within the declared 900-second budget and 120-second per-fit
cap. Including two smoke and fourteen launcher initialization fits, all 28
native calls complete without retry. Frozen source, plugins, all seven input/
clock artifacts and capture hashes are verified. The panel used `scc-zj4`,
28 allocated physical cores, with the declared 1/4/16-worker masks.

Diagnostic clocks include snapshot overhead and establish no speed claim.
For context, dense 4M's sixteen-worker strict phase is 2.295 seconds, including
0.492 seconds of reduced assembly and 0.007 seconds of matrix packing; the
snapshot itself takes 0.150 seconds. Total native time is 11.606 seconds.
These single instrumented observations cannot be pooled with the older A3 or
H3 panels or used as clean qualification timings.

Independent Astra review found and repaired analyzer generation, invalidation
and snapshot-integrity gaps before analysis. The repaired reader passes 24
synthetic witnesses and verifies every captured file's manifest membership,
size, SHA-256 and binary layout. The held SCC analysis job `8050157` remained
queued after release and was canceled without execution; the same reviewed
analyzer completed on Mac in 0.764 seconds after collection. Two earlier local
diagnostic compilation setup failures (missing Cargo on PATH, then an incorrect
generation accessor) were repaired before deployment. The smoke launcher's
two deletion-count keys were absent from its metadata schema; a retained
post-capture audit verifies all 26 actual non-timing fields exactly, including
`num_singletons` and `separated`, without rerunning a fit. No failed fit is omitted.
Final independent review confirms the stop decision and the successful sparse
fallback interpretation.

The [compact evidence](../bench/results/h9-20261013-inventory.json) retains every
observation, scientific table, normalized H9 event, comparison, snapshot hash,
artifact identity and analysis program. The [diagnostic patch](../bench/results/h9-20261013-inventory.patch)
reproduces the observation hooks against the frozen baseline; it is not a
production change. Local references, patch applicability and `git diff --check`
pass. Existing baseline runtime evidence is reused under the documentation-only
rule; no new full Mac runtime suite, Mac timing qualification or Windows check
is claimed. Task captures and build copies are removed at closure.

The inventory stops this bounded design. Preconditioner quality at the observed
larger drift, a paid later producer refresh, or another general admission rule
would need a revised proposal and budget. Ordinary-frame reuse and A2 cannot
advance on the failed strict prerequisite. Certificate fusion remains separate:
the canonical certificate is already fused, while general FE projection sums
depend on prior norm scales and some group means require a scaled fallback.
No certificate implementation changed in this inventory.

## Adaptive two-FE OLS: accepted tradeoff and Mac qualification, 2026-10-13

Johannes accepted the adaptive version after reviewing the accuracy and work
tradeoffs. Ordinary automatic two-FE OLS now targets half the requested tolerance
first and attempts one tighter residual correction only after a failed requested
certificate. Original RHS normalization, cumulative PCG/MAP limits and retained
FE reconstruction are preserved. Explicit solvers, PPML and final-residual targets
are unchanged. Independent Astra review found no implementation blocker.

The initial candidate failed the retained H8 default projection witness:
9.843576231904905e-9 maximum absolute error against 5e-9, despite passing the FE
certificate. The first focused run passed 92/93 tests; the first full suite stopped
at that same failure after 1,235 passing core tests. Johannes explicitly accepted
this disclosed tradeoff on October 13. The default witness now permits 1e-8 and
also requires exact agreement with the passing half-request route. The strict
1e-14 request retains its 5e-9 bound (observed error 3.615219235086897e-11); the
separate 5e-11 projection witness and all other scientific bounds are unchanged.

Final validation passes: all 93 focused tests and `./scripts/run_tests.sh`
(1,749 Rust test executions across 64 invocations, tooling, allocation audits,
portable and bridge checks). The unchanged candidate artifact reuses its passing
universal Mac build/load evidence, 128 independent oracle tests, 73-case parent
replay with zero exact or scoped differences, all 292 worker checks and all 17
mapped licensed OLS/PPML fixtures. Linux-only ELF checks are skipped on Mac; no
new Linux or Windows qualification follows. Baseline runtime is `b4b4f6d3`,
with subsequent prose-only parent `dd6329d6`; artifact hashes and individual
observations are in the [compact evidence](../bench/results/h8-adaptive-20261012.json).

All four Manu input files match the retained H8 SHA-256 values. The bounded Mac
panel completes 48 licensed fits: two alternating pairs per input at 1/4/16
workers, robust VCE and 16 controls. All 24 paired numerical/discrete checks,
24 fixed-worker repeat checks and 16 cross-worker comparisons pass. Native changes
below are paired geometric-mean percentages; tracing is enabled and two pairs
are descriptive, not confidence-qualified speedup claims.

| Frozen input | 1 worker | 4 workers | 16 workers | Main/final iterations, H8 → adaptive |
| --- | ---: | ---: | ---: | --- |
| AKM 1M | -6.27% | -8.38% | -6.50% | 18/0 → 14/0 |
| Dense 1M | +0.14% | +3.40% | -1.66% | 1/0 → 1/0 |
| Dense 4M | +0.05% | +3.10% | -0.58% | 1/0 → 1/0 |
| Sparse 1M | -10.86% | -11.12% | -10.43% | 120/0 → 92/11 |

The stable candidate then passes all twenty frozen quiet Mac timing workloads:
one warmup and seven alternating pairs, 320 native calls, 629.483 seconds, no
retry or continuation. Every paired-median bootstrap upper bound lies within
max(5%, 1 ms); the largest ratio upper bound is 1.028886. Every plugin repeats
its scientific results and solver signatures exactly. Cross-plugin protected
matrix fields are exact and coefficient/covariance agreement passes; affected
row-output witnesses come from scoped replay and Manu/F1 checks. Unaffected
no-FE, three-FE and PPML timing results are exactly identical across plugins.
There are no competing builds, tests or agent compute during either panel;
Activity Monitor is temporarily paused with a recovery guard and restored.
This is incremental qualification, not a retrospective pass for the older
whole-H boundary. Workspace-capacity and allocator tests pass; no new row-by-RHS
bank is introduced, and no new observed-RSS claim is made.

The retained F1 inputs still pass ordinary full-dummy QR and protected-decision
checks; their 80-digit strict F witness remains failed. Weak changes from H8's
1.076 times the unchanged bound to 595.049 times (absolute F error about 1.44e-5);
barbell changes from 2.480 to 2.465 times. Weak main/final iterations change
140/20 to 138/137. These diagnostics, disclosed before approval, show why early
acceptance is neither a stronger accuracy certificate nor a universal saving.

## H2 follow-up: certified dense three-FE gains, 2026-10-12

The second reopened H2 candidate passes incremental Mac qualification against
`be357256` and is adopted. Automatic unit-weight, symmetric three-FE OLS can
use a geometry-qualified MAP attempt, accepted only by the unchanged requested
certificate. Exact marginal saturation avoids unnecessary sparse pair scans;
integer moments accumulate once per occupied cell. Complete geometry predicts
a sweep at the current recurrence target. Checking the certificate there saves
the fourth sweep on the measured dense inputs. Initial, configured-cadence and
fourth-sweep checks remain; a finite miss continues and a failed pilot falls
back on untouched inputs within the shared work and memory budget.

Fifteen alternating measured pairs after one warmup per cell, tracing disabled,
complete 192 licensed Stata fits in 1,041.614 seconds. The maintained 10,000-draw
paired-median bootstrap (seed 739391) uses its 95% upper bound against
max(5%, 1 ms). All six cells pass nonregression. Dense 1M native reductions are
13.05%/6.46% at 1/4 workers; dense 4M reductions are 37.38%/32.31%.
Command reductions are 12.21%/5.72% and 35.80%/29.34%, respectively.
Percentages use paired ratios, so they need not equal ratios of marginal medians.

| Frozen input | Workers | Native median seconds | Paired native ratio [95% interval] | Command median seconds | Command reduction |
| --- | ---: | --- | --- | --- | ---: |
| dense-1m | 1 | 1.915437 → 1.667007 | 0.86953 [0.86871, 0.87241] | 2.049060 → 1.799667 | 12.21% |
| dense-1m | 4 | 0.664414 → 0.625105 | 0.93542 [0.92980, 0.94204] | 0.769980 → 0.730213 | 5.72% |
| dense-4m | 1 | 11.224951 → 7.032553 | 0.62620 [0.62381, 0.62903] | 11.737197 → 7.533507 | 35.80% |
| dense-4m | 4 | 3.579673 → 2.425196 | 0.67690 [0.67539, 0.68378] | 3.982470 → 2.816806 | 29.34% |
| sparse-1m | 1 | 6.346304 → 6.345033 | 1.00016 [0.99775, 1.00229] | 6.478861 → 6.477881 | -0.02% |
| sparse-1m | 4 | 1.886580 → 1.891642 | 1.00481 [1.00041, 1.01097] | 1.992559 → 1.998368 | -0.47% |

The twenty standard workloads also pass: two fixed fifteen-pair blocks,
warmup in each, combined once into thirty pairs per cell (1,280 native calls,
2,564.605 seconds within the 3,600-second total and 1,800-second block limits).
All captured science is exact across both plugins and both blocks. The largest
observed standard cost is wide64/four workers: 1.82%, interval [0.46%, 3.28%].
Sparse 1M/four workers costs 0.48%, interval [0.04%, 1.10%]. These are retained
costs inside the allowance. No observations were dropped, fits retried or
panels continued. Frozen plugin/source/input hashes were checked. The Mac ran
without competing development builds, tests or agent tools; ordinary application
activity remained. Retained preflight snapshots are not continuous monitoring.

The numerical scope covers successful automatic three-FE results and solver
iteration diagnostics. Sample, rank, omission, separation and requested tolerance
are unchanged. Explicit solvers, weighted routes, PPML and final residual
projection retain their policies. All large-case pairs pass coefficient and
covariance agreement; each artifact repeats captured science and solver
signatures exactly at fixed workers. Sparse captured science remains exact
across artifacts. Cross-worker science and discrete solver decisions pass at
1/4 on Mac and 1/4/16 on Linux. This does not strengthen the requested certificate
or resolve the separate strong-F question recorded under H8.

`./scripts/run_tests.sh` passes: 1,746 Rust test executions in 64 successful
invocations, plus the required tooling, allocation, FFI and complexity checks.
Fourteen Rust test executions are ignored and thirteen optional Python checks
are skipped because the local Python lacks NumPy/SciPy; these are not claimed
as passes. Universal Mac build, both binary slices, actual ABI/load guards,
73-case replay with zero exact parent differences and all 292 worker checks,
and 26 mapped licensed Stata fixtures pass. New witnesses include independent
integer pair moments, eight pinned F2 geometries, generated designs with every
FE order, dense dummy-QR robust OLS, saved-FE reconstruction, Fast/Low memory,
parallel repeats, Break, allocation, consumed-work accounting and fresh norms
and coefficients after fallback. A finite predicted-checkpoint miss reaches a
fourth-sweep pass with exact residual/coefficient agreement. Independent
high-effort Astra reviews found no blocking defect. Production complexity rises
by 449 core lines and eleven dispatch candidates; the scoped ratchet records why.

Before qualification, a fixed sixteen-fit profile against candidate 1 confirms
four → three main sweeps and native reductions of 13.95%/11.73% at 1M/4M with
four workers. This trace-enabled profile is diagnostic, not pooled with release
timings. An artificial 256-row failed-pilot witness retains the added-check cost:
median 66.26 → 71.21 microseconds per attempt (five alternating pairs of 1,000
attempts in optimized CI). Geometry normally declines that input. Temporary
profiling code was removed before the final full suite and artifact freeze.

The bounded Linux check also completes: setup `8044213` on `scc-tb4` and probe
`8044217` on `scc-zj4`, both failed=0/exit=0. Linux x86-64 binary/ABI/load guards,
thirteen focused core tests and six independent oracle tests pass. The 36-fit,
two-pair-per-cell trace-enabled panel passes science and repeats. Dense paired
native reductions are 13.23%/11.48%/10.64% at 1M and 34.23%/22.82%/20.88% at
4M for 1/4/16 workers. Sparse geometry is 6.17–6.33 ms on this host. However,
sparse at sixteen workers costs 15.08% (individual paired costs 10.01% and
20.38%); its mean native times are 2.502 → 2.888 seconds. That unfavorable
observation remains unresolved: it is neither dismissed as noise nor used to
claim confidence-qualified Linux performance. No new Windows qualification or
fixest comparison follows from this work.

Candidate 1 remains **not adopted**: cheaper geometry alone gave dense 4M
native gains of 25.97%/23.17%, but dense 1M/four workers regressed 6.85%
(interval [1.05489, 1.07918]) and the standard wide129/four-worker cell remained
inconclusive. Its 54-fit SCC panel and independent checks passed. Its source
and observations are retained separately; none are relabeled as candidate 2
evidence. The original H2 29.8–30.9 ms sparse failure against 21 ms also remains
failed. This reopening supports a new incremental Mac gain, not a retrospective
pass for the earlier H-wide inconclusive cell.

Evidence: [candidate 1 record](../bench/results/h2-followup-20261012-first.json),
[its checked source patch](../bench/results/h2-followup-20261012-first.patch), and
[candidate 2 record](../bench/results/h2-followup-20261012-second.json).
Every warmup and measured timing is retained there. Raw captures, frozen source,
plugins, drivers, logs and exports are preserved on SCC, with remote hashes
verified before local cleanup:

- `/projectnb/welfgr/fereg/runs/h2-followup-20261012/mac-first-captures.tar.gz`,
  SHA-256 `9abcff312dc0b15c82c2b5d1a152c326e946564ab956027721c0077af0d041c1`.
- `/projectnb/welfgr/fereg/runs/h2-followup-20261012/second/mac-second-captures.tar.gz`,
  SHA-256 `9e30a10d742a8591a779e93498596db57c8e9639636be9407dfc6e97f3d925c4`.

Baseline Mac plugin:
`a97f0c28ec913a6b26b4f1a2388eb95688a60479f402d1bcb57d4cdcfc80d1ce`.
Adopted Mac plugin:
`545bebd0ad51354e17403f5e764b69aadd65232acd8d900eebbf5739bb1d175b`.
Frozen source archive and manifest hashes, input/driver hashes, all SCC
observations and cross-worker checks are in the compact record. Dataset bytes
remain in their original SCC campaign; standard cases are regenerated from the
pinned specification and hash-verified. No ignored local path is retained evidence.

## H complete workstream Mac boundary, 2026-10-12

The final H6 plugin was frozen after runtime/native/replay/licensed qualification
and compared with the retained pre-H3 universal Mac baseline on the twenty
regenerated, hash-verified standard workloads. The host was coordinated quiet;
no agent tools, builds or tests competed with native timing. Seven alternating
measured pairs followed one excluded warm-up pair per workload: 320 native
calls, completed in 643.298 seconds within the 1,800-second panel
and 90-second process limits. Plugin, harness and input hashes match before
and after. No continuation, retry, tuning or selected resampling occurred.

The locked Mac prevented UI coordination. A bounded process pause stopped
Activity Monitor's sampling without terminating it; its original process
resumed automatically after the panel. Post-restoration Spotlight indexing
settled before launch. Prelaunch checks and pre/post CPU deltas are retained;
the highest surviving background process averaged 5.20% of one core on this
24-core Mac. This is snapshot evidence, not continuous host monitoring.

Nineteen workloads pass nonregression; one remains inconclusive. No cell
meets the declared criterion for a material regression. The whole-H timing
gate does not pass; no overall release-performance claim follows.

| Frozen workload | Baseline ms | Final H ms | Paired ratio [95% interval] | Result |
| --- | ---: | ---: | --- | --- |
| ols-no-fe-k5-t1 | 5.406 | 5.422 | 1.0056 [0.9846, 1.0613] | nonregression |
| ols-no-fe-k5-t4 | 4.948 | 4.847 | 0.9768 [0.9537, 0.9957] | nonregression |
| ols-three-unit-k5-t1 | 597.467 | 592.861 | 0.9898 [0.9859, 0.9937] | nonregression |
| ols-three-unit-k5-t4 | 300.353 | 296.408 | 0.9888 [0.9835, 0.9899] | nonregression |
| ols-two-pw-k5-t1 | 616.578 | 620.980 | 1.0043 [1.0003, 1.0205] | nonregression |
| ols-two-pw-k5-t4 | 345.114 | 344.917 | 1.0011 [0.9916, 1.0076] | nonregression |
| ols-two-unit-k20-t1 | 1080.894 | 1073.602 | 0.9943 [0.9914, 1.0038] | nonregression |
| ols-two-unit-k20-t16 | 401.055 | 384.896 | 0.9636 [0.9578, 0.9735] | nonregression |
| ols-two-unit-k20-t4 | 515.078 | 503.198 | 0.9794 [0.9735, 0.9831] | nonregression |
| ols-two-unit-k20-t8 | 431.148 | 417.789 | 0.9663 [0.9634, 0.9763] | nonregression |
| ols-two-unit-k5-t1 | 453.701 | 441.698 | 0.9774 [0.9692, 0.9805] | nonregression |
| ols-two-unit-k5-t4 | 234.829 | 225.833 | 0.9599 [0.9568, 0.9652] | nonregression |
| ppml-two-unit-k5-t1 | 427.760 | 424.873 | 0.9942 [0.9930, 0.9974] | nonregression |
| ppml-two-unit-k5-t4 | 233.805 | 234.139 | 1.0007 [0.9990, 1.0023] | nonregression |
| ols-wide-k129-t1 | 746.397 | 759.753 | 0.9945 [0.9810, 1.0424] | nonregression |
| ols-wide-k129-t4 | 379.652 | 396.701 | 1.0386 [0.9811, 1.0593] | inconclusive |
| ols-wide-k64-t1 | 292.702 | 295.491 | 1.0112 [0.9982, 1.0247] | nonregression |
| ols-wide-k64-t4 | 162.367 | 164.951 | 1.0068 [1.0001, 1.0443] | nonregression |
| ols-wide-k65-t1 | 305.234 | 311.299 | 1.0207 [1.0096, 1.0495] | nonregression |
| ols-wide-k65-t4 | 170.262 | 171.271 | 1.0112 [1.0087, 1.0238] | nonregression |

The inconclusive cell is 129-column OLS at four workers: baseline/final
medians are 379.652/396.701 ms, with a paired ratio of 1.038577 and 95% interval
[0.981125, 1.059293]. Its upper bound exceeds the 1.05 allowance. This does
not establish a material regression or host noise. The bounded panel is
closed; a new experiment requires a new declaration, not a replacement run.

The unchanged rule uses a 10,000-draw paired-median bootstrap with seed 739391
and its 95% upper bound against max(5%, 1 ms). A lower bound above that allowance
is reported as a material regression; a straddling interval is inconclusive,
never a pass. Every measured and warm-up observation remains in the compact
record. Gains below the allowance are observations rather than failures.

Each plugin repeats science and solver signatures exactly on all twenty timing
workloads. Cross-plugin qualification uses the separate 73-case replay at
2/4/8/16 workers and the predeclared H8 recurrence scope: all 292 worker checks
pass, with exact unaffected fields and discrete decisions. This is distinct
from claiming cross-plugin bitwise identity on the twenty timing workloads.
H1 refuted the locality hypothesis and introduced no runtime change. H2 was
not integrated because sparse geometry exceeded its 21-ms cap. H4 left overlap
unchanged; H5 changes campaign placement only. H3, H8 and preserving H6 slices
are the integrated runtime work.

Baseline plugin: `9ced317e7cb995510995490e76dd492b491ee7a830a7147884ac5b76c73e38c8`.
Final plugin: `a97f0c28ec913a6b26b4f1a2388eb95688a60479f402d1bcb57d4cdcfc80d1ce`.
Evidence source commit: `da1ab207abf19e201b1c9f8ac33ab9a5fe2c61f6`;
frozen runtime source: `53337e65d625edeb8167391aa233d78dd71900f5`.
Production identity remains exact across 177 modules and 58 artifact inputs.
Recalculation from stored observations reproduces all twenty reported medians,
intervals and verdicts. The prior 1,726 Rust test executions, 73-case replay,
292 worker checks and 25 licensed transaction checks remain valid unchanged.
Raw artifacts, inputs, host snapshots and observations remain in
`/projectnb/welfgr/fereg/runs/h-20261013-boundary/boundary-validation.tar.gz`
(1,323,733,201 bytes; SHA-256 `d75bd2be4a9d4bf4c43dc67f8b5204ca123ba18695ffd5a0fd2fb8b2a6d719fe`).
The [compact record](../bench/results/h-20261013-boundary.json) retains the
panel and scoped replay. No Windows performance claim follows.

## H final Mac boundary prerequisites, 2026-10-12

The final H6 universal Mac plugin
`a97f0c28ec913a6b26b4f1a2388eb95688a60479f402d1bcb57d4cdcfc80d1ce`
passes the pre-H3 baseline
`9ced317e7cb995510995490e76dd492b491ee7a830a7147884ac5b76c73e38c8`
comparison on all 73 replay cases and 292 checks at 2/4/8/16 workers. The
pinned H8 affected scope and intentional-change policy are verified. Exact,
intentional, scientific, repeated-run and discrete difference lists are empty.
The result reuses the independently witnessed H8 declaration; it does not
replace the retained weak-graph diagnostic or claim stronger certification.

All twenty standard timing inputs match the frozen manifest. Independent
review checked the launch and reporting tools, 320-call count, artifact
identities, per-plugin repeat rule and unchanged paired-bootstrap allowance.
Before any timing, it found and repaired a missing launch check for scope and
per-case worker identities. The final plan binds those prerequisites and keeps
`quiet_host_confirmed=false` in that historical capture. Activity Monitor was
still using CPU after correctness checks. No timing observation existed at
that stage. The subsequent completed panel is recorded in
[the final result](#h-complete-workstream-mac-boundary-2026-10-12); it leaves
one cell inconclusive and establishes no whole-H timing pass.

The prepared panel remains seven alternating measured pairs plus one excluded
warm-up pair per workload, 90 seconds per process, 1,800 seconds total, with
no retry or continuation. Those exact artifacts were later restored and
hash-checked, and the completed scientific evidence was reused unchanged for
the final panel. The original pending record remains historical evidence.

Raw plugins, frozen inputs, replay captures, host snapshots, reviewed tools and
plan are retained in
`/projectnb/welfgr/fereg/runs/h-20261013-boundary/prerequisites.tar.gz`
(1,423,154,235 bytes; SHA-256
`009d6a83ae59301aa3f674a202b6940e94fb4b16586be7102b7a24b5a470f87c`).
The [compact record](../bench/results/h-20261013-boundary-prerequisites.json)
uses archive-relative identifiers and records that timing was pending at capture.

## H6 finalized FE numeric-column release, 2026-10-12

After final owned-OLS singleton processing, reencoding, rebasing and cluster
certification, redundant FE f64 columns can be released. A sealed source-indexed
bank holds raw vectors or privately proved encoded slots. No numeric values are
reconstructed and no empty slice impersonates a released source. Original
request roles and finalized bindings retain ordinary sources (including omitted
terms), slopes, interactions and clusters. The dependent outcome has its own
owner and remains intact. Raw public samples and PPML retain their contracts.

The optional conversion proves exact canonical raw/code correspondence. Failure
keeps the raw owner and original later diagnostic. Both requested and actual
slot capacity must fit while the full old owner remains live, under a ceiling
that includes metadata, loaded RHS and the full concurrent moment workspace.
The physical result must not exceed original ownership. A separate immutable
original-owner byte scalar preserves execution, output and recovery admissions,
including Fast/Low choice, FE retention and solver budgets. No scientific bits,
solver signatures, tolerance, certificate, sample, rank, omission, separation,
mandatory diagnostic, estimate acceptance or publication behavior changes.

All 37 focused executions pass: eight core release witnesses, two plugin
admission/overlap witnesses, thirteen PPML generation tests, twelve RHS tests
the moment-budget boundary and the finalized owner layout witness. They cover pointer/capacity identity, exact
savings, signed zero, malformed correspondence and diagnostic order,
exact/short/overcapacity conversion admission, singleton reencoding, protected
source roles, outcome ownership, Fast/Low, workers and finite budgets.
Independent high-effort static review approves all 18 transferred file hashes.
It assessed the concurrent finite ceiling statically; packed/unpacked overlap,
historical finite budgets and descriptor limits have separate focused witnesses.

The full suite passes 1,726 Rust executions in 62 invocations and 200 Python
tests with 13 optional dependency skips. Universal Mac native ABI/load/guard
checks, 73 exact plugin replay cases, 292 worker checks and 25 licensed
preparation, precedence, singleton, cluster, FE recovery, weight, memory and
transaction fixtures pass. Linux ELF checks are appropriately skipped on Mac;
H7 will build and load this source on Linux. No new Windows claim is made.

Seven unsuccessful attempts are preserved: empty-array access and existing sample
adapters, a private fit-field fixture, the missed raw-vector topology consumer,
a plugin fixture's core-private matrix method, an incorrect small-sample
refusal expectation, and a raw-versus-finalized struct-size assertion. The final small-sample witness uses actual byte savings
without a row-count fence. Two early checks (wrong working directory and a
shared Cargo fingerprint) do not validate the isolated edits; transferred sources
were explicitly touched before full qualification. A Python 3.9 extraction
failure occurred before writing files and was repaired with the pinned modern
interpreter. No failed observation was discarded or tolerance weakened.

Three exclusive FEs remove approximately 24N bytes of numeric capacity, less
descriptor expansion. Costs are the exact correspondence scans and temporary
slot descriptors; historical admission charges intentionally remain unchanged.
Core adds 371 production lines and three dispatch sites; plugin adds 27
lines. No new file/function size or chunk violations. Whole-fit timing is deferred
to the single final H panel, so no wall-time improvement is asserted here.

Candidate plugin SHA-256:
`a97f0c28ec913a6b26b4f1a2388eb95688a60479f402d1bcb57d4cdcfc80d1ce`;
parent plugin: `d5e240f6da750a6b104571e4fdcb13efb429613117dbd7391d852d7e1a20ae75`.
Raw source, plugins, review, failed attempts and checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/release-validation.tar.gz`
(45,457,009 bytes, SHA-256
`7ab82792cab15597d7de1fb7f3440e08e108a04f8967330429df7fac4687a680`).
The [compact record](../bench/results/h-20261013-h6-fe-columns.json) binds all
qualification identities.

## H7 small-input fixed-cost check, 2026-10-12

One isolated SCC node compared final H6 source `53337e65d625`
with the retained October 10 `e29164e6` plugin through the Stata campaign
entrypoint, sharing the current frontend, frozen 100k AKM inputs and four
socket-local physical CPUs with matching memory binding. The panel used one
excluded warm-up pair and seven alternating measured pairs per OLS/PPML
model. All 28 measured fits, four warm-up fits and four 1,200-row smoke fits
completed; the existing process initialization adds 36 small fits, for 72
estimator invocations total. The declared 120-second process and 900-second
panel limits were unchanged, with no retry, continuation or tuning.

| Cell | October 10 seconds | Current seconds | Paired ratio [95% interval] | Verdict |
| --- | ---: | ---: | --- | --- |
| OLS command | 0.169399 | 0.167542 | 0.9937 [0.8375, 1.0917] | inconclusive |
| OLS runtime total | 0.151418 | 0.149957 | 0.9990 [0.8194, 1.1022] | inconclusive |
| PPML command | 0.594051 | 0.501664 | 0.8453 [0.8321, 0.8506] | nonregression |
| PPML runtime total | 0.557795 | 0.466115 | 0.8357 [0.8195, 0.8439] | nonregression |

The paired median bootstrap used 10,000 draws, seed 739391 and the declared
95% interval and max(5%, 1 ms) allowance. Individual observations, all phase
clocks, solver signatures and differences remain in the compact record. The
comparison includes all intervening changes, including H8; it cannot isolate
C1/C2/E1 or independently establish the historical host-noise cause. An
inconclusive interval is not a nonregression pass.

The existing independent smoke oracle passes all four fits. All sixteen
paired scientific comparisons, including warm-ups, pass unchanged campaign
tolerances, with exact sample/rank/omission/singleton/separation/DoF metadata.
Both plugins repeat their scientific results and clock-stripped solver
signatures exactly at fixed workers. OLS recurrence iterations increase from
13 to 16 under H8; its diagnostic residuals decrease. PPML solver signatures
are unchanged. These diagnostics do not isolate H8's timing contribution. Native Linux audits, the campaign harness
checks, runtime/input hash checks and output completeness checks pass. The harness suite ran 42 checks with one Rscript/fixest availability skip
in this Stata-only setup. The
draft harness received independent static review; its four-slot smoke CPU
selection and complete runtime hashing were corrected before launch. This is
a Linux diagnostic, not Mac performance or Windows qualification.

The original postprocessor stopped on Stata's missing optional
`time_separation_mu` clock in both PPML arms. Its retained replacement keeps
all seven values per arm explicitly unavailable; primary clocks stay mandatory
and the comparison/bootstrap function is unchanged. Ten focused analysis
checks and independent hash-bound review pass. No captured fit was repeated.
A premature accounting-log read and initial scheduler-accounting delay are
retained as collection failures; both final accounting records are complete.

Setup job `8039240` on `scc-tb4.scc.bu.edu` used four
slots for 181 seconds, maximum virtual memory
3.469G. Panel job `8039241` on
`scc-wc4.scc.bu.edu` used 28 slots for 40
seconds, maximum virtual memory 1.498G. Both have
`failed=0`, `exit_status=0`; complete placement, THP and accounting are retained.
Current plugin SHA-256: `78b9bb70ab8438597f1c9414d367cbbc828e6362ec86e952f7022b3f071fd893`;
October 10 plugin: `57cc9789efd6ecc810d6dd82c55822dac3bd537bd7be32bb7a4bcd82e2a574a1`.
Raw inputs, sources, captures, observations and accounting remain under
`/projectnb/welfgr/fereg/runs/h-20261013-h7`; the
[compact record](../bench/results/h-20261013-h7.json) binds their hashes.
No production code or scientific rule was changed by H7.

## H6 dual-order LSMR norm handoff, 2026-10-12

H6 item 7 shares each weighted residual read between the original fixed
512-row certificate leaves (merged in ascending order) and the uninterrupted
row-order dense OLS scaled norm. The two states remain separate; neither
consumer changes its arithmetic. Bounded stack column blocks use the installed
team policy. Private initial prepared-OLS provenance, Fast mode and the matching
normalized weight scale enable the handoff for retained and automatic LSMR.
Low, PPML, raw frames, final residuals and continuations retain their prior
paths; existing MAP norm handoff is unchanged.

The single-use certificate workspace lends its initially unused RHS scales,
then its dead reference-norm bank. Original RHS scale checks and group-certificate
folds retain their exact order. Only after all original coefficient allocations
and actual-capacity admissions can a passing certificate transfer that bank.
No extra heap bank or mandatory admission precedes the original checks.
Dense-only norm invalidity disables the cache and preserves the caller's old
error path. Failed LSMR certificates discard it before unchanged continuation.
Tolerances, certificates, scientific results, solver signatures, sample, rank,
omission, separation, publication and rollback remain exact.

All 13 focused executions pass. Independent retained recurrences cover widths
1/6/17/65, leaf boundaries, zero/subnormal/extreme values, nonuniform weights,
worker counts and invalid-input precedence. Other witnesses cover private frame
provenance, normalized-scale mismatch, workspace pointer/capacity, original
RHS-scale errors, complete fits, automatic/retained LSMR with and without
coefficients, finite budgets and MAP continuation. Candidate and comparator
are checked at identical budgets: automatic routing can legitimately use MAP
at or below the observed LSMR peak, or after a failed LSMR certificate.
The cache never determines that choice.

Workspace Clippy/formatting and independent high-effort architecture review
pass. The full suite passes 1,714 Rust executions in 62 invocations and
200 Python tests with 13 optional dependency skips. Universal Mac native
ABI/load/guard checks, 73 exact plugin replay cases, 292 worker checks and
twenty licensed preparation, FE, LSMR, weight, memory, saved-output and
transaction fixtures pass. The expected Linux-ELF audit skip remains on Mac;
the Linux diagnostic kernel and one focused workspace smoke pass, but they
do not establish Linux plugin qualification. There is no new Windows check.

Seven unsuccessful fixture/migration attempts remain in raw evidence: direct
call adapters and moved access, a reference-module path, then five overly
specific expectations about rejection or continuation/norm-bank behavior.
They did not cause a production solver, certificate or diagnostic-policy repair.
Collection also corrected a parser that omitted the first timing line's Rust
test prefix and a mismatched review-schema key in the record writer. The
original raw timing log and failed writer are retained; no checks or timings
were repeated to obtain a pass.
Core adds 218 production lines and two dispatch sites; plugin production
and size-violation counts are unchanged. FE numeric-column release remains
separate work.

The single declared diagnostic panel on an isolated SCC node completed 24 cells
and 72 measured pairs, checking both norm banks bitwise after every warm-up and measured pair.
Paired median fused/separate ratios range from 0.7406
(width 1, 8 workers, unit weights) to 1.1910
(width 65, 4 workers, unit weights). All cells and their three-pair ranges remain in the compact
record. Six columns at four workers cost 12.77%/12.13% more for unit/nonuniform
weights; 65 columns cost 19.10%/16.76% more at four workers and 14.98%/15.19%
more at eight. Those costs remain visible in this preserving candidate; there
is no claim that the fused kernel is faster for every shape. Three pairs
quantify observed variability only; they do not establish a release-performance
claim. Sharing a read changes the distribution of parallel work, and both arithmetic states still require their original
updates. Whole-fit nonregression remains the complete H boundary.

SCC setup job `8038695` on `scc-gr4` used four slots for 141 seconds,
maximum virtual memory 3.959G. Panel job `8038696` on `scc-yj3` used
28 slots for 46 seconds, maximum virtual memory 1.424G; the diagnostic test
itself took 31.02 seconds. Both jobs report `failed=0`, `exit_status=0`.
The panel used CPUs `0,2,4,6,8,10,12,14` and memory node/socket 0, with
THP `[always] madvise never`. Its Linux diagnostic binary SHA-256 is
`d6f10e06ca3454bc3017eeba5fc0983c4562dea509fdb9f6ec2c3419c599e3ce`.

Candidate plugin SHA-256:
`d5e240f6da750a6b104571e4fdcb13efb429613117dbd7391d852d7e1a20ae75`;
parent plugin: `e29f2e71b41f95953d7d68b398227e8cb33f9bd6e35097c25d8b9a37acc18a9b`.
Raw source, plugins, review, diagnostic panel and all checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/norm-validation.tar.gz`
(52,497,401 bytes, SHA-256
`25bc603a79bc82fca42b3e07fff55be136dc598cd0cb898e6c6b9e930239a15d`).
The [compact record](../bench/results/h-20261013-h6-norms.json) binds the
source, diagnostic binary and qualification identities.

## H6 uniform candidate markers, 2026-10-12

H6 item 6b retains all-candidate inputs as a repeated true scalar. A concrete
borrowed logical view carries candidates through numeric classification,
chunked SPI transfer and PPML adjustment loading, with no materialization or
second classifier. All-candidate chunked loading keeps its existing no-mask
path. Explicit marker reservations, read/write ordering, interruption polling
and string encounter order remain intact. Repeated true or false is raw data,
not a receipt: column dimensions, metadata/source-use validation, candidate
weight domains, factor validation and exclusion precedence remain authoritative.
All arithmetic, solver signatures, tolerances, certificates and sample/rank/
omission/separation decisions stay exact.

PPML keeps its existing explicit staged original-axis mask. It swaps the entire
candidate owner and restores the prior repeated variant or explicit pointer/
capacity, together with probability-weight bits, before propagating a returned
failure. No fallible conversion is inserted inside that restoration interval.
Raw owner accounting uses actual capacities, while loading, compaction and PPML
future-mask forecasts remain unchanged. Thus the retained all-candidate input
saves N bytes without undercounting a later explicit replacement. Eliminated
allocations remove their own failure points. Logical all-true and candidate
counts take constant time for repeated values.

All 78 focused/mapped executions pass: seven new witnesses, thirteen PPML
owner tests, twenty-seven PPML numeric tests, fourteen independent numeric
sample cases, sixteen plugin candidate tests and one owned-capsule test.
Coverage includes repeated/explicit true and false inputs, malformed shapes,
excluded invalid values/weights, range/lifetime/capacity semantics, numeric and
string scalar/batched SPI paths, Break on excluded chunks, PPML provenance and
restoration, and retained versus future mask admission. Workspace compilation,
Clippy, formatting and independent high-effort architecture review pass.
The full suite passes 1,707 Rust executions in 62 invocations and 200 Python
tests with 13 optional dependency skips. Universal Mac ABI/load/guard checks,
73 exact replay cases, 292 worker checks and seventeen licensed preparation,
FE, weight, memory, output and transaction fixtures pass. The expected Linux-ELF
skip remains on Mac; there is no new Linux or Windows qualification.

Two initial test-compilation checks found missing fixture adapters (a permutation
collect, then reverse/array-view conversions). The first focused PPML all-false
fixture incorrectly unwrapped the existing insufficient-observations error;
it now compares repeated/explicit errors. These three attempts remain visible;
no production numerical or diagnostic policy was repaired. Core adds 71
production lines and plugin 23; size-violation, dispatch and chunk counts are
unchanged. Norm handoff and FE numeric-column release remain separate H6 work.
Timing remains at the complete H boundary.

Candidate plugin SHA-256:
`e29f2e71b41f95953d7d68b398227e8cb33f9bd6e35097c25d8b9a37acc18a9b`;
parent plugin: `80f89d6ae8057be42e533492d1f1bb06c0ea6e72de88daf93333eb1656a1265b`.
Raw source, plugins, review and checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/candidate-validation.tar.gz`
(25,494,059 bytes, SHA-256
`d9467040a35b36253d5d075595f01a6509100e107394208dfa3533e033e124af`).
The [compact record](../bench/results/h-20261013-h6-candidate-mask.json) binds
source identities and evidence.

## H6 uniform drop reasons, 2026-10-12

H6 item 5b replaces the complete unweighted OLS sample's allocated Retained
mask with a logical repeated scalar. Classified builders keep their original
reserved mutable vectors and wrap them after classification. Logical reference
access and equality allocate nothing; empty sequences ignore the stored scalar.
Explicit storage moves without copying. Raw repeated values are not receipts:
dimensions, counts, numeric/weight checks and original row/drop consistency
still govern acceptance and diagnostic order. Arithmetic, solver signatures,
tolerances, certificates and sample/rank/omission/separation decisions stay exact.

No-drop compaction returns the implicit owner. Actual deletion materializes
fallibly after replacement value/map/weight/adjustment reservations and before
mutation. The unchanged OLS worst-case forecast includes the full original-axis
reason bank alongside both numeric banks and keep mask. PPML generation and
replacement budgets remain unchanged; production initial/final mutable ledgers
remain explicit. The legacy owner's consuming conversion is test-only; production
from_selection already supplies a Vec. Sealed publication reads logical reasons
and retains its dimensions, residual, receipt and transaction checks.

All 39 focused/mapped executions pass, including five new witnesses: logical
sequence access and empty equality, impossible materialization failure, explicit
pointer/capacity transfer, sixty malformed repeated/explicit raw-mask comparisons,
no-drop/deleting compaction and exact/one-byte-short admission. Mapped complete
transfer, numeric sample, PPML ownership/rebase, allocation, singleton, sealed
owner and publication checks pass. Workspace compilation, Clippy, formatting
and independent high-effort architecture review pass. The full suite passes
1,698 Rust executions in 62 invocations and 200 Python tests with 13 optional
dependency skips. Universal Mac ABI/load/guard checks, 73 exact plugin replay
cases, 292 worker checks and seventeen licensed preparation, FE, weight, memory,
output and transaction fixtures pass. The expected Linux-ELF skip remains on
Mac; no new Linux or Windows qualification follows.

The saving is one N-byte heap mask for complete OLS preparation. Actual deletion
still allocates that mask under the existing admission; classified and mutable
PPML ledgers keep actual-capacity accounting. Eliminated allocations remove
their own failure points. Core adds 82 production lines and one structural
logical-equality condition; plugin, chunk and size-violation counts are unchanged.
An initial lead inventory helper failed to import rust_source before writing;
correcting its Python import path restored the ratchet update. The first
collector missed an interleaved nested-test summary header; matching complete
passed/failed suffixes recovered all 62 successes without a test rerun. All
runtime checks pass without production or numerical repair. Timing remains at the H
boundary; candidate storage, norm handoff and FE-column release are separate.

Candidate plugin SHA-256:
`80f89d6ae8057be42e533492d1f1bb06c0ea6e72de88daf93333eb1656a1265b`;
parent plugin: `0c5495fce859f761d934fe65b24bd7c23bf12b05c41ebca5e017bf1b2ce06482`.
Raw source, plugins, review and checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/drop-validation.tar.gz`
(25,464,434 bytes, SHA-256
`e9b5701555fdb1f022ae12499c9538132f83e2839d308b5c8554b226403dc704`).
The [compact record](../bench/results/h-20261013-h6-drop-mask.json) binds all
source identities and evidence.

## H6 identity row axis, 2026-10-12

H6 item 5a moves the existing logical identity/selected axis to the data module
and uses it for authoritative sample row maps. Complete owned and borrowed
unweighted preparation creates Identity after existing checks; classification
and singleton compaction retain Selected. OLS publication borrows immutable
sealed PreparedOlsData, retaining residual shape/finiteness, output dimensions,
receipts and transactions without rescanning membership. Public raw samples
still undergo dimensions, values, weights, counts, order/bounds and row/drop
consistency validation. Owned-load-hint provenance is unchanged. All arithmetic,
solver signatures, tolerances, certificates and sample/rank/omission/separation
decisions remain exact.

There are 54 passing focused/mapped executions: fourteen axis-filter tests,
twelve PPML model tests, thirteen numeric-sample oracle tests, fourteen
categorical sample/cluster oracles and one coefficient-shadow witness. The PPML
axis witness occurs in two filters. These cover malformed Identity inputs,
logical equality, empty and oversized maps, selected pointer/capacity transfer,
signed zero, no-drop/deleting compaction, complete/no-FE/clustered/excluded PPML
generations, rebase/pweight rollback and publication failure paths. Workspace
compilation, Clippy and independent high-effort architecture review pass.
The full suite passes 1,692 Rust executions in 62 invocations and 200 Python
tests with 13 optional dependency skips. Universal Mac ABI/load/guard checks,
73 exact plugin replay cases, 292 worker checks and seventeen licensed
preparation/FE/weight/memory/output/transaction fixtures pass. The expected
Mac Linux-ELF skip remains; there is no new Linux or Windows qualification.

Complete OLS identity axes own no heap bank, removing 8N bytes on a 64-bit
host. Selected buffers retain their actual capacities. PPML materializes an
identity fallibly only at its existing explicit-row ownership boundary; Selected
moves intact. Eliminated allocations remove their own failure points; invalid
raw-data ordering and remaining fallibility are preserved. Core production
grows 36 lines and plugin shrinks 27; size, dispatch and chunk counts stay
unchanged. Drop-reason and candidate masks remain explicit in this slice.
Timing remains at the whole-H Mac boundary.

Retained attempts include two incomplete API-migration compile failures, two
PPML fixture metadata errors (unused source, then noncontiguous IDs), a test
slice-size lint and an initial Python without tomllib. Repeated publication
access briefly exceeded the function-size limit; a local sample binding and
removal of a redundant debug membership assertion restored the limit. The first
full suite passed its runtime tests but stopped at stale inventory method
positions after a documentation line moved; refreshed positions and the final
full driver pass are retained. No numerical failure or tolerance change occurred.

Candidate plugin SHA-256:
`0c5495fce859f761d934fe65b24bd7c23bf12b05c41ebca5e017bf1b2ce06482`;
parent plugin: `65ce7a7925bc16f9a7c82e6211a35d92914e148d92196b451ca3a38326c0b6ab`.
Raw source, plugins, review and checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/axis-validation.tar.gz`
(25,590,918 bytes, SHA-256
`d69729d308a0c5915898a867f742a05cc8886fbfca4846fdeca664cda106516e`).
The [compact record](../bench/results/h-20261013-h6-axis.json) binds source
identities and validation, including the removed axis module.

## H6 membership initialization, 2026-10-12

H6 item 6a initializes the membership observation bank through the existing
safe indexed-collection helper, generalized from f64 zeroes to copied scalar
values. It uses the installed team's scheduling policy and the same fallible
reservation and actual-capacity admission. Scatter destinations and per-level
observation order, numerical kernels, solver signatures, requested tolerances,
certificates and sample/rank/omission/separation decisions remain unchanged.

Ten focused checks pass: f64/usize initialization at empty, single-element and
chunk-boundary lengths outside a pool and at 1/4/16 workers preserves pointer,
capacity and bits; serial/parallel membership, optional reservation failures,
invalid codes, excess capacity and high-cardinality admission remain exact.
The full suite passes 1,687 Rust executions in 62 invocations and 200 Python
tests with 13 optional dependency skips. Universal Mac ABI/load/guard checks,
73 exact plugin replay cases, 292 worker checks and seventeen licensed
preparation/FE/weight/memory/output/transaction fixtures pass. The expected
Mac Linux-ELF audit skip remains; no Linux or Windows claim follows.

This parallelizes the serial fill but does not remove its writes: safe mutable
scatter slices require initialization, so the bank still receives 8N zero
bytes followed by 8N scatter bytes on a 64-bit host. There is no extra
observation bank, unsafe conversion or new allocation. Core production grows
four lines; dispatch and size-violation counts are unchanged. No unsuccessful
repair occurred. Candidate markers and complete-sample drop reasons remain
separate H6 changes. Timing evidence is deferred to the whole-H Mac boundary.

Candidate plugin SHA-256:
`65ce7a7925bc16f9a7c82e6211a35d92914e148d92196b451ca3a38326c0b6ab`;
parent plugin: `fc315ad83d18f6074375dd16b93e9cb0d4318852f3bd3e91b5644dbadeaa54c7`.
Raw source, plugins and checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/initialization-validation.tar.gz`
(25,442,866 bytes, SHA-256
`d4a3ebdf2c7b6b9269e21e679354408758dae008af70df6be8c9a730fde8f592`).
The [compact record](../bench/results/h-20261013-h6-initialization.json)
retains source identities, validation and limitations.

## H6 owned load hints, 2026-10-12

H6 item 1 moves early OLS categorical columns into a private immutable owner
during overlapped factorization. Core restores ordered matching vacant slots
after request compilation, then runs the established loaded-input, weight,
availability and categorical-domain checks. Only the actual complete unweighted
transfer with matching source identities bypasses the hint's identity and
code/value rescans. Weighted/classified inputs, independent borrowed hints and
PPML retain the existing checked reuse. All arithmetic, signatures, requested
tolerances, certificates and sample/rank/omission/separation decisions remain.

Three focused core tests pass, covering 104 capsule/plain comparisons, missing
values, candidates, weights, clusters, singleton pruning, changed requests,
malformed slots, raw pointer/capacity preservation and positive-weight checked
encoding reuse. Plugin tests verify early/late completeness errors and unchanged
moment-cache and membership-scratch budget decisions. The overlap test exercises
both ownership modes at 2/4/16 workers, including exact-budget success,
one-byte-short refusal before any SPI read, optional factorization refusal,
late SPI interruption/panic and joined cancellation. The full suite passes
1,687 Rust executions in 62 invocations, plus 200 Python tests with 13 optional
dependency skips. Universal Mac ABI/load/guard checks, 73 exact replay cases,
292 worker checks and seventeen licensed preparation/FE/weight/memory/output/
transaction fixtures pass. Independent high-effort architectural review approves.
The Mac Linux-ELF skip remains; no new Linux or Windows qualification is claimed.

The capsule adds 56 descriptor bytes per early source on this host; its raw
buffers are moved without copying and descriptors are freed before
classification. Completed encodings plus descriptors fit the original
preparation envelope, preserving the moment-cache decision. Core production
grows 176 lines and two dispatch candidates; plugin production grows 99 lines
and three candidates. There is no new size violation. The first draft discarded
classified hints and would have added weighted refactorization; checked reuse
was restored. A function-size violation was resolved through column-preparation
and target-ownership methods, and a test-only Clippy failure through lexical
borrow scope. These attempts and initial toolchain-path failures remain recorded.
No timing gain precedes the whole-H Mac panel.

Candidate plugin SHA-256:
`fc315ad83d18f6074375dd16b93e9cb0d4318852f3bd3e91b5644dbadeaa54c7`;
parent plugin: `00a9daac0366fdb9f308406fda618cff96cdf026984a69f9a39d5afa7c33020c`.
Raw source, plugins, captures and checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/loadhint-validation.tar.gz`
(25,673,546 bytes, SHA-256
`695d41c31f310158c2671d188881c3beaf7c21b3d07eb11dd9d7ddafa88c55b5`).
The [compact record](../bench/results/h-20261013-h6-load-hint.json) preserves
source identities, failed attempts, checks and review limitations.

## H6 checked initial RHS, 2026-10-12

H6 item 4 keeps the immutable `PreparedOlsRhs` owner through initial ordinary
OLS projection. Structural automatic and reusable categorical entrypoints
consume its finite-value proof; raw matrices, generic categorical builders,
PPML, final residuals and other backends retain their checks. Configuration,
dimensions, values and weights keep their existing failure order. Arithmetic,
solver signatures, certificates and sample/rank/omission/separation decisions
are unchanged, with no new heap allocation or RHS lifetime overlap.

Twelve focused RHS tests pass. They include 32 checked/raw comparisons across
zero to three effects, unit/provided weights, fresh/reused RHS construction and
one/four workers, checking exact residuals, norms, FE coefficients and complete
reports after removing clocks. Malformed axes, empty inputs, nonfinite values
and competing weight errors retain their diagnostics. The full suite passes
1,682 Rust executions in 62 invocations, plus 200 Python tests with 13 optional
dependency skips. Universal Mac ABI/load/guard checks, all 73 exact replay
cases, 292 worker checks and seventeen licensed preparation, FE, weight,
memory, saved-output and transaction fixtures pass. Independent high-effort
review finds no blocker. The expected Mac skip of the Linux ELF audit remains;
no new Linux or Windows qualification is claimed.

The first compile missed a raw conversion in the internal graph fallback,
and the added witness initially used the wrong `ThreadTeam::new` return type.
Both were corrected before validation. An inventory-update command used system
Python without `tomllib`; rerunning it with Python 3.13 repaired the tooling
failure. Core production grows from 95,189 to 95,241 lines; dispatch and size
counts are unchanged. The whole-H Mac timing panel remains outstanding.

Candidate plugin SHA-256:
`00a9daac0366fdb9f308406fda618cff96cdf026984a69f9a39d5afa7c33020c`;
parent plugin: `752d759d991524e667f68bc0053fb3b7f458a702c4e34b2efa62147f8ad889e4`.
Raw source, plugins, captures and checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/finite-validation.tar.gz`
(25,411,290 bytes, SHA-256
`704fc219dafeb4404e30d389f776b1622d856ecc6ef24cbd8707474f5f06209e`).
The [compact record](../bench/results/h-20261013-h6-finite-inputs.json) retains
source identities, passing checks, failed attempts and review limitations.

## H6 membership validation reuse, 2026-10-12

H6 item 3 seals the exact codes/membership buffers checked during single-source
transfer and reuses the immutable topology owner in ordinary and categorical
absorbed DoF. A foreign owner is rejected before validation is bypassed; raw
adapters and metadata, cluster and slope checks retain their checks. Rebuilt
topology is validated at the original finish argument after source retention
and rebase, preserving its scratch lifetime and failure ordering.

All 87 focused topology tests pass, including checked/owned DoF equality for
multiple effects, interactions, duplicate recipes, nesting and slopes;
foreign-owner rejection; transfer order, pointer and alias checks; malformed
axes and reservation failures; and weighted/perturbed/NaN recovery. The final
full suite passes 1,680 Rust executions in 62 invocations, plus 200 Python
tests with 13 optional-dependency skips. Universal Mac ABI/load/guard checks,
73 exact replay cases, 292 worker checks and seventeen licensed FE, DoF,
singleton, cluster, slope, saved-output, memory and transaction fixtures pass.
Scientific results and solver signatures remain exact. No new Linux or Windows
qualification follows; the Mac skip of the Linux ELF audit remains explicit.

Independent review caught an earlier scratch-lifetime regression: rebuilt
interaction validation ran before candidate buffers were released. The final
transferred/rebuilt ownership state restores the original point. The earlier
full numerical run passed but failed the new file-size ratchet; placing the
construction state in the existing preparation-topology module and moving a
test-only import resolved the two tooling failures. All attempts remain in
the raw evidence. Core production increases from 95,165 to 95,189 lines;
dispatch and size-violation counts do not rise. No timing gain is claimed
before the whole-H twenty-workload quiet Mac panel.

Candidate plugin SHA-256:
`752d759d991524e667f68bc0053fb3b7f458a702c4e34b2efa62147f8ad889e4`;
parent plugin: `6e2fb08f91c862c5e9c45b6a3df652335cc8521ca689663a7279cca6860db55b`.
Raw source, plugins, captures and checks remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/membership-validation.tar.gz`
(25,448,553 bytes, SHA-256
`309940f93ec426e7b816b93958902102470bc081319a9a8fd8763d803aef7e43`).
The [compact record](../bench/results/h-20261013-h6-membership.json) preserves
the source/case identities, failures and final results.

## H6 sample scans, 2026-10-12

H6 item 2 uses the installed worker team's row-chunk policy for complete-sample
validation and numeric-source finiteness, including narrow models. The complete
unweighted identity shortcut proves the old finite-value, retained-flag and row
map conditions together; a failed shortcut retains the old diagnostic order.
There is no numerical reduction or scientific-policy change.

Three focused tests cover malformed samples, boundary row identities, competing
nonfinite/axis errors, weighted and selected fallback, and signed zero at
1/4/8 workers. The full suite passes 1,680 Rust executions in 62 invocations;
200 Python tests have 13 optional-dependency skips. Universal Mac build,
ABI/load/guard checks, 73 exact plugin replay cases and 292 worker checks pass.
Solver signatures are unchanged. Ten licensed preparation, factor-availability,
weight, candidate-sample, rebase and OLS/PPML transaction fixtures pass, with
application return codes and terminal sentinels. Independent high-effort review
finds no blocker. The expected Mac skip of the Linux ELF audit remains explicit;
this item makes no new Linux or Windows qualification claim.

The candidate plugin SHA-256 is
`6e2fb08f91c862c5e9c45b6a3df652335cc8521ca689663a7279cca6860db55b`,
compared with H8's `15824d5b0ce070d56312b9ca7af31428a8086a1064cc655f14e572b90c15f139`.
Raw captures, both plugins, cases and source patch remain in
`/projectnb/welfgr/fereg/runs/h-20261013-h6/sample-validation.tar.gz`
(25,389,514 bytes, SHA-256
`ff74cb830b83c22168d54519e5ee165fa906188d5cc7154639b7ea9611293287`).
The [compact record](../bench/results/h-20261013-h6-sample-scans.json) retains
case identities and all check outcomes. An initial zero-test filter and two
test-compilation corrections are retained, not counted as passing evidence.

Core production grows 32 lines (95,133 to 95,165) and loses one dispatch
condition; the ratchet records the complete-sample proof and diagnostic-order
reason. A failed shortcut pays for its probe before ordinary validation.
The twenty-workload quiet Mac performance panel remains at the complete H
boundary; no wall-time gain is asserted here.

## H5 socket and NUMA placement, 2026-10-12

Campaign tooling now groups allowed physical cores by socket, starting with
the largest allowed socket so partial affinity restrictions cannot force a
small fit across sockets unnecessarily. SMT siblings are deduplicated.
`numactl` applies the same physical mask and memory-node binding to native and
reference arms; missing topology or binding failure has no unbound fallback.
Every raw observation contains the mask, socket/node IDs, `lscpu` and THP.
Runtime tokens and all 58 artifact inputs remain identical to `67cb34ce`.

The fixed 48-fit panel uses frozen AKM 1M inputs, OLS/iid and PPML/robust,
three alternating pairs at 1/4/8/16 workers, current accepted H8 runtime and
fixest 0.14.2. All 24 comparisons are scientifically eligible; all sixteen
three-repeat groups have exact captured science, and both tools pass the
cross-worker comparisons at unchanged campaign tolerances. The table gives
median command seconds, with speedups relative to each tool's one-worker cell.

| Model | Workers | fereg s | fereg speedup | fixest s | fixest speedup |
| --- | ---: | ---: | ---: | ---: | ---: |
| OLS | 1 | 2.291639 | 1.000 | 4.096 | 1.000 |
| OLS | 4 | 1.100400 | 2.083 | 2.115 | 1.937 |
| OLS | 8 | 0.803931 | 2.851 | 1.688 | 2.427 |
| OLS | 16 | 0.750220 | 3.055 | 1.483 | 2.762 |
| PPML | 1 | 10.851459 | 1.000 | 19.397 | 1.000 |
| PPML | 4 | 5.283432 | 2.054 | 8.147 | 2.381 |
| PPML | 8 | 4.248238 | 2.554 | 5.945 | 3.263 |
| PPML | 16 | 4.445408 | 2.441 | 4.944 | 3.923 |

Native PPML is 4.64% slower at sixteen than eight workers despite placement
control. This cost is retained; three pairs on one SCC node do not establish
a universal gain or a controlled old/new placement effect. No extra fits or
timing retries were used.

Four pre-panel 1,200-row single-FE native/reference smoke fits pass the existing
independent OLS/Poisson oracle, including covariance, predictions, sample and
PPML rank. The actual child confirms the requested affinity and `bind` policy
in `/proc/self/numa_maps`. Four placement tests cover interleaved sockets,
partial masks, SMT siblings, missing topology and identical arm commands;
all 42 SCC campaign harness tests pass. Mac `run_tests.sh` passes (1,679 Rust
executions in 62 invocations, 200 Python tests with 13 optional-dependency
skips, portable checks and native guards). The expected Mac skip of the Linux
mock-plugin audit remains explicit; the unchanged H4 Linux plugin's successful
build/audit/load evidence is reused. Five analysis checks include signed zero,
one-ulp exactness, tolerance rejection and exact sample counts. The reused
130-file comparator/clock environment matches its original frozen manifest.

Job `8035870` on `scc-tj3` passes failed=0/exit=0, 479 seconds, 3.886G maxvmem.
Its two fourteen-core Xeon E5-2680 v4 sockets use THP `[always] madvise never`;
the measured panel consumes 452.634 of 1,800 seconds.
Raw source, scripts, plugins by retained H4 identity, datasets by frozen links,
captures, checks and accounting remain under
`/projectnb/welfgr/fereg/runs/h-20261013-h5`; the
[compact record](../bench/results/h-20261013-h5.json) retains every fit and full
hashes. The full H boundary still requires the twenty quiet Mac workloads.
No new Windows qualification is claimed.

## H4 overlap check: no load inflation, 2026-10-12

Five alternating overlap-on/off pairs on frozen 1M AKM OLS at 1/4/16 workers
complete all thirty planned fits on `scc-wf2`, with identical physical masks
within each pair and tracing disabled. The diagnostic off artifact changes
only `cancellable::while_loading` to load before running the same preparation
closure on the same team; production remains unchanged. All fifteen paired
science comparisons and six five-repeat groups are exact, including solver
signatures except clocks. No fit was retried.

RRUNTIME rows 18–21 below are mean seconds (on / off); command time is also
retained in the [compact record](../bench/results/h-20261013-h4.json).

| Workers | 18: preflight | 19: input | 20: native | 21: total |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 0.000359 / 0.000358 | 0.305581 / 0.305297 | 2.257310 / 2.257538 | 2.563250 / 2.563193 |
| 4 | 0.000360 / 0.000350 | 0.299695 / 0.311857 | 0.906805 / 0.900010 | 1.206859 / 1.212218 |
| 16 | 0.000354 / 0.000350 | 0.273927 / 0.287376 | 0.464803 / 0.463340 | 0.739083 / 0.751066 |

The declared primary four-worker load-inflation rule fails: all five paired
on-minus-off differences are negative, with median −15.219 ms and ratio
0.95107. Sixteen-worker load also favors overlap (median −13.482 ms), while
one worker gives −0.049 ms and mixed signs. Mean command times on/off are
2.672148/2.671837, 1.275795/1.279567 and 0.805579/0.815720 seconds.
Small native costs at four/sixteen workers remain visible. The earlier C1
load pattern is not reproduced; this bounded check attributes it to host
variability and stops without a runtime change. These five-pair SCC
observations do not establish a confidence-qualified release performance claim.

Source comparison covers 760 files and finds only the intended diagnostic
coordinator difference. Both Linux builds, native ABI/load/guard checks and
the existing prepared-input numeric test pass, including worker counts
2/4/16, optional allocation fallback, interrupt and SPI panic handling. The
analysis is checked against synthetic inflation, mixed timings and a one-ulp
scientific mismatch. Scheduler setup `8035773` passes (failed=0/exit=0,
396 s, 3.469G); panel `8035774` passes (117 s, 2.622G). The timed panel uses
112.251 of 900 seconds. Raw archives, plugins, patch, scripts and captures
remain under `/projectnb/welfgr/fereg/runs/h-20261013-h4`.

Production identity equals `42c8248c` (171 modules and 58 artifact inputs).
This prose/evidence closure reuses valid H8 Mac runtime evidence; the twenty
quiet Mac workloads remain due once at the full H boundary. No new Windows
qualification is claimed.

## H2 dense three-FE prototype: sparse-cost gate failed, 2026-10-12

The approved prototype completed its fixed 36-fit panel and all scientific
checks, but **was not adopted**: sparse geometry took 29.785–30.855 ms,
exceeding the declared 21 ms limit in all six candidate captures. The production
runtime remains `a05ae5d7`. No cell was retried and no threshold was relaxed.
The [complete prototype patch](../bench/results/h-20261013-h2.patch) applies to
that parent and reproduces all eleven saved source/test/inventory files exactly;
the [compact record](../bench/results/h-20261013-h2.json) retains the outcomes.
This commit is an unchanged artifact with retained experimental source.

The rule was frozen in Workstream H section 6 before implementation. Unit-weight
ordinary three-intercept OLS caches exact-integer centred pair Frobenius bounds
once per session, predicts at most four symmetric sweeps at the H8 recurrence
target, and accepts only through the unchanged requested-tolerance certificate.
A missed pilot releases its candidate before LSMR uses the untouched original;
actual pilot sweeps debit the existing first-half allowance. The final MAP
reserve, explicit/weighted requests, PPML and final-residual route remain intact.
Candidate, histogram, norm workspace and preallocated canonical coefficients
are admitted; caller norm kernels are unchanged. Numerical and route scope is
recorded in section 6; sample, rank, omission and separation remain exact.

Two alternating pairs per frozen input/worker cell give these native means and
paired geometric-mean changes. Both arms use the same physical-core mask.

| Input | Workers | Parent native s | Prototype native s | Paired change |
| --- | ---: | ---: | ---: | ---: |
| dense-1m | 1 | 3.474 | 3.415 | -1.70% |
| dense-1m | 4 | 1.502 | 1.449 | -3.52% |
| dense-1m | 16 | 0.838 | 0.864 | +3.05% |
| dense-4m | 1 | 22.806 | 16.315 | -28.46% |
| dense-4m | 4 | 7.407 | 5.981 | -19.26% |
| dense-4m | 16 | 4.372 | 3.712 | -15.07% |
| sparse-1m | 1 | 13.458 | 13.449 | -0.07% |
| sparse-1m | 4 | 4.742 | 4.587 | -3.20% |
| sparse-1m | 16 | 2.766 | 2.799 | +1.31% |

All eighteen paired science/discrete checks pass, as do all eighteen fixed-worker
repeats. Maximum coefficient/VCE differences are 3.86e-14/1.84e-20. Sparse remains
on LSMR with exact captured coefficients, covariance, statistics and solver
signatures, apart from clocks. Main/final iterations are 4/1 → 4/1 at dense 1M,
10/1 → 4/1 at dense 4M, and unchanged 22/2 at sparse 1M. All dense candidates
predict three sweeps and certify at the four-sweep checkpoint; all final solves
remain LSMR. Geometry costs 37.7–40.1 ms at dense 1M and 169.2–182.1 ms at 4M.
The 1M/sixteen-worker native cost rises 3.05%; all observations are retained.

At 4M/four workers command time is 8.684 → 7.244 seconds. The prototype still
exceeds the retained fixest 4.224-second observation by 3.020 seconds (71.5%);
closing that numerical gap would require another 41.7% of its command time.
Those fixest bytes were measured on another host/source, so this is a remaining
scale comparison, not a new paired fixest claim. Two pairs with tracing are
descriptive SCC evidence; the twenty quiet Mac workloads remain at the H boundary.

Mac prototype validation passes: full `scripts/run_tests.sh` (1,691 Rust test
executions in 63 invocations, tooling/oracle/transaction checks), universal
build and actual-plugin audit/load, all 73 replay cases with zero exact parent
differences, all 292 checks at 2/4/8/16 workers, and fourteen licensed fixtures.
The added licensed eligible-MAP fixture checks saved effects, reconstruction,
predictions and robust covariance against full dummy regression in Fast/Low at
1/4 workers. Five independent oracle tests cover nonuniform and perturbed
products, extra intersections, disconnected designs, weights and explicit LSMR.
Seven core tests cover rounding, acceptance at the request, allocation refusal,
coefficient reconstruction and shared work. A subsequent focused test reproduces
all four bounds of all eight retained F2 rational-projection fixtures bitwise;
its Clippy check passes. No production tokens changed after the candidate freeze.

Independent Astra review identified and resolved an inappropriate norm handoff,
a missing fallback diagnostic, post-certificate coefficient allocation, and
geometry error/Break handling. Initial compile/test-name/import mistakes and
Clippy's decimal-precision warning are retained; exact binary64 bit constants
preserve the frozen F2 witness. A local Python cache-path refusal was resolved by
syntax-only parsing. There were no scientific fit failures or retries. These
prototype tests are retained in the patch rather than installed without their
runtime implementation. Current runtime/artifact identity equals the parent,
so its valid H8 runtime evidence is reused for this documentation-only closure.

SCC setup `8035579` on `scc-gd4` passes with failed=0/exit=0, 364 seconds and
3.967G maxvmem; its seven focused Linux tests and native guards pass. Panel
`8035584` on `scc-wf2`, 28 physical cores, passes with failed=0/exit=0, 423 seconds
and 6.209G maxvmem. Its timed panel uses 406.846 of 2,400 seconds. Source archives,
plugins, scripts, raw captures and both Mac validation archives remain under
`/projectnb/welfgr/fereg/runs/h-20261013-h2`. The final Mac archive is 25,396,960
bytes, SHA-256 `3d5dbd3a0096eef90a7b641bcbe82427907af2740d95470beba51825df8fd64a`.
The prototype would add 388 production lines and eight dispatch candidates;
this closure adds none. No new Windows qualification is claimed.

## H8 recurrence consistency with measured sparse cost, 2026-10-12

The approved ordinary automatic two-FE OLS change uses the existing automatic
LSMR target: a hundredth of the request, floored at 1e-14 and capped at the
request. Both PCG readiness checks receive that target directly. Explicit
requests, PPML's scheduled projection, the final-residual target, certificate
at the request, iteration budget, MAP reserve and acceptance/fallback rules
are unchanged. A passing certificate still publishes `Certified` after proxy
budget exhaustion; trace-only RHS reasons report the recurrence stop.

The result-changing scope names projected columns, FE coefficients, coefficients,
covariance, fit statistics, downstream saved numerical outputs, numerical
diagnostics and inner work counts. Sample, rank, omission, separation,
acceptance/stop codes, fallback and warnings remain exact. See the
[predeclared input-bound Mac scope](../tests/replay/scopes/2026-10-12-h8-two-fe-recurrence-macos.json)
and [compact evidence](../bench/results/h-20261013-h8.json).

All 48 SCC fits completed without retries: two alternating pairs for each
frozen AKM 1M, dense 1M/4M and sparse 1M cell at 1/4/16 workers. The table
shows native mean seconds and the paired geometric-mean percentage change.
All 24 coefficient/VCE and discrete comparisons pass; all 24 within-arm
fixed-worker repeats are exact after removing clocks. Maximum coefficient
and covariance changes are 3.20e-14 and 3.71e-16. Both arms use the same mask.

| Input | Workers | Parent native s | H8 native s | Paired change |
| --- | ---: | ---: | ---: | ---: |
| akm-1m | 1 | 2.060 | 2.177 | +5.71% |
| akm-1m | 4 | 0.973 | 1.063 | +9.26% |
| akm-1m | 16 | 0.523 | 0.547 | +4.68% |
| dense-1m | 1 | 1.439 | 1.417 | -1.48% |
| dense-1m | 4 | 0.597 | 0.615 | +2.88% |
| dense-1m | 16 | 0.355 | 0.373 | +5.06% |
| dense-4m | 1 | 6.548 | 6.470 | -1.20% |
| dense-4m | 4 | 2.884 | 2.564 | -11.08% |
| dense-4m | 16 | 1.463 | 1.529 | +4.54% |
| sparse-1m | 1 | 4.603 | 5.216 | +13.31% |
| sparse-1m | 4 | 2.025 | 2.272 | +12.17% |
| sparse-1m | 16 | 0.890 | 0.986 | +10.95% |

Sparse exceeds the ten-percent reporting threshold at all three worker counts;
these costs were reported to Johannes before integration. The approved rule
is retained unchanged. Main/residual iterations are 14/0 → 18/0 for AKM,
1/0 → 1/0 for both dense sizes, and 92/11 → 120/0 for sparse, identically
in both pairs and at every worker count. A zero final projection reflects the
unchanged initial certificate accepting the more accurate main result. These
two-pair, trace-enabled observations are descriptive, not confidence-qualified
release claims; the twenty frozen quiet Mac workloads remain at the H boundary.

The four retained F1 captures reproduce the exact barbell/weak request hashes.
Full-dummy QR coefficient, covariance, fitted/residual, within-TSS and null-LL
checks pass, and protected decisions are exact. The 80-digit Wald and
mixed-covariance algebra uses exact binary64 inputs. The strong-F bound remains
rtol=1e-12/atol=1e-14 and still fails: barbell is 2.465 → 2.480 times its bound;
weak is 595.049 → 1.076. Weak F error relative to the QR witness falls from
1.440e-5 to -2.603e-8. No stronger certificate or repair of this historical
failure is claimed. Four captures use 1.525 seconds of the 600-second native
budget, without retries; two unused captures are not spent.

Validation: `scripts/run_tests.sh` passes (1,679 Rust test executions across
62 invocations, tooling/oracle/audit checks). Universal Mac and both Linux
native builds pass their audits, loader and boundary guards. All 73 actual
plugin replay cases pass with **zero exact differences from the parent** on
this small corpus, including the three declared cases; all 292 checks at
2/4/8/16 workers pass science, repeat and discrete checks. Fourteen licensed
OLS fixtures pass, including transactions, saved outputs, memory modes,
weights, clusters and independent CMG controls. The focused Linux suite passes
74 tests. A derived three-vertex Schur fixture proves acceptance at the request
after one exhausted recurrence step, with no available MAP step. Exact-boundary,
helper-floor and PPML/final-residual comparisons pass. Independent Astra source
review found no production blocker. The complexity ratchet records 62 additional
production lines and no added dispatch candidates.

Failures remain recorded: a new boundary-test fixture rounded above its nominal
threshold and was replaced by exact power-of-two scaling; categorical test calls
were migrated after the API split; the task-only SCC driver had an indentation
error caught before transfer; and the licensed runner rejected a noncanonical
plugin basename before launching Stata. The corrected basename has identical
plugin bytes. The initial trace summary counted stderr and trace-file copies
twice; their event sequences were checked equal and the final summary counts
each once. Raw captures and the initial summary remain intact. No scientific
fit was retried.

Linux setup job `8035308` on `scc-gr4` has failed=0/exit=0, 406 seconds and
4.625G maxvmem. Panel job `8035310` on `scc-yk4`, 28 physical cores, has
failed=0/exit=0, 321 seconds and 4.153G maxvmem; the timed panel uses 299.584
seconds of its 2,400-second cap. Raw inputs, source archives, plugins, scripts,
logs, observations and Mac validation archive remain under
`/projectnb/welfgr/fereg/runs/h-20261013-h8`; their identities are in the compact
record. Mac plugin parent/candidate hashes are `a9818592...` / `15824d5b...`.
No new Windows qualification claim is made.

## H3 ordered parallel assembly, 2026-10-12

The canonical two-FE Schur builder now partitions packed matrix rows across
workers while preserving every cell's summation order. Each pair is evaluated
twice; each lane repeats the neighbour totals and sorting. Optional scratch
uses spare bytes inside the existing admitted phase peak and is released
before factorization. One worker or unavailable scratch uses the original
serial assembly. Solver selection, certificates, tolerances, sample, rank,
omission, separation and published stop decisions are unchanged.

Before timing, the retained A3 geometries passed raw bitwise matrix comparisons
at 1/2/4/16 workers, including restricted scratch, interleaved disconnected
components, singleton blocks and extreme weight scales. Both frozen dense
PPML inputs then passed all four current-weight matrix comparisons at sixteen
workers. Their dataset hashes equal the retained A3 hashes. The 24-fit panel
used two alternating pairs per cell, the same affinity mask for both arms,
120 seconds per process and a 2,400-second total cap, without retries.

| Rows | Workers | Baseline native s | Candidate native s | Paired reduction |
| --- | ---: | ---: | ---: | ---: |
| 1M | 1 | 9.176909 | 9.185863 | −0.10% |
| 1M | 4 | 5.303372 | 4.900105 | 7.62% |
| 1M | 16 | 3.991165 | 3.075145 | 22.95% |
| 4M | 1 | 40.915626 | 40.580177 | 0.82% |
| 4M | 4 | 24.615295 | 19.401213 | 21.18% |
| 4M | 16 | 19.540483 | 11.635927 | 40.45% |

Times are arithmetic means; reductions use geometric means of paired ratios.
All twelve pairs have exact captured coefficients, covariance, sample hashes,
scientific metadata and timing-stripped solver traces. Repeated scans do not
outweigh the observed gain at four workers on these inputs. The one-worker
changes are below the workstream allowance and are observations, not gains.
Maximum sampled process-tree RSS is 4,712.77/4,715.99 MiB for baseline/candidate.
The older A3 current/implicit native times at sixteen workers were
3.8060/2.7205 seconds at 1M and 18.4272/10.8961 at 4M; at one worker they were
9.3315/8.3492 and 39.0529/31.6992. Different hosts, sources and native harnesses
prevent pooling those observations with this Stata-side panel. A3 cost-model
calibration remains stopped; no implicit-route change was made.

Mac checks pass: `run_tests.sh` (1,678 Rust test executions and the Python,
oracle, allocation, ABI and tooling checks), universal build/load audits,
73 exact baseline replay cases, 292 additional thread/repeat checks and all
17 mapped licensed PPML scientific/transaction fixtures. Linux builds and
native audits pass; its 21 canonical-product tests pass. Independent Astra
review found no production correctness blocker. The complexity ratchet records
220 additional production lines and three resource-related dispatch candidates.
`--compare-production ac8aa81f` identifies the two changed runtime modules;
this is arithmetic-preserving work, not an unchanged artifact.

Build job `8034161` passed (520 seconds, 3.875G maxvmem); panel `8034163` passed
on `scc-zj4` (574 seconds, 6.631G), both failed=0/exit=0. The timed panel took
531.72 seconds. The first Mac full-suite invocation stopped on a new test's
Clippy iterator lint; the corrected full suite passed. The first analysis
mistakenly included clocks among scientific fields; removing only `time_*`
and trace `seconds` clocks gives the exact comparisons above. Both attempts
remain retained, as does the first compact-record size check. No fit was rerun.

The [compact record](../bench/results/h-20261013-h3.json) binds source, plugins,
inputs, observations and validation captures. Raw material remains under
`/projectnb/welfgr/fereg/runs/h-20261013-h3/`. These two-pair, trace-enabled SCC
observations are not a release performance claim. Quiet nonregression timing
on the twenty frozen Mac workloads remains due once at the H workstream
boundary; no new Windows qualification is claimed.

## H1 locality-sort confirmation: hypothesis refuted, 2026-10-12

The ten-fit diagnostic panel on `scc-zj4` rejects H1 under its predeclared
20% projection-time gate. Sorting rows is a sample-order permutation:
coefficients and covariance agree within the unchanged SPEC tolerances;
sample, rank, omission, DoF, singleton accounting, published solver decisions
and indexed per-RHS convergence/iteration decisions are identical. All five
cross-worker comparisons also pass. No H1 production change or within pin
bump was made; the next item is H3.

| Input | Workers | As-is projection s | Sorted projection s | Reduction |
| --- | ---: | ---: | ---: | ---: |
| Fixed64, 1M | 4 | 0.343621 | 0.312060 | 9.2% |
| Fixed64, 1M | 28 | 0.205888 | 0.180632 | 12.3% |
| Fixed64, 16M | 4 | 11.789560 | 12.438800 | −5.5% |
| Fixed64, 16M | 28 | 4.735098 | 4.804807 | −1.5% |

The selected dominant term is `g3`, with 64 coefficients, unsorted in the
original input and sorted in the paired arm. The 16M main-projection
`time_setup/time_solve` values are 0.844177/9.388235 seconds as-is versus
0.516879/10.470788 sorted at four workers, and 0.644289/3.065378 versus
0.377849/3.425807 at 28 workers. Across each complete as-is 16M fit, frame
gathers sum to 0.161995/0.162676 seconds, incoming permutations to
0.336065/0.330757, and outgoing permutations to 0.374020/0.394567 at 4/28
workers. These are summed call clocks with concurrent RHS work; they are not
additive elapsed-time components. Sorted arms remove frame gathers and reduce
the permutation calls to no-ops, yet do not improve the measured projection.

Every fit constructs two Solvers and runs two batches: the main projection
and final residual projection. The fixed64 main batch has six RHSs; the dense
control has eighteen. No retained `BorrowedLsmrPrepared` session is used by
these automatic fits. The 4M three-dense controls take 4.781414/1.897625
seconds in projection at 4/28 workers. The largest coefficient difference
under row sorting is 7.63e-14; covariance differences are at most 1.86e-20.

Panel `8034071` completed all ten fits in 91.63 seconds; scheduler accounting
is failed=0/exit=0, 97 seconds, 8.653G maxvmem. Two allowed fits remain unused;
there was no measurement retry. The diagnostic plugin (`3802d41b…`) builds
source `c8851b5c` with trace-only changes to within `297f32d9`. Its Linux binary
audit, actual-plugin ABI/load tests and numeric guards pass. Initial setup
`8034036` exited 101 because the offline cache lacked the within test dependency
`proptest`; after fetching pinned test dependencies, `8034061` passed two
observation and eight design tests (failed=0/exit=0). Four interleaved trace
writes retain complete phase-tagged payloads; the decoder recovers both events
and checks every expected call count. Indexed fereg traces establish RHS
decisions independently of completion order. No observation was dropped.

The [compact record](../bench/results/h-20261013-h1.json) retains the plan,
hashes, clocks, comparisons, environment and failed setup. Raw captures and
diagnostic sources remain in its SCC run directory. These single diagnostic
fits establish the stopping-rule outcome, not a confidence-bounded speed claim
or Mac qualification. Production tokens and artifact inputs remain unchanged;
this documentation/result commit reuses current runtime validation.

<a id="c2-qualification-follow-up-stopped-without-adoption-2026-10-12"></a>

## C2 adoption with unresolved timing, 2026-10-12

After reviewing the stopped qualification results below, Johannes chose to adopt
the frozen C2 implementation and move on, accepting the remaining performance
uncertainty. Main now includes the candidate without further production edits.
This is a scoped adoption decision: the inconclusive timing cells remain
inconclusive, the existing performance allowance and confidence rule are
unchanged, and uniform nonregression or a release speedup is not established.
No further timing or CPU-placement experiment was run for this integration.

The [retained qualification evidence](../bench/results/c2-20261012-qualification.json)
checks the frozen arithmetic-preserving candidate
[`3a3c5d1b`](https://github.com/johannes-schmieder/fereg/commit/3a3c5d1b63f0ca9c751e38b9baa7dd5bde1979f2)
against `d9865e57`. The candidate parallelizes bounded integer/membership
validation and uses optional, stable counting/scatter scratch after existing
memory admission.
The [earlier evidence](https://github.com/johannes-schmieder/fereg/blob/3a3c5d1b63f0ca9c751e38b9baa7dd5bde1979f2/bench/results/c2-20261011-performance.json)
retains its large-row gains, rejected policies and original inconclusive results.

This follow-up made no production edits. Original task binaries had been deleted
under the cleanup policy. Rebuilt Linux plugins have different hashes, although
the runtime source, artifact inputs, frozen case bytes and harness hashes match.
Old and fresh timing batches are therefore kept separate. No additional samples
were taken after the fixed panel, and no allowance or confidence rule was relaxed.

Mac correctness passes: `run_tests.sh`, universal binary audit and actual-plugin
ABI/load, 73 exact replay cases, 219 additional worker checks at 2/4/16 workers,
eight mapped licensed fixtures, and both OLS and public-scope PPML Break tests.
The checks cover the existing independent scientific witnesses, repeatability,
discrete decisions, memory admission and transaction/rollback boundaries.
Mac baseline/candidate plugin hashes are `b5d1f650` / `cf4769b4`; full hashes are
in the retained record. There is no new Windows qualification.

The Mac panel used all twenty hash-verified standard inputs, one excluded warmup
and fifteen alternating pairs per cell. All 640 calls preserve exact science and
solver signatures. Nineteen cells clear the existing paired-bootstrap upper
bound against max(5%, 1 ms). K=129/four workers remains inconclusive:

| Fresh batch and case | Median paired candidate/baseline ratio | Ordinary 95% bootstrap interval | Outcome |
| --- | ---: | ---: | --- |
| Mac, wide OLS K=129, four workers | 1.02620 | [0.96849, 1.05702] | Nonregression not established |
| SCC, no-FE OLS K=5, four workers | 0.97783 | [0.95123, 0.99317] | Ordinary bound clears |
| SCC, three-FE OLS K=5, one worker | 0.99658 | [0.97447, 1.01189] | Ordinary bound clears |
| SCC, probability-weighted two-FE OLS K=5, four workers | 0.96019 | [0.92581, 0.96910] | Ordinary bound clears |
| SCC, AKM OLS 1M/K=16, four workers, RRUNTIME total | 1.02393 | [0.97922, 1.16442] | Nonregression not established |

The Mac boundary cell's separate time medians are 0.370598 and 0.373725 seconds
(+0.84%); that different statistic cannot replace the predeclared paired result.
Other Mac observations include paired reductions of 3.6% for weighted two-FE/four
workers, 1.7% and 2.0% for K=20 at four/sixteen workers, and 1.3% for PPML/four
workers. Single-worker point estimates stay within 1%. These are bounded
observations, not uniform gains or a completed release-performance claim.

Johannes made the Mac available. No competing qualification builds, tests,
transfers or agent tool workloads ran during timing; monitoring only read the
existing process output. The 24-core Mac's bookend snapshots still show desktop
activity: Activity Monitor at 59.6%/35.1% of one core and sysmond at 21.3%/9.9%.
These observations neither establish continuous idleness nor identify the cause
of the inconclusive cell. Every pair remains in the analysis.

SCC job `8033696` reserved all 28 cores of `scc-wo4`, the original timing host,
with affinity permitting all 28 CPUs and library thread pools capped at one.
The three previously unresolved standard configurations received one warmup plus
fifteen pairs (96 exact calls). Their ordinary intervals clear, but the
predeclared conservative selection/repeated-look sensitivity uses an exact
order-statistic upper bound at tail `0.025/(28*2)`. The corresponding bounds are
1.04324, 1.05981 and 1.05030: only the no-FE cell clears that sensitivity.
The new targeted observations do not retroactively pass the original SCC panel.
Ordinary per-cell intervals do not constitute a familywise confidence claim.

The separate SCC AKM follow-up used the same 1M-row input and four workers,
with one excluded warmup followed by fifteen pairs. All 32 calls remain exact.
Total-time medians are 2.052897 and 2.363395 seconds, a ratio of 1.15125 (+15.1%).
The median of within-pair ratios is instead 1.02393, as reported above; the wide
interval and this difference expose substantial variability, not a reliably
established slowdown magnitude. Native-only time has paired ratio 1.02327 and
interval [0.97515, 1.20610]. Loading has paired ratio 1.00513.

Four separate traced pairs consistently reduce the recorded OLS preparation
phase from about 138 ms to 103 ms (25%). Later fitting, score certification and
covariance times vary in both arms and can more than offset that saving.
These diagnostic clocks are nested, and separate phase medians are not an
additive explanation of the untraced total. Four separate ABBA `perf stat` runs
have nearly equal whole-process instruction counts, but include substantial
loader/input/output work outside native timing. They cannot establish unchanged
native instruction cost or prove a frequency, cache, scheduling or NUMA cause.
User-only software-event zero counts do not establish absent migrations.

The first setup job `8033663` passed both Linux builds/load/numeric guards, then
failed because the input generator rejected an unsupported option (accounting
0/2). Focused job `8033677` reused those builds, corrected the invocation and
verified the original input/harness hashes (0/0). Timing job `8033696` completed
with accounting 0/0, 1,081 scheduler seconds and 1.029 GB maximum virtual memory.
The retained record also names two Mac launcher failures before their affected
checks: a required plugin-basename guard and SIGPIPE in the pre-timing process
snapshot. No timing observations were discarded or replaced for these repairs.

Independent design and evidence review recommended keeping C2 unmerged under the
existing gate before Johannes's adoption decision above. The proposed diagnosis
compares broad affinity with four fixed physical cores sharing a NUMA node/LLC, using
unchanged binaries and a consistent memory-placement policy. It would test a
placement hypothesis, not select favorable timings to waive qualification.
No such experiment has run; the bounded qualification experiment remains stopped.

The original follow-up commit changed only retained evidence and prose. Its
production-identity, retained-record consistency, relative-link and whitespace
checks passed; temporary local and SCC captures were deleted after the record
and hashes were committed. Integration preserves the reviewed candidate's
runtime and artifact inputs, so its Mac scientific, transaction, build/load and
timing evidence remains applicable. `check_complexity.py --compare-production
3a3c5d1b` passes for all 170 runtime modules and 58 artifact inputs, and source,
test and tooling files are byte-identical to that candidate. The full
`./scripts/run_tests.sh` integration gate passes on Mac, including Clippy,
scientific witnesses, allocation audits, complexity and native bridge/numeric
guards (log SHA-256
`3a6a5591cdb7e6e0c8475cad8cba7a8c224ea1cd011ccebd8c4383325a4f7726`).
The Linux-only mock audit is skipped on Mac as intended; prior Linux evidence
remains separate. Both retained C2 records are unchanged from their source
commits. Documentation links, the prior section anchor and whitespace checks
pass. Integration task outputs are removed after committing this summary.

## SCC optimized campaign rerun, 2026-10-12

The [benchmark comparison](fixest_benchmarks.md#optimized-rerun-october-12)
retains the completed rerun of source `d9865e57` (qualified B, C/D and E1),
using the exact 36 October 10 datasets. All 56 configurations and 579 planned
fit positions are represented; default and newly added strict OLS fixest arms
remain separate. Linux plugin SHA-256 is
`75faa55005e95787d8b1a711af738340c36bcf97205c6c8bde1d977aaa41ae23`.

Build `8028839`, independent smoke `8028840` and production array `8028841`
tasks 1–4 have complete successful accounting and application sentinels.
Frozen runtime and decoded dataset identities pass, as do Linux build/ABI/load
and numeric guards, 42 benchmark tooling tests and all 11 native smoke oracles.
All 168 native fits complete; exact repeatability passes on 56 configurations,
and cross-worker checks pass. The 165 completed historical native counterparts
have unchanged sample/discrete fields and agree within the coefficient,
covariance and objective tolerances. Historical saved fitted arrays were not
compared; each campaign's own output checks remain binding.

The largest fixest gaps persist. Four-million-row dense two-FE PPML is 26.767
versus 15.068 seconds; dense three-FE OLS is 8.253 versus 4.224. Fereg wins
26/35 eligible default comparisons, with one former loss becoming a win and
one small new observed loss. Reference failures, skipped fits and timeouts
remain explicit: 154 eligible arm cells, 32 descriptive-discrepancy cells and
seven with no completed timing. All corresponding configurations changed hosts
between campaigns; paired within-node ratios describe relative performance,
but their changes do not isolate the optimizations' causal effect. These are
bounded Linux observations, not release performance qualification.

Tier: retained benchmark evidence and documentation; no runtime, frontend,
scientific tolerance or artifact input changed. The original scheduler entrypoints
were wrapped only to copy and verify frozen datasets and require successful
predecessor accounting. Local copy/decoded-identity/tamper guards and shell syntax
passed before submission; their launcher hashes are retained. A login-Python
submission syntax error was corrected before any job was launched. No scientific
retry occurred. Evidence identities, CSV reconstruction, local links and
whitespace are checked at this documentation boundary; existing qualified
runtime evidence is reused. Raw captures remain at the report's SCC location.

## Performance-plan qualification

### C2 parallel preparation candidate, 2026-10-11–12

The [retained C2 evidence](../bench/results/c2-20261011-performance.json)
compares the isolated candidate with `d9865e57`. This is arithmetic-preserving
work: integer range checks and membership validation use the installed worker
team, and an optional counting/scatter constructor preserves the original
ascending observation order within every canonical level. Ordered error joins
preserve first-error precedence. Factorization, singleton semantics, sample,
rank, omission, separation, floating-point reductions and solver policy are
unchanged. Independent source factorization and a general topology rewrite
remain outside this candidate.

The counting/scatter path serves eligible unweighted reusable membership.
Optional membership scratch is admitted only after the original Fast/Auto/Low
route and existing cache decisions. Low mode receives none; insufficient spare
memory or optional allocation failure falls back to the original constructor.
The offer is capped at min(8N bytes, 128 MiB), source constructions are sequential,
and actual scratch capacities are checked. PPML consumes the offer only in
generation zero; later solver/cache admission excludes this already-freed
scratch. At 16M rows and 160,000 firm levels, twelve useful blocks require
46,080,288 transient bytes on the measured 64-bit hosts. Retained preparation
bytes are unchanged.

The final general scheduling rule requires at least four useful blocks and
limits block-count × level-count to N/8. A broader N/4/two-block prototype
regressed 1M-row membership construction by 43–51% at N/8 levels. Raising the
minimum to four still regressed the N/16-level boundary by 13–30% on a second
host. The final N/8 work cap declines those cases at every size, including large
cases where the broader policy had helped. Its one bounded final density
screen found no median increase beyond max(5%, 1 ms). All three policies'
observations remain separate in the evidence; no further constant tuning or
sampling continuation was used.

Final preparation diagnostics on SCC `scc-ed3` used two alternating process
pairs, each with one warmup and two measured preparations. Generation, cloning
and comparison were outside timing; the gated stage clocks were enabled.
Complete-result fingerprints and retained bytes agree across arms.

| AKM preparation, four controls | Workers | Baseline s | Candidate s | Reduction |
| --- | ---: | ---: | ---: | ---: |
| 1M rows | 16 | 0.052202 | 0.036342 | 30.4% |
| 16M rows | 16 | 0.930398 | 0.555805 | 40.3% |

These are preparation observations, not whole-fit or Mac speed claims. Earlier
three-way diagnostics separate the validation and membership changes; the final
artifact has its own measurements. At 16M rows, unchanged source factorization
still takes about 0.147 s in this probe, so substantial serial work remains.
The probe supplies no overlapped-load hint; complete native fits retain C1's
existing overlap and must be measured separately.

The final native panel reserved all 28 cores of SCC `scc-wo4` (two Xeon
E5-2680 v4 sockets), with tracing disabled. The twenty frozen standard workloads
used one warmup and seven alternating pairs: all 320 calls retain exact science
and solver signatures. Seventeen cases pass the prescribed paired-bootstrap
nonregression bound. No-FE K=5/four workers, three-FE K=5/one worker, and weighted
two-FE K=5/four workers remain inconclusive; their point ratios are 0.9989,
1.0347 and 0.9605, but upper bounds are 1.0666, 1.0569 and 1.0782 against 1.05.
Every point estimate is within its allowance. There was no continuation or
resampling. Thus this panel does not establish full standard nonregression.

Separate campaign-shaped AKM inputs used seed 739391, unit weights, robust
inference and three alternating pairs without a dedicated warmup. The table
uses median RRUNTIME **total** seconds, including OLS loading, rather than the
OLS native-only field used by the driver's default summary. These are
descriptive observations, not confidence-qualified performance passes.

| Native transaction | Workers | Baseline s | Candidate s | Change |
| --- | ---: | ---: | ---: | ---: |
| OLS 1M, K=16 | 1 | 3.481277 | 3.516486 | +1.0% |
| OLS 1M, K=16 | 4 | 2.050752 | 2.185939 | +6.6% |
| OLS 1M, K=16 | 16 | 1.350190 | 1.296027 | −4.0% |
| PPML 1M, K=16 | 1 | 17.742044 | 17.762970 | +0.1% |
| PPML 1M, K=16 | 4 | 8.844992 | 8.584369 | −2.9% |
| PPML 1M, K=16 | 16 | 6.125746 | 5.978354 | −2.4% |
| OLS 16M, K=4 | 4 | 21.586503 | 19.842369 | −8.1% |
| OLS 16M, K=4 | 16 | 12.824662 | 11.467164 | −10.6% |

The four-worker 1M OLS slowdown remains unresolved. Existing load and solver
clocks do not isolate its cause, and no extra samples were taken to erase it.
At sixteen workers, PPML's complete preparation phase changes from 0.921854 to
0.868597 seconds (5.8% lower), much less than the isolated unit-weighted OLS
preparation probe. Faster preparation therefore does not imply a corresponding
whole-fit gain or monotonic improvement with worker count.

The 16M panel's original 900-second budget includes replay transport and result
validation. It stopped during the ninth call, after eight successful calls.
One fixed continuation on the same host was limited to the four missing
originally planned calls and 600 additional seconds. The original timeout and
every successful observation are retained separately. No completed call was
repeated, and the intended design remains three pairs per cell. This operational
continuation makes no confidence-qualified claim and does not revisit any
standard-panel cell.

All 48 successful campaign-shaped calls retain exact baseline/candidate science
and solver signatures. Across worker counts, the retained headers preserve the
existing discrete-decision policy and the complete streamed columns have equal
hashes; coefficients and covariance agree within the existing replay tolerances.
The continuation finishes with scheduler accounting 0/0. The original timing
job's 0/1 exit remains explicit because of its budget stop. The larger OLS case
supports useful C2 gains, while the full record still falls short of uniform
nonregression or Mac qualification.

Independent review covered stable ordering, fallible allocation, original
admission boundaries and PPML scratch lifetimes. Witnesses include an independent
row-filter membership reference, malformed membership/error ordering, worker
counts 1/2/3/4/16, exact budget and density boundaries, injected reservation
failures, OLS moment-cache admission and PPML generation/rollback behavior.
The complexity ratchet records scoped increases of 231 core and 38 plugin
production lines, with no increase in oversized file/function counts.

The final source passes `./scripts/run_tests.sh` on SCC, including the
independent scientific oracles, allocator checks, Clippy, formatting and the
complexity gate. The Linux release artifact passes the glibc/dependency/export
audit, actual-plugin ABI/load/selftest and both UBSan numeric guards. Native
replay has 73/73 exact comparisons and 219 additional worker-count checks at
2/4/16 workers, with scientific agreement, fixed-worker repeatability and stable
discrete decisions. All eight mapped Stata fixtures and both OLS/PPML Break
checks pass. The evidence retains failed setup/launcher attempts and identifies
the completed components reused by their focused continuations.

Eighteen Linux-regenerated standard inputs differed from the pinned Mac hashes;
none were timed. Existing Mac generators recreated all twenty exact frozen
files in 9.7 seconds, then transferred them to SCC. This was data generation
only: no Mac build, estimation test or timing was run during this initial
experiment. At that point the candidate remained separate from main without
Mac qualification. The later Mac checks and adoption decision are recorded
[above](#c2-adoption-with-unresolved-timing-2026-10-12); neither experiment
establishes a release performance claim.

### A2 retained-guess prototype stopped at discovery, 2026-10-11

The [prototype record](../bench/results/a2-20261011-prototype.json) preserves
all twenty-four results, twelve baseline-bound scopes and recomputed comparisons,
phase rows, ordered traces, artifact identities and failed attempts. Every
scientific comparison passes, including exact sample, rank, omission, separation,
solver/fallback decisions and outer counts across workers. Six canonical calls
accept a guess. Total cost fails the predeclared benefit condition:

| Frozen PPML case | Workers | Baseline native s | Candidate native s |
| --- | ---: | ---: | ---: |
| AKM 1M | 1 | 9.604 | 9.483 |
| AKM 1M | 4 | 4.822 | 4.996 |
| AKM 1M | 16 | 3.832 | 3.757 |
| Dense 1M | 1 | 9.333 | 8.973 |
| Dense 1M | 4 | 5.082 | 5.058 |
| Dense 1M | 16 | 3.831 | 3.811 |
| Dense 4M | 1 | 38.958 | 38.843 |
| Dense 4M | 4 | 23.319 | 23.254 |
| Dense 4M | 16 | 18.999 | 19.025 |
| Sparse 1M | 1 | 35.673 | 35.215 |
| Sparse 1M | 4 | 17.458 | 21.671 |
| Sparse 1M | 16 | 20.116 | 20.537 |

Native totals are 191.026 versus 194.623 seconds, a 1.883% increase; RRUNTIME
totals are 191.013 versus 194.610 seconds. These are single traced, baseline-first
pairs on shared SCC host `scc-gd4`, not repeatable quiet-Mac performance evidence.
Candidate tracing and all setup, conversion, initial residual products, retries
and certification are included. The result stops discovery; it does not isolate
the cause of the observed difference. The plan separately explains why the
iteration counter alone does not establish saved operator work.

The [disposable reproducer](../bench/results/a2-20261011-prototype.patch)
binds base `e975bf3d`, eight runtime/test files and portable research tooling.
All twenty-one resulting file hashes match an exact-base application using
`git apply --unidiff-zero --whitespace=error`. The failed near-roundoff variant
is preserved as encoded patch bytes with a checked decoder. Two substantive repairs address a
nonimproving initial residual and retained-bank cleanup on early errors.
Sixteen focused runtime witnesses and thirty-four driver guards pass; all
seventeen test invocations and their failures/costs are retained (379.524 seconds).

Build 8028183 on `scc-ei3` passes Linux binary/ABI/load and numeric guards,
with accounting 0/0, 113 scheduler seconds and 4.230 GB maximum virtual memory.
Initial panel 8028199 rejects valid output metadata before fitting: zero fits,
10.680 application seconds, accounting 0/1. The narrow header-decoder correction
preserves this failure and the frozen runtime/input bytes. Corrected panel
8028338 completes twenty-four fits in 598.391 application seconds, accounting
0/1, 611 scheduler seconds and 6.226 GB maximum virtual memory. Exit 1 preserves
the failed cost gate; every terminal identity check passes. The discovery book,
including four eligibility fits, totals twenty-eight fits and 736.220 seconds.

Tier: decision and retained research evidence, unchanged production artifact.
The prototype has a declared numerical and inner-work scope but is not integrated.
No dependent full integration suite, mapped strict/dummy/covariance qualification
or quiet-Mac timing was run after the failed discovery prerequisite. Production
remains qualified E1, with no tolerance, sample/rank/omission/separation, route,
certificate, fresh strict PPML, memory-control or SPI change. Existing full-suite
evidence for identical production inputs is reused; retained-data integrity,
reproducer hashes, local links, production identity and whitespace are checked
at this documentation boundary.

### F2 geometry pricing and the missing total-cost model, 2026-10-11

All six frozen Auto fits retain exact public science with the task-only
geometry observer. The [pricing record](../bench/results/f2-20261011-geometry.json)
binds inputs, artifacts, both observations per fit and ten independent feature
checks. The [disposable reproducer](../bench/results/f2-20261011-geometry.patch)
preserves the checked contingency calculation and tooling against base
`67ba321a`. It changes no solver or numerical acceptance policy.

The dense 1M/4M four-sweep bounds are 1.916e-16 and 1.752e-17; sparse pair
bounds saturate at one and give a vacuous bound of 1393. Bounds match across
workers and primary/residual calls. They concern exact arithmetic with true
unit weights and one common constant; they are not floating-point convergence
certificates. Scratch capacities are 496, 792 and 80,000 bytes respectively.

| Frozen OLS case | Workers | Primary geometry ms | Residual geometry ms | Diagnostic allowance ms |
| --- | ---: | ---: | ---: | ---: |
| Dense 1M | 1 | 32.52 | 32.76 | 143.57 |
| Dense 1M | 16 | 33.97 | 36.25 | 42.30 |
| Dense 4M | 1 | 142.56 | 154.84 | 782.95 |
| Dense 4M | 16 | 146.02 | 162.27 | 177.26 |
| Sparse 1M | 1 | 21.06 | 21.00 | 595.94 |
| Sparse 1M | 16 | 20.94 | 21.50 | 178.00 |

Each feature computation includes three counting and three clearing passes,
allocation, marginal checks and trace cost. The allowance is max(5%, 1 ms)
against current-host native time minus both observer closing clocks. This
subtraction is an approximate diagnostic reference: it cannot remove cache
effects or supply an uninstrumented confidence interval. A hypothetical cache
charges the primary computation once; sparse overhead is 0.18%/0.59%, while
uncached dense sixteen-worker overhead is 8.30%/8.70%. No cache was implemented.
Earlier MAP/checkpoint timings on another host were not pooled with these costs.

The feature passes this discovery cost gate, but a total-cost selector remains
unidentified. N, levels, preconditioner and observed LSMR iterations are
confounded; rollback-copy and failed-attempt costs were not measured. The
plan names the missing controlled calibration and conservative comparison
against unavoidable Auto setup. No coefficients, route prototype or holdout
were produced. A passing geometric prediction would still require the ordinary
independent numerical certificate and the unchanged shared fallback budget.

Pricing job 8028143 used 127.343 application seconds on `scc-gd4`; all six
captures and terminal identity checks passed, without retry. Its isolated
build, job 8028142 on `scc-tb4`, passed Linux binary/ABI/load and numeric guards
in 170 scheduler seconds with accounting 0/0. Pricing accounting is also 0/0,
with 154 scheduler seconds and 5.125 GB maximum virtual memory. This is bounded
Linux diagnostic evidence, not quiet-Mac qualification.

Tier: decision and diagnostic tooling, unchanged production artifact. Independent
contingency/projection witnesses and exact retained outputs support the feature
observations only. No tolerance, sample, rank, omission, separation, route,
certificate, fresh strict PPML, memory control or SPI ownership changed.
Existing full repository evidence for identical E1 production inputs is reused;
retained-data integrity, local links and production identity are checked at
this documentation boundary.

### A3 calibration stopped at the sparse work limit, 2026-10-11

The [calibration record](../bench/results/a3-20261011-calibration.json) retains
all 62 attempted kernel calls and their ordered trace events. The first 36
passed the independent paired-dyadic projection and direct-Gram witnesses.
The additional panel completed twenty calls, refused five under unchanged
admission gates and timed out on its twenty-sixth call. That case has 16,384
rows, 2,048 levels on each side, degree four and seventeen RHS columns. Explicit
iterative geometry completed eleven RHS solves of 962–963 iterations before
the unchanged 30-second subprocess limit. It has no final certificate or
completed timing ratio. The remaining 154 calls were not attempted; there was
no retry, fitted rule, holdout or production selector.

The completed small kernels show both sides of the setup/solve tradeoff. On
the first host, degree-four one-worker direct solving took about 0.0238 seconds
versus 0.0430 for implicit; degree-64 sixteen-worker implicit took about 0.00895
versus 0.0144 for direct. These are two traced kernel repetitions, not quiet-Mac
qualification. The first and additional panels ran on different SCC hosts;
their wall times were not pooled. The frozen starting-book hash and final
artifact identities match exactly. Additional raw spectral features explicitly
remain uncertified diagnostics, not floating-point iteration guarantees.

The first kernel panel used 4.787 application seconds; the additional panel
used 34.136 seconds including validation. Together with the earlier eight
whole fits, the shared A3 book totals 253.263 seconds. Corrected plugin-FMA
builds cost 563 and 429 scheduler seconds, both with accounting 0/0. Kernel
jobs 8028122 and 8028140 took 17 and 46 scheduler seconds, respectively;
the latter reports `failed=0`, `exit_status=1`, preserving the timeout.
No unused call or time allowance authorizes continuing a stopped panel.

The [disposable reproducer](../bench/results/a3-20261011-calibration.patch)
binds base `57c5cd1e`, the diagnostic source, both frozen kernel generators and
drivers. It installs the unchanged runtime-selected plugin FMA table while
the generic core is compiled without global FMA. It is retained research
material, not an instruction to rerun the stopped campaign.

Tier: decision and diagnostic tooling, unchanged production artifact. The
independent witnesses above support completed kernel observations only.
Tolerances, sample, rank, omission, separation, default routes, certificates,
fresh strict PPML, memory controls and SPI ownership remain unchanged. Existing
full repository checks for identical E1 production inputs are reused; local
links, retained-data integrity and production identity are checked at this
documentation boundary. No new platform or release-performance claim follows.

### A2/A3/F2 first diagnostic panels, 2026-10-11

The [retained discovery record](../bench/results/fable-20261011-discovery.json)
binds twenty-four traced SCC calls to the qualified E1 numerical program and
the committed diagnostic patch. Production defaults remain unchanged. This
record contains opportunity and rejection evidence, not performance qualification.
It packs the complete scientific matrices without changing any bits and keeps
the input identities, comparisons, phase costs and failed attempts.

All four A2 preflight scientific fingerprints exactly match retained E1 Linux.
Initial/strict anchor tuples match on every case. AKM has a compatible iterative
strict consumer whose reduced solve costs 0.959 seconds; dense 4M has only
0.056 seconds of such work, and dense 1M uses direct solving. Sparse has 445
matching component anchors but both strict canonical attempts decline. These
results support a bounded A2 prototype, chiefly for AKM, while requiring sparse
fallback decisions to remain exact. No guess was retained or warm solve run.
The four calls took 127.149 seconds including capture and validation.

All eight A3 whole-fit calls completed, and all four scoped scientific
comparisons passed with four outer iterations in every fit:

| Frozen dense PPML | Workers | Current native s | Forced implicit native s |
| --- | ---: | ---: | ---: |
| 1M | 1 | 9.332 | 8.349 |
| 1M | 16 | 3.806 | 2.721 |
| 4M | 1 | 39.053 | 31.699 |
| 4M | 16 | 18.427 | 10.896 |

Implicit reduced solving costs more in every cell. Avoiding serial pair
assembly saves about 1.08 seconds over two canonical calls at 1M and 7.9 seconds
at 4M, explaining most of the total differences. Planned canonical storage
increases from 12.05 to 24.46 MB and 48.51 to 96.92 MB respectively. No pair-work
admission rejection was exercised, so these results do not support broadening
that gate. This is one traced pair per cell on a shared host; neither a general
iteration predictor nor a quiet-Mac performance claim follows. The stage used
214.340 seconds of its shared 720-second first-subset budget.

F2 completed all twelve checkpoint calls in 229.751 seconds: ten successful
captures and two sparse MAP iteration-exhaustion failures. The four dense
comparisons pass the unchanged coefficient, covariance and strong fit-statistic
bounds. Sparse's four MAP sweeps plus checkpoint certificate cost 1.725 seconds
at one worker, against a 0.614-second allowance from same-host Auto. Even granting
free final-residual work leaves at least 11.30% modeled overhead before copying,
setup and the initial certificate. This rejects the general four-sweep pilot.
At sixteen workers, numerical work alone is 0.286 seconds against 0.138 seconds;
the hypothetical free-residual bound is inconclusive. No failed fit receives a
completed timing ratio, and no conditional geometry rule has been qualified.

Bundle job 8028069 reports scheduler `failed=0`, `exit_status=1`, 600 seconds and
6.253 GB maximum virtual memory; the nonzero exit preserves the two expected MAP
failures. Frozen input reconstruction first failed its complete-file identity
gate because new templates changed OLS VCE and PPML separation metadata. The
original extractor was recovered, and all seven original complete-file hashes
then matched: corrected job 8028057 reports `failed=0`, `exit_status=0`, 45 seconds
and 2.401 GB. No fit used the failed extraction. A generic core calibration build
was canceled before measurements because it did not install the plugin FMA table;
its 283 seconds remain a cost, not calibration evidence. Corrected calibration
and all prototype/holdout results are outside this record.

Tier: decision and diagnostic tooling, unchanged production artifact. No
tolerance, sample, rank, omission, separation, default route, independent
certificate, fresh strict PPML requirement, memory control or SPI ownership
changed. The recorded full repository checks for the identical E1 production
inputs remain applicable; these observations add no platform qualification.

### F1 numerical diagnosis: accepted projected controls, 2026-10-11

The [retained diagnosis](../bench/results/f1-20261011-diagnosis.json) isolates
the material weak-control Auto robust-F discrepancy in its initial accepted
within-X bank. Against independent FE QR, the largest within-X error is
2.57e-6 for Auto and 3.27e-13 for LSMR. An independent inverse of the same
accepted score design differs from Auto's bread by only 1.03e-18. The
80-digit F decomposition attributes +1.4398833e-5 to the accepted score design,
+1.08e-11 to bread inversion and -1.95e-11 to remaining covariance arithmetic;
final residual and Wald effects are also small. This diagnoses the error source
on the captured weak control; it does not declare a production repair.

Eight one-worker Mac captures used qualified E1 source `d3fed590`: Auto at
1e-8, diagnostic Auto at 1e-10, LSMR and valid CMG on the hash-matched barbell
and weak controls. All coefficient, covariance, fitted/residual and within-statistic
witnesses passed. Four strong-F checks passed and four failed at the unchanged
rtol=1e-12/atol=1e-14 bound. Tighter Auto still failed both controls, by factors
2.8194 and 1.1763; scalar tightening is not an established repair. Valid CMG
and LSMR passed these two independent F witnesses. The original sixty-fit
panel's twenty invalid requests and eight comparison failures remain failed.

Two follow-up Auto/LSMR captures used a
[task-only diagnostic patch](../bench/results/fable-20261011-diagnostic.patch).
Every public scientific bit, solver signature and external observation byte
matched its uninstrumented arm. Both capture lengths and terminal markers were
exact. The record binds qualified/probe/harness artifacts, all scientific
matrix banks, projection/bread/scale mappings, and independent QR and 80-digit
algebra. Full-data QR remains binary64; compensated Gram/meat product rounding
and independent same-score QR cross-checks are recorded explicitly.

The eight-call stage took 4.876 seconds within 120 seconds per call and 600
seconds total; the two-capture stage, including decomposition, took 4.081
seconds within 60 seconds per call and 120 seconds total. Neither stage retried.
These are diagnostic task costs, not performance qualification: the probe's
8 KiB capture writer runs inside existing native/certificate clocks. No
production runtime, tolerance, model decision or default route changed, and
no handoff threshold follows from these captures.

The unchanged-artifact check against `57c5cd1e` matches all 170 runtime modules
and 58 artifact inputs. `./scripts/run_tests.sh`, local documentation links and
whitespace checks pass; the test transcript digest is
`fa1fc1a468aa4f4c571225c908833e57569c9375cf268cac07e202315128a2f9`.
The transcript is not retained, and these checks make no new timing claim.

### Performance-plan F: comparative failures and route decisions, 2026-10-11

The automatic routes remain unchanged. The
[retained F1/F2 comparisons](../bench/results/f-20261011-comparisons.json)
use qualified E1 source `d3fed590` and its Linux native artifact
`1f6de670a17c6bc08d2fa3e482c4cdec80e1903cadb5313fc27531c80502cf9f`.
They are bounded diagnostic comparisons of existing explicit requests, with
phase/solver tracing and two cold, reversed-order repetitions on a shared SCC
host. They do not qualify a route change or a performance claim. Frozen input
and body hashes, every fit status, exact-repeat checks, phase/solver/memory rows,
certificate gates and scheduler accounting are retained. Large campaign cases
have cross-route checks and independent runtime certificates; the three smaller
graph controls also have frozen full-dummy QR/HC1 witnesses.

F1 job 8027562 attempted all sixty fits: 32 passed and 28 failed. Twenty CMG
requests failed syntax because the task driver retained `preconditioner=auto`,
which correctly requires `off` for explicit CMG. These are request-rejection
costs, not CMG solve observations. Eight LSMR comparisons on barbell and weak
mobility failed the unchanged stronger robust-F bound (rtol=1e-12, atol=1e-14),
by factors 1.63 and 595.04. Both routes passed independent coefficient,
covariance, fitted/residual and within-statistic checks. An additional
independent QR/HC1 Wald calculation puts Auto farther from that reference than
LSMR on those controls; it does not override the failed comparison, establish a
production bug by itself, or authorize a relaxed bound.

F1's raw one-worker native costs include about 0.175 seconds for Auto versus
0.114 for LSMR on the chain, but 1.77 versus 7.29 on frozen 1M AKM and 1.18
versus 3.75 on dense two-FE 1M. No failed cell receives an accepted ratio.
The panel took 221.734 process seconds (224 scheduler seconds); accounting
reports `failed=0`, `exit_status=1`, as expected for a failed comparison.
There was no retry. The observed chain reports 127 iterations; the historical
4,246-iteration reproducer was unavailable, and ordinary PCG's iteration-eight
progress was not observed. A handoff threshold cannot be calibrated from this
panel.

Before F2's first fit, the invalid MAP request construction was corrected to
`preconditioner=off`, guarded by an exactly-one-original-auto check. All other
arguments and body bytes, tolerances and the predeclared limits remained
unchanged. The original held job 8027564 was cancelled without running; new
job 8027931 used separately hashed tooling and qualification bindings.

F2 attempted all 24 fits: twenty passed, and all four sparse MAP fits timed out
at 120 seconds, at both one and sixteen workers. The 1,200-second panel budget
was not extended: elapsed process time was 931.087 seconds, scheduler time
932 seconds, and maximum virtual memory 5.517 GB. Accounting reports
`failed=0`, `exit_status=1`; there is no success sentinel. Sparse Auto completed
in 12.18–12.32 native seconds at one worker and 2.95–2.98 at sixteen. Timeout
durations are whole-subprocess limits, not completed native timings.

Dense median native seconds, diagnostic only:

| Frozen case | Workers | Auto | Explicit MAP |
| --- | ---: | ---: | ---: |
| 1M three-FE | 1 | 3.140 | 2.724 |
| 1M three-FE | 16 | 1.035 | 0.911 |
| 4M three-FE | 1 | 16.803 | 12.919 |
| 4M three-FE | 16 | 4.814 | 3.519 |

The 4M Auto sixteen-worker observations range from 4.324 to 5.303 seconds;
two shared-host repetitions cannot establish a confidence bound. Auto used
Diagonal LSMR with four primary iterations on dense 1M, Schwarz LSMR with ten
on dense 4M, and Schwarz LSMR with 22 on sparse 1M. Dense MAP took four primary
iterations. Costs include different setup, projection and final certificate
work, not just the primary iteration count. For example, one-worker modeled
native peaks were 1.125/0.554 GB (Auto/MAP) on dense 1M and 5.461/2.216 GB on
dense 4M. These planner estimates are not observed RSS. Timeout fits have no
completed solver/certificate/memory packet and receive no diagnostic ratio.

The [F1 and F2 follow-ups](fable_performance.md#f1-authorized-diagnosis-resolve-the-comparison-before-handoff)
recommend retaining Auto. F1 needs bounded numerical/request/progress diagnosis;
F2 needs a defensible geometry-and-total-cost model and a separately authorized
holdout before any route prototype. Johannes subsequently authorized their
bounded follow-up, including separate A2/A3 discovery and costing, as recorded
in the plan; that authorization changes none of these failed outcomes. No default
route, tolerance, model semantics, certificate, fresh strict PPML solve, memory
policy or SPI ownership changed in this comparison work. Production identity
against E1 remains exact across 170 runtime modules and 58 artifact inputs;
its qualified runtime evidence is reused for this documentation-only change.

### Performance-plan E1: qualified declared numerical scope, 2026-10-11

E1 passes against qualified C/D (`aeb6ee64`). The
[retained observations, scopes and artifact identities](../bench/results/e1-20261011-performance.json)
include every Mac timing pair, Linux instruction counts, independent numerical
witnesses and failed attempts. Fixed 512-row leaves and ordered merges make the
norm and independent PPML finite-score reductions independent of worker count.
They reuse idle, already-admitted scratch and preserve the single-leaf arithmetic.
No tolerance, sample, rank, omission, separation, solver route, inner-work counter,
certificate decision or outer iteration changed. Strict/canonical PPML solving
and final-Hessian checks remain fresh. No worker touches SPI.

Scaled norm summaries retain their largest rounded weighted element, preserving
the D1 rejection proof. Score leaves retain both compensated components: the
first attempted implementation rounded each leaf to a single total and lost a
residual of one across `1e30`, `1`, and `-1e30`. Independent review rejected it
before qualification. The repair has signed-cancellation and exact dyadic
witnesses. Norm witnesses cover fixed leaves, dynamic scales, errors, scratch
capacities and 1/4/16 workers; independent full-dummy weighted QR covers the three
32,768-row chain/barbell/weak-mobility controls and their robust covariance.

The first large comparison also failed its scope: OLS Gaussian null likelihood
depends on the already-scoped within-TSS. The original manifest and failed
comparison remain failed in the retained record. Before fresh candidate calls,
the corrected manifest explicitly named that leaf and added independent FE-only
weighted-QR witnesses at the existing rtol=1e-12/atol=1e-14 statistic bounds.
No runtime, tolerance, decision or work-counter exception repaired this failure.
Other recorded repairs concern external-column validation, constant visibility,
Clippy style and a superseded scalar-order test assertion.

The full repository gate passed: 1,176 core tests, 143 non-FFI plugin tests,
173 FFI plugin tests, ten isolated allocator audits, portable checks, independent
oracles, transaction mocks and complexity. All 73 small replay cases remain
exact against C/D; additional 4/16-worker checks retain exact repeats and
protected decisions. All 67 licensed fixtures and public OLS/PPML Break probes
passed. These small cases do not certify the new multileaf arithmetic by
themselves: the separate Mac 36-fit graph panel, SCC 84-fit campaign/graph panel
and Mac 40-fit timing-input science preflight all passed their explicit scopes,
protected decisions and numerical witnesses. The final source matches the frozen
measured artifact inputs across 170 runtime modules and 58 other inputs.

SCC job 8027197 completed 84 discovery fits at 1/4/16 workers in 1,278 scheduler
seconds (6.543 GB maximum virtual memory), with successful accounting. Profiling
job 8027203 completed eight native witnesses and sixteen instrumented fits in
482 scheduler seconds. Native/instrumented science and repeats agree exactly.
Two instruction-count repetitions show dense PPML +1.179%, sparse PPML +1.046%,
AKM PPML +0.215%, and dense three-FE OLS -0.269%. Library-FMA instruction cost
is zero in every profile. These are costs as well as gains; prior timing was
not reused because the positive changes exceed the observed repeat ranges.

Fresh Mac timing completed all 640 fits across twenty standard cells in
1,269.685 seconds of the 1,800-second budget. Every adjusted upper bound passes
max(5%, 1 ms), with no continuation. The exact two-look rule is declared in the
[plan](fable_performance.md#e-declaration-before-implementation-2026-10-11).
Observed K=20 median reductions are 2.53%, 2.16% and 4.39% at 4/8/16 workers; single-worker
K=20 costs 0.27%. Weighted two-FE OLS at four workers is 1.48% lower, and unit-weight
K=5 is 2.82% lower. Single-worker PPML costs 1.36%; four-worker PPML is near zero.
No-FE four-worker OLS costs 5.47% (about 0.26 ms), within the unchanged 1 ms
allowance. Other cells are mixed and mostly near zero. The largest upper bound
among the 5%-allowance cells is 1.049008, so this is a nonregression qualification,
not a claim of a general speedup. Per-cell intervals and all observations remain
in the retained record. Background desktop applications stayed open; there were
no competing builds/tests/agent workloads or recorded thermal warnings.

E2 and E3 are deferred for lack of compatible work, as explained in the
[coverage decision](fable_performance.md#e2e3-coverage-decision-2026-10-11).
All observed ordinary PPML frames used the joint basis, bypassing E2's proposed
direct projection. E3 cannot subtract an FE fit twice from an already-within
residual, and the raw-residual paths lack the required retained bank. No new
route or storage was introduced to manufacture eligibility.

### Performance-plan C/D: qualified combined workstream, 2026-10-11

C/D passes arithmetic-preserving qualification. The
[compact observations and artifact identities](../bench/results/cd-20261011-performance.json)
retain the Mac pairs, confidence bounds, Linux instruction counts, campaign
phase rows and installation follow-up. No tolerance, sample, rank, omission,
separation, strict PPML solve or independent certificate changed.

The preserving candidate overlaps optional factorization of Rust-owned
categorical inputs with caller-thread numeric loading, for OLS and PPML.
Full classification, pruning, topology, DoF and generation ownership remain
authoritative after loading. A capsule can replace encodings only when original
rows, source identities and canonical values agree; unsupported layouts or
insufficient admitted storage decline the optimization. PPML consumes it once
in generation zero and charges its complete transient capacity. No worker calls
SPI. Errors, Break and panics join workers before rollback. Cancellation during
factorization can wait for the current source's factorization; the polling
interval is not a bound on that inner kernel.

C3 schedules residual reconstruction and completeness scans by rows without
changing each numerical recurrence. C4 safely initializes already-reserved
observation banks on the installed team, retaining allocation order and capacity
checks. D1 can reject the zero-fit shortcut from the first group's unchanged
certificate arithmetic only inside the independently reviewed finite-error
bounds; accepting shortcuts and final certificates still run fully. D2 reuses
certified norms only for the identical matrix and normalization. D3 tiles Schur
and Gram updates while preserving each element's observation order. The norm
and score reductions that would reassociate arithmetic remain in E. The C2
diagnosis below did not support speculative topology changes.

Costs include validation of reused encodings, an extra raw-weight minimum scan
for the bounded weighted screen, and zero initialization before parallel
residual reconstruction. Parallel initialization can expose fewer lanes than
requested when binary splitting encounters awkward sizes. These costs must be
judged with the combined measurements, not omitted from the performance claim.

The full repository gate passed, including 1,167 active core tests, 173 active
FFI plugin tests and all ten separately launched allocator audits, independent
numerical oracles and transaction mocks. The first gate stopped at two Clippy
style findings (an equivalent boolean form and a test struct initializer);
neither repair changed arithmetic or scientific policy. Independent review
found no blocking ownership, error-precedence or certificate-proof issue.
The universal Mac build passed ABI, dependency and actual-plugin load checks.
All 73 replay cases passed with zero exact differences against qualified B,
unchanged solver signatures, scientific witnesses and fixed-worker repeats.
All 67 licensed fixtures and OLS/PPML public Break probes passed.
Before commit, all 170 production module token streams and 58 other artifact
inputs were checked against the frozen measured source and remained identical.

Predeclared measurement bounds: SCC uses four frozen 50k shapes,
two repetitions per plugin, one worker, 600 seconds per instrumented fit and
2,700 seconds total; the seven full frozen campaign cases use 1/4 workers,
two alternating pairs, 120 seconds per fit and 2,400 seconds total. The C2/C4
AKM installation follow-up uses 1/4/8/16 workers, one warmup and three measured
pairs, 120 seconds per fit and 900 seconds total. Linux phase and solver traces
are diagnostic. The quiet Mac uses all twenty regenerated, hash-verified
standard workloads, including K=20 at eight and sixteen workers, with tracing
disabled and no competing measured-host work.

Mac timing has two predeclared looks: fifteen paired measurements initially
(1,800-second budget, 90 seconds per fit), then at most fifteen additional pairs
for cells whose point estimate passes but upper bound is inconclusive, in the
same sorted case order (900-second total continuation budget). An initial point
estimate above its allowance stops all continuation. Each look uses the larger
of the 98.75th-percentile paired-median bootstrap endpoint (10,000 draws, seed
739391) and an order-statistic median upper bound with binomial tail at most
0.0125. The latter uses rank 13 of 15 and rank 22 of 30; it avoids relying on
nominal small-sample bootstrap coverage. All valid pairs are retained and pooled,
and the ordinary max(5%, 1 ms) allowance is recomputed from the pooled baseline.
Artifact/input identities and science fingerprints must agree across blocks.
Incomplete blocks cannot qualify, and there is no third look. The median-ratio
inference assumes stationary independent pairs; the estimated-baseline allowance
retains the existing protocol's limitation. These are per-cell bounds, not
simultaneous intervals; overall acceptance requires every cell to pass.

Candidate/check SHA-256 identities: source archive
`bd9a50e3a0c39458abf13f4185b3576a9600a01eb91f268281e8d8072c8c11e5`,
source patch `237262a329cc496a4d0b85180ffe95f7a079cf448df0c5b1324886c45cba6e94`,
full gate `0d673bf089eb0235903f7116eadb6d3b19a5f4c1b834701b73e7d04242472f1d`,
initial style failure
`16476f2a994366016edf1a5cf2c3fd6b229c7b6618820272535229bad3aa13b8`,
Mac plugin `4e06dd13bd5d363a197c4d9cd0a0f8cfac99a7f46f815da1b137e0e4666d0d83`,
Mac exact-replay summary
`de03f92fe00bbca41c576dce5a8eca1191a1297a495b2565d4f60ef6798f7114`,
and timing protocol
`4e411fac43fe84201dba8aad34df2367edf3a6e453dacc1b4003bdb4a6097b1e`.

While SCC queued, a bounded Mac phase probe ran one traced baseline/candidate
pair on each frozen K=20 OLS case at 1/4/8/16 workers. All eight fits agreed
exactly. Native seconds were 1.19236/1.02968, 0.56890/0.53278,
0.46653/0.45228 and 0.44441/0.41507 (baseline/candidate). These single pairs
are diagnostics, not qualification. The probe summary hash is
`9fdf772cb0d1a86f96f6c66ee0e6164025c51c71ccb08e3afba481a9f426f378`.

SCC instruction job 8026782 stopped after 16 seconds because its copied driver
still required an executed `__fma_fma3` symbol in the comparator. Qualified B
already removed those calls. The corrected driver retains the exact libm build
ID, matching debug-file hash, validated Valgrind identity, full instruction
totals and native/scientific comparisons; it allows the expected empty FMA count.
No runtime artifact changed. The failed summary hash is
`fa7d0755d9cccf35a7d7a048dc28ccf8dade526d0787fd9afa2177536775b1dd`;
its log hash is
`ae3ec5a177430a44a8ae5487d6aa0a81a555efb5d87b552412bab5766953edc1`.
The bounded retry is job 8026800.

The quiet Mac panel completed all 640 fits in 1,280.54 seconds, below its
1,800-second bound. Every cell passed the predeclared upper bound; no
continuation was needed. The host was Mac14,14, Apple M2 Ultra, with 24 logical
CPUs. There were no competing builds, tests or agent tool workloads. Ordinary
desktop activity remained visible, including Activity Monitor and sysmond;
the qualification is not a claim of a bare operating system. All measured
outputs and solver signatures were exact across baseline, candidate and repeats.

Paired-median native-time changes (negative is faster):

| Frozen Mac family | Workers | Change |
| --- | --- | --- |
| OLS, no FE, K=5 | 1 / 4 | +0.10% / −0.21% |
| OLS, three FE, K=5 | 1 / 4 | −0.04% / −0.72% |
| OLS, two FE, pweights, K=5 | 1 / 4 | −9.77% / −4.80% |
| OLS, two FE, unit weights, K=5 | 1 / 4 | −10.56% / −6.86% |
| OLS, two FE, K=20 | 1 / 4 / 8 / 16 | −13.35% / −8.45% / −5.60% / −5.13% |
| OLS, wide K=64 | 1 / 4 | −5.36% / −4.24% |
| OLS, wide K=65 | 1 / 4 | −4.26% / −5.95% |
| OLS, wide K=129 | 1 / 4 | −3.19% / −2.53% |
| PPML, two FE, K=5 | 1 / 4 | +0.26% / −0.40% |

These point estimates do not establish every apparent gain. In particular,
the K=129 four-worker interval includes no improvement. The largest acceptance
upper bound among cells with the 5% allowance was 1.012652, below 1.05.
Small gains and the small PPML/no-FE costs remain observations, not failures.
Mac summary and assessment hashes are respectively
`82c4fcc86a14e13772b2ec6a5d190ca8e81fda8ad05d5534aacf4715295e2dc3`
and `e54085e4a599f141aac5263a99d51dd2ad65a35ecdf4a432001f4c020f892249`.

SCC build job 8026744 passed native/profiling ABI and load checks, all eight
hardware witnesses and the 73-case exact replay. Jobs 8026800, 8026783 and
8026784 passed with `failed=0`, `exit_status=0`, and scheduler wall times
472, 1,181 and 315 seconds. They shared `scc-ei3`, Xeon Gold 6226R at 2.90 GHz,
and overlapped, with 1/4/16 slots and observed affinity exposing 32 logical CPUs.
These are bounded Linux diagnostics, not quiet timing or Linux qualification.

The four 50k instruction cells had 16 instrumented fits and eight native
witness fits. Both repetitions and both plugins were exact; executed library
FMA instruction totals were zero under the validated symbol/tool bindings.
Mean instruction changes were −8.0583% (AKM PPML), −0.4261% (dense three-FE
OLS), **+0.0294%** (dense PPML) and **+0.0159%** (sparse PPML). The latter two
are small measured costs, not reductions or values erased as noise. The
retained counts include both repetitions; two samples do not establish a
precise sampling distribution. The panel took 468.56 seconds of its 2,700-second
budget.

The full campaign panel's 56 fits were exact at 1/4 workers and took 1,168.99
seconds of 2,400. OLS AKM improved in the observed pairs. Most PPML total times
were close; dense PPML at four workers had approximately 2.7% (1M) and 2.3%
(4M) positive costs. Two pairs on a shared host do not support a Linux speedup
or nonregression claim. All PPML phase rows and OLS rows 18–21 are retained in
the compact results, including these costs.

The 32-fit installation follow-up compared C/D with the earlier preparation
snapshot and remained exact. Its medians were:

| Workers | Preparation, before → after (s) | Installation, before → after (s) | Candidate observation bank |
| ---: | ---: | ---: | ---: |
| 1 | 0.67691 → 0.67561 | 0.11581 → 0.11586 | 8 MB |
| 4 | 0.53475 → 0.50528 | 0.12975 → 0.11197 | 32 MB |
| 8 | 0.50843 → 0.47562 | 0.13927 → 0.11497 | 64 MB |
| 16 | 0.51789 → 0.47624 | 0.15640 → 0.12048 | 128 MB |

This supports the preparation/installation hypothesis at higher worker counts,
but the comparison combines C1, D1 and C4 and is not an isolated component
ablation. Whole-fit PPML changes were much smaller than the subphase reductions.
Sixteen-worker involuntary context switches varied substantially; all three
measured observations and their ranges remain in the compact results. The
panel took 312.38 seconds of its 900-second budget.

Linux hashes: native plugin
`35b9b214faf160349a4dab2b2cffb31b025bb1c59169d555719a161b17c4d5a5`,
profiling plugin
`e2b9b9df0ad1a3b3867e8aa63227c82337a41c0136ef52bdfa646103c6adb297`,
exact replay
`7d805fc4e15debfc1ae643d7c0c5b5b8b871fe5419b8e071462484208ac42c07`,
instruction summary
`8295fc9aa5e4eb6cf0a2a393420d574c46b20de37e42012af7cca5fe360ed8dd`,
full campaign summary
`386ee98ff14ce729d4a4266ff2f931aa62346980c4fda910dd271e05a0aebd15`,
and installation summary
`dfd93521cb08bf016b284367c9a9bbc240448c5edb05faba895ec8042d95286d`.

### Performance-plan C/D: preparation diagnosis, 2026-10-11

The first C/D diagnostic snapshot contained C3 residual/sample scheduling,
D2 norm handoff, D3 ordered tiles and gated preparation clocks. It preceded
C1 overlap, D1's initial-certificate screen and C4 workspace first-touch.
Its 73 Mac replay cases had zero exact differences against qualified B,
including solver signatures, independent science checks and fixed-worker
repeats. The universal Mac build passed ABI, dependency and actual-plugin
load checks. The initial core suite passed 1,154 tests (three ignored), and
Clippy passed; these are snapshot checks, not final C/D qualification.

C2 first measured the preparation interval instead of assuming that the
plan's earlier worker-count growth persisted. SCC job 8026546 used the frozen
campaign inputs for AKM, dense and sparse two-FE PPML at one million rows,
1/4/8/16 workers, one B comparator, one candidate warmup and three candidate
measurements per cell. Every one of the 60 fits passed its certificates and
exact comparison, including solver signatures. Both phase and solver tracing
were enabled. The 120-second per-fit and 1,800-second total bounds held:
the panel took 980.68 seconds. The shared host was `scc-gd4`, Xeon Gold 6242
at 2.80 GHz, with 16 SGE slots and observed affinity exposing 32 logical CPUs.
Build job 8026545 and panel job 8026546 both had `failed=0` and
`exit_status=0`. These are descriptive Linux phase measurements, not quiet-Mac
qualification.

Median preparation seconds (RRUNTIME row 28), with the observed range across
three measured repetitions in parentheses:

| Frozen PPML case | 1 worker | 4 workers | 8 workers | 16 workers |
| --- | ---: | ---: | ---: | ---: |
| AKM 1M | 0.67560 (0.67530–0.67726) | 0.53218 (0.53154–0.53607) | 0.50815 (0.50425–0.51582) | 0.52153 (0.51199–0.54837) |
| Dense 1M | 0.62779 (0.62698–0.62916) | 0.48013 (0.47073–0.48105) | 0.44747 (0.44391–0.44766) | 0.45815 (0.45552–0.46162) |
| Sparse 1M | 0.64470 (0.64408–0.64761) | 0.50394 (0.48610–0.50459) | 0.47362 (0.47257–0.47670) | 0.48691 (0.48370–0.49475) |

Preparation spans input classification, initial model-generation construction
and IRLS setup; it is not just factorization/topology. On AKM, their respective
one-worker medians were 0.06481, 0.14424 and 0.46657 seconds. The nested
`shared_model` clock includes materialization, factorization, memberships,
topology and cluster binding; it fell from 0.13251 to 0.10719 seconds at sixteen
workers. IRLS workspace setup fell from 0.17291 to 0.05624 seconds and the
initial independent score from 0.08309 to 0.01724 seconds. Session installation
instead rose from 0.11657 to 0.15697 seconds; dense and sparse installation
also rose, from 0.08479 to 0.11610 and 0.10944 to 0.15101 seconds. Thus the
broad one-to-sixteen growth hypothesis was not reproduced. The modest
eight-to-sixteen increase points to installation, not factorization, as the
next bounded experiment. Three samples and shared-host scheduling do not
support a precise speedup claim. Sparse total fit time also rose at sixteen
workers despite the relatively small preparation change; that cost is not
explained away by this diagnosis.

Inspection found a distinct serial initialization of an N-row observation
scratch bank for each admitted coefficient worker. At N=1M and seventeen
right-hand sides its nominal size rises from 8 MB to 128 MB between one and
sixteen workers. This is a hypothesis for part of installation's growth,
not proof of causality. The next first-touch comparison must report actual
workspace admission, installation time and total preparation time, while
retaining reservation order, capacities and exact scientific outputs.

The snapshot's gated clocks do not take timestamps when phase tracing is
disabled. Its four diagnostic lines separate inclusive and nested intervals;
the initial-generation envelope must not be added to its own subphases.
Hashes (SHA-256): source archive
`f2c31d5d19a6660619c922566835814ece632dbe3c780f2ac350d0108135510c`,
source patch `cedb853403ca72b269fec8384847ccd0f44a0c5470550f2421c78fcac6b5be68`,
Mac plugin `fac5196b442e82d67a98a45316603053c6f8423d5b7980bd4f648cdd36dd9850`,
Linux plugin `ee5b2a8bc6f8a725c82fcaddbf563d806117eaf133e9f61d8dcdca3407ed125f`,
Mac exact-replay summary
`114ae77ae2982b5986449624d0ed8635d06c7366561911adbaaa31160c9d92a6`,
and completed SCC panel summary
`f2d08c8c3c4ea3db77e56a750fd9aff7945eb1de6f8aa08fe6c18574c0e61162`.
The Linux build job 8026545 also ran all eight hardware-FMA witnesses,
including the new ordered upper-triangle tiles.

During witness development, the first core compile required an explicit `f64`
annotation in a test vector; no runtime repair was needed. A standalone plugin
test invocation also omitted the prescribed serial test runner and failed seven
process-global allocator assertions. Repeating it with `--test-threads=1` passed
all 171 active tests (ten isolated audits ignored at this focused stage). The
full repository gate remains responsible for running those audits in separate
processes. These invocation failures are not scientific passes or hidden
tolerance changes. The concurrent-run log hash is
`982f5c138b444975de40f186662c102ef821d9b98b27ec62a2a0d7e96efa9b1f`;
the serial focused-run hash is
`d391c4665088fab438addbb93f84fefa54b0584c62aa491bdf14a2a9ce9b8c5c`.

The new PPML encoding-reuse witnesses also exposed test-fixture defects before
qualification. The exact-plan comparison initially gave only one arm a clone,
which changed its measured spare capacity; both arms now clone identically.
The direct-core allocation witness initially omitted the existing allocator
observation baseline, so the observed peak included the fake SPI map and other
preexisting test owners. Beginning the observation before each fit fixes that
boundary; the required reduction of at least two allocation events per cached
source then passed. Ordinary Rust equality also rejected identical NaNs in
omitted standard errors and z statistics; these fields require IEEE-bit
comparison. None of these repairs changes runtime arithmetic or a tolerance.
The separation comparison also excludes observed method durations, as the
replay contract requires, while comparing every decision and certificate.
The failed core capacity and inference runs have hashes
`1c4e423aa199c5e4e1b6788c600fd4db1cd71dcc0ca94382de40fe9960da93c1`
and `284bd858c6a47abb9869cdc93866fe6632e379ad8ee02fdcef401163279bb305`;
the failed plugin baseline run has hash
`45f1244d946b2d6fc589b6435908bd8468b01c03f7967f7fd8c2fcb010fcee7a`.

### Performance-plan A1: failed frame-adoption experiment, 2026-10-11

A1 did not meet its conditional acceptance criteria and is not in the runtime.
The experiment used source `68efe752` plus fresh canonical provenance, one-use
strict adoption, exact response/weight/scale/generation and canonical-basis
checks, and saved pre-acceptance movement. Both accepted convergence passes,
independent certificates, exact accepted-Hessian validation, objective replay,
eta reconstruction, rollback and all existing tolerances remained required.
This was an unqualified declared-scope experiment, not an accepted ulp-scope
change; SPEC was not amended and no affected-scope exception was installed.

The conservative first variant excluded profiled curves and never attempted
reuse on the target two-FE AKM case. Removing that exclusion while retaining
exact candidate identity exposed two unchanged core failures: the small
DynamicRange Low fixture had eta error `1.89640009941443824e-9` against its
`1e-10` bound, and the disconnected-slope fixture observed one accepted canonical
product where zero was required. Neither bound nor expectation was changed.
The third variant restricted precomputation to the existing admitted fast
categorical-product family; all 1,148 core tests passed (three ignored), including
freshness, response/weight/scale/tolerance invalidation and same-rank basis
rollback witnesses. This family restriction introduced no workload-size rule.

The universal Mac build passed ABI, dependency and actual-plugin load checks.
All 73 independent replay science checks and fixed-worker repeat checks passed,
but exact comparison with B failed in seven PPML fixtures: `ppml-09`, both bias
modes, and the FE/IR/Mu/simplex separation fixtures. The other 66 were exact;
all 38 PPML outer counts and discrete views were exact. No scientific tolerance
was loosened and these differences were not silently exempted. All 67 licensed
fixtures and both OLS and public-scope PPML Break probes passed. The additional
named coefficient/covariance/explicit-dummy qualification panels and quiet-Mac
performance gate were not run after the savings hypothesis failed.

An exploratory, traced Mac one-worker AKM-shaped one-million-row pair cost
`5.400968 -> 5.782367` seconds. Outer iterations stayed five, canonical reduced
calls rose from two to three, and no frame was adopted. The saved movement was
`2.94154580693373191e-8`, above the unchanged `1e-8` bound; accepted eta also
failed exact identity. This oracle-RNG input is distinct from the frozen campaign.

The SCC diagnostic used the frozen campaign
`20261010T192805Z-ols-repair-e29164e6`, four PPML cases, 1/4/16 workers, two
alternating pairs, both phase and solver tracing, a 180-second fit limit and a
3,000-second total budget. All 48 fits completed in 1,331.76 seconds on shared
`scc-ei3` (Xeon Gold 6226R, 16 SGE slots; observed process affinity exposed 32
logical CPUs). Build job 8026507 and panel job 8026509 both had `failed=0` and
`exit_status=0`. These two-pair medians are descriptive Linux observations,
with scheduler and sampling uncertainty; they are not quiet-Mac qualification.

| Frozen PPML case | Workers | B total s | A1 total s | Total cost | B strict s | A1 strict s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| AKM 1M | 1 | 10.35863 | 11.29985 | +9.09% | 2.33377 | 2.34964 |
| AKM 1M | 4 | 4.61770 | 4.89156 | +5.93% | 0.89525 | 0.89400 |
| AKM 1M | 16 | 3.97067 | 3.96606 | -0.12% | 0.57766 | 0.56633 |
| Dense two-FE 1M | 1 | 10.00108 | 10.67410 | +6.73% | 2.34171 | 2.25299 |
| Dense two-FE 1M | 4 | 5.14177 | 5.62802 | +9.46% | 1.30114 | 1.20351 |
| Dense two-FE 1M | 16 | 4.16419 | 4.72266 | +13.41% | 1.05971 | 1.02974 |
| Dense two-FE 4M | 1 | 52.87482 | 56.03334 | +5.97% | 14.09082 | 12.71264 |
| Dense two-FE 4M | 4 | 28.74721 | 32.02216 | +11.39% | 8.94523 | 7.79166 |
| Dense two-FE 4M | 16 | 22.96235 | 29.07915 | +26.64% | 7.04041 | 7.10605 |
| Sparse two-FE 1M | 1 | 40.31158 | 43.49182 | +7.89% | 17.37118 | 8.92324 |
| Sparse two-FE 1M | 4 | 15.46146 | 16.08025 | +4.00% | 6.38076 | 3.63414 |
| Sparse two-FE 1M | 16 | 30.52704 | 25.02154 | -18.03% | 3.75873 | 2.62285 |

Every candidate run had zero adoptions. AKM and both dense cases retained four
outer iterations and 3 ordinary + 2 checkpoint frames; the sparse case retained
nine and 8 + 3 frames. Canonical reduced calls rose 2 -> 3 on AKM/dense and
stayed one on sparse. Actual solves were not saved. AKM strict time did not
fall by a frame's cost. Sparse strict time fell while ordinary projection time
rose; that phase movement alone does not pass A1. The apparent sparse 16-worker
total gain is retained above, alongside all costs, without a qualification claim.
All original-weight and reconstruction certificates passed, fixed-worker repeats
were exact, and sample, rank/status and DoF outputs were exact across artifacts.
Float/solver differences remain unqualified. The SCC record retains RRUNTIME
rows 28-40, solver routes, actual frame counts, trace counts and exact-difference
paths; the table reports the principal acceptance quantities.

Exact eta identity failed in all tested attempted adoptions. Saved movement was
about `2.022e-6` on frozen AKM, `1.070e-8` on dense 1M, `7.032e-10` on dense 4M,
and `3.613e-4` on sparse; even the dense 4M case below the movement bound failed
eta identity. No eligibility condition was relaxed to manufacture adoption.
A separate review found that catching a failed fresh staging solve would need a
cold, recertified recovery path: fallible in-place reprojection can invalidate its
old frame. The experiment propagated that error atomically; such recovery was
not implemented or claimed. A1 was removed before independent C/D work. A2
remains the separate decision in the performance plan.

Johannes requested a compact reproducer when deferring A1. The retained
[unqualified patch](../bench/results/a1-20261011-reproducer.patch) is a 23 KB
experiment fixture against `68efe752`, excluded from every production build.
Its zero-context form reproduces all seven original changed source files exactly.
Apply it only to a disposable source copy. From the repository root, the small
AKM-shaped reproducer is:

```sh
task=target/tmp/a1-reproduction
mkdir -p "$task/source"
git archive 68efe752 | tar -x -C "$task/source"
git apply --unidiff-zero --directory="$task/source" bench/results/a1-20261011-reproducer.patch
(
  cd "$task/source"
  scripts/build_macos.sh
  bench/build_timing_harness.sh ../timing-harness
  cargo run --locked -p fereg-oracle --example replay_large --profile ci -- \
    --output ../akm.case --rows 1000 --levels 200,10 --regressors 16 \
    --weights unit --vce robust --graph akm-campaign --solver auto --seed 739391 \
    --ppml --candidate-all --non-deterministic --no-observation-outputs
  FEREG_PHASE_TIMING=1 FEREG_SOLVER_TRACE=1 ../timing-harness \
    dist/fereg_macos.plugin --replay ../akm.case ../result.json
)
```

The retained small input hash is
`e5f58c14bb9bdc655d8868ba8d0773c404bbc7ac5938f99f8d494d02a2b87306`.
Its fresh ordinary attempt fails exact eta identity and adopts no frame. The
one-million-row form uses `--rows 1000000 --levels 200000,10000`; the frozen SCC
panel still requires the retained campaign inputs, not regeneration. Use the
usual task cleanup after inspecting the reproducer; it is not a shipping option.

Artifact and witness SHA-256 identities:

| Artifact | SHA-256 |
| --- | --- |
| First conservative patch | `0a579e92e22ae412ed97aab1ba22565c3235c9198a6385916d8baecd9b2ec3c1` |
| Failed unrestricted curve patch | `b1bd209358533d7452e1710ff9ef302938b7606403cb5ae0af201ed91083cca6` |
| Failed curve core run | `51fc0bcdc9cac573a09546ffc7975532d3ac2179f0c32fb9e165dd45a07d4d9d` |
| Third candidate patch | `551f3ccc06eaa7a50c7c9926d20fadf95fec6fedfa20d241bf1febeae65d2cd5` |
| Retained zero-context reproducer | `38e61f5c7aa45c34c3620a4c464836324022af67d1226ab47d97bbcd4f8fb874` |
| Third candidate source archive | `a6b17228d2ee9a8421de34ff9bd31b5835e0f0a6d4d2d21cefc515201fca84b8` |
| Third candidate core run | `aebd16ebee27dfe8a1f2033887e5de8f8975d3d4d9d0af5685a77395216e9c69` |
| Third candidate universal Mac plugin | `b75b5165c8a8feda53427783e6733984b690c8b5fa84c4ec1d0bac402200a148` |
| Third candidate Linux plugin | `959b72336d219729e2b00d3a0dfa81b1b9ace92ff9dd39ae9855c7cc451d5601` |
| Exact replay failure/science summary | `6bdc5f5468daa637f54659aedc5e9a560e9dcb818bb34642dae92c144f31f196` |
| Discrete/outer replay check | `8b2f8c7ff84b5ea2b164da6547458c60884e76f48be5f884b34a684fd9d72550` |
| Licensed 67-fixture summary | `9397e27d95a4515abac5be37f451eaeb22436fe690b213a016707403e1becbf3` |
| OLS Break probe | `6e34ae90d9ae35896ca63ce58bd1dd3b893325aa47dcfbe3cf9dfe2f763e3238` |
| PPML public Break probe | `f5aada3ea8df93ebc394b615bb07031d17c06bbe03831bcbf2f1ade2b1a2f3fb` |
| Mac traced pair summary | `aff7895d5fbafeb3b9b5de2154830de7727f67a555d486f75fe94145b4de98ef` |
| SCC 48-fit summary | `7dd95b5aa3d6422050c12733c4bfafcfe28657e7fbdb90b1146978309c843c0d` |

### Performance-plan B: ordered FMA coverage, 2026-10-11

**B qualifies with cached-dispatch source patch `4aa37b80`, Mac plugin
`68c21a90` and Linux plugin `447fe148`.** All twenty Mac bounds pass; all
73-case Mac/Linux replays and 56 full-size Linux fits remain exact. The final
instruction panel has zero library FMA on every measured case. Earlier failed
candidates and their costs are retained below.

The initial candidate `2c6d1832` is arithmetic-preserving, **not an accepted B
workstream**. It extends the existing detected FMA/AVX table from five to twenty entries.
The six areas named in the plan now dispatch ordered dot/projection, joint
iteration, two-FE PCG and canonical upper-Gram loops. Core bodies remain safe
and shared with the portable path; the plugin owns target-feature wrappers.
Per-element arithmetic, reduction order, padding and inactive-RHS behavior are
preserved. The intentionally non-fused reduced-matrix product is unchanged.
There are no tolerance, sample, rank, omission, separation, route, SPI ownership,
certificate or strict-PPML-solve changes. Some fallback/reconstruction,
reusable-certificate and smaller canonical-product loops remain scalar;
profiling must determine whether more coverage is needed.

An independent high-effort Astra review found no correctness issue. Its request
for explicit padding/inactive sentinels and the active `-1.0` sentinel was
implemented. The complexity ratchet records +231 core and +252 plugin production
lines for the shared bodies and typed wrappers, with no new oversized function
or file. The existing joint iteration gains eleven lines from explicit typed
call arguments. Dispatch-call cost is included in the Mac measurements below.

Mac checks pass: `scripts/run_tests.sh`, universal native build/audit/load,
73/73 exact replay cases against the previous plugin (zero differences and
unchanged solver signatures), all 67 licensed full-profile fixtures, and both
OLS and public-scope PPML PTY Break checks. The replay's independent scientific
witnesses pass in all 73 cases. These are working-tree artifact checks bound by
the source-patch and plugin hashes below; the plugin's embedded source label
predates the runtime patch.

The x86 Mac slice compiles all twenty entries. Static disassembly of each of
the fifteen added target-feature bodies contains no library-FMA call; fourteen
contain hardware FMA instructions and PCG direction uses the equivalent
multiply/add form for its unit multiplier. This is instruction-selection
evidence, not an executed instruction count. Rosetta exposes neither FMA nor
AVX here: all five hardware tests skip, despite their test-runner `ok` labels.
SCC access was restored through interactive SSH; the bounded Linux checks below
supersede the earlier authentication blocker. Linux executed all five hardware
tests. Cachegrind found residual library FMA calls, so the initial candidate
fails B's disappearance criterion. B stays unqualified and subsequent
workstreams have not begun. Johannes
conditionally approved A1 on 2026-10-11 under the [plan's eligibility and
validation requirements](fable_performance.md#4-johanness-decisions-and-remaining-gates).
That decision does not waive B qualification. SPEC §13 is unchanged.

The quiet Mac run used the twenty hash-verified frozen workloads, one warmup
and seven alternating pairs per cell, a 90-second process limit and an 1800-second
budget. It completed in 643.59 seconds, with all 320 fits exact and all twenty
paired-median 95% bootstrap upper bounds within max(5%, 1 ms). No builds or
agents ran during timing; ordinary desktop background processes were present.
The largest upper bound is 1.035739. Paired medians range from 0.966264 to
1.009575; costs as well as gains are retained below. These small observations
do not establish a speedup or a release-performance claim. Times are seconds;
ratios are medians of paired ratios, not ratios of the displayed medians.

| Frozen workload | Baseline median | Candidate median | Paired ratio | 95% bootstrap interval |
| --- | ---: | ---: | ---: | --- |
| `ols-no-fe-k5-t1` | 0.005282 | 0.005280 | 0.999621 | [0.991737, 1.005359] |
| `ols-no-fe-k5-t4` | 0.004686 | 0.004704 | 1.001930 | [0.999575, 1.007042] |
| `ols-three-unit-k5-t1` | 0.585763 | 0.585808 | 1.000077 | [0.996792, 1.001798] |
| `ols-three-unit-k5-t4` | 0.299557 | 0.299698 | 1.000087 | [0.997239, 1.001967] |
| `ols-two-pw-k5-t1` | 0.653873 | 0.656214 | 1.000259 | [0.997755, 1.011564] |
| `ols-two-pw-k5-t4` | 0.372277 | 0.372096 | 1.001750 | [0.989284, 1.005846] |
| `ols-two-unit-k20-t1` | 1.203934 | 1.205990 | 1.005488 | [0.994109, 1.010733] |
| `ols-two-unit-k20-t16` | 0.445681 | 0.442693 | 0.999367 | [0.987844, 1.008383] |
| `ols-two-unit-k20-t4` | 0.574316 | 0.567609 | 0.990337 | [0.982508, 0.999854] |
| `ols-two-unit-k20-t8` | 0.471380 | 0.474140 | 0.997908 | [0.978926, 1.034033] |
| `ols-two-unit-k5-t1` | 0.478811 | 0.480858 | 1.001146 | [0.987009, 1.004367] |
| `ols-two-unit-k5-t4` | 0.263885 | 0.263871 | 0.995747 | [0.994644, 1.005684] |
| `ols-wide-k129-t1` | 0.763857 | 0.752693 | 0.974630 | [0.970251, 1.005925] |
| `ols-wide-k129-t4` | 0.398176 | 0.401504 | 0.990082 | [0.965212, 1.035739] |
| `ols-wide-k64-t1` | 0.295673 | 0.295657 | 0.993140 | [0.988308, 1.023617] |
| `ols-wide-k64-t4` | 0.168455 | 0.163700 | 0.966264 | [0.959143, 1.002210] |
| `ols-wide-k65-t1` | 0.312483 | 0.311014 | 0.996615 | [0.978531, 1.011322] |
| `ols-wide-k65-t4` | 0.172959 | 0.176653 | 1.009575 | [0.957675, 1.029921] |
| `ppml-two-unit-k5-t1` | 0.405354 | 0.404991 | 1.000059 | [0.993464, 1.004761] |
| `ppml-two-unit-k5-t4` | 0.229143 | 0.228294 | 1.001377 | [0.991459, 1.003422] |

A separate instrumented Mac diagnostic used locally generated campaign-shaped
AKM inputs: one million rows, 16 controls, 200,000 worker and 10,000 firm
levels, seed 739391, one worker, OLS IID and PPML robust inference, unrestricted
candidate, nondeterministic worker policy, and only the required sample output.
These inputs are not the frozen SCC campaign inputs and cannot substitute for
them. Two alternating pairs per model, with a 90-second fit limit and 600-second
budget, give eight successful fits with exact science and solver signatures.
PPML has four outer iterations in every fit and keeps its fresh strict solve.
The following means are descriptive; no timing acceptance is inferred from
two pairs with tracing enabled. Unobserved phases stay missing.

| Diagnostic runtime field | Baseline mean | Candidate mean |
| --- | ---: | ---: |
| OLS harness native call | 0.884388 | 0.885726 |
| OLS preflight (row 18) | 0.000202 | 0.000200 |
| OLS input (row 19) | 0.133730 | 0.134039 |
| OLS native (row 20) | 0.748780 | 0.749800 |
| OLS total (row 21) | 0.882712 | 0.884039 |
| PPML harness native call | 4.773539 | 4.703071 |
| PPML total (row 1) | 4.771736 | 4.701227 |
| PPML preparation (row 28) | 0.289071 | 0.288944 |
| PPML separation FE (row 29) | 0.004087 | 0.004130 |
| PPML separation simplex (row 30) | missing | missing |
| PPML separation IR (row 31) | missing | missing |
| PPML separation mu (row 32) | missing | missing |
| PPML weight/response refresh (row 33) | 0.060927 | 0.061665 |
| PPML FE projection (row 34) | 1.963144 | 1.927000 |
| PPML finite solve (row 35) | 0.560771 | 0.560114 |
| PPML objective/line search (row 36) | 0.279796 | 0.280245 |
| PPML strict final solve (row 37) | 1.165864 | 1.133371 |
| PPML inference (row 38) | 0.085691 | 0.083245 |
| PPML FE recovery (row 39) | 0.000001 | 0.000001 |
| PPML publication (row 40) | 0.121995 | 0.122015 |

Failed attempts: initial Clippy checks rejected complex function-pointer types
and a macro placed after tests; named types and placement fixed both. The plan's
literal `output_sample=0` recipe failed before estimation because both wire
parsers require a positive sample output. The corrected case keeps sample
column 21 and disables only optional outputs; the plan recipe is corrected.
A generator retry refused to overwrite its existing template; reusing that
unchanged template resolved setup. These failures do not count as successful
scientific or timing observations. No estimator repair or tolerance change was
made in response.

SHA-256 records (raw task captures are disposable; these summaries and hashes
are the retained record). The source patch is relative to `38fd8296` and includes
the new ordered-kernel file and complexity inventory. Its production code is
identical to the timed build; only the later sentinel test assertions differ.

| Artifact / observation | SHA-256 |
| --- | --- |
| Previous universal plugin | `f59d2a2989dc672045fdda86dd88fa2a4191ab0e15a42d833f7bd0711c4d8ca4` |
| Candidate universal plugin | `b04e08a2a085e249f8f72eb6df8c6979e431a7062d210864c48e5e79c205da75` |
| Runtime source patch | `15f29992707e954323d93e8ea86b9e9c4f63357b4114dfb382fafa38470b41e2` |
| Full gate | `439bafa7ff5bba99524e15021d19ed2bdcc7320f2e6dece573719f9c81a3dbf5` |
| Exact replay summary | `519c063ef73872ae8b27d9a8a1d639e900ccf12aca0a42efa8e42c01797c1760` |
| Licensed fixture summary | `9397e27d95a4515abac5be37f451eaeb22436fe690b213a016707403e1becbf3` |
| OLS Break | `6e34ae90d9ae35896ca63ce58bd1dd3b893325aa47dcfbe3cf9dfe2f763e3238` |
| PPML Break | `f5aada3ea8df93ebc394b615bb07031d17c06bbe03831bcbf2f1ade2b1a2f3fb` |
| Mac timing summary | `43286b38795729eff6a9005a4d0e6c3c5a11e411a95eb5301928f0640e131bad` |
| Local phase input manifest | `42e783208ab7e61d84bacf38afa0d3e3d0ad734b27be9dc41f80a51767497322` |
| Rejected zero-sample-output probe | `bea85baec7aa5e5a97d1535d63173a4e745da91cd675af6399a47f39792c207c` |
| Successful phase diagnostic | `c106aa856c2b2f3c6459ceed8ec6cacbb03339379292923940f996ed0d003fb9` |
| Static x86 instruction summary | `19a6e94524b6afb5195ca9da9a482bed30ada4f2f80bbea9ee247baebdcad4d9` |
| x86 tests (hardware skipped) | `a340e4da6363a29dddcbea72ebbb1decb133b3c7239c4e7238a5201ad7d93552` |
| Candidate build/audit/load log | `b14b52bccc47b8986c2e2deeb05c48c34e3c653b758332b5e9c8779e199bc96e` |
| Local OLS AKM case | `6503640e23c0955ba2b711b02f82917085503ccd16f2c58011bc64b74f04fd3e` |
| Local PPML AKM case | `029fae2a6399992230044ff0847d11cdac4e05758704c3d6eef17538c8c1404e` |

### B Linux observations and extension, 2026-10-11

The initial twenty-entry candidate was built from `eb838c85` (production equal
to `2c6d1832`) against `38fd8296`, with Rust 1.85.1, GCC 12.2 and glibc 2.28.
SCC job 8023715 completed release/profiling build, binary audit/load, numeric
buffer guards, all five actual FMA/AVX hardware tests, and 73/73 exact replay
cases with unchanged solver signatures and independent witnesses.

Inputs are binary64 extractions from the seven frozen campaign DTAs in the
retained `20261010T192805Z-ols-repair-e29164e6` campaign, with hashes checked
before and after use; they were not regenerated. Calls retain the required
sample output, unrestricted candidates and campaign solver/separation policy.
The following native means use two alternating pairs, phase/solver tracing,
1/4 workers and 120-second per-fit/1500-second total limits. All 56 fits are
exact across plugins and repeat runs at each fixed worker count, with both
certificates passing. Dense/AKM PPML keeps four outer iterations; sparse PPML
keeps nine, two strict cycles and one refinement. This does not assert
cross-worker bit identity. All OLS rows 18–21 and PPML rows 1, 28–40 were
captured, preserving missing phases.

Job 8023727 completed in 1245.23 measurement seconds on Xeon Gold 6242
(scc-gr4). The requested four-CPU binding did not constrain the reported
32-CPU affinity. The node was shared and tracing was enabled; these are
phase diagnostics, not quiet timing or a Linux speedup claim. Strict-phase
means are PPML row 37. Both costs and gains are shown.

| Frozen case | Workers | Base native s | Candidate native s | Base strict s | Candidate strict s |
| --- | ---: | ---: | ---: | ---: | ---: |
| ols-akm-1m | 1 | 2.069848 | 1.982841 | — | — |
| ols-akm-1m | 4 | 1.096788 | 1.093844 | — | — |
| ppml-akm-1m | 1 | 11.185748 | 10.736614 | 2.718806 | 2.549752 |
| ppml-akm-1m | 4 | 5.519300 | 5.254834 | 1.119544 | 1.052400 |
| ppml-dense-1m | 1 | 10.631114 | 9.862782 | 2.567979 | 2.245452 |
| ppml-dense-1m | 4 | 5.580860 | 5.432845 | 1.405876 | 1.312754 |
| ols-three-dense-1m | 1 | 3.239322 | 3.245102 | — | — |
| ols-three-dense-1m | 4 | 1.633578 | 1.652328 | — | — |
| ppml-sparse-1m | 1 | 42.871195 | 39.525806 | 19.554498 | 16.715510 |
| ppml-sparse-1m | 4 | 21.045159 | 18.929957 | 7.646044 | 7.244020 |
| ppml-dense-4m | 1 | 45.032531 | 42.844466 | 11.922271 | 10.963437 |
| ppml-dense-4m | 4 | 25.499336 | 24.711165 | 7.301107 | 6.964740 |
| ols-three-dense-4m | 1 | 17.688177 | 17.553592 | — | — |
| ols-three-dense-4m | 4 | 7.487824 | 7.605072 | — | — |

Cachegrind 3.22 job 8023764 measured only the substantive native call, using
profiling builds, one worker and the first 50,000 rows of four frozen inputs.
Two alternating pairs per case completed in 980.09 seconds under the declared
600-second per-fit/2700-second total bounds. All sixteen fits were exact.
Hardware tests also ran under Valgrind without skipping. Cache and branch
simulation were disabled. Complete symbol totals, rather than top-function
absence, determine library FMA counts. Matching signed AlmaLinux glibc debug
symbols identify `__fma_fma3`; debug and loaded-library build IDs were checked.

| Frozen prefix | Base instructions | Candidate instructions | Change | Base library FMA instructions | Candidate library FMA instructions |
| --- | ---: | ---: | ---: | ---: | ---: |
| ppml-dense-1m | 4,167,300,758.5 | 3,928,120,662.0 | -5.7394% | 175,977,738 | 60,566,754 |
| ols-three-dense-1m | 1,368,018,067.0 | 1,368,017,711.5 | -0.0000% | 4,814,409 | 4,814,409 |
| ppml-sparse-1m | 11,443,414,520.0 | 8,679,314,606.5 | -24.1545% | 930,283,374 | 135,790,188 |
| ppml-akm-1m | 66,683,371,106.0 | 43,995,700,951.5 | -34.0230% | 5,855,481,021 | 1,010,663,925 |

Between-pair instruction differences were at most 0.0006%; the three-FE
change is noise. Valgrind wall times are not performance evidence. A separate
Callgrind caller diagnostic (8023768) found remaining work in group basis
operators, retained residual reconstruction, numeric coefficient updates,
canonical cross products, certificate folds and triangular solves. It was
whole-process caller attribution, not the native instruction measurement.

Failed measurement attempts remain nonpassing: 8023728 exhausted its 180-second
first-fit bound on the AKM prefix; the completed run used the larger declared
bound. Probe 8023745 had exact fits but missing libm symbols, so its unresolved
counts cannot support FMA attribution. Job 8023755 failed the attribution
guard after an incorrect debug-symbol prefix; correcting the task-local lookup
and verifying the build ID enabled 8023764. No scientific tolerance or solver
policy changed in response.

The follow-up extends twenty entries to thirty-three, targeting those observed
callers. It keeps every per-element operation and validation order, allocations,
rank/sample/omission/separation decisions, worker policy, independent
certificates and fresh strict PPML solve. Small certificate folds retain their
interleaved checks and use a scalar detected entry; its cost must be measured.
The first extension passed 1,146 active core tests. The subsequent certificate
extraction failed two existing tests by applying zero-width chunks to an empty
group; restoring the original empty-group behavior and adding explicit coverage
made all 1,146 active tests pass (three ignored). Two independent high-effort
Astra reviews found no remaining correctness issue, while stressing parent
replay and measured dispatch cost. The seven x86 Mac tests compile but skip
hardware execution under Rosetta. The current full `scripts/run_tests.sh` gate
passes, including allocator audits, transaction mocks and independent numerical
witnesses. The ratchet adds 288 core and 238 plugin production lines for the
thirteen bodies and wrappers, with no additional oversized file/function.
A Mac comparator build launched from the wrong working directory was stopped
before producing comparison evidence and restarted inside the baseline checkout.
The thirty-three-entry candidate passed the Mac universal build/audit/load,
73/73 exact replay cases with unchanged solver signatures, all 67 licensed
Stata fixtures, and both public-scope OLS/PPML Break checks. The fixture runner
first rejected a noncanonical plugin filename before running Stata; copying the
identical artifact to its required basename resolved that tooling error.
SCC build job 8023807 passed the native audit/load, all seven executed FMA/AVX
tests and 73/73 exact replays with independent witnesses.

This candidate **does not qualify**. The standard Mac panel completed all 320
fits exactly in 644.00 seconds (one warmup, seven alternating pairs per case,
90-second per-fit and 1800-second total bounds). Nineteen timing bounds passed.
The K=129, four-worker OLS upper bootstrap bound was 1.051078661 against 1.05,
although its paired median ratio was 0.977195353. This is a nonpassing bound,
not evidence of a demonstrated regression or permission to relax the allowance.
No agents, builds or tests competed during the run; ambient desktop processes
were present. The following table retains costs as well as gains.

| Standard case | Base median s | Candidate median s | Paired median ratio | 95% interval | Bound passes |
| --- | ---: | ---: | ---: | --- | --- |
| ols-no-fe-k5-t1 | 0.005275 | 0.005261 | 1.002286 | [0.968808, 1.009217] | yes |
| ols-no-fe-k5-t4 | 0.004616 | 0.004725 | 1.018100 | [1.011482, 1.066769] | yes |
| ols-three-unit-k5-t1 | 0.585446 | 0.585340 | 1.000311 | [0.996862, 1.000776] | yes |
| ols-three-unit-k5-t4 | 0.301226 | 0.301576 | 1.001162 | [0.998409, 1.012663] | yes |
| ols-two-pw-k5-t1 | 0.654002 | 0.656754 | 1.000448 | [0.994806, 1.004548] | yes |
| ols-two-pw-k5-t4 | 0.374342 | 0.374774 | 0.995349 | [0.980985, 1.013267] | yes |
| ols-two-unit-k20-t1 | 1.192590 | 1.192593 | 0.996402 | [0.992877, 0.999713] | yes |
| ols-two-unit-k20-t16 | 0.444602 | 0.443497 | 0.994737 | [0.993586, 0.999665] | yes |
| ols-two-unit-k20-t4 | 0.569982 | 0.576994 | 1.014324 | [1.003129, 1.024588] | yes |
| ols-two-unit-k20-t8 | 0.468603 | 0.474596 | 1.015585 | [0.984932, 1.025868] | yes |
| ols-two-unit-k5-t1 | 0.481379 | 0.477421 | 0.992746 | [0.978231, 1.001479] | yes |
| ols-two-unit-k5-t4 | 0.264745 | 0.264925 | 1.004851 | [0.992034, 1.014969] | yes |
| ols-wide-k129-t1 | 0.754905 | 0.757999 | 1.015816 | [0.977636, 1.021017] | yes |
| ols-wide-k129-t4 | 0.400861 | 0.398980 | 0.977195 | [0.949603, 1.051079] | **no** |
| ols-wide-k64-t1 | 0.302252 | 0.297055 | 0.981568 | [0.966294, 0.995895] | yes |
| ols-wide-k64-t4 | 0.169353 | 0.167513 | 0.985753 | [0.973005, 1.015971] | yes |
| ols-wide-k65-t1 | 0.315579 | 0.314946 | 1.000732 | [0.989827, 1.007048] | yes |
| ols-wide-k65-t4 | 0.172801 | 0.175230 | 1.020985 | [1.012910, 1.048891] | yes |
| ppml-two-unit-k5-t1 | 0.407041 | 0.402097 | 0.987854 | [0.982752, 0.992735] | yes |
| ppml-two-unit-k5-t4 | 0.229931 | 0.229954 | 0.994263 | [0.991941, 1.001507] | yes |

Linux native-call instruction job 8024253 completed sixteen exact fits in
980.55 seconds with the same frozen prefixes, limits and attribution rules.
Library FMA remained on every case, so the literal disappearance criterion
also failed. These instruction changes do not establish quiet wall-time gains.

| Frozen prefix | Base instructions | Candidate instructions | Change | Candidate library FMA instructions |
| --- | ---: | ---: | ---: | ---: |
| ppml-dense-1m | 4,167,357,006.0 | 3,728,351,548.0 | -10.5344% | 14,400,006 |
| ols-three-dense-1m | 1,368,016,815.5 | 1,350,884,057.0 | -1.2524% | 15 |
| ppml-sparse-1m | 11,443,432,959.5 | 8,404,214,660.0 | -26.5586% | 38,251,782 |
| ppml-akm-1m | 66,683,364,069.5 | 43,721,041,900.0 | -34.4349% | 913,804,857 |

The corresponding 56-fit Linux native panel (8024271) completed in
1222.12 seconds, with all fixed-worker science and signatures exact.
All certificates pass; dense/AKM PPML retains four outer iterations and sparse
PPML retains nine. Host scc-gd4 is shared; tracing remains enabled,
so these phase observations establish neither Mac qualification nor a Linux
speedup claim. Means include both alternating pairs.

| Frozen case | Workers | Base native s | Candidate native s | Base strict s | Candidate strict s |
| --- | ---: | ---: | ---: | ---: | ---: |
| ols-akm-1m | 1 | 2.054525 | 1.929889 | — | — |
| ols-akm-1m | 4 | 1.075328 | 1.087040 | — | — |
| ppml-akm-1m | 1 | 11.134865 | 10.017541 | 2.722945 | 2.366631 |
| ppml-akm-1m | 4 | 5.438297 | 5.021609 | 1.104971 | 0.996896 |
| ppml-dense-1m | 1 | 10.304446 | 9.244334 | 2.431706 | 2.082191 |
| ppml-dense-1m | 4 | 5.540954 | 5.258065 | 1.346294 | 1.278112 |
| ols-three-dense-1m | 1 | 3.221288 | 3.169217 | — | — |
| ols-three-dense-1m | 4 | 1.629021 | 1.593238 | — | — |
| ppml-sparse-1m | 1 | 42.801137 | 37.569458 | 19.502466 | 16.243620 |
| ppml-sparse-1m | 4 | 20.854550 | 18.088248 | 7.732623 | 7.137208 |
| ppml-dense-4m | 1 | 44.647278 | 39.293597 | 11.869542 | 9.865655 |
| ppml-dense-4m | 4 | 25.357428 | 23.529659 | 7.290157 | 6.639839 |
| ols-three-dense-4m | 1 | 17.555877 | 17.354827 | — | — |
| ols-three-dense-4m | 4 | 7.444361 | 7.180352 | — | — |

The dense caller probe (8024275) attributes 4,800,000 remaining calls to the
row-major joint residual/score loops and two to the final score certificate.
The next candidate adds detected entries for those ordered row loops and uses
the existing scalar entry for the certificate. Its 1,146 active core tests and
Clippy checks pass; this source is not yet qualified. The AKM caller diagnostic
(8024807) completed in 371.80 seconds within its 540-second bound. It accounts
for all 304,601,619 remaining library calls: 152,783,966 in incidence center/
scatter, 139,243,955 in reduced CG updates, 12,573,696 in the joint row loops
and two in the final score certificate. Whole-process Callgrind is caller
attribution only; native-call Cachegrind remains the instruction acceptance test.

The resulting thirty-seven-entry candidate adds ordered incidence and CG
step/energy bodies and reuses the existing affine direction entry. Original
nonfused curvature, initial energy and speculative matrix products remain
unchanged. Scales, iteration limits, tolerances, finite checks, allocations,
solver routes and scientific decisions are unchanged. Independent dense
elimination and known-solution CG witnesses remain in the core suite. All
1,146 active core tests and Clippy pass; the independent high-effort review
found no issue. Hardware tests cover empty and repeated incidence coordinates,
CG tails, subnormals and signed zero. The final ratchet adds 366 core and 338
plugin production lines over `ded7f627`, without another oversized file or
function. The full repository gate passes, including allocator audits, transaction mocks,
portable checks and independent numerical witnesses. The Mac universal build,
ABI audit/load, 73/73 exact replays with unchanged signatures, all 67 licensed
fixtures, and OLS/PPML Break checks pass. Linux instruction and hardware
qualification and the final Mac timing panel remain pending. The final Mac
panel is predeclared at fifteen alternating pairs for every standard workload,
one warmup, 90 seconds per fit and 1800 seconds total, with the same
max(5%, 1 ms) allowance. This does not retroactively accept the failed
thirty-three-entry panel.


The thirty-seven-entry Linux build (8025080) passed native release/profiling
load/audit, all eight hardware tests and 73 exact replay cases. The initial
measurement launches did not run fits: native job 8025103 omitted its required
setup binding/mode, and instruction job 8025102 stopped at the hardware guard.
Under Valgrind 3.22, the normalized selected-score signed-zero test returned
positive zero where native hardware preserved negative zero. No assertion was
relaxed and no production repair was made for this instrumentation failure.

Independent C probe 8025110 calls libm FMA, scalar FMA and packed FMA on four
lanes: `a=[1,1,-1,-1]`, `b=[-0,+0,+0,-0]`, `c=[-0,-0,-0,+0]`. All native
implementations return the IEEE results `[-0,+0,-0,+0]`; all three under
Valgrind 3.22 return `[+0,+0,+0,+0]`. Upstream's
[release notes](https://valgrind.org/docs/manual/dist.news.html) document AMD64
FMA-emulation accuracy improvements in 3.23. The older instruction/caller
observations above remain diagnostic and nonqualifying.

A task-local build of the official Valgrind 3.27.1 release is bounded at four
cores, 8 GB and twenty minutes, with 180/780/120-second configure/build/install
limits. Its source archive matches the published MD5/SHA-1 and the SHA-256
record below. The first queued build (8025117) was canceled before execution
to correct the probe source path; 8025123 is the corrected build. Acceptance
requires all eight unchanged hardware tests and the independent signed-zero
probe to pass under the new tool. The ensuing instruction panel also compares
every instrumented result with a native run of both plugins on that exact
prefix. Corrected native phase job 8025111 retains the original fit/time bounds.


Valgrind 3.27.1 build 8025123 passed all eight unchanged hardware tests and the
four-lane signed-zero probe. Native-call instruction panel 8025135 completed
sixteen fits in 578.55 seconds. Each of the eight native prefix runs
matched every corresponding instrumented result exactly, including solver
signatures and observation-output hashes. The complete function totals were:

| Frozen prefix | Base instructions | Candidate instructions | Change | Candidate library FMA instructions |
| --- | ---: | ---: | ---: | ---: |
| ppml-dense-1m | 4,167,331,794.5 | 3,693,815,352.0 | -11.3626% | 0 |
| ols-three-dense-1m | 1,368,017,720.0 | 1,350,883,636.0 | -1.2525% | 9 |
| ppml-sparse-1m | 11,443,434,202.0 | 8,312,554,042.0 | -27.3596% | 0 |
| ppml-akm-1m | 66,683,392,949.0 | 41,747,983,075.5 | -37.3937% | 0 |

This still did not complete coverage: the three-FE OLS row retained three
scalar library calls (nine instructions). Caller probe 8025143 completed in
23.86 seconds and located two in `weighted_sum_of_squares` and one in
`certify_statistic_identities`. The Mac fifteen-pair panel was deliberately
interrupted after 251 exact guarded fits and seven completed passing workload
bounds, before changing these sites. That interrupted panel is not a timing
pass. Its records and hash are retained separately from the prior failed
seven-pair bound.

The completed-coverage source keeps the thirty-seven-entry table and dispatches
the two remaining scalar source expressions through its existing FMA entry.
The norm square is called twice; the RMSE square is called once on this case.
Operands, positive-zero addends, evaluation order and interleaved checks are
unchanged. The independent statistic-identity witnesses and full repository
gate pass. The ratchet now adds 368 core and 338 plugin production lines over
`ded7f627`, with no additional oversized file/function. Final native builds passed their ABI/load checks. Mac replay remained exact
on all 73 cases; all 67 licensed fixtures and OLS/PPML Break checks passed.
Linux build job 8025154 passed all eight executed hardware witnesses and all
73 replay cases. Valgrind 3.27.1 job 8025206 completed sixteen fits in
622.41 seconds, with eight native preflights agreeing exactly with the
instrumented runs. Library-FMA instruction totals are now zero in all eight
candidate fits. This completes measured FMA coverage, not B qualification.

| Frozen prefix | Base instructions, median | Candidate instructions, median | Change | Candidate library FMA instructions |
| --- | ---: | ---: | ---: | ---: |
| ppml-dense-1m-50k | 4,167,361,710.5 | 3,693,852,366.5 | -11.36% | 0 |
| ols-three-dense-1m-50k | 1,368,018,900.0 | 1,350,884,951.5 | -1.25% | 0 |
| ppml-sparse-1m-50k | 11,443,432,587.5 | 8,312,522,589.0 | -27.36% | 0 |
| ppml-akm-1m-50k | 66,683,391,134.0 | 41,748,005,545.5 | -37.39% | 0 |

The largest within-arm repeat spread is 32,960 instructions. These are
bounded single-worker Linux instruction observations, not Mac timing gains.
Native Linux job 8025207 hit its declared 1500-second total budget after
47 of 56 successful guarded fits. Every completed comparison was exact;
the interrupted four-million-row dense PPML/four-worker fit and the remaining
fits are unavailable. Its slower shared host does not establish a wall-time
regression or a pass. The earlier complete 56-fit panel remains attributed to
its earlier artifact.

The final Mac panel also **does not qualify**. All 640 fits (twenty workloads,
one warmup and fifteen alternating pairs) were exact in 1281.42 seconds, but
K=129/four-worker OLS again failed the unchanged bound: baseline/candidate
medians 0.395245/0.398175 seconds, paired median ratio 1.010543, 95% interval
[0.992008, 1.057668], limit 1.05. The other nineteen bounds passed. Costs remain
visible: K=20/eight-worker OLS had paired median ratio 1.016910 (upper 1.022419),
and four-worker PPML 1.016870 (upper 1.019245). The failed bound is retained;
it is not replaced by an unchanged-artifact rerun.

A bounded independent review identified repeated `OnceLock` table reads in
scalar certificate loops as a concrete overhead hypothesis. For the wide
case, roughly 130 RHS across 5,500 FE levels can cause about 715,000 table reads
per complete certificate. The next arithmetic-preserving candidate caches the
table outside certificate/coupling folds and residual row traversal. Public
installation semantics, operands, check positions, reduction order, solver
policy and tolerances remain unchanged. This is a source-level hypothesis;
the timing variation alone does not establish causality. The complexity
ratchet adds 391 core and 338 plugin production lines over `ded7f627`, including
the previous coverage extension. New qualification is required.

The cached-dispatch candidate passes the full repository gate, universal Mac
ABI/load, 73/73 exact Mac replays, all 67 licensed fixtures and both public
Break checks. Bounded independent review found no changed operands, check
positions, reduction order, public installation semantics or unsafe code in
the lookup-hoisting delta. Linux build job 8025383 passed native audit/load,
all eight hardware witnesses and 73/73 exact replays.

Mac qualification used the same twenty frozen workloads, one warmup and fifteen
alternating pairs under a predeclared 1800-second total/90-second per-fit bound.
All 640 fits were exact and all bounds passed in 1283.54 seconds. No local
builds or agents ran during timing. The Mac14,14 host had 24 logical CPUs;
load averages at start/end were 1.71/2.31 (one minute), with no thermal or
performance warning. Activity Monitor and ordinary desktop processes remained
open; their observed load and the failed earlier panels are retained. These
results establish the workload nonregression check, not a release-speed claim
or proof that lookup overhead alone caused the previous failures.

| Frozen Mac workload | Base median s | Candidate median s | Paired median ratio | 95% upper ratio bound | Pass |
| --- | ---: | ---: | ---: | ---: | :---: |
| ols-no-fe-k5-t1 | 0.005265 | 0.005269 | 1.001899 | 1.010703 | yes |
| ols-no-fe-k5-t4 | 0.004673 | 0.004662 | 0.998091 | 1.014960 | yes |
| ols-three-unit-k5-t1 | 0.586072 | 0.586364 | 1.000768 | 1.007410 | yes |
| ols-three-unit-k5-t4 | 0.300342 | 0.301070 | 1.001049 | 1.002883 | yes |
| ols-two-pw-k5-t1 | 0.655622 | 0.656263 | 1.000818 | 1.003743 | yes |
| ols-two-pw-k5-t4 | 0.373327 | 0.372299 | 0.997883 | 1.004250 | yes |
| ols-two-unit-k20-t1 | 1.192626 | 1.190379 | 0.997721 | 1.001369 | yes |
| ols-two-unit-k20-t16 | 0.444991 | 0.443226 | 1.000321 | 1.003870 | yes |
| ols-two-unit-k20-t4 | 0.566583 | 0.572692 | 1.008211 | 1.016746 | yes |
| ols-two-unit-k20-t8 | 0.471166 | 0.469543 | 0.995826 | 1.009364 | yes |
| ols-two-unit-k5-t1 | 0.487497 | 0.490294 | 1.007776 | 1.013533 | yes |
| ols-two-unit-k5-t4 | 0.264627 | 0.264720 | 1.001893 | 1.009534 | yes |
| ols-wide-k129-t1 | 0.762863 | 0.752804 | 0.988493 | 1.020497 | yes |
| ols-wide-k129-t4 | 0.384350 | 0.391891 | 1.006384 | 1.017147 | yes |
| ols-wide-k64-t1 | 0.298219 | 0.297289 | 1.005429 | 1.012541 | yes |
| ols-wide-k64-t4 | 0.166151 | 0.166675 | 1.012796 | 1.022620 | yes |
| ols-wide-k65-t1 | 0.313229 | 0.311255 | 0.991602 | 0.997856 | yes |
| ols-wide-k65-t4 | 0.172737 | 0.174030 | 1.006231 | 1.028476 | yes |
| ppml-two-unit-k5-t1 | 0.405339 | 0.405013 | 0.999573 | 1.001791 | yes |
| ppml-two-unit-k5-t4 | 0.229393 | 0.234549 | 1.019294 | 1.027793 | yes |

Costs as well as gains remain in that table: four-worker PPML has a 1.93%
paired median increase; K=64/four-worker OLS has a 1.28% increase. Both are
within the unchanged allowance. The former K=129/four-worker failure now
has interval [0.954403, 1.017147], with paired median ratio 1.006384.

Linux Valgrind 3.27.1 job 8025456 completed sixteen instrumented fits in
581.92 seconds. Eight native preflights matched the instrumented science
exactly; every paired/repeated result and solver signature was exact. All
candidate library-FMA instruction totals are zero. Native hardware tests and
the independent signed-zero witness retain the corrected instrumentation
prerequisite; no rounding assertion or tolerance was relaxed.

| Frozen prefix | Base instructions, median | Candidate instructions, median | Change | Candidate library FMA instructions |
| --- | ---: | ---: | ---: | ---: |
| ppml-dense-1m-50k | 4,167,348,700.5 | 3,694,004,287.5 | -11.36% | 0 |
| ols-three-dense-1m-50k | 1,368,018,265.5 | 1,350,338,462.0 | -1.29% | 0 |
| ppml-sparse-1m-50k | 11,443,407,213.0 | 8,313,466,961.0 | -27.35% | 0 |
| ppml-akm-1m-50k | 66,683,350,571.5 | 41,749,824,121.0 | -37.39% | 0 |

The largest within-arm repeat spread is 70,273 instructions.
The separate full-size Linux native panel (8025457) completed all 56 fits
in 1890.32 seconds under the predeclared 2400-second total and
120-second per-fit limits. All completed base/candidate pairs and repetitions
have exact science and solver signatures; all certificates pass. Dense/AKM
PPML retains four outer iterations and sparse PPML nine. Build and both
measurement jobs have scheduler failed=0/exit=0. The shared Xeon host and
phase/solver instrumentation make these descriptive cost observations, not
quiet Linux or Mac timing qualification. Means include both alternating pairs.

| Frozen case | Workers | Base native s | Candidate native s | Change | Base strict s | Candidate strict s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| ols-akm-1m | 1 | 3.641185 | 3.556964 | -2.31% | — | — |
| ols-akm-1m | 4 | 2.230758 | 2.214248 | -0.74% | — | — |
| ppml-akm-1m | 1 | 18.879920 | 18.073212 | -4.27% | 4.227770 | 3.709725 |
| ppml-akm-1m | 4 | 9.928838 | 9.073932 | -8.61% | 1.950056 | 1.613718 |
| ppml-dense-1m | 1 | 19.015679 | 19.548936 | +2.80% | 4.190574 | 4.817517 |
| ppml-dense-1m | 4 | 11.544038 | 10.518159 | -8.89% | 2.537457 | 2.472142 |
| ols-three-dense-1m | 1 | 4.973539 | 4.942548 | -0.62% | — | — |
| ols-three-dense-1m | 4 | 2.891701 | 2.902296 | +0.37% | — | — |
| ppml-sparse-1m | 1 | 62.953963 | 60.134788 | -4.48% | 25.867078 | 25.064503 |
| ppml-sparse-1m | 4 | 33.993516 | 28.996312 | -14.70% | 13.161102 | 10.548958 |
| ppml-dense-4m | 1 | 74.896901 | 72.574617 | -3.10% | 18.288112 | 17.400576 |
| ppml-dense-4m | 4 | 45.419287 | 41.870218 | -7.81% | 11.649183 | 10.912688 |
| ols-three-dense-4m | 1 | 22.388316 | 22.218360 | -0.76% | — | — |
| ols-three-dense-4m | 4 | 12.118458 | 12.675189 | +4.59% | — | — |

B is qualified at this artifact: zero exact replay differences and unchanged
solver signatures, independent numerical witnesses, zero measured library-FMA
instructions, and all standard Mac nonregression bounds passing. No tolerance,
sample, rank, omission, separation, solver-family policy or SPI ownership
changed. The fresh strict PPML solve and independent certificates remain.


Retained SHA-256 records:

| Artifact / observation | SHA-256 |
| --- | --- |
| Linux baseline release | `166a23b18ab00a8f1929c0e44b6f57f5febb0e577d15229284b9cd2320d3807c` |
| Linux initial candidate release | `8fb51f4669f8e4f4c1d0d992bbb37bcd1ea8bb174177e50ac3de799d341b3cb9` |
| Linux baseline profile | `3fc36476c55de77facfcf43d228370b89392b3333a6bc85f36e976973dca6c48` |
| Linux initial candidate profile | `e6942e88afb6b22454488dcd262df5df25d9c0d54a83af812648805441605492` |
| Initial Linux hardware tests | `5803d7860cd60719bd08fc809035fb495bfbe9aeb422d9c01a386adc813822d4` |
| Initial Linux replay log | `b48cf722ac01c4a37543b299c0b6b281861d0ffc931ffd2e662e66b7946e1be4` |
| Loaded libm | `eff3c73980ea0c10daeeb029e998500d1ba643a9a8ad49a43f8978b4e013312a` |
| Matching libm debug symbols | `c718009c8ff3f040e0a941f17212fd87db079824205435a38d9b7e4263941ed9` |
| Follow-up source patch against ded7f627 | `980d5a34a0f5fb0d4e096f07aabaf2f084fc0e9fd86c4b1da0f9a5d24ba1d283` |
| Follow-up full gate | `945dadf648a2f20f2d774aec3ffd72a32f82e10593cbe4deb9d0e7fad4e34d8b` |
| Rejected zero-width certificate test run | `51a5a424944083943c94957a87b8c20d97102cd41af16593ff36768b6280ed62` |
| Corrected core test run | `8120fea89fee81095fc4efa961e333b1cf9503e8c7b04198a6230b6b1e11f33d` |
| Rejected comparator build | `4aa882d1e7e56eb53057c5b845ce1de864eb4a6e54885b896e316fd27464cf36` |
| Follow-up SCC source snapshot | `a731b48c9c29eaed8dcf623cba4e5a35db2e842a8ab8571d7fad21a02d82b345` |
| Mac comparator rebuild | `5ab89f3834e33eec571cda4ea85d008b3ae36dabb794cbb868df5b67a053671d` |
| Mac thirty-three-entry candidate | `d927004c347261c5349635f0567fce791b2772be90000cac9d39f6cc02848340` |
| Mac follow-up replay summary | `b2d8fe6b9ca0a3b248299e490538f7a59d3afc5775515d58cd520b609b6dfea9` |
| Mac follow-up licensed fixtures | `9397e27d95a4515abac5be37f451eaeb22436fe690b213a016707403e1becbf3` |
| Mac follow-up OLS Break | `6e34ae90d9ae35896ca63ce58bd1dd3b893325aa47dcfbe3cf9dfe2f763e3238` |
| Mac follow-up PPML Break | `f5aada3ea8df93ebc394b615bb07031d17c06bbe03831bcbf2f1ade2b1a2f3fb` |
| Rejected fixture filename | `28a351d62776a1b9edd78ebf82660b3d82f91a32a0e09a6e3cc65b3f331b192e` |
| Mac follow-up nonpassing timing panel | `b650f8439453d34a2c5359a680b9680df0bfcb089df404d11063f9b9adef0a99` |
| Linux follow-up instruction summary | `9d355710c03fd4f2f2c248da24c2b70384c7c631747ab8d3da4e60f147aa9f7b` |
| Linux follow-up native summary | `505b0733d3dfc45c4e3bf3599e9ab3c25c89812ab4fe4541c794219ff5637bc2` |
| Linux follow-up accounting | `f323f1b208033555dcc8bd467a254fadbf66c7e66653426580c9e8bd06a0bd48` |
| AKM follow-up caller attribution | `c89307ef5bd3dc979c14fb0df68dd0d8f72c88a42ab208d3deb6e7515bc2f12e` |
| Thirty-seven-entry source patch against ded7f627 | `157e9ec6d30144d426b663e900f63f345423a2bb626a1aec8ad206ce3ec2a9d2` |
| Thirty-seven-entry SCC source snapshot | `86c9abf823a7aed3da5aeb59ed6b09588342ea1e64776a7bcae2b3a43ded3924` |
| Thirty-seven-entry full gate | `02688a398e33aa2bb517c2595050d13c616ce48263edd3e321e56f2d346f18ff` |
| Thirty-seven-entry Mac plugin | `ac74d34954b1ff3fdf6613d6f6ab80c9381ad0762efcdc7ccfaece1276becfba` |
| Thirty-seven-entry Mac replay | `a8710ac1effa10b8e0b5af46022ecd97854877d4c6802e10eae96cf8d7f6849e` |
| Thirty-seven-entry Mac licensed fixtures | `9397e27d95a4515abac5be37f451eaeb22436fe690b213a016707403e1becbf3` |
| Thirty-seven-entry OLS Break | `6e34ae90d9ae35896ca63ce58bd1dd3b893325aa47dcfbe3cf9dfe2f763e3238` |
| Thirty-seven-entry PPML Break | `f5aada3ea8df93ebc394b615bb07031d17c06bbe03831bcbf2f1ade2b1a2f3fb` |
| Rejected Valgrind 3.22 hardware tests | `57354524dd832b2cf2b482d7888dafd6e771302bff7c9bf49ce3915d5e91a9ca` |
| Independent FMA zero probe source | `de1aa78f2bbec19101f4a56553e59bbab236030607235a08b4cb0ae31a7a99c4` |
| Native FMA zero results | `3dbca8400c03dad9461538a71f7fa2ddd986bc31c3d64eb2a28cb84c72b7774a` |
| Valgrind 3.22 FMA zero results | `88c16ef6aca7be918b76e7a82bc0c4380fc4e1c6eb2ba1b31b61a7e68c32a5ea` |
| Official Valgrind 3.27.1 source archive | `5d589152eb8071c02feab8ce6ab719e431a1fbc3e2b1700f5432632a8b9264dc` |
| Valgrind 3.27.1 hardware tests | `98340e74a02c2a36d287fd07465e233f9d8523656d8a5b250365d5cccddbd47e` |
| Valgrind 3.27.1 tool binding | `311da0ccd9ac0bff8ff10861260d179c01f20b4893f06ff2b5bae1f124d6670d` |
| Valgrind 3.27.1 zero probe results | `3dbca8400c03dad9461538a71f7fa2ddd986bc31c3d64eb2a28cb84c72b7774a` |
| Thirty-seven-entry symbolized native instruction panel | `179291d0f93858a77f55426b3eb89881a193a8b8573446d02f6dedae05e69972` |
| Final three OLS FMA callers | `3fd8a635763b00695583757be6db24d751cac28812d96906adf00a308935e8f6` |
| Interrupted fifteen-pair Mac panel | `a2b05c18d1b65da6547ee3088c8d3fdb86a8587050db1534d2f7f2372d0664be` |
| Completed scalar coverage source patch against ded7f627 | `41198869f47dd53d9f24e4610254013f0eee9e0e8e1c47efbe0c8b9b6e7ee1d8` |
| Completed scalar coverage source snapshot | `6ecbaa1535fb22e1328ecec0a3789d1608ef87d2861e302cf13b789d7b6de3c3` |
| Completed scalar coverage full gate | `7f65358d49940d4a0d8ce33c4f4b48bd61a2791fb23ebd17b85570430175c9bf` |
| Completed scalar coverage Mac plugin | `272e4c6019864ee3951533bedd8f7595b910f4e062f0ed75dc7d79e570fccea7` |
| Completed scalar coverage Mac replay | `9ae793ecebf0a7ea307b55a99f4ffeab01eab1991e52412c2154fccf9cb4eab9` |
| Completed scalar coverage licensed fixtures | `9397e27d95a4515abac5be37f451eaeb22436fe690b213a016707403e1becbf3` |
| Completed scalar coverage OLS Break | `6e34ae90d9ae35896ca63ce58bd1dd3b893325aa47dcfbe3cf9dfe2f763e3238` |
| Completed scalar coverage PPML Break | `f5aada3ea8df93ebc394b615bb07031d17c06bbe03831bcbf2f1ade2b1a2f3fb` |
| Completed scalar coverage zero-FMA panel | `eeb417e48c2f51073f2d17b1e1bb241cabd766096b53ea59e1b59ca65665adc6` |
| Completed scalar coverage incomplete native panel | `0cc0fd1fb2fc3eec625feee4fd673713d00812de66459f39bf247aed0981cfdf` |
| Completed scalar coverage failed Mac timing | `8fa0455b9d9da448277d26ea1111606fb711a55c3076c8bf35d5724178ad5bdb` |
| Cached-dispatch source patch | `4aa37b80f717a716942de0a541f5f00f2823dd739beb4737ef6de9302d7911be` |
| Cached-dispatch source snapshot | `ab073eb7574581140e0cc2493bf5d2064e6ff30d534eaae3f767830d1ed658b8` |
| Cached-dispatch full gate | `be1b8276e83a3c79f68d4cf15a95354110557426b3b377ca0addcc99c5237a10` |
| Cached-dispatch Mac plugin | `68c21a90ef763b78c1029aeeb41011606cf0c210225873d9695b5ccacb67edc1` |
| Cached-dispatch Mac exact replay | `5c23e755bc40806ba7f7325a374cb43fee88f21f75d2c96df47767f82ca35cdf` |
| Cached-dispatch Mac timing | `04b36b0d19550c41d86a16af8a528812e8320dc9861190330b4b3ba1b980c055` |
| Cached-dispatch Mac host before | `ec198af26f055c7e09d42ef7a4ea4da9764d2796140decd5ebc9aba799bfc051` |
| Cached-dispatch Mac host after | `0be00fb52fbffd29d0a2883a163f0cceb02349681edab1d11e2462e269ee23dd` |
| Cached-dispatch zero-FMA panel | `fdd0fedf6ed8440d3566040f43a207df4a367e239dfe4f5432d14f53524fcf1d` |
| Cached-dispatch Linux release | `447fe148222d8339a603b043135ca9921088f68ab3298bafa53b8d3bfe096b48` |
| Cached-dispatch Linux profile | `27b66f821c647ed20b01c0556847271fde362c932ea3649a9a4e2616283804f4` |
| Cached-dispatch full-size native panel | `694f6c53b43ffa3da6a88d570d8a53ea2eb4adaa920e27528d9253ec85be206d` |
| Cached-dispatch SGE accounting | `8df3211a5a97d8ef604a3774dc7c0e59f9b270972579b1097a4a23444d65b113` |
| Completed scalar coverage Linux release | `898115854130c8b369d5c41ec3e1424a028a828ec7217db028a821ee6c29ab29` |
| Completed scalar coverage Linux profile | `442cc82692a43c2f51a914b9f29976bb83ff9b1fa6e88c6985f5e71d9b86d6b3` |
| Native panel summary | `26592189e7ab6d8050eadadfbd29316938794ee4a1b44b3651f51f076580a245` |
| Symbolized instruction summary | `c147ce2c3e750e61387d811c31b6135ac8020dc3854c72be348f784de2ab2158` |
| SGE measurement accounting | `c0fa1e5b453c85ee57b5d1212d23ab706a0e4e83466acbc7528adaedae145b9e` |
| Caller attribution | `2088d1978995a04c75b59bd532fbaa62fd5e7245acd7b421d1b4d400d59e2cf0` |
| Timed-out panel | `8e37e5a1c0d3b3f07012ca6b381f9a22b94dc50e5b7106fa5786ec7850bd85ae` |
| Unattributed probe | `3395ce0cc79eba5e68b4a50783b4fc7a692a38740d30857cb853711d0337becd` |
| Rejected symbol lookup | `ac363e09a977089061a751286f342cad7221e62be3a7554f996a12144166d7e2` |

## Accepted evidence

### Campaign-shaped replay tooling (G3), 2026-10-11

The large replay generator adds explicit unrestricted-candidate,
nondeterministic and sample-only-output options, plus an AKM topology with
five observations per worker and 80% persistence. Existing defaults remain
unchanged. The topology uses the oracle RNG; it is not a byte reproduction of
the frozen SCC campaign. All SCC comparisons continue using frozen inputs.
This is an unchanged native artifact: only oracle/tooling code and documentation
change. Production identity against `4d2e873b` passes for all 169 runtime
modules and 58 artifact inputs. No estimator arithmetic, tolerances, routes, sample or rank semantics
change.

The full repository gate and all 42 benchmark-tool tests pass. All 73 default
replay cases are byte-identical before/after. Generator witnesses check worker
shape, determinism, persistence and the required sample output. Small OLS/PPML
native probes each repeat exactly twice, with both certificates passing and
21 physical columns in the two-FE/sixteen-regressor/unit-weight example. The
shared replay wire module stays unchanged. Two helper-placement attempts failed
the unused-code lint before the helper was scoped to the large generator;
no lint or assertion was relaxed. Missing matplotlib/reportlab initially
prevented report tests; installing them in the task-local environment allowed
all tests to run.

| Check | SHA-256 |
| --- | --- |
| G3 commit-boundary full gate | `a90b12905211d5ac131d49374c686ed112bcca20f2f76ba4d7f7937c4855f030` |
| Native probe results | `4b787115fc017526b454195855766cbb7d487af3bdceac3ab6d7c59d1b8d67b2` |
| Default-case identity | `2a5c4447ef7863d449cd265f32623ec70e8b4b3e3fcbb99e3122e72a49de6003` |
| Benchmark-tool tests | `d66b9fcaa555d76fca0295b0d5ed0758e164bfe611c32548618db32f17a30913` |
| Rejected shared helper | `d8828b7d404c9f9bf09681dd76f1b0d6f97ba332e818750dd221d6f6fcbbf3cb` |
| Rejected helper visibility | `6f1175eefd54a2024d761e1c8909eabab09b8ad9afc445e576b972a7e7f4e5bd` |
| Missing matplotlib | `715a473dcc33ff91c61b30f72fdd8805b14d85f4d99a149cb56af49db3d8654d` |
| Missing reportlab | `e52be28333988596fb55d1221cce967f5f1fdafb566501bc3d5df76c6d34c5bc` |

### Performance-plan tooling, 2026-10-11

G1/G2 are arithmetic-preserving tooling changes with an unchanged production
artifact relative to `cad23f41`: the production-identity check passes for all
168 runtime modules and 58 artifact inputs. The campaign captures OLS runtime
rows 18–21 and all 13 PPML phase rows, and new manifests explicitly retain
separate default/strict OLS arms. Strict OLS uses `fixef.tol=1e-11` and
`ssc(K.exact=TRUE)`, reporting covariance DoF separately from raw `nparams`.
Historical campaign manifests and retained observations are unchanged.

All 42 benchmark tests and `scripts/run_tests.sh` pass on Mac. The R witness
uses fixest 0.14.2 and checks disconnected IID/robust OLS coefficients and
covariance against explicit-dummy least squares and an independent sandwich.
A licensed Stata 19 probe executes the campaign metadata block for both models,
returns normally with `CAMPAIGN_METADATA_PASS`, and records four OLS fields
and PPML total plus 13 phases, including an unobserved phase as missing.
Local documentation links and the complexity ratchet pass. No new estimation,
sample, rank, omission, separation, solver, tolerance or certificate behavior
is introduced, and no performance gain is claimed.

Failed setup attempts remain recorded here: the default shell selected Python
3.9 without the scientific packages, and the first gate lacked Cargo on PATH.
The successful checks used a disposable Python 3.13 environment and the installed
pinned Rust toolchain. A disconnected clustered diagnostic with strict fixest
reported `df.K=13`; its expected nesting-adjusted count was 7. That comparison
is not a passing covariance witness. Strict clustered campaign cells retain
their actual count and must pass the existing comparison gate; no native
convention or comparison tolerance was changed to accommodate fixest.

SHA-256 records: benchmark tests
`35b70ecc6a18a22e7d21fba5d7662fa389f180dd8b34009b8cf4221f3a990d97`;
full gate
`e9e85645d44ffa168e8d039381dad1e7c0765137c8b5dc502958b4b7c9d211fb`;
licensed metadata probe
`c0e664c0b33221fa48372db5fece7f8fa9c4d4e91bd0b62f5e273d49cf92d903`;
OLS metadata
`63600d9f89e51edc7d60779df4a7cebc11ec904b62c0dae1b6e0a74b010429dc`;
PPML metadata
`afd7b1d23cae9af6a224b19c7c2fc0960f0b248e1a9fc333e1e0417bd11e6120`.

A downstream audit caught that the comparison whitelist initially omitted the
new `ols-*-K.exact=TRUE` policy label. The follow-up admits that OLS label and
tests that differing correction DoF still fail. All 42 benchmark tests pass;
log SHA-256
`6deb4315f564ba536984df97986fef3111afad4568dd98360ef4fa487bf09860`.
This follow-up changes no runtime or comparison tolerance.

Artifact digests are SHA-256 prefixes; the ledger gives full values.

| Date | Scope | Host | Artifact | Result |
| --- | --- | --- | --- | --- |
| 10-07 | Integrated runtime: 375 families, 98 holdouts, 6,375 native commands at 1–16 workers | Mac | `20a9d3d9` | No scientific failures, exact repeats, stable decisions |
| 10-07 | Single-runtime cleanup: gate, 64 licensed fixtures, 73 replay cases, staged installs, Break | Mac | `14dd962c` | Pass; one parity fixture only after its repair |
| 10-07 | Windows build, PE audit, loader, Stata smoke (`a8cdd888`) | Windows | — | Pass; the first run failed |
| 10-08 | Handover integration `e3380ede`: 1,647 tests, 73 exact replay cases, mapped fixtures | Mac | `66e35883` | Pass |
| 10-08 | Merged FMA group means: Cachegrind, SCC 7968913 | Linux | `07e5e590` | Exact; group-function instructions −26% and −54% |
| 10-08 | Guarded Linux loading (SCC 7969139) and Stata 19 load-time A/B | Linux | `45c54141` | Pass; bit-identical captures |
| 10-08 | Windows rounding and loading, clean `e3380ede` | Windows | — | Pass |
| 10-08 | Wide-Gram dispatch and `within` CSR pin `297f32d9`, quiet timing on 22 workloads | Mac | `5eb93024` | Exact; largest upper bound 1.044 |
| 10-09 | Windows controller repair rerun, `81c51e7e` | Windows | `08995120` | Pass |
| 10-09 | Cleanup phase 1 (`f5eace5c`–`c4d91623`) against baseline `9debf9bf` | Mac | `49ab9e29` | Exact replay; timing within allowance |
| 10-09 | Phases 2–3 (`09fc5e4c`–`261536ac`); ulp scope `296cbc23` rebaselined replay | Mac | `c0f9aff9` | Exact replay; timing within allowance |
| 10-09 | Phase 4 shipped fault plans (`5dcaaf81`–`e97d355e`) | Mac | `fdd48d56` | Exact replay; timing within allowance |
| 10-09 | Integration checkpoint `8fb67bb5`: Mac gate, Stata full (68), Break, Linux build and stores, Windows driver | All three | `c4363362`, `91933073`, `961afc7c` | Pass, except full staging (see below) |
| 10-09 | Saved-output fixture `9c08204b` on Windows | Windows | `ac92ad81` | Pass; Windows batched stores verified |
| 10-09 | Measurement-driven work (`02936956`–`7dd56c01`); one-pass group-mean ulp scope; baseline `d4b68d26` | Mac | `4c1a432f` | Scoped replay; quiet timing pass |
| 10-09 | Windows FMA kernels and module pin `d5eb37a7`, `04a0e255` | Windows | `c3783aec` | Pass after one intermittent loader crash, fixed by the pin |

Timing rows use paired-median bootstrap upper bounds against max(5%, 1 ms) on
the frozen workloads. They are bounded native-call observations on one host,
not release or end-to-end Stata claims.

## Failures, resolutions and gaps

- Resolved on Mac 10-10: `ols_three_fe_batch_oracle.do` had failed with
  `r(459)` on every binary: explicit
  LSMR stopped its recurrence at `tolerance()` and certified per-level FE
  moments at that same tolerance, leaving one weighted column at 1.0167e-8
  against 1e-8. The same gap remained at the 1e-14 final residual projection
  used for saved outputs. In a 128-configuration sweep at 100,000 rows, 2 of 64
  failed on the base plugin. Johannes's decisions A and A' (2026-10-10, commits
  `5413cfe` and `a551feb`) make explicit OLS LSMR iterate to a hundredth of the
  tolerance, floored at 1e-15 and never looser than it, and certify at the
  tolerance. With that, the native witnesses
  `explicit_lsmr_three_fe_batch_matches_explicit_dummy_oracle` and
  `explicit_lsmr_final_residual_projection_keeps_a_margin` pass, and the sweep
  has no failures. Mean LSMR iterations change from 16.7 to 20.7 in the main
  projection and from 16.1 to 13.7 at the residual step on strong graphs.
  Instructions on one 100,000-row explicit-LSMR case rise 3.7%. Linux replay
  against `e237c217` passes under the declared scope
  `tests/replay/scopes/2026-10-10-explicit-lsmr-direct-tolerance.json`: only
  ols-10 and ols-13 change (coefficients at most 1.4e-15 relative, covariance
  at most 2.1e-10 relative), and 71 cases stay exact with 2- and 4-worker
  checks. The licensed fixture and Mac boundary timing now pass; see the
  qualification record below.
- Full release staging failed at `8fb67bb5` because its input held only the
  Mac plugin; only the Mac review archive passed.
- One passing Windows run after the module pin bounds, but cannot exclude, a
  residual loader race; the mapped-after-release assertion is deterministic.
- The LSMR helper's unpreconditioned retry after a positive-definiteness
  breakdown keeps the full allowance and reports only its own iterations.
- Default solver routing is unchanged pending a decision: automatic PCG
  exhausted a reduced budget on weak and chain graphs where LSMR certified.
- Deferred review items: PPML-10 (result-changing), FE-11, OLS-4, FE-16 and
  the PLUG-12 runtime missing value (next wire bump).
- Not adopted: PGO (three-FE regression), the experimental all-FMA build and
  the automatic PCG continuation (failed an 80-level chain at `iterate(128)`).

## Standing caveats

- Mac-only qualification does not establish Linux, SCC, Windows or Intel-host
  Stata evidence; Rosetta exposes no FMA/AVX hardware path.
- FFI allocation tests observe the whole process and run serially; a parallel
  invocation fails by design and is not a runtime defect.
- Retained native allocation tests do not claim exhaustive detection of every
  new allocation.

## 2026-10-10 improvement programme

PR #31 was fast-forwarded from `140dda27` to `06ed5f94` on 10-10. Each stack was
checked on Linux with `./scripts/run_tests.sh` and the 73-case replay against a
`140dda27` baseline plugin (`e237c217`), with zero exact differences; commit
messages record each result. Behaviour changes are confined to admission and
failure paths: the separation report bank's memory charge (plan D9; admission
only loosens), Break reaching saved-FE recovery and long serial row passes, the
PPML validator keeping `r(1)`, PPML rejecting string numeric inputs at layout,
test fault plans compiled out of shipped builds (plan D4, reversing phase 4's
choice), and the OLS multiway PSD certificate floor becoming `f64::MIN_POSITIVE`
(plan D1; it can only reject, and every replay case was unchanged). At
`2c34533`, Linux Cachegrind on two 100,000-row single-worker cases (OLS two-FE
robust; PPML two-FE clustered) compared the base plugin `e237c217` with
`3a4fdc32`: exact outputs, total instructions −0.0027% (OLS) and −0.015% (PPML),
against repeat spreads of at most 5,415 instructions. This is an instruction-count
observation, not timing. The explicit-LSMR decisions (`5413cfe`, `a551feb`) are
the programme's only deliberate result change. After them the Linux replay
anchor moved from `e237c217` (`140dda27`) to `ba350134` (`71c289a`), which
reproduces the declared scope against the old anchor and replays exactly
against itself. The Mac qualification and replay re-anchor are recorded below.

Mac follow-up found two qualification defects at `5d548274`. Its replay had
71 exact cases and only `ols-10`/`ols-13` differences, but both input hashes
differed from the Linux scope; they match the existing Mac corpus. Separate
Mac scope `2026-10-10-explicit-lsmr-direct-tolerance-macos.json` preserves the
Linux scope unchanged and covers two additional one-ulp fields: adjusted R²
and the rank condition proxy. Input-derived 70-digit Decimal witnesses now
check both fields at rtol 1e-12/atol 1e-14. Scoped replay passes all 73 cases
with 2/4-worker checks against a fresh `140dda27` Mac plugin (`4842d6f3`).

The bounded separation report bank also retained an unintended logical ceiling:
600 rows, 257 zero outcomes with separate indicator regressors, then positive
outcomes, with `separation(simplex) keepsingletons`, succeeded on the base but
returned `r(3499)` at the PR tip. Separating initial reservation from the full
logical ceiling and reserving exact-FE follow-up records fixes it. The native
257-generation witness and public sample/intercept/robust-variance checks at
1/4 workers pass; all 73 replay cases remain exact against the original PR
plugin (`04157bcc`). Both that public fixture and
`ols_three_fe_batch_oracle.do` pass on Mac Stata/MP 19. These focused results
use candidate plugin `2d2dd904`; final integration results follow.

| Final qualification (10-10) | Artifact / receipt SHA-256 prefix | Result |
| --- | --- | --- |
| Mac build and replay, merged `06ed5f94` | plugin `336015be`; replay `c8e08323` | Universal audit and ABI/load pass; 73 scoped cases and 146 worker checks pass. Re-anchored from `4842d6f3`; only ols-10/13 differ. |
| Mac timing against `140dda27` | timed plugin `8c1b779e`; summary `18e5f922`; host observations `d67117e9` | All 18 workloads exact and within max(5%, 1 ms), 15 alternating pairs each. Largest bootstrap upper bound 1.04017 (K=129, four workers); paired medians 0.97417–1.00678. Johannes accepted practical quiet; brief background bursts and every pair were retained. Earlier seven-pair upper bound 1.05118 failed; two interrupted confirmations were inconclusive. No release-speed claim. |
| Licensed Mac Stata/MP 19 and PTY Break | full `4ea95203`; hidden `1cab8469`; development `4a8fe8dc`; Break `6e34ae90` / `756572c6` | Full 69/69, ppml-hidden 12/12, development 64/64, including the three-FE oracle; OLS/PPML Break pass. Initial development failures led to fixture repairs in `4f6d0427`; tracing was disabled for allocation assertions. |
| Bounded Windows build/load | plugin `de000832`; receipt `f18dba21` | Native build, ABI/load and licensed smoke pass; machine stopped. |
| Bounded SCC Linux build/load and report-bank regressions | job `8016714`; plugin `eff64e7b`; result `03b58901`; accounting `220ed385` | glibc 2.28 audit, ABI/load, both UBSan variants and both targeted tests pass; scheduler failed=0/exit=0, 216 s, maxvmem 3.886G. Original job `8016626` failed linking system UBSan; explicit GCC 12.2 fixed it and is pinned in campaign setup by `06ed5f94`. |

Full hashes are in the documentation commit. Evidence built at `ac7a7a9` and
`4f6d0427` is reused through `06ed5f94`: all 168 production modules and 58
artifact inputs are identical; `run_tests.sh` passes at the final source.

### Packed SCC campaign after PR #31

Run `20261010T155133Z-pr31-5e2951a4` uses merged source `5e2951a4`
(archive SHA-256 `7b168204`) and Linux plugin `a259496b`. Its report and
complete per-configuration comparison are retained under that run in
`/projectnb/welfgr/fereg/runs/`, in `report/` and `comparison/`; compact
accounting is in `collected/`. This is a descriptive comparison with the
reconstructed summaries of `20261007T171717Z-akm-current` (production
`7942263`), not a universal speed or platform claim.

| Evidence | Result |
| --- | --- |
| Setup `8016862`, smoke `8016895`, production `8016905.1–4` | All six accounting records have failed=0/exit=0; sentinels, frozen identities and all 492 planned fit positions verified. glibc 2.28 audit, ABI/load, UBSan guards, 40 harness tests and 11 native smoke oracles pass. Production used 28 slots on 28-core nodes, without the exclusive consumable. |
| Scientific outcomes and unavailable cells | Native repeatability and cross-worker checks pass for 55/56 configurations; four-million-row three-sparse PPML times out as before. Across 164 arm cells: 131 eligible, 25 asterisked, 8 without timing, versus 129/18/17 previously. The capture repair exposes seven remaining reference discrepancies and makes two strict-fixest cells eligible; no formerly eligible cell becomes ineligible. All failed fits and reference smoke failures remain reported. |
| Timing and scaling | Native current/previous median ratio 0.9521 across 55 configurations; four-worker medians 0.9390 OLS / 0.9582 PPML. OLS AKM at 8/16 workers is 20.2%/20.4% slower on the same node, despite nearly unchanged fixest controls; the follow-up below isolates and repairs the cause. Its 16-worker speedup falls from 2.573× to 1.692×. PPML AKM gains only 2.6% in elapsed time from 8 to 16 workers. No explicit-LSMR arms were scheduled. These separate campaigns do not replace the Mac qualification timing. |
| Memory and report | Native peak sampled RSS 5.032 GiB, essentially unchanged; median 1.120 versus 1.174 GiB across 51 resolved cells. Unresolved memory stays NR. All 19 PDF pages rendered and visually inspected. PDF SHA-256 `ba133153`, configuration summary `f0ab36f9`, full comparison CSV `6c8b9cc9`, retained-file manifest `a4bb00bc`; full hashes are in the documentation commit. |

### OLS high-worker slowdown investigation

Three bounded SCC experiments on `scc-zj4` used nine alternating or rotating
pairs per arm at 1/4/8/16 workers, with 28 slots on a 28-core node and no
exclusive consumable. The frozen AKM input has one million rows, 16 regressors,
200,000 worker and 10,000 firm levels, iid inference and seed 739391 (input
SHA-256 `5fd1a5cf`). Full-size warmups precede timing; diagnostic traces follow
it. Inputs and the timed Stata command are unchanged from the campaigns.
Reproducers, observations and accounting are retained under
`/projectnb/welfgr/fereg/dev/20261010-ols-slowdown/`.

Commit `b21244aa` replaced private scalar norm accumulators with repeated writes
to adjacent shared scratch slices. Keeping its contiguous column partition but
privatizing the recurrence removes the regression. The selected repair uses
bounded 64-column stack tiles (1 KiB per active worker), then publishes scratch
once. It preserves each column's observation order and arithmetic and adds no
heap allocation. The intervention supports a shared-scratch implementation
cause, consistent with false sharing; no hardware cache-counter measurement
establishes that mechanism exclusively. Restoring the older per-column loop was
also measured, but retaining blocks performed better at four/eight workers.

| Evidence | Result |
| --- | --- |
| Endpoints, job `8017329` | PR31/October-7 paired ratios at 8/16 workers: 1.0942 / 1.1274; 95% bootstrap intervals [1.0772, 1.1890] / [1.0883, 1.2688]. Norm setup and certificate time increase; operator time and 14 solver iterations remain stable. This reproduces the campaign slowdown with smaller magnitudes under paired measurement. |
| Controlled intervention, job `8017369` | Private-block/control ratios at 1/4/8/16 workers: 1.0278 / 0.8076 / 0.7322 / 0.7981; upper bounds 1.0367 / 0.8358 / 0.7949 / 0.8524. The observed single-worker cost is retained. Exact science and solver signatures across variants; scalar norm/scratch witnesses through 257 columns and 1/2/4/8/16 workers pass. Linux glibc 2.28 audits, ABI/load and both UBSan guard variants pass. |
| Direct repaired/October-7 confirmation, job `8017720` | Ratios 0.8269 / 0.7950 / 0.8649 / 0.9210 at 1/4/8/16 workers; all upper bounds below 1.0. Coefficients, covariance and model metadata are exact. Historical residual/certificate diagnostics differ already before the repair; current-to-repaired diagnostics are exact. Peak sampled RSS remains about 650 MiB. All three jobs have failed=0/exit=0 (181/697/172 s). |
| Mac science and transactions, plugin `c5190816` | `run_tests.sh`, universal audit/load, 73 exact replay cases and 292 checks at 2/4/8/16 workers pass. Working-tree licensed profiles: full 69/69 and ppml-hidden 12/12, including OLS/PPML PTY Break. Source patch and plugin hashes bind these profiles; their source label is the parent commit. No fresh Windows check is claimed. |
| Mac timing and future coverage | All 20 frozen workloads retain exact science/signatures. Seven-pair run passes 19: PPML/four-worker upper 1.06765 fails during observed backup/sync activity. One predeclared 15-pair confirmation passes (upper 1.00211); pooling all 22 pairs also passes (1.00538). Largest upper among the other 19 is 1.03420. Practical-quiet observations and every failed pair remain retained; no release-speed claim. The new eight/sixteen-worker K=20 guards pass, and the verifier now rejects unlisted/missing/modified workloads. |

The repair's Linux plugin is `e5eed20e`, built from `5e2951a4` plus the measured
private-block patch; its production tokens equal the integrated repair. Full
artifact, source-patch and result hashes are in the repair commit. The old 18
frozen workload hashes are unchanged. These are bounded workload observations,
not a universal speed or platform claim.

### Packed SCC campaign after the OLS repair

Run `20261010T192805Z-ols-repair-e29164e6` compares source `e29164e6`
(archive SHA-256 `3129d28a`, Linux plugin `57cc9789`) with the actual preceding
PR31 campaign `20261010T155133Z-pr31-5e2951a4`. Compact results, all 164
per-configuration arm comparisons and the 19-page visually inspected PDF are
retained under the new run in `/projectnb/welfgr/fereg/runs/`, in `collected/`,
`comparison/` and `report/`. Full hashes are in this documentation commit.

| Evidence | Result |
| --- | --- |
| Setup `8018263`, smoke `8018275`, production `8018282.1–4` | All six scheduler records have failed=0/exit=0; sentinels, frozen identities and 492 fit positions verified. glibc 2.28 audit, ABI/load, both UBSan variants, 41 harness tests and 11 native smoke oracles pass. Production used 28 slots on 28-core nodes without exclusive. |
| Scientific and failure outcomes | Native repeatability and cross-worker checks pass for 55/56 configurations; the same four-million-row three-sparse PPML case times out. All 164 arm cells retain eligibility and failure reasons: 131 eligible, 25 asterisked, 8 NR. The 15 reference smoke capture failures and 18 oracle failures are unchanged. |
| OLS scaling on identical inputs and `scc-zj4` | AKM one-million-row times at 1/4/8/16 workers are 2.2325/1.2916/1.0426/0.9551 s; current/previous ratios 1.0342/0.7961/0.7134/0.7486. This confirms the controlled repair at 8/16 workers while retaining the 3.4% single-worker cost; fixest controls remain within 1%. Sixteen-worker speedup improves from 1.692× to 2.337×, but 8→16 still saves only 8.4%. |
| Breadth, PPML and memory | Four-worker median ratios are 0.8857 OLS / 1.0124 PPML. Native ratios across 55 eligible cells range 0.7134–1.0870; increases remain visible. PPML AKM robust 1/4/8/16 times are 12.8068/6.4174/5.3236/4.9661 s on a different node, with only 6.7% improvement from 8→16. Peak sampled native RSS is 5.03249 GiB versus 5.03227; common 51-cell median RSS ratio 0.99999. Unresolved memory remains NR. |
| Cross-campaign input audit `8018721` | All 150 successful records in 50 configurations with identical numeric inputs have exact captured estimator science, including all OLS configurations. Five PPML inputs generated on different nodes differ only in response values by at most two ulps (verified hashes; failed=0/exit=0). Their 15 fits differ only in coefficient/covariance rounding, within unchanged SPEC tolerances; discrete metadata and reported objectives remain exact across all 165 successful fits. The three existing unavailable records remain unavailable. |

SHA-256 prefixes: PDF `5dc95256`, configuration summary `4a3440b3`, full
comparison `c4c91585`, retained-file manifest `5603b923`. The complete comparison
flags input and host changes.
Separate campaigns are descriptive: they neither replace Mac qualification nor
establish universal speed or platform claims. No explicit-LSMR arms were run.

### General scaling investigation against fixest

The [detailed investigation](fixest_benchmarks.md#generalization-follow-up)
uses unchanged runtime `e29164e6`, Linux plugin `57cc9789` and pinned fixest
0.14.2. Reproducers, frozen synthetic inputs, captures and CPU samples remain
under `/projectnb/welfgr/fereg/dev/20261011-fixest-general-scaling/`.
Compact results and full hashes are in the linked report's retained evidence.

| Evidence | Result |
| --- | --- |
| Phase/solver panel `8022196`, holdout `8022206`, CPU sample panel `8022215` | All 70 planned fits complete; all three scheduler records failed=0/exit=0, 284/519/90 s. The holdout extends to 16m rows, 4/64 controls, skewed/weak networks and 28 workers. Native captured science agrees across workers; all four original fixed-worker repeats are exact. Four default weak-network reference comparisons remain ineligible; they are not counted as speed wins. |
| Generalization and memory | Fixed-cardinality, 16m-row OLS takes 20.441/11.758 s at 4/28 workers versus fixest 7.427/6.829; PPML 63.258/54.484 versus 31.992/25.888. Peak sampled native RSS is 6.422/10.614 GiB for OLS/PPML. These are single instrumented fits, not qualification timings. |
| Root causes | Dense two-FE PPML setup remains about 8.9 s at 4/16 workers. Its serial neighbour-pair assembly grows 8× for 4× rows; annotation attributes 96.33% of builder-local sampled cycle weight to that loop. Dense three-FE OLS MAP controls take 22–24% less time than auto in the sampled panel, while retaining scientific agreement. Runtime and solver defaults are unchanged. |
| Independent weak-network audit `8022296` | Direct sparse OLS agrees with native slopes within 1.11e-13, RSS within 7.22e-9 and exact graph rank/DoF. Tight fixest convergence plus exact FE counting resolves the OLS discrepancy; two strict PPML arms time out at 120 s and remain unavailable. Audit failed=0/exit=0, 318 s, 1.947G; original launcher `8022280` exited 1 before science and was repaired for four-slot affinity. |

The [compact follow-up evidence](../bench/results/scc-20261011-scaling-diagnostics.json)
has SHA-256 `3d864721`; its full hash and retained-file manifest hash are in
the documentation commit. The stale PPML help description of auto selecting
LSMR was corrected to match the existing MAP-owned policy. `run_tests.sh`,
the 16 focused help checks, unchanged-production identity and local link checks
pass; the Mac run reports the expected Linux mock-plugin audit skip. No new
Mac timing, Windows qualification or release-performance claim is made.
