# Validation status

The many-FE, two-FE structural-gate and ownership/deferred-cell improvements are
accepted, as are the many-FE geometry policy with one documented timing exception
and the PPML two-FE CMG reuse and lean protocol released in v0.5.0-beta.7.
The OLS method-reporting correction changes diagnostic labels and
fallback flags only; solver selection, iteration budgets, numerical calculations
and PPML reporting are unchanged.

## Upstream migration candidate

Beta.11 is the published current migration from beta.10. The
[compatibility report](within_migration.md) records the frozen dependency bases,
patch evidence, qualification results and accepted performance tradeoffs.
The final source pin is `8d401d82`: the essential migration stack plus counts,
back-substitution, gather and the general Fold running accumulator. Johannes
accepted all four optimizations and their disclosed individual-patch tradeoffs
on 2026-10-30, and authorized combined building and qualification. Scatter slices
remain excluded; no processor-specific dispatch or scientific-semantic change
is included. Combined source, native and preflight checks passed on Mac and Linux.
Johannes conditionally accepted the disclosed eight-/sixteen-FE performance
regressions on 2026-10-30, provided investigation of the two large fitted-value
disagreements supports the accuracy of the estimates. That numerical condition
is now met: independent optimum enclosures and actual beta.10/beta.11 checks pass
for both full designs on Mac and Linux. His acceptance of the disclosed
eight-/sixteen-FE performance tradeoffs therefore applies. This selects the
beta.11 migration candidate without relabeling the adverse performance results.
The final Linux two-FE PPML control completion now passes all four cells,
including the previously timed-out baseline. The original failure remains
retained. Johannes subsequently authorized finishing release qualification and
publishing beta.11 as the current GitHub version; the package checks below
record native and installation qualification. The earlier counts-only `534ea64f` candidate had
55 passed Mac cells, two inconclusive cells and one regression; its five valid Linux hosts
were below the six-host requirement. Those classifications remain unchanged.

The final combined artifacts bind fereg `65500be6` and within `8d401d82`.
[Mac source checks](../bench/results/within-final-source-macos-20261030.json)
and [Linux source checks](../bench/results/within-final-source-linux-20261030.json)
pass the complete source suites and dependency checks. The
[Mac native record](../bench/results/within-final-native-macos-20261030.json)
passes all twelve native stages, including replay, licensed fixtures, actual
Break/rollback, memory checks and sixteen large-design calls. The
[Mac final preflight](../bench/results/within-final-preflight-macos-20261030.json)
passes all 28 actual beta.10/final-candidate calls, fourteen arm comparisons
and seven cross-worker family checks. Six families execute within; the dense
three-FE MAP-completed family remains an exact control. These records preserve
source/artifact identities, scientific results and output hashes; large captures
are disposable after verification. They establish correctness prerequisites,
not combined performance acceptance or publication. The corresponding
[Linux native record](../bench/results/within-final-native-linux-20261030.json)
and [Linux final preflight](../bench/results/within-final-preflight-linux-20261030.json)
pass the same twelve native stages, sixteen large-design calls and final
28-call science grid, with successful terminal SCC accounting. The larger
companion panel and combined performance comparison remain separate gates.

The [final Mac comparison](../bench/results/within-final-macos-20261030.json)
retains all 1,508 calls across 58 settings and six mirrored blocks, with no
retries, exclusions or scientific failures: 53 settings pass, four regress and
one is inconclusive. The sparse three-FE full-call medians improve from 5.268 to
5.026 seconds at one worker and from 1.916 to 1.681 seconds at four; the original
Mac slowdown is recovered. Eight-FE medians increase from 181.2 to 200.7 ms at
one worker and from 82.6 to 92.2 ms at four. Sixteen-FE medians increase from
444.3 to 497.0 ms and from 201.0 to 229.9 ms. Their qualified median block ratios
are respectively +10.54%, +11.63%, +11.78% and +14.36%; these are distinct from
ratios of the displayed call medians. The four-worker 64-regressor AKM setting
is inconclusive. Fixed-worker scientific fingerprints, independent references
and cross-worker checks pass. Ordinary desktop activity remains in the timings;
the Mac is one machine, not a set of independent Linux hosts. These performance
results alone did not accept beta.11; Johannes's subsequent acceptance and
release instruction are recorded above. No further optimization round is included.

The [initial final Linux panel](../bench/results/within-final-linux-initial-20261030.json)
retains 1,981 calls: 580 on each of three completed physical hosts and 241 in
three incomplete attempts on a fourth host. The first process-observation gap
exceeded 7.5 seconds after the observer added a fresh five-second delay to an
initial scheduler census. All 116 scorer comparisons remain insufficient at
three hosts. Eight- and sixteen-FE settings at both worker counts have adverse
host medians on all three completed hosts; these observations remain retained.
The bounded observer-only completion preserves these hosts and all failed
attempts, and permits only the three missing independent units.

That [completion](../bench/results/within-final-linux-completion-20261030.json)
now supplies six distinct isolated physical hosts and all 3,480 eligible calls;
all 241 original failed-attempt calls also remain retained. All array tasks and
the report have successful terminal accounting, and report validation has no
errors. Of 58 unique endpoints, 50 pass, four regress and four are inconclusive.
Eight-FE complete-call medians increase from 421.1 to 462.5 ms at one worker
and 213.2 to 229.9 ms at four; sixteen-FE medians increase from 895.8 to
992.4 ms and 468.9 to 516.7 ms. Their median block ratios are respectively
+9.72%, +8.20%, +10.74% and +9.75%, distinct from ratios of the pooled call
medians. All six host excesses are positive for each of these four endpoints.
Sparse three-FE at one worker is inconclusive (+4.89% median block ratio;
12.731 to 13.362 seconds in full-call medians). The four-worker 16-regressor
AKM, four-FE T1 and small-third T1 endpoints are also inconclusive. The original
near-confounded-third T4 setup regression is recovered: its complete-call
comparison passes at a -2.28% median block ratio. Johannes subsequently accepted
these eight-/sixteen-FE regressions conditionally
on resolving the numerical concerns below. The original classifications and
inconclusive settings remain retained. No further timing or optimization loop
is authorized.

The large companion panel first stopped on a baseline OLS reference convention:
fereg's documented pairwise absorbed DoF is 2,095,755, whereas the strict fixest
reference reports 2,095,753. Johannes approved preserving the pairwise policy.
The [independent input-graph witness](../bench/results/within-companion-reference-conventions-20261030.json)
reproduces fereg's counts and supplies the matching OLS covariance normalization
without changing raw references, production semantics or tolerances. It also
checks PPML's separate convention: sixteen identified slopes, reported finite
rank seventeen including the normalized constant, and one direction removed
from absorbed rank. PPML covariance remains unchanged. Full checks of retained
native arrays and the unfinished companion calls remain required; the graph
witness alone does not qualify those estimates.

The subsequent [retained-array diagnostic](../bench/results/within-companion-fitted-diagnostic-20261030.json)
records a separate unresolved fitted-value disagreement on beta.10. Rechecking
saved captures started no new native calls. After the approved reference
normalization, coefficients, covariance and scalar criteria pass, but 1,317 of
ten million OLS rows and 19 of one million PPML rows exceed the unchanged
`1e-9 + 1e-7 * max(abs(native), abs(reference))` fitted criterion. Maximum
absolute differences are `4.5843e-6` and `5.6730e-7`; RMS differences are
`3.9842e-9` and `1.1754e-9`. Ordered input hashes, sample, reconstruction,
objectives and normalized model scores pass for both fits. Beta.10's maximum
normalized scores are `2.4602e-13` and `2.4550e-11`, compared with reference
scores `4.6608e-9` and `6.7322e-9`. This raises a reference-accuracy concern
but does not establish pointwise correctness. The diagnostic completed in
15.98 seconds with successful terminal SCC accounting and zero fits. Both
fitted gates remain failed. Johannes subsequently approved continuing the seven
unaffected families, retaining both failures. The bounded continuation covers
28 native calls per platform; Mac adds four weighted-OLS memory calls, while
Linux reuses the four already passed weighted-OLS admission/refusal checks.
Both held families remain outside this continuation, and a new failure stops
the subset. No numerical tolerance or production behavior changed.

The subsequent [bridge witness](../bench/results/within-companion-bridge-witness-20261030.json)
establishes reference error on a subset, independently of either estimator.
In the worker/firm incidence graph, a bridge cell has a free intercept even
with the finite regressors and third FE present. At an OLS or positive-outcome
Poisson optimum its fitted sum must equal its outcome sum. A single-row bridge
therefore has the exact optimum `fitted = y`. All 7,506 OLS and 728 PPML
single-row bridges pass for fereg; the reference fails on 104 and 12 rows.
Maximum fereg errors there are `1.1257e-11` and `2.5644e-10`, respectively.
The graph algorithm passes eleven independent edge-removal checks and every
claimed bridge cut among the failing cells is checked against all original
unique mover edges. Job 8358713 completed on `scc-gr4` in sixteen seconds with
successful terminal accounting and no estimation calls. The maintained
[graph oracle](../crates/fereg-oracle/python/bridge_optimum_witness.py) and
[four independent tests](../tests/python/test_bridge_optimum_witness.py) also
pass locally, including weighted dummy OLS with extra regressors and an extra FE.

Conditionally optimizing each bridge-cell intercept removes 399 OLS and eighteen
PPML discrepancies, leaving 918 OLS rows and one PPML row. Multirow cell
corrections hold their within-cell contrasts fixed and are not an enclosure of
the full optimum. They therefore do not close either original fitted gate or
qualify the candidate. No reference, tolerance or runtime behavior has changed.

The [independent optimum enclosure](../bench/results/within-companion-optimum-enclosure-20261030.json)
now covers both complete retained beta.10 fits, including all 1,317 OLS and
nineteen PPML disputed rows. The [derivation](within_numerical_enclosure.md)
constructs explicit score-balancing graph flows and a small stayer contrast
matrix, separately checks model-space feasibility, and converts these into
prediction and identified-coefficient bounds. OLS fitted error is bounded by
`1.93753e-10` and finite coefficient error by `3.60405e-12`. PPML's sharpened
curvature bound places every fitted error below 70.46% of its unchanged tolerance
and finite coefficient error below `6.74899e-10`, versus the smallest coefficient
tolerance `1.27198e-9`. This supports the estimates independently of fixest.
The original comparator failures remain failed comparisons, with proved
reference errors on the exact bridge subset; they are not migration bugs.

The first diagnostic's PPML report failed on NumPy boolean serialization after
retaining its looser OLS bound. Its failed exit and result remain in the record.
The bounded refinement passed on `scc-gr4`, with successful terminal accounting,
67 seconds wall time and 4.179 GB maximum virtual memory. It shortened the
explicit graph-flow paths and tightened score reduction bounds; it ran no new
native estimate or reference fit. Twelve dense analytic checks passed on Linux;
eighteen expanded OLS/Poisson cases plus the scalar curvature refinement pass
on Mac. Standard floating-arithmetic allowances apply; this is not a formally
verified interval-arithmetic implementation. The subsequent completed Mac/Linux
checks below include the center's uncertainty inside the original tolerance
when checking the actual beta.11 artifacts against this enclosed solution.

The [completed Mac enclosure checks](../bench/results/within-enclosed-companions-mac-20261030.json)
pass all eight beta.10/beta.11 calls on the two formerly held families, at one
and four workers, plus all eight cross-arm/cross-worker comparisons. Each call
adds the independently proved center uncertainty to its difference from that
center before applying the unchanged accuracy tolerance. The worst fitted bound
uses 19.3736% of tolerance for OLS and 70.4552% for PPML; the worst coefficient
bound uses 0.2651% and 53.0589%, respectively. Both arms execute within, with
403 OLS and 149 PPML dependency iterations in every call. Scalar/covariance,
model-score, reconstruction and discrete checks also pass. The controller
completed in 524.2 seconds, with zero retries. These are correctness checks,
not a new performance or fixed-worker repeatability comparison. Together with
the earlier unaffected subset, all 36 planned Mac native cells now pass.

The [Mac PPML memory completion](../bench/results/within-enclosed-memory-mac-20261030.json)
passes the four remaining original memory cells: beta.10 and beta.11 both meet
the independent optimum enclosure when admitted with 64 GiB available; both
refuse at 512 MiB available (zero usable bytes), preserving every matrix and
observation column and performing no solver work. It completed in 78.9 seconds,
with zero retries. Combined with the earlier weighted-OLS checks, all eight
planned Mac memory cells pass. This tests early refusal, not late allocation
failure or performance, and does not close the separate Linux coverage gaps.

The [Linux enclosure attempt](../bench/results/within-enclosed-companions-linux-20261030.json)
passes both retained-center preflights and revalidates the original beta.10 OLS
cell, but the first new beta.11 ten-million-row OLS T1 call reaches its fixed
600-second limit without returning estimates. Its last trace reports a certified
FE solve, with all seventeen RHSs converged; that does not establish completion
of estimation or explain the timeout. The sequence stops, leaving six cells
unstarted. Job 8359778 on `scc-pf8` records scheduler `failed=0`, process exit 1,
988 seconds wall time and 11.256 GB maximum virtual memory. This is incomplete
qualification, not an observed numerical mismatch or a performance verdict.
Johannes approved one retry at 1,200 seconds plus the six unstarted checks,
within a further 3,800-second total limit. Both original Linux timeouts and all
original reference comparisons remain retained.

The [approved Linux completion](../bench/results/within-enclosed-continuation-linux-20261030.json)
passes all eight beta.10/beta.11 cells on the two formerly held designs at one
and four workers. It reuses the original baseline OLS T1 result and runs exactly
seven new calls, including the one approved retry; no further resampling occurs.
The previously timed-out candidate OLS T1 call completes in 153.94 seconds on
`scc-b11`. This does not diagnose its earlier timeout on `scc-pf8` or establish
a performance comparison against the reused baseline from a different host.
All eight cross-arm/cross-worker comparisons pass. Maximum fitted bound ratios
are 0.193735 for OLS and 0.704552 for PPML; maximum finite-coefficient ratios are
0.002651 and 0.530589. All calls execute within, with 403 OLS and 149 PPML
dependency iterations. Scalar/covariance, model-score, reconstruction and
discrete checks pass; all 1,317/19 raw reference discrepancies remain recorded.
Job 8360696 has successful terminal accounting, 557 seconds wall time and
14.519 GB maximum virtual memory. The checker/controller takes 555.1 seconds,
within the approved 3,800-second budget. This closes both fitted-value accuracy
questions for the actual candidate on Mac and Linux. At this checkpoint Linux
passed 32 of 36 large native companion cells; the final PPML completion below subsequently
closed the four remaining cells. No performance or new fixed-worker repeatability
claim is added by this science-only completion.

The [Linux memory completion](../bench/results/within-enclosed-memory-linux-20261030.json)
passes all eight original admission/refusal cells: it revalidates five retained
cells and runs only the three unstarted PPML cells, with no native retries.
Both PPML arms pass the independent optimum enclosure with 64 GiB available.
Both refuse with 512 MiB available (zero usable bytes), preserve every matrix
and all 28 observation columns, and enter no solver work. Job 8360885 on
`scc-fi2` completes with successful terminal accounting, 179 seconds wall time
and 2.533 GB maximum virtual memory. The prior memory jobs plus this completion
use 470 seconds of the original 3,600-second budget. This verifies early
refusal and rollback, not late allocation failure or performance.

The large companion coverage is therefore:

| Platform | Native cells passed | Memory cells passed | Remaining limitation |
|---|---:|---:|---|
| Mac | 36/36 | 8/8 | No remaining cell in this panel |
| Linux | 36/36 | 8/8 | Original execution timeouts retained; completed controls pass |

The [final Linux PPML completion](../bench/results/within-ppml-release-linux-20261030.json)
passes both versions at one and four workers and all cross-version/cross-worker
comparisons on the ten-million-row two-FE control. Job 8361190 on `scc-tn3`
completes within its 4,000-second controller budget, with successful terminal
accounting. It retains the original 600-second baseline timeout and does not
establish its cause. No runtime, input or scientific tolerance changed. This
completes the two numerical investigations and the full Mac/Linux companion
panel, supporting the selected beta.11 candidate with Johannes's accepted
eight-/sixteen-FE tradeoffs. Windows and package-installation evidence are separate.

The [Linux continuation](../bench/results/within-unaffected-companions-linux-20261030.json)
passes all eight weighted-OLS Auto/LSMR calls, then stops when the ten-million-row
two-FE PPML beta.10 call reaches its 600-second limit. No candidate PPML or slope
call starts after that timeout. The final trace reports an inner reduced solve
with nineteen iterations; it does not establish the cause of the timeout.
Terminal accounting records `failed=0`, `exit_status=1`, and 12.363 GB maximum
virtual memory. This is incomplete qualification, not a numerical mismatch or a
performance comparison. Johannes approved the sixteen previously unstarted
Linux slope calls within a further 4,800 seconds of the original budget. That
[slope-only continuation](../bench/results/within-slope-companions-linux-20261030.json)
passes all sixteen calls and the cross-arm/cross-worker comparisons on
`scc-ei3`, covering sorted and permuted ten-million-row one-/three-slope designs.
Job 8357906 completed in 856 seconds with `failed=0`, `exit_status=0`, and
5.052 GB maximum virtual memory. Each call executes within and passes its
independent analytic witness. There were no retries or tolerance changes; the
PPML timeout was not retried. At this checkpoint, the earlier eight weighted-OLS
calls brought Linux to 24 of the 28 calls in the approved unaffected subset.
The final PPML completion above subsequently closed the remaining four checks.
The four previously passed Linux weighted-OLS memory checks remain valid.

The [Mac continuation](../bench/results/within-unaffected-companions-mac-20261030.json)
passes all 28 native calls and 28 cross-arm/cross-worker comparisons across the
seven approved families: weighted OLS Auto and explicit LSMR, ten-million-row
two-FE PPML Auto, and ten-million-row one-/three-slope designs with sorted and
permuted rows. Six families execute within; the PPML family is an independent
unaffected route control. The four weighted-OLS memory calls also pass: both
plugins meet the reference when admitted, and both refusals preserve every
matrix and observation column without solver work. The controller completed in
2,405.3 seconds including transfer and staging, with zero native retries. One
preparation-only failure occurred before any native call: the original helper
confused the reference JSON and NPZ hashes. Its narrow correction checks each
file against its own pinned hash, and both the failure and correction remain in
the record. Completed captures and staged inputs were removed after verification.
These checks establish correctness for the approved subset, not performance or
new fixed-worker repeatability evidence. The later optimum enclosures and Mac/Linux
completion above resolve the two numerical concerns on both platforms. The original
Linux two-FE PPML control timeout remains retained; the final PPML completion
above closes those four large-panel cells.

The separate [Fold accumulator comparison](../bench/results/within-fold-performance-20261030.json)
completes all 516 calls with exact science and solver work. The affected Mac
single-worker fit improves 14.89%; solve medians fall from 5.201078 to 4.313869
seconds, with nearly unchanged setup and 390 iterations over 19 RHS solves. All twelve
Mac intervals pass. Linux has five passed primary intervals and one inconclusive
weighted categorical T1 control, including an 11.15% slower host with both
blocks adverse. Actual sparse fits slow by host medians of 2.96% at one worker
and 1.87% at four. Two construction intervals are also inconclusive. All four
adverse primary and four adverse construction blocks remain retained; no host
is replaced. Johannes accepted this tradeoff for inclusion in the final candidate. Fold
arity-two-through-four performance remains unmeasured, and this independent
comparison did not accept beta.11. The combined beta.10 comparison and subsequent
acceptance are recorded above.

The [final bounded diagnosis](../bench/results/within-final-slowdown-diagnosis-20261030.json)
uses retained evidence and independently reproduced disassembly, with no new
timing or profiling. Loss of FMA does not explain the inspected hot loops.
Changed categorical accumulation is the strongest supported Mac explanation;
the general Fold patch restores register accumulation without restoring the old
arithmetic or whole-operator fusion. Counts removes unnecessary compensation
for exact dense counts, and the five completed Linux hosts descriptively recover
the original setup penalty. Hardware-level attribution, the missing sixth host
and adverse optimization controls remain unresolved. The diagnostic effort is
closed; final optimization inclusion and combined verification remain separate.
The separate counts/back-substitution commit `d1721a25` passes independent source
review, 26 focused solver tests and strict library Clippy. The final four-patch stack now proceeds to combined source and native checks
and a separately declared beta.10 comparison.

Johannes approved retiring the general more-than-two-FE pair-upgrade experiment;
its explicit h/k and builder-Break requests must fail clearly. Two-FE h/k and
builder-Break behavior, public Auto escalation, supported OLS/PPML paths and
independent scientific witnesses remain required. Approval of that retirement
does not accept beta.11 or waive the recorded residual-witness failure.

Johannes accepted upstream's whitening correction, supported by the independent
weighted slope-boundary oracle, for continued qualification. The original
fork/candidate rank-parity failure remains recorded. The native checks below
cover declared published results, factor slots/base flags, PPML and replay;
beta.10 performance must still pass. This decision does not authorize production
adoption or acceptance of the documented rejected near-boundary fits. The completed
original `d3407a16` main panels below each contain a regression and inconclusive
comparisons; the new counts-only panels also do not qualify beta.11.

Focused Rust 1.85.1 adapter checks passed 79 main tests and four subprocess
witnesses, including explicit LSMR, controlled routing, selector retirement,
two-FE CMG and PPML. The separate memory/admission set passed 97 distinct tests;
85 memory and four retained-storage tests passed again after the helper
refactor. The core complexity ratchet decreases from 113,243 to 113,029
production lines and from 1,458 to 1,451 dispatch candidates. Essential host
cancellation and the sanctioned iteration-Break witness increase plugin lines
from 15,527 to 15,675 and dispatch candidates from 141 to 143. The combined
runtime is 66 production lines smaller, with all large-size counts unchanged.

The historical extreme-weight fixture initially failed an expected fallback
flag, while Auto and Off both passed the original `1e-10` certificate and
matched its independent analytic projection within `2.23e-16`. The approved
test update preserves the fixture and now requires analytic error below
`1e-14`; counted-failure boundaries and upstream failing-operator witnesses
test fallback budgets independently of roundoff. The original expectation and
diagnostic values remain in the compatibility report. The full source suite
failed at `automatic_structural_map_uses_admitted_shared_operations`, which
expected positive reported solver workspace, after 1,535 core passes and three
ignored tests. Diagnosis showed Auto correctly admitted shared certificate and
coefficient banks while refusing an optional LSMR checkpoint; its separate
solver-workspace charge was legitimately zero. A test-only correction now
checks the actual reusable charges and total selected bytes against the cap,
preserving policy and numerical comparisons. Both this test and the explicit
LSMR checkpoint-admission test passed focused reruns. The original full-suite
failure remains recorded. The rerun passed all 1,536 core tests (three ignored),
then stopped at the PPML separation routing census: retained-CG developer
configurations 1/2 for seed 17 now fail their LSMR retry. Public Auto retains
exactly the incumbent sample and separation decisions in all three diagnosed
configurations and matches the independent 228-row LP witness. Causal isolation
found identical coefficients and one solver iteration, but different residual
reconstruction: upstream's weighted-residual unscaling gives an FE certificate
of `1.0478754743e-8`, just above this call's `1e-8` tolerance; direct unweighted
model subtraction gives `3.6898581565e-11`, matching the incumbent. No census
expectation or scientific tolerance has been changed. The opt-in prepared-model
output option passed its isolated and combined dependency checks; beta.11 now
pins `d3407a16` and selects it explicitly. Both PPML separation routing tests
now pass, including fixed-worker repeatability and cross-worker agreement;
the independent diagnostic remains ignored. The final Mac source-only suite
completed with exit status 0: 273 Python tests (12 skipped), 1,537 core unit tests (three
ignored), workspace scientific/integration and plugin/FFI tests, isolated
allocation audits, strict Clippy, fuzz compilation, portable/C guards and the
complexity ratchet passed. The compatibility report records its exact log
identity and environment. Installed-package admission was explicitly omitted.
Native correctness checks are recorded separately below; source integration
is not production acceptance.

The [native record](../bench/results/within-native-20261029.json) binds fereg
`d96eccfc`, within `d3407a16` and both actual plugins. Mac and SCC builds,
all 67 licensed fixtures per platform, and four OLS/PPML LSMR Break runs per
platform passed. Break traces prove completed solver work, distinct cancellation,
rollback and successful reuse. The 77-case/693-call replay passes science,
fixed-worker repeats and cross-worker checks on both platforms. The original
Mac declaration failed three PPML diagnostic magnitudes; a source-reviewed
declaration correction passes reassessment without native reruns, preserving
that failure. Linux's first replay launch failed on an ABI-only environment
setting; a separate replay-only repair passed without repeating the independent
fixture/Break stages. Both platforms also pass the four-case public slope
contract: eight oracle fits, eight expected atomic rank-boundary rejections and
four frontend refusals each. Earlier failed expectations remain recorded under
Johannes's instruction to preserve existing rejections. Two additional mapped
fixtures pass on each platform, covering four weighted OLS reuse fits and
eight offset PPML fits against Poisson references. Their traces establish
ordinary Within OLS and augmented joint-PPML execution; the Linux timing
harness was compiled but made no performance calls. Package admission,
beta.10 performance and production acceptance remain outstanding.

The qualification science preflight passes all 28 calls and 14 comparisons
on each platform across seven prospective many-FE families, both actual binaries
and one/four workers. Independent oracles and cross-worker scientific/discrete
checks pass. Six families reach Within in both binaries; the dense three-FE
family completes in the unchanged MAP pilot and remains exact. Linux job
8327872 on `scc-pg7` completed with scheduler failure/exit status 0/0 and all
terminal markers. Its 28 collected result/log pairs and path traces were
independently verified; column files remain on SCC. The native record retains
the exact identities and accounting. This is no timing or additional
fixed-worker repeatability claim; the completed main timing panels follow.

The [main qualification record](../bench/results/within-qualification-20261029.json)
retains every call, order, warmup, timing, solver row and scientific fingerprint
in lossless compact payloads, with artifact/input/source hashes and decoding
instructions. Both panels use the unchanged actual beta.10 and beta.11 plugins,
29 families at one/four workers, and the frozen max(5%, 1 ms) allowance.

| Panel | Native calls | Independent units per cell | Passed / inconclusive / regression |
| --- | ---: | --- | --- |
| Mac | 1,508 | Six mirrored blocks on one desktop | 51 / 6 / 1 |
| Linux | 3,480 | Six physical hosts, two blocks each | 55 / 2 / 1 |

Linux array `8328471` used six distinct whole-node allocations; all six jobs
and report job `8338074` ended with scheduler failure/exit 0/0. There were no
native errors, retries or discarded timings on either platform. The maintained
reports passed scientific, fixed-worker and cross-worker checks; six families
actually execute Within, while the dense three-FE MAP control remains exact.
Independent read-only review reproduced all block orders, report classifications
and bounds. These are one-sided `.025` bounds under the declared unit assumptions,
with no simultaneous-coverage claim. The two reference labels alias the same
beta.10 artifact and are counted once in the table.

Mac `campaign-three-sparse-t1` regresses: median paired ratio `1.12666`, with
raw timed medians 5.59002 versus 6.30906 seconds. Both arms record 19 RHS solves,
390 iterations and outer counts 23+14. Linux `near-confounded-third-t4` regresses:
ratio `1.11520`, with excess above the allowance bounded at 1.58–8.60 ms.
Both arms retain five RHS solves, 71 iterations and outer counts 22+10.
Recorded Linux setup medians increase 14.90–16.70 ms across hosts while solve
medians decrease 1.62–4.41 ms. These phase observations do not identify a cause.
All these calls retain LSMR with additive Schwarz and zero fallback.

Linux's other inconclusive cells are `campaign-akm-k16-t4` and
`campaign-three-sparse-t1`. The record lists all six Mac inconclusive cells.
Mac retains the explicitly permitted Claude background activity: 47.95 sampled
CPU seconds across five bound processes; this is neither an isolation claim nor
a reason to discard adverse measurements. Both complete panels remain
`needs_discussion`; no repeat sampling, production adoption or performance
acceptance follows from them.

Johannes subsequently authorized exactly one function-level profile per frozen
plugin for each platform's regressing cell, followed by a justified fix in the
fork only. All four calls completed and passed their independent oracle, exact
per-artifact scientific fingerprint and recorded solver-work checks. The
[qualification record](../bench/results/within-qualification-20261029.json)
retains their diagnostic summaries and source/profile identities. Mac sampling
and disassembly point to categorical accumulation's per-row bounds checks;
Linux sampling and allocation inspection point to unnecessary compensation
arrays for exact unweighted categorical counts. These motivate separate patches
that preserve arithmetic. They do not establish causal effects or recovered
performance: each profile contains one instrumented call, and Linux's user-space
CPU event excludes kernel allocation costs. Linux job `8338538` completed on
`scc-we4` with scheduler failure/exit 0/0. Any revised candidate requires a
separately declared comparison before acceptance; the completed main-panel
verdicts and all adverse observations remain unchanged.

The two independent fork optimizations now have Mac and Linux public science evidence
in the [patch record](../bench/results/within-optimizations-20261029.json).
Dense categorical counts compare `32dde1bd` with `ba409ada`; categorical Fold
slices compare `6f2ead13` with `1f16b437`. Both use Rust 1.85.1 with generic CPU
release builds, preserve arithmetic and leave the frozen beta.11 plugin
unchanged. The [migration account](within_migration.md#independent-fork-optimization-checks)
links the frozen commits on Johannes's fork and defines their narrow production
scopes and retained independent tests. Neither has been pushed upstream or
submitted as an upstream pull request.

| Patch panel, per platform | Native processes | Exact base/patch comparisons | Exact one/four-worker comparisons | Maximum independent error |
| --- | ---: | ---: | ---: | ---: |
| Dense counts, Mac and Linux | 24 each | 12 each | 8 preconditioner apply each | `7.20e-14` |
| Scatter slices, Mac and Linux | 40 each | 20 each | 8 full solves each | `1.231e-9` |

Every process includes a repeated four-RHS group, with no timers or retries.
Both panels retain the `1e-8` independent witness bound. The two-FE harness uses
solver tolerance `1e-11`; the weighted/sloped entrypoint harness uses the already
approved `1e-12`. Counts' full solves are restricted to one worker because its
frozen upstream base has a known four-worker LSMR repeatability failure; the
combined candidate's fixed-worker requirement remains unchanged. Read-only
reviews checked both completed panels, their exact artifact/source bindings
and every declared comparison. Linux public jobs `8339222`/`8339223` completed
on `scc-tb4` with scheduler failure/exit 0/0, using four-slot shared correctness
allocations, generic CPU builds and GCC 12.2.0. They establish no performance or
isolation claim. Successor `8339224` passed the eight count and three scatter
private witnesses on that host with scheduler failure/exit 0/0; this is a
focused-filter result, not a Linux full-suite claim. These checks do not
establish beta.11 performance acceptance;
the original regressions and inconclusive results remain in force.

The [independent performance record](../bench/results/within-optimization-performance-20261029.json)
now retains all 840 calls on six whole-node Linux hosts: 168 untimed preflights
and 672 timed calls, with exact order, scientific fields, block/host aggregates
and all source/artifact/configuration/accounting identities. Array `8340554`
completed with scheduler failure/exit 0/0 on all six tasks; science and isolation
checks pass, with no native errors, retries or discarded timings. Read-only
reconstruction confirms 23 passed and five inconclusive endpoint comparisons,
with no positive lower-bound regression. Both patches remain `review_required`:
counts has an inconclusive sparse construction control; scatter has three
inconclusive cache construction controls and a weighted memory four-worker
solve. The two endpoints are protected separately; six-host median bounds at
one-sided alpha `.025` have no simultaneous-coverage guarantee. Neither patch
is performance-accepted. The [migration account](within_migration.md#independent-performance-and-combined-source-candidate)
reports the improvements and unfavorable bounds without claiming beta.11 recovery.

Mac's two operational launches retain 56 untimed science calls and zero native
performance timings. A denied hardware query stopped the first; competing
unrelated Stata Monte Carlo work stopped the second at admission. No process
was stopped and no adverse measurement was excluded. Mac timing remains blocked.

Historical combined within source [`4beaa8af`](https://github.com/johannes-schmieder/within/commit/4beaa8afcdc56ddee6f29b3cbd5bf151f0f2d5c5)
was integrated at fereg `379bf7c8` and published on Johannes's fork only.
Its [source record](../bench/results/within-optimized-integration-20261029.json)
retains 588 passing top-level tests, two passing isolated subprocess tests and
one ignored fixture-regeneration test on Mac Rust 1.85.1. A missing cancellation
argument first caused test compilation to fail; the repair changes only the
private witness. fereg's updated pin passed 324 Python tests, 1,537 core tests,
189 non-FFI and 219 FFI plugin tests, scientific integration/oracles, strict
Clippy and portable checks. The original script exited 1 at stale complexity
locations; the first follow-up exited 1 for unused historical growth entries.
After approved metadata-only repairs, size/ratchet, mock-SPI and numeric guards
passed. All measured totals and limits are unchanged, both failures remain
recorded, and no broad rerun occurred. No new actual plugin has been built:
the earlier native evidence and performance verdicts still apply to `d3407a16`.
Fereg `65449fd6` restored the pin to `d3407a16`. Its manifest and both lockfiles
match tested fereg `d96eccfc` exactly; the only shipping-source difference is an
explanatory memory comment. The earlier source/native correctness evidence is
reused, with successful locked offline metadata checks for the 59-package
workspace and 64-package fuzz workspace. No plugin rebuild or new fereg native
test was run.
The original beta.11-versus-beta.10 regressions remain distinct from the standalone
scatter weighted-control concern. No recovery or acceptance follows from the
restoration. At that stage, additional strict-reference fits remained unapproved;
the approved extension below now has results. The held back-substitution patch,
upstream submission and
production adoption remain outside this result.

Johannes subsequently approved testing counts-only `534ea64f`; it is now the
active source pin, with no scatter or back-substitution change. Its single Mac
source-only suite passed 324 Python tests, 1,537 core tests (three ignored),
189 non-FFI and 219 FFI plugin tests, independent scientific oracles, allocation
audits, strict Clippy, fuzz, portable/C guards and complexity checks. The
[migration account](within_migration.md#independent-performance-and-combined-source-candidate)
binds the exact pin diff, environment and log hash. Actual-plugin checks now pass
on both platforms: all 67 licensed fixtures, four iteration-Break runs, mapped
memory/reuse/slope checks and the 77-case/693-call replay. Replay matches protected
`d3407a16` exactly with 308 fixed-worker and 231 cross-worker checks. Each platform
also passes 24 targeted calls covering dense unweighted Schwarz counts, actual
generation-2 retained-CG-to-LSMR retries and public Auto controls. The unchanged
28-call preflight against beta.10 passes on each platform. Operational launch,
build and environment failures remain recorded with their bounded repairs.

The [counts-only record](../bench/results/within-counts-qualification-20261029.json)
retains all 4,489 main-panel calls (713 warmups and 3,776 timed calls), full
ordered scientific/work fingerprints, source/artifact/environment hashes and
report decisions. Mac's 1,508 calls produce 55 passed cells, two inconclusive
cells and one regression. Sparse three-FE at one worker has median paired ratio
`1.130373` (+13.0373%). Across all 12 timed calls per arm, complete-call medians
increase from 5.294788 to 5.983382 seconds, summed solver-row solve medians from
4.527583 to 5.218120, and setup medians decrease from 0.151976 to 0.150099.
Both warmup traces retain 390 iterations and 19 RHS solves. This identifies the
affected phase, not its cause. Bound desktop activity remains measurement noise,
with no retry or excluded adverse measurement.

Linux retains 2,981 calls. Five whole-node hosts complete 580 calls each; the
sixth stops after 81 calls on a startup observer gap of 7.843798 seconds against
the frozen 7.5-second limit. No native/scientific failure or competing process
is recorded. The unchanged report excludes that entire host but the compact
record retains every call, including its final unpaired warmup. All five-host
cell medians are within the allowance descriptively; all 58 cells are still
`insufficient_hosts`, with null one-sided `.025` bounds. Report job `8345728`
completes with scheduler failure/exit 0/0. There are no actual retries or
discarded attempts. The confirmed Mac regression and Linux uncertainty block
acceptance; every earlier adverse observation remains retained.

The three strict large references now pass under Johannes's approved bounded
extension (job `8342499`, four workers, no retries): sparse three-FE OLS at ten
million rows, sparse three-FE PPML at one million and AKM PPML at ten million.
Scientific thresholds are unchanged, and earlier timeouts remain failures.
The separate large companion and eight-call memory-admission panels remain
unrun. Their status does not change the completed main-panel decisions.
The gather-zeroing investigation remains separate from this fixed candidate.

The [bounded diagnosis](../bench/results/within-scatter-diagnosis-20261029.json)
finds real weighted caller code changes but no cause for the adverse controls.
Job `8341332` failed a counter-label prerequisite before native work; a narrow
parser repair preserved that failure. Job `8341371` completed exactly two
profiled calls with exact science, zero lost samples and scheduler failure/exit
0/0. This single pair on previously stable `scc-zj4` did not reproduce the
slowdown. Samples cover the whole process, include unresolved parallel-helper
work, and cannot clear the earlier adverse hosts. Scatter remains held; counts
and the original beta.11 regressions retain their prior decisions. The record
also preserves the restored-pin size check's Python-selection failure and
successful interpreter-only repair. No new qualification or acceptance is claimed.

The Linux separation-fallback companion passes its 16 declared actual-plugin
calls on `scc-pi2` (job 8328529, scheduler failure/exit 0/0). The seed-17 weighted
PPML design retains the independent LP's 228 original rows and passes the
dense Poisson oracle in both binaries. All eight injected calls actually take
the successful generation-2 CG-to-LSMR retry, with 147 iterations over ten
RHS solves; public Auto is a control and uses no Within work here. Eight
fixed-worker repeats are exact, and four cross-worker comparisons pass science
and preserve decisions. At each worker count, public Auto and injected CG have
identical estimates and observation outputs. Across binaries, only the declared
IR interior/span certificate magnitudes and their trace digest differ; all
other scientific fields stay exact. The digest remains exact within a binary
across workers and repeats. Because it also aggregates unpublished discrete
internals, the cross-plugin exception does not prove full internal execution
identity. All 16 Linux result/log/column captures and the scheduler record were
independently rechecked without new native calls. The native record retains
their identities. The same declared Mac companion now passes all 16 calls,
including eight actual generation-2 retries, eight fixed-worker comparisons and
four cross-worker comparisons. Independent review confirms the same sample,
oracle and comparison scope. Its first launch stopped at output-directory
validation before native execution; correcting only that path required no
repeated native calls. Neither platform's checks make a performance or adoption
claim.

The separate sparse back-substitution optimization completed its practical Mac
comparison: 312 calls, 72 paired blocks and all 12 descriptive intervals passed
max(5%, 1 ms), with exact scientific signatures. All interrupted attempts and
unfavorable timings remain in the
[compact evidence](../bench/results/within-backsub-20261029.json), including
the explicitly retained Claude background activity. Linux passed all 12
host-median intervals across six isolated physical hosts, but one of 432
blocks exceeded the allowance by 2.528 ms. Johannes reviewed the retained
observation and instructed us to hold the optional upstream PR. This patch is
absent from the combined candidate and does not establish beta.11 performance
acceptance.

## Published beta.11 descriptive campaign

The October 30 SCC campaign of the published Linux plugin `af65682b...` (runtime
source `65500be6`) adds clustered-mobility and gravity designs, strict PyFixest,
32 instead of 64 controls, the Stata-comparison cells on an Ice Lake node, and
the Veneto long panel on the leave-one-out and largest connected sets. All 357
native fits outside two superseded cells passed their repeat, cross-worker and
independent smoke checks; the superseded clustered-mobility cells were rerun once
after the generator stopped leaving a singleton firm-year. Setups passed their
harness suites, and accounting, sentinels, host observations and frozen
inventories verified. No timed fit was retried or excluded and no tolerance
changed; five roots stopped before any timed fit are recorded in the
[completion record](../bench/results/scc-20261030-beta11-completion.json). The
PPML AKM bundle ran on a node whose reference fit was 1.8 times slower; that
observation is retained. This is descriptive measurement of a published release,
not new runtime qualification. See the [synthetic](fixest_benchmarks.md) and
[Veneto](veneto_benchmarks.md) reports and the
[accuracy study](../bench/results/scc-20261030-beta11-accuracy.json).

## Published beta.10 descriptive campaign

The October 28 SCC rerun of the published Linux plugin `a7964116...` (runtime
source `3348fe45`, harness `7c5d9f39`) retains the beta.8 inputs, pins, tolerances
and all 1,646 positions: 1,448 synthetic and 99 per Veneto variant. All 286
native fits passed. Synthetic fixed-worker repeatability passed in 56 cells
and cross-worker science in 14 groups; the panels passed 20 exact repeats and
30 worker-reference comparisons. All setup suites passed 421 tests, every
accepted scheduler record and application sentinel passed, and the retained
host observations and frozen inventories verified. Original pre-fit input
discovery failures and the bounded canonical-path repair are recorded separately;
no timed fit was retried or excluded.

Sparse three-FE OLS at ten million rows completed in 53.18 seconds (three
repetitions, 52.94–54.82 seconds), using 12.50 GiB. Comparator failures and
memory tradeoffs remain explicit in the [synthetic report](fixest_benchmarks.md)
and [Veneto report](veneto_benchmarks.md). The slower reference observation on
`scc-xc1` is retained. This campaign describes an already-qualified release;
it is not new runtime qualification and makes no absolute cross-host
beta.8/beta.10 speedup claim. See the
[completion and repair record](../bench/results/scc-20261028-beta10-completion.json)
and its linked evidence hashes.

## Accepted results

| Change | Evidence and scope |
| --- | --- |
| Many-FE Auto | [Final compact result](../bench/results/incremental-many-20261018-compact.json): Linux qualification and practical Mac recovery passed all 126 protected comparisons per platform; public activation and independent science passed. |
| Two-FE structural gates | [Linux](../bench/results/two-fe-policy-gates-qualification-linux-20261022.json) and [Mac](../bench/results/two-fe-policy-gates-qualification-mac-20261022.json): all 84 protected bounds per platform passed. [Activation](../bench/results/two-fe-policy-gates-activation-20261023.json): exact public/hidden science, worker, gate, hierarchy and resource checks passed after the documented transport-accounting amendment. |
| Ownership and deferred cells | [Linux](../bench/results/ownership-qualification-final-linux-20261024.json): 10,440 calls, 58 settings and 12 distinct physical hosts, all 116 protected bounds passed. [Practical Mac check](../bench/results/ownership-qualification-practical-macos-20261024.json): 870 calls and all 116 paired-median comparisons within max(5%, 1 ms). The Mac check is descriptive, not a confidence-bound claim. |
| Many-FE geometry Auto | [Decision record](../bench/results/many-geometry-20261025.json): Linux qualification passed 80 of 84 protected comparisons and public activation 122 of 126, on 12 distinct physical hosts each. Every miss is the near-confounded third FE, accepted by Johannes as a documented exception; public/hidden agreement was exact. |
| PPML two-FE CMG reuse and lean protocol | [Lean record](../bench/results/ppml-two-fe-lean-20261026.json): every two-FE cell faster than beta.6 at 4 and 28 workers on Linux (0.32-0.84), science within the §20 envelopes, 73/73 replay cases. Merged and released as v0.5.0-beta.7; the [beta.7 SCC comparison](fixest_benchmarks.md#october-26-2026-beta7-rerun-with-pyfixest-and-areg) found all 168 native fits passing and every OLS fit bit-identical to beta.6. |
| Two-FE Auto speed | [Qualification record](../bench/results/two-fe-auto-qualification-20261027.json): Linux 7,560 calls on 31 physical hosts, all 126 protected comparisons within max(5%, 1 ms) after Johannes waived the 12-distinct-host rule (84 cells had 11); practical Mac check 119 of 126 passed, the other 7 inconclusive with paired medians within the bound. [Veneto comparison](../bench/results/two-fe-auto-veneto-20261027.json): 1.24-2.08 times faster than beta.7 without saved FE and 1.06-2.18 with saved FE; faster than PyFixest's `within` in 9 of 10 cells. |
| Two-FE Auto round two | [Round-two record](../bench/results/two-fe-round-two-20261027.json): Linux 10,080 calls on 28 physical hosts, 123 of 126 protected comparisons within max(5%, 1 ms) after the 12-distinct-host waiver (every cell had 11) and 3 inconclusive under 1 ms; Mac check 115 of 126 passed, 11 inconclusive; Veneto candidate faster than beta.8 in all 20 cells and than PyFixest `within` in 9 of 10; PPML exact timing 12 of 12 within bound with identical science. |
| Public integration | [Final activation](../bench/results/ownership-public-activation-20261024.json): 174 actual hidden/public/public-repeat calls per platform, independent witnesses, fixed-worker exactness, cross-worker science, resource/gate/hierarchy checks, Mac/Linux ABI/load/panic checks and the full source suite passed. Six licensed Mac Stata fixtures and actual public OLS Break/rollback passed. |
| Beta.10 integration | [Round record](../bench/results/beta10-2026-10-28.json): Linux 13,920 calls on 35 physical hosts, 173 of 174 protected comparisons within max(5%, 1 ms) (one inconclusive by 0.02 ms; 20 cells one host short of twelve under Johannes's renewed waiver); Mac 162 passed, 11 inconclusive and the accepted sixteen-FE one-worker exception of the within fused LSMR step; Veneto candidate within 4% of beta.9 and faster than PyFixest `within` in 9 of 10 cells; PPML 12 of 12 within bound and bit-identical; sparse three-FE series finishes at ten million rows where beta.9 does not. Publication is a separate authorization. |

Ownership/deferred cells activate only for OLS Auto/Auto with a global intercept
and exactly two categorical-intercept FE terms. Explicit injections and paths
outside the scope retain their contracts. The Break test interrupts at the
native pre-preparation checkpoint; it does not establish interruption inside a
cell constructor. Scientific behavior is specified in [SPEC](SPEC.md).

## Two-FE Auto speed

Public OLS Auto/Auto with exactly two categorical-intercept FEs applies its CMG
hierarchy on the worker team, runs the deferred cell operator in parallel,
builds the hierarchy without CMG's low-effective-degree correction and hands
off early under gate v2; Fast fits without saved outputs certify their within
residual by its derived bound. Specified in [SPEC](SPEC.md) and
[the gate notes](two_fe_auto_gates.md); the investigation behind it is in
[the PyFixest comparison](pyfixest_twoway_investigation.md).

The fixed-candidate qualification used beta.7 public Auto as original and
incumbent and the hidden `two-gates-v2` arm, with the 42 routing cells. Linux
passed every protected comparison; the weak and high-degree-weak cells ran
4-11% faster and the others within noise. Science, gate decisions, resources
and cross-worker agreement passed. The routing cases all request saved outputs,
so they do not exercise the derived residual; its witnesses are the dense
explicit-OLS oracle, the 68 licensed Mac Stata fixtures and the Veneto runs.

On one whole SCC node with the audited Veneto panels (seconds, medians of three):

| Panel, workers | Candidate | beta.7 | PyFixest `within` |
| --- | ---: | ---: | ---: |
| short, 1 | 2.01 | 2.49 | 2.08 |
| short, 4 | 1.03 | 1.85 | 1.08 |
| short, 16 | 0.78 | 1.61 | 0.96 |
| long, 1 | 10.53 | 13.32 | 26.94 |
| long, 4 | 5.28 | 8.54 | 9.84 |
| long, 16 | 3.62 | 7.08 | 6.23 |

Every fixed-worker repeat was exact and every cross-worker, beta.7 and
PyFixest comparison passed the campaign tolerances. The 73-case replay corpus is
exact against beta.7's runtime, and PPML's campaign two-sparse design ran 2%
faster from the hierarchy options with the other PPML designs unchanged.

## Two-FE Auto round two

Round two keeps beta.8's policy and changes three mechanisms, specified in
[SPEC](SPEC.md) and [the gate notes](two_fe_auto_gates.md): the (worker, firm)
cells are built on the worker team, bit for bit as the serial build, and before
the diagonal pilot, so the pilot's Schur products use the cell operator, with
block cancellation polls in the serial kernel; the pinned CMG (`f6a044c9`)
dispatches a level's row-operator matvec to the team when its entries times
lanes reach the parallel threshold, so dense coarse levels with few rows no
longer run serially; and PPML's joint recurrence merges helper products, forms
the finite cross block and the level masses in exact parallel form and reuses a
finished canonical probe. The CMG and PPML changes are arithmetic-preserving;
the cells change moves low bits of the pilot within the certified tolerances
and is bound by the replay scope
[`2026-10-27-two-fe-eager-cells-macos.json`](../tests/replay/scopes/2026-10-27-two-fe-eager-cells-macos.json)
(three two-FE cases; solver codes, iterations, stop reasons and every discrete
decision unchanged). The [round-two record](../bench/results/two-fe-round-two-20261027.json)
holds every number below.

The fixed-candidate Linux qualification (beta.7 public Auto as original, beta.8 as
incumbent, the hidden `two-gates-v2` arm, 42 routing cells, 3 shards by 12
replicas, 10,080 native calls on 28 physical hosts) reported `needs_discussion`
because every cell reached 11 of the required 12 distinct physical hosts;
Johannes waived that rule for this round as for the previous one. Under the
waiver 123 of 126 comparisons satisfy max(5%, 1 ms) and three are inconclusive
by under a millisecond (sixteen-fe-t4 and small-retained-t4 against the
incumbent, small-retained-t4 against the original, medians below zero). No
scientific, gate, resource or cross-worker check failed and no native call was
repeated. Against beta.8 the repeated-cells, disconnected and wide-rhs cells
ran 19-21% faster at one worker and the high-degree and weak cells 10-15%;
weighted-t1 ran 4% faster, so the eager build's cost on that no-aggregation
design needs no exception on Linux.

The practical Mac check (42 cells, blocks of 6, 1,638 native calls, beta.8 as
original and incumbent) passed 115 of 126 comparisons with 11 inconclusive and
no regression or scientific error; weighted-t1, which the first eager-cells
candidate had regressed by 9% because the serial kernel polled once per cell,
closed to a paired ratio of 1.021 after the block-poll change. The licensed
Mac Stata development profile passed its 68 fixtures on a fresh build. On the
synthetic Veneto-shaped AKM panels (profiling builds, no competing process),
native time relative to beta.8 at 1/2/4/8 workers was 0.944/0.931/0.937/0.886 on the short panel
and 0.949/0.887/0.884/0.914 on the long panel. Attribution on the same
panels: eager cells cut the pilot's operator phase (long panel at two workers
0.72 s to 0.41 s) and the entries-gated matvec cut the short panel's recurrence
by 14/32/40% at 2/4/8 workers (its 3,043-row level is newly parallel; the long
panel's planned levels all exceed 4,096 rows and its dispatch is unchanged).

On one whole SCC node with the audited Veneto panels, three rotated
repetitions, every fixed-worker repeat exact and every cross-worker, beta.8 and
PyFixest comparison eligible (seconds, medians):

| Panel, workers | Candidate | beta.8 | PyFixest `within` | vs beta.8 | vs `within` |
| --- | ---: | ---: | ---: | ---: | ---: |
| short, 1 | 1.81 | 2.01 | 2.07 | 0.90 | 0.88 |
| short, 2 | 1.39 | 1.42 | 1.34 | 0.97 | 1.03 |
| short, 4 | 1.03 | 1.06 | 1.08 | 0.97 | 0.95 |
| short, 8 | 0.78 | 0.82 | 0.94 | 0.95 | 0.82 |
| short, 16 | 0.73 | 0.78 | 0.96 | 0.94 | 0.77 |
| long, 1 | 9.64 | 10.53 | 26.95 | 0.92 | 0.36 |
| long, 2 | 6.96 | 7.38 | 15.42 | 0.94 | 0.45 |
| long, 4 | 4.83 | 5.29 | 9.84 | 0.91 | 0.49 |
| long, 8 | 3.54 | 3.99 | 7.04 | 0.89 | 0.50 |
| long, 16 | 3.22 | 3.63 | 6.19 | 0.89 | 0.52 |

With saved fixed effects (PyFixest's saved effects exceed the command limit):

| Panel, workers | Candidate | beta.8 | vs beta.8 |
| --- | ---: | ---: | ---: |
| short, 1 | 2.83 | 3.27 | 0.86 |
| short, 2 | 2.11 | 2.14 | 0.98 |
| short, 4 | 1.49 | 1.51 | 0.99 |
| short, 8 | 1.11 | 1.15 | 0.97 |
| short, 16 | 1.03 | 1.13 | 0.91 |
| long, 1 | 13.19 | 14.61 | 0.90 |
| long, 2 | 9.68 | 10.25 | 0.94 |
| long, 4 | 6.61 | 7.15 | 0.93 |
| long, 8 | 4.78 | 5.35 | 0.89 |
| long, 16 | 4.31 | 4.97 | 0.87 |

PPML exact timing (campaign-DGP cases through `bench/ppml_cases.py` and
`bench/time_exact.py`, one whole node, five alternating repetitions, science
identical between beta.8 and the candidate in every run; candidate over
beta.8 native-time ratio, candidate median in parentheses):

| Design, rows | 4 workers | 28 workers |
| --- | ---: | ---: |
| two-sparse, 1,000,000 | 0.991 (5.66 s) | 0.984 (4.01 s) |
| two-sparse, 4,000,000 | 0.990 (25.42 s) | 0.967 (17.23 s) |
| akm-baseline, 1,000,000 | 0.983 (4.08 s) | 0.990 (2.74 s) |
| akm-baseline, 4,000,000 | 0.985 (18.32 s) | 0.995 (12.15 s) |
| two-dense, 1,000,000 | 0.996 (3.83 s) | 1.005 (2.48 s) |
| two-dense, 4,000,000 | 0.996 (14.76 s) | 0.999 (10.07 s) |

All twelve PPML cells pass the nonregression bound. The team application of
CMG to PPML's joint frames above 350,000 finest edges was built, measured at
0.98 on the long Poisson panel at 8 workers and removed; separation detection
is 42-61% of PPML native time on Veneto-scale Poisson panels and is the
subject of a separate workstream.

## Beta.10 integration qualification

The integration candidate `codex/beta10-integration` (runtime commit
3348fe45, qualification tooling 77a45d18) joins the beta.10 workstreams below
on the beta.9 baseline: exact many-FE Schwarz admission (the ten-million-row
sparse three-FE fix), the shared Jacobi-trial design, RHS-transport and
projected-matrix release, the fused three-FE MAP pilot sweep, the ordered
additive-Schwarz reduction in every mode, the within fused LSMR pin, the
derived residual on the one-FE and many-FE routes, the CMG admission level
cap, the PPML separation re-execution, re-certification and Auto-route
activation, and the wide private Gram bank. The
[round record](../bench/results/beta10-2026-10-28.json) keeps the
identities, declarations, comparisons and failure history.

- **Replay.** 77 of 77 cases pass against the published beta.9 macOS plugin
  with the within fused-LSMR, derived-residual and PPML separation scopes;
  only `ols-derived-fe1` and `ols-derived-fe3` differ outside a scope, in
  their residual row's stop code (2 to 5) and certificate norms, and the
  integration head is their exact replay parent from beta.10 on.
- **Declaration.** The frozen configurations declared scope `two` with a
  `two-gates-v2` hidden arm. The first Mac attempt and the first Linux array
  stopped at their first three-FE cell with the exactness rule, because the
  within fused LSMR pin changes many-FE science in the last bits and the
  derived one-FE residual changes the residual row's stop code on the
  no-output one-FE campaign cell. Both configurations were amended before any
  timing was read (scope `many`; candidate families `two` and `many` with
  `public_policy: two-gates-v2`; hidden `many-geometry-v1`), the tool learned
  to bind a hidden arm only to the family it activates and to validate a
  projection-to-derived residual row ([README](../bench/README.md)), and the
  discarded attempts are listed in the record. With the actual plugins, the
  candidate's published values and observation columns on the no-output
  one-FE cell are bit-identical to beta.8 and beta.9.
- **Linux qualification** ([configuration](../bench/qualify_configs/beta10-linux.json),
  array 8313820, report 8313821): 58 cells, beta.7 original, beta.9 incumbent,
  candidate and hidden arms, 13,920 native calls on 35 distinct physical
  hosts. 173 of 174 protected comparisons are within max(5%, 1 ms); the one
  inconclusive comparison is `small-retained-t4` against beta.7, whose upper
  bound exceeds the allowance by 0.02 ms (median 0.26 ms faster). Twenty
  cells reached 11 of the required 12 distinct hosts (60 comparisons, all
  within the bound), the same one-host shortfall Johannes waived for round
  two; the tool therefore reports `needs_discussion`, and Johannes applied the
  waiver again on 2026-10-28, so the Linux result stands as passed with one
  inconclusive comparison. Against beta.9 the
  median cell ratio is 0.992 and no cell is slower than 1.017; the many-FE
  cells improve most: eight FE 0.77 and 0.84, four FE 0.80 and 0.87, small
  third 0.81 and 0.88, campaign three-sparse 0.82 and 0.84,
  campaign AKM k64 0.86 and 0.88 at one and four workers.
- **Mac practical check** ([configuration](../bench/qualify_configs/beta10-darwin.json),
  beta.8 original, 3,016 calls): 162 of 174 comparisons pass, 11 are
  inconclusive on small or noisy cells (`slopes-t4`, `campaign-two-dense-t4`,
  `disconnected-t4`, `weak-t1/t4`, `sixteen-fe-t1` against beta.9, where the
  medians are within noise or negative) and one is a regression:
  `sixteen-fe-t1` is 6% slower than beta.8 (0.413 s against 0.389 s in every
  block; 0.390 s for beta.9). The same cell is 6% faster than beta.9 on Linux.
  A development attribution with arm64 release builds of the integration
  head selecting within's `StepKernel::Separate` instead of the default fused
  step reproduces beta.9's time (0.389 s) and output bits on that cell, while
  the fused kernel is also the source of the one-worker gains on three, four
  and eight effects (small third 0.110 against 0.120 s, four FE 0.110 against
  0.121 s, eight FE 0.181 against 0.211 s). The fused step therefore costs
  about 6% at sixteen effects and one worker on the M2 Ultra and nothing at
  four workers; no platform-specific kernel fence was added. Johannes
  accepted this one-cell Mac exception on 2026-10-28; a within follow-up for
  many small unsorted intercept terms on arm64 remains open.
- **Veneto panels** (candidate against the beta.9 plugin on one whole 28-core
  node, three rotated repetitions): with saved effects the candidate is within
  2% of beta.9 at every core count (short 0.98-1.00, long 0.99-1.00) with
  exact fixed-worker repeats and cross-worker agreement. Without saved
  effects the first job's fereg arms agree within 1% while its PyFixest arm
  failed the pinned-source check (main now pins the beta.8 campaign's
  PyFixest commit, the reused beta.7 environment carried the older one); the
  rerun against the beta.8 Veneto environment (job 8315525, scc-xf4) has the
  candidate within 4% of beta.9 everywhere (short 0.96-1.01, long 0.98-1.00)
  and faster than PyFixest `within` in 9 of 10 cells (short 0.77-0.92 at 1, 4,
  8 and 16 cores, 1.00 at 2 cores; long 0.36-0.51), with exact fixed-worker
  repeats and cross-worker agreement. Both first jobs ran on scc-uf3, where
  every phase of every arm was about 2.5 times slower than on the other
  E5-2680 v4 nodes with no competing process observed; their within-job
  ratios stand, their absolute levels do not compare with round two.
- **PPML timing** (campaign-DGP PPML cases from `bench/ppml_cases.py`: two-sparse,
  AKM baseline and two-dense at 1M and 4M rows, 16 regressors, robust VCE;
  beta.9 against the candidate, five rotated repetitions, one whole node, job
  8313485 on scc-uf3): all 12 cells pass the max(5%, 1 ms) bound with paired
  median ratios 0.985-1.005 at 4 and 28 workers, and every published result
  is bit-identical to beta.9 (the beta.10 PPML changes act only on separation
  detection, which these designs do not reach). The node ran about 2.4 times
  slower than the round-two node for both arms, so only the ratios compare.
- **Sparse three-FE size series** (the campaign design: 16 controls, iid VCE,
  `threads(4)`, Auto/Auto; one whole 28-core node, scc-wo4, jobs 8316297 and
  8318321; three repetitions per cell in a fresh Stata process, each fit
  bounded at 30 minutes; 100k-7M rows regenerated with the campaign generator
  and the campaign's ten-million-row input reused read-only). The candidate
  takes 0.73, 1.87, 5.02 and 15.7 s at 100k, 300k, 1M and 3M rows against
  beta.9's 0.84, 1.98, 5.49 and 16.9 s (7-13% faster, 10-20% less resident
  memory, same LSMR route and iteration counts, coefficients equal to twelve
  digits with last-bit differences under the within fused-LSMR scope), and
  28.0, 39.4 and 58.9 s with 6.8, 9.2 and 12.6 GiB at 5M, 7M and 10M rows
  (23-24 LSMR iterations). beta.9 exhausted the 30-minute bound at 5M and 7M
  rows on this node, and Johannes skipped its ten-million-row attempt, which
  the beta.8 campaign had already shown not to finish. The campaign's recorded
  timings stand; this series is the candidate's.

## Beta.10 exact engineering

Two arithmetic-preserving changes from the beta.8 campaign analysis were
measured first and kept; the [record](../bench/results/beta10-exact-2026-10-28.json)
holds every number below. Trace-only `FEREG_PHASE_TIMING` laps split the MAP
wrapper, the finalize score certificate, the dense projection's RHS/moment
construction, the PPML one-FE profile and the loader pipeline. Each item was
kept only if its phase reached 5% of native time on a 1M-row campaign design
at 1 or 4 workers, or (for the 64-column cluster) if the control series jumped.

- The borrowed MAP wrapper and the bounded three-FE MAP pilot apply the exact
  one-group initial rejection screen before the complete initial certificate,
  and entry finite scans run as a parallel predicate (MAP wrapper overhead was
  16-18% of one-FE native time).
- Wide raw-moment caches use the widest 64/32/16/8/1-column band that keeps
  every worker busy, each column in its serial row order, and designs wider
  than the 64-column stack binding fill the `[y, X]` batch through a heap
  descriptor charged by the fast memory estimate. At 63 to 64 controls the
  native time had jumped by 0.36 s (one worker) for one control; the jump is gone.

Native seconds on an M-series Mac (medians, step 0 against final): one FE
0.417/0.178 to 0.382/0.161 at 1/4 workers, dense three-FE 1.132/0.417 to
1.047/0.410, AKM with 64 controls 2.633/1.072 to 1.947/0.977, with 96 controls
5.390/1.925 to 3.477/1.561; PPML one FE unchanged within noise. The score
certificate (at most 2.6%), converged two-FE lanes (under 0.4%), CMG
application (at most 2.1%), PPML one-FE serial passes (at most 3.7%) and
loader packing (fully overlapped with the SPI read) fell below the rule and
were not changed. A follow-up removed the last cliff: Gram dimensions wider
than the 64-column stack block accumulate in a private heap bank with the same
partition of Gram rows and the same per-row rank-1 pass, charged by the fast
memory estimate. In paired sessions against the previous head, the finite
solve at 65 controls fell from 0.780 to 0.519 s (one worker) and 0.327 to
0.247 s (four), at 96 controls from 1.300 to 0.988 s and 0.543 to 0.459 s;
native time at 65 controls fell from 2.234 to 1.975 s and 1.108 to 1.011 s,
with identical coefficients and residual sums.

Plugin replay is 74/74 exact against beta.9 after each change, including the new compact
`ols-wide-k67` case (67 regressors), and the universal release build is exact
at 2/4/8 replay threads; Stata `e(b)`, `e(V)` and posted scalars are
bit-identical to beta.9 on all eight 1M-row designs at 1 and 4 workers. The
42-cell Mac protocol (beta.9 as original and incumbent, the hidden
`two-gates-v2` arm, 1,638 native calls) found no scientific, agreement or
resource error; 102 of 126 comparisons passed and 24 were inconclusive with
negative median excess and upper bounds of at most 1.9 ms, so the status is
`needs_discussion`, as in round two.
## Beta.10 round two: dense many-FE route, residency and repeatability

Descriptive Mac evidence on the 1M-row campaign designs; every number is in
the [record](../bench/results/beta10-round2-2026-10-28.json).

- **Fused pilot sweep (kept, exact).** On dense three-FE the bounded MAP pilot
  was 70% of native time at one worker (two symmetric sweeps 0.57 s of
  1.02 s), and group statistics outweighed the subtraction pass about 2.5 to 1
  at four workers. The pilot now raises the next effect's column scales while
  it subtracts the current effect's means. Native time fell from 1.036 to
  0.974 s at one worker and 0.404 to 0.376 s at four, with identical
  `e(b)`/`e(V)`; sparse three-FE (no pilot) is unchanged. Unit tests compare
  fused and unfused sweeps and whole pilot attempts bit for bit at 1/2/4/8
  workers, and replay was 74/74 exact against beta.9.
- **Jacobi trial instead of the pilot (refuted).** Without the pilot, dense
  three-FE certifies after four Jacobi LSMR iterations per RHS but takes
  1.325/0.508/0.325 s against 0.959/0.377/0.260 s at 1/4/8 workers and peaks
  about 450 MB higher. Sparse three-FE has no predicted sweep count, so it
  already starts the trial. No pass-cost rule improves on the geometry policy
  on these cells; none was added.
- **Fast residency (design only).** The Fast increment over the loaded Stata
  process is about 425-459 B/row on AKM with 16 controls, 529-538 on dense
  three-FE and 1,750-2,250 on sparse three-FE. After projection the
  column-major prepared sources are read by the independent raw residual
  `y - Xb`, which gives the reported intercept, and by requested `xb`/residual
  outputs. Removing them needs a different intercept arithmetic (losing the
  witness) or a second SPI read after the solve (a memory and transaction
  design change), so nothing changed. An exact option remains for dense
  three-FE: the pilot keeps the untouched row-major original beside its
  candidate only for the LSMR fallback and could rebuild it from the sources
  instead (about 144 B/row).
- **Implicit level index (skipped).** 149 member-slice call sites and 28
  kernel parameters, including installed function-pointer types, take
  `&[usize]`; a u32 or implicit identity index cannot be introduced
  bit-identically in one bounded pass.
- **Ordered additive reduction (kept, declared scope in SPEC §18).** Every
  additive-Schwarz build now uses the Ordered reduction in default mode too.
  An escalating default-mode fit repeats bit for bit at 1/2/4/8 workers. On
  sparse three-FE the public trial does not escalate at 1M rows, and
  explicit `solver(lsmr) preconditioner(schwarz)` gave identical bits under
  both reductions with native times within -2.3% to +0.7% at 1/4/8 workers.
  Replay against the Auto-reduction build of the same tree is 74/74 exact;
  against beta.9 the only differences are the within fused-LSMR scope (14
  cases, passing under its scope file) and the separation activation's
  `ppml-separation-ir` certificate digest.

## Derived residual on the one-FE and many-FE routes

Fast OLS Auto without residual, xbd, d or saved-FE outputs now certifies its
within residual by the derived bound (SPEC §7, §16) on the direct one-FE route
and the many-FE geometry policy as well as the two-FE owner; see the
[compact record](../bench/results/derived-residual-routes-2026-10-28.json).
It is not yet qualified with `bench/qualify.py`; that is the integration round's
task.

- **Independent witnesses.** Dense explicit-dummy OLS (one FE; three FE through
  the balanced MAP pilot and dense/sparse LSMR; unit and analytic weights)
  matches the derived residual at the prediction tolerance, and explicit level
  moments meet 1e-10 at the derived scale. On 60,000-row designs fixed-worker
  repeats are bit-identical and 1/2/4/8 workers agree with each other and
  with the polished projection. Unit tests cover the forced certificate failure
  (bit-identical coefficients), saved FE, requested outputs, low memory and
  explicit solvers.
- **Replay.** Against the published beta.9 plugin, 73/73 cases pass with zero
  exact differences. Every OLS replay case requests residual, xbd and absorbed
  outputs, so the corpus does not exercise the derived path on any route; the
  [scope](../tests/replay/scopes/2026-10-28-derived-residual-routes-macos.json)
  therefore lists no cases. Three replay cases now request only the sample
  and drop reasons in Fast memory (`ols-derived-fe1`, `-fe2`, `-fe3`: one FE
  iid, two FE analytic-weight robust, three FE robust); their science check
  requires the residual row's stop code 5 with the absorption's solver, zero
  iterations and zero pilot work, beside the dense explicit-dummy
  coefficients and covariance. On the integration head all three take the
  derived path and pass with 4- and 8-worker repeats and cross-worker
  agreement (replay against the same build, 77/77). Against beta.9,
  `ols-derived-fe2` is exact (beta.9 already derived the two-FE owner's
  residual), while beta.9 projects the one- and three-FE residuals (stop
  code 2), so those two cases differ in the residual row's stop code and
  certificate norms (and, for three FE, the within fused LSMR leaves). A
  stop-code change is a route signature that no affected scope may declare,
  so these cases take the current build as their parent from beta.10 on.
- **Mac Stata end-to-end.** Coefficients are bit-identical to beta.9 in every
  cell below. One FE and dense three FE are bit-identical apart from the
  residual solver row, because beta.9 already accepted their residual at the
  initial certificate. Sparse three FE changes r2, rmse, rss, F and the iid
  covariance in the last bits through the residual sum of squares; this
  corrects the earlier claim that iid covariance is unaffected. Used solver,
  preconditioners, fallback and pilot fields of the residual row are unchanged.

Arm64 profiling builds, Stata 19 MP, 1,000,000 rows and 16 controls, medians of
three interleaved runs under the frozen quiet-host rule (three contaminated
beta.9 runs were discarded and repeated once):

| Design, workers | beta.9 native s | Candidate native s | Residual phase ms, beta.9 -> candidate |
| --- | ---: | ---: | ---: |
| one-fe, 1 | 0.422 | 0.414 | 19.4 -> 19.8 |
| one-fe, 4 | 0.182 | 0.185 | 13.6 -> 13.2 |
| three-dense, 1 | 1.161 | 1.149 | 23.1 -> 22.3 |
| three-dense, 4 | 0.437 | 0.441 | 12.3 -> 12.3 |
| three-sparse, 1 | 6.526 | 6.418 | 88.2 -> 26.6 |
| three-sparse, 4 | 1.928 | 1.945 | 44.7 -> 15.9 |

Only the sparse LSMR route saves its second solve, about 1-1.5% of native time
and within the run-to-run spread of the totals; on one FE and the MAP pilot the
derived certificate costs the same as the certificate it replaces.

## Many-FE geometry policy

Public OLS Auto/Auto for at least three categorical-intercept FEs uses the
geometry policy specified in [SPEC](SPEC.md#qualified-auto-routing-and-developer-comparisons).
Johannes accepted it on 2026-10-25 with one documented exception, below. The
`many-geometry-v1` arm and `routing_many_geometry_v1` token select the same
policy for comparisons; hidden `routing_incremental_many` keeps the previous one.

Linux evidence used the candidate plugin built at `99f0134d` with the release
toolchain and the beta.5 plugin (`1ac7125b`) as original and incumbent. Each of
12 replicas ran on its own whole generic 28-core node, all arms in the same
allocation; accounting recorded `failed=0` and exit 0 for every replica. Ratios
are candidate/beta.5 median native-call times; bounds are one-sided .025
bounds on the excess over max(5%, 1 ms) across physical hosts.

| Cell, 1 / 4 workers | Qualification | Public activation |
| --- | ---: | ---: |
| Four FEs | 0.68 / 0.64 | 0.68 / 0.64 |
| Eight FEs | 0.60 / 0.61 | 0.60 / 0.62 |
| Sixteen FEs | 0.62 / 0.67 | 0.61 / 0.67 |
| Small third FE | 0.65 / 0.63 | 0.65 / 0.61 |
| Near-confounded third FE | 1.125 / 1.066 | 1.122 / 1.063 |
| All other cells | 1.00-1.05 | 0.98-1.02 |

Qualification (5,040 native calls) passed 80 of 84 protected comparisons.
Public activation (7,560 calls) compared public Auto with the selected hidden
arm and passed 122 of 126, with exact scientific agreement and solver
signatures between public and hidden calls, fixed-worker repeatability and
cross-worker agreement. In both runs the misses were the near-confounded third
FE against both references: a one-worker regression (excess +16.1 and +15.4 ms,
lower bounds +10.7 and +11.2 ms) and an inconclusive four-worker result
(+1.9 and +1.8 ms, bounds spanning zero). This design needs the additive
preconditioner from the start; the Jacobi trial spends a few iterations before
escalating and finishes in 19 rather than 17 LSMR iterations. Johannes accepted
this cell as a documented exception rather than requiring a successor. A
`within` change sharing one prepared design between the trial and additive
solvers would remove roughly half of that overhead.

Two harness and wiring amendments, approved by Johannes before measurement,
made the contracts match the policy: the scientific harness checks the geometry
arm at its requested certificate and admits its MAP pilot, rather than the
unseeded portfolio's private hundredth; and hidden `routing_many_geometry_v1`
keeps public two-FE ownership/deferred-cell activation so public/hidden
agreement is exact on every design. Operational failures are recorded in the
decision record: an earlier run blocked on the old certificate contract before
any timing, one bench unit test lost a timer race and passed on its single
retry, and two activation replicas failed to open the shared placement lock
before any native call and were resubmitted.

Before qualification, `./scripts/run_tests.sh --source-only` passed (268 Python tests, Rust unit and
independent numerical integration tests, Clippy, FFI allocation audits,
portable checks, mock SPI and the complexity ratchet). New witnesses compare the
public policy with dense explicit-dummy QR on a mixing panel that finishes on
the Jacobi trial and a weak AKM panel that escalates, with exact fixed-worker
repeats and exact public/hidden agreement. The plugin replay against main's
plugin was exact for 72 of 73 cases. In `ols-30`, main rejected a three-FE
LSMR solve at the private hundredth of the request and fell back to 28 MAP
sweeps with a warning; the candidate accepts a 12-iteration solve at the
request. Its 502 numerical leaves stay within the
[declared scope](../tests/replay/scopes/2026-10-25-many-fe-geometry-linux.json)
and science passes. The used-solver, fallback and warning fields are the
intended route change. Four-worker repeats and cross-worker science passed for
every case. The suite and replay were rerun after the recurrence margin below
and after merging main.

On Apple Silicon, the source-only suite passed at `7b641ceb` (319 Python tests).
The candidate's universal Mac plugin passed its binary audit and actual
ABI/load/panic checks. Its replay against main's Mac plugin was exact for 72 of
73 cases. `ols-30` takes the same route change as on Linux, with 510 numerical
leaves inside a separate
[macOS scope](../tests/replay/scopes/2026-10-25-many-fe-geometry-macos.json),
because the macOS replay input has its own hash. Science, four-worker repeats
and cross-worker checks passed for every case. Fourteen focused Stata 19/MP
fixtures with three or more absorbed effects passed, including dense-dummy,
reghdfe parity, saved-output, memory-mode and LSMR-reuse checks, and so did
public OLS Break/rollback. Two development fixtures had asserted main's route
and were updated without changing any numerical threshold:
`ols_three_fe_map_oracle.do` now expects the geometry pilot to certify the
final-residual projection with MAP, and `ols_mixed_solver_reports.do` expects
the accepted LSMR solve where main fell back to MAP. Main's plugin still passes
both original fixtures; the candidate agrees with the dense references within
4e-14 in both.

Local diagnostic timing on a shared cloud VM, four workers, alternating
candidate/main replays, medians of five runs (ten for the last two rows),
`NATIVE_SECONDS`. These are descriptive engineering checks, not qualification
evidence; repeated runs varied by several percent.

| Design | Rows | Candidate/main |
| --- | ---: | ---: |
| Three dense FEs, unit weights | 100,000 / 1,000,000 / 4,000,000 | 0.55 / 0.87 / 0.61 |
| Three dense FEs, analytic weights | 100,000 / 1,000,000 / 4,000,000 | 0.96 / 1.01 / 0.83 |
| AKM-like three sparse FEs | 1,000,000 | 0.38 |
| Mixing designs with 3, 4, 8 and 16 FEs | 100,000 | 0.52-0.62 |
| Near-confounded third FE | 100,000 | 1.02 |
| Campaign-shaped three sparse FEs | 1,000,000 | 1.04 (1.03-1.10 across runs) |

Stopping the recurrence a decade below the request instead of a hundredth cut
one to three LSMR iterations from every LSMR-routed solve above (for example
17 to 15 on the AKM-like design and 26 to 23 on the campaign-shaped one). No
case needed the correction. Against the candidate without this margin, ten
alternating runs on a slower replacement VM gave 0.95 on both one-million-row
sparse designs; the 100,000-row ratios (0.97-1.06) were within that host's
noise.

The last two rows need the additive preconditioner from the start; the Linux
near-confounded result above is the corresponding accepted exception. Weighted
dense designs do not reach the unit-weight pilot.

A one-pass shifted projection route from the research plan failed two existing
witnesses (CG energy roundoff and tiled-bank cross-thread identity), so MAP
projections keep the scaled two-pass route. A bit-identical cache-blocked
row-streaming kernel for dense group statistics and certificate norms was
faster on the development Mac but slower on Linux at 4, 8 and 28 workers.
Complete four-million-row dense three-FE fits took 5% longer at four workers
and 21% longer at 28, so it was not adopted
([record](../bench/results/r4-row-streaming-20261026.json)).

## Many-FE Schwarz admission at scale

The beta.8 campaign's sparse three-FE OLS fit did not finish at ten million
rows. Traced SCC reruns of main (`a023073e`) showed why: at 1M and 3M the
geometry policy's Jacobi trial certified in 23 LSMR iterations, but at 10M the
general categorical route refused the additive LSMR attempt and ran the
original-input MAP fallback (a perf attach saw only borrowed-MAP kernels, 5.0
GiB RSS at about 3.6 cores). The refusal came from the additive-Schwarz memory
bound, which charged `T + R` level-sized banks per concurrent RHS with
`R = levels * (effects - 1) / 10,002`, quadratic in the levels: 11.9 GB at 1M,
95 GB at 3M and about 1 TB at 10M for 17 RHS and four workers, against measured
peaks of 1.96 and 5.91 GiB. Exactly three pair components of this design have a
block of at least 10,000 levels at every size. The repair counts them when the
level bound refuses ([SPEC](SPEC.md#qualified-auto-routing-and-developer-comparisons));
the 10M solver estimate becomes 32.4 GB and the fit takes
the unchanged geometry route (24 iterations, no escalation).

On one whole E5-2680 v4 node (three repetitions, baseline and candidate
alternating up to 3M), median native seconds were 0.38, 1.33, 4.51, 14.47,
25.9, 36.8 and 65.0 at 100k, 300k, 1M, 3M, 5M, 7M and 10M rows, with peak
process RSS 0.23 to 17.0 GiB. Baseline and candidate were bitwise identical
through 3M, as was the candidate with the campaign's beta.8 3M capture. At 10M
the coefficients agree with FixedEffectModels.jl at 1e-10 tolerance and with
PyFixest to 2e-13 relative, and the covariance to 1.5e-13 of the
standard-error scale after the absorbed-DoF convention. The campaign's 10M
references were 55 s (PyFixest), 105 s (FixedEffectModels.jl) and 338 s
(fixest); their peak memory, 4.4 to 12.6 GiB, stays below fereg's. Deterministic
mode repeated bit for bit at 10M; default mode differed in the last bits there
because within's non-reproducible adjoint uses an atomic scatter for terms of at
least 100,000 levels (the 100,000-level firm effect), which no earlier size
of this design reached.

Local checks: `./scripts/run_tests.sh --source-only` passed on the M2 Ultra; the
candidate arm64 plugin's replay against the published beta.9 macOS plugin was
exact for 73 of 73 cases; the many-FE corpus cells' level bounds are at most
1.9 GB, so none reaches the refinement. A one-million-row campaign-topology
test admits the additive route under a budget one byte below the level bound
with the unbudgeted fit's bits; with the refinement disabled it stayed in the
MAP fallback past 600 seconds. Record:
[three-fe-scale-2026-10-28](../bench/results/three-fe-scale-2026-10-28.json).

## Many-FE memory, shared trial design and candidate session windows

Branch `codex/many-fe-windows` (from the sparse three-FE scale fix) adds four
changes for beta.10; the record is
[many-fe-windows-2026-10-28](../bench/results/many-fe-windows-2026-10-28.json).

1. The exact yielding-component admission now also covers explicit
   `solver(lsmr)` (borrowed and owned designs), the automatic owned-design
   LSMR attempt and the explicit PPML LSMR preflight, only after the
   level-count bound refused ([SPEC](SPEC.md#qualified-auto-routing-and-developer-comparisons)).
   Witnesses run each route on the sparse three-FE topology under a budget one
   byte below the level bound: the fits are admitted with the unbudgeted
   fit's bits (PPML: agreement within 2e-9 and exact repeats), at one and four
   workers, while a budget below the exact bound and a sloped design are still
   refused.
2. The Jacobi trial and its additive escalation share one prepared pinned
   design (bit-identical; a digest of escalating fits matches the parent
   revision). With the pinned `within` only the design is shareable: it costs
   0.1-0.3 ms on sorted designs and 5-7 ms at one million locality-sorted
   rows, against 45-56 ms for the additive build; the larger saving this
   section's predecessor anticipated needs a `within` change (shared square
   root weights, stable adjoint and solver frame).
3. The many-FE windows transport and release their own RHS columns, and OLS
   Fast releases its projected matrix after the finite solve. Admission
   totals are unchanged, so `memory(auto)` chooses as before. Campaign data,
   one million rows, 16 controls, four workers, Mac process peak (including
   Stata and 0.21 GB of data): three-sparse 1.876 to 1.778 GB, three-dense
   0.726 to 0.724 GB; bits unchanged. The implicit identity `LevelIndex` and
   u32 observation indexes were not made: `members()` returns `&[usize]` to
   about 200 call sites across MAP, OLS, PPML and slope code.
4. Hidden `routing_many_windows_v1` is a candidate policy, not a public
   default: at most one live LSMR session per worker, with the Jacobi
   escalation barrier at each window's end. Without an escalation it is
   bit-identical to public Auto; after one, later windows start on the
   additive solver. Dense explicit-dummy QR witnesses pass at 1, 2, 4 and 8
   workers with exact fixed-worker repeats. Of the routing families,
   small-third, four-, eight- and sixteen-FE are bit-identical to public at
   one and four workers; near-confounded-third is identical at four and at
   one worker takes 18 instead of 19 projection iterations (coefficients
   within 2.7e-14, covariance within 2.2e-11).

Mac timing (M2 Ultra, arm64 profiling build of `acdfb097`, Stata 19 MP batch,
fresh process per fit, three alternating repetitions, native seconds median,
peak RSS GB). Rule frozen before collection: wait until no other Stata,
cargo, rustc or Python process runs; a fit during which one appears is
discarded and repeated once (one public fit was repeated).

| Design | Workers | Public s | Hidden s | Public GB | Hidden GB |
| --- | ---: | ---: | ---: | ---: | ---: |
| three-sparse | 1 | 6.64 | 5.81 | 1.77 | 1.05 |
| three-sparse | 4 | 1.94 | 1.79 | 1.79 | 1.37 |
| three-sparse | 8 | 1.36 | 1.17 | 2.00 | 1.79 |
| three-dense | 1 | 1.17 | 1.21 | 0.72 | 0.72 |
| three-dense | 4 | 0.45 | 0.45 | 0.73 | 0.73 |
| three-dense | 8 | 0.30 | 0.30 | 0.73 | 0.73 |

The sparse design's outcome RHS escalates; with windows the later RHSs start
on the additive solver (19 instead of 22 iterations), and results agree with
public within 2.3e-14 of the coefficient scale. The dense design certifies in
the MAP pilot, so both arms run identical code and results; its differences
are noise.

SCC confirmation at ten million rows (campaign three-sparse input, threads(4),
one whole E5-2680 v4 node, two repetitions per arm in a fixed alternating
order, every fit reported): base is the branch parent `cf5ddfc0`, candidate
`acdfb097` uninjected, hidden `acdfb097` with the windows token.

| Arm | Native s (rep 1, 2) | Peak RSS GiB (rep 1, 2) |
| --- | --- | --- |
| Base public | 73.6, 72.4 | 16.89, 17.03 |
| Candidate public | 75.0, 74.3 | 15.63, 15.48 |
| Candidate hidden | 68.8, 69.1 | 9.65, 9.64 |

Public coefficients agree with the base to 1e-17 of their scale (default mode
repeats differ in the last bits, as in the scale record); the hidden arm
agrees with public within 1.1e-14 (coefficients) and 1.1e-13 (variance, RSS).
Its later windows finish in 19-20 instead of 22-23 iterations. The public
candidate's 2% longer native time over two repetitions is not resolved by
this confirmation; its peak is 1.3 GiB lower.

Local checks: `./scripts/run_tests.sh --source-only` passed (319 Python
tests, 1500 core library tests, plugin and oracle tests, Clippy, allocator
audits, portable checks, complexity ratchet). The candidate arm64 plugin's
replay against the published beta.9 macOS plugin was exact for 73 of 73 cases.
Public activation of the windows policy needs the bounded qualification and
Johannes's decision.
## Pinned within fused LSMR step

Branch `codex/within-fused-lsmr` pins `within` `3b5b9b9a` of
`codex/fused-lsmr-pass` in three reviewed steps
([record](../bench/results/within-fused-lsmr-2026-10-28.json)).

- `17fcf288` shares a session's response with its unit-weight RHS and an
  unseeded recurrence's RHS. A unit-weight seeded session holds one fewer
  observation-length vector and an unseeded one two fewer. Storage only: the
  macOS replay against the published beta.9 plugin was exact for 73 of 73 cases.
- `2ba7239d` runs each modified-LSMR step as one pass over observations and
  sums every design adjoint in fixed row lanes, with no atomics. `A v`, the `u`
  update and its norm are bitwise unchanged; the declared scope is the adjoint's
  low bits. Default-mode `within` adjoints are now repeatable and identical
  across worker counts; before, the atomic scatter for terms with at least
  100,000 levels made repeated ten-million-row fits differ in their last bits.
  Whole solves are repeatable and worker-count independent with Off, Diagonal
  and ordered additive preconditioners. Without `deterministic`, fereg's
  additive Schwarz still uses `within`'s Auto reduction, which may select an
  atomic scatter; on the campaign sparse design below, fused fits nevertheless
  matched bitwise at 1, 4 and 8 workers while beta.9's did not. Fourteen three-FE
  replay inputs change within the
  [declared scope](../tests/replay/scopes/2026-10-28-within-fused-lsmr-macos.json)
  (467 leaves); solver codes, used solvers, fallbacks, stop reasons, iteration
  counters and discrete decisions are unchanged, and no one- or two-FE case
  changes. Scoped replay passed all 73 cases with four- and eight-worker
  repeats and cross-worker science.
- `3b5b9b9a` adds `Solver::with_preconditioner`, which swaps a solver's
  preconditioner on its prepared frame and is bitwise a fresh construction.
  fereg does not use it yet; replay against the `2ba7239d` plugin was exact.

`./scripts/run_tests.sh --source-only` passed at both runtime pins, including
the many-FE witnesses: public geometry trial against dense QR with and without
escalation, the portfolio against strict QR with discrete decisions across
workers, near-confounded escalation, and explicit three-FE LSMR against
explicit dummies. The complexity ratchet is unchanged. Inside `within`, the
separate kernel matches 24 session digests recorded at the previous pin, and
the fused kernel repeats at 1, 2, 4 and 8 workers.

Descriptive Mac timing (Apple M2 Ultra, shared with other agents; frozen
competitor rule, medians of six clean fits): native seconds for 1M rows and 16
controls, fused pin against beta.9.

| Design | 1 worker | 4 workers | 8 workers |
| --- | ---: | ---: | ---: |
| Campaign three sparse FEs | 0.79 | 0.84 | 0.90 |
| Campaign three dense FEs (MAP, no LSMR) | 1.00 | 0.99 | 0.98 |

The `within` step kernel alone, on the same sparse topology with 16
right-hand sides and 23 steps, ran at 0.65-0.79 of the separate kernel with
the diagonal preconditioner and 0.78-0.93 with additive Schwarz. On that
design the lane merge costs 1.93 MB of buffers per live session and about 11%
of a single-RHS step at eight workers (merge plus zeroing). Peak RSS for the
sparse fit at eight workers fell from 2.12 to 2.00 GB. fereg's LSMR admission
is unchanged and remains an upper bound. These timings are not qualification
evidence; `bench/qualify.py` has not been run.

## PPML two-FE CMG reuse and lean protocol

PPML Auto with exactly two intercept FEs reuses one CMG hierarchy across IRLS
frames and canonical products, and publication fits enter the strict
checkpoint once the predictor settles, as specified in
[SPEC](SPEC.md#qualified-auto-routing-and-developer-comparisons) and §13.
Johannes accepted the result; it ships in v0.5.0-beta.7. The
[lean record](../bench/results/ppml-two-fe-lean-20261026.json) retains every
timing of the final qualification and the
[CMG record](../bench/results/ppml-two-fe-cmg-20261026.json) the earlier
CMG-reuse round.

On one whole node (`scc-wk1`, Xeon E5-2680 v4), campaign-generator PPML inputs
with 16 regressors ran five rounds per cell with rotated arm order: shipped
beta.6, the CMG-reuse head `7db43e41` and the lean head `277c5d11`. Times are
medians; ratios are medians of paired rounds.

| Design | Rows | Workers | beta.6 s | `7db43e41` s | Lean s | Lean/beta.6 | Lean/`7db43e41` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Two sparse FEs | 1,000,000 | 4 | 17.75 | 11.05 | 5.82 | 0.324 | 0.528 |
| Two sparse FEs | 1,000,000 | 28 | 10.86 | 7.56 | 4.13 | 0.380 | 0.543 |
| Two sparse FEs | 4,000,000 | 4 | 81.30 | 45.28 | 26.37 | 0.327 | 0.582 |
| Two sparse FEs | 4,000,000 | 28 | 48.36 | 32.92 | 18.04 | 0.374 | 0.549 |
| AKM | 1,000,000 | 4 | 5.07 | 5.02 | 4.20 | 0.833 | 0.850 |
| AKM | 1,000,000 | 28 | 3.36 | 3.34 | 2.76 | 0.820 | 0.826 |
| AKM | 4,000,000 | 4 | 22.94 | 22.65 | 18.66 | 0.811 | 0.828 |
| AKM | 4,000,000 | 28 | 14.71 | 14.55 | 12.29 | 0.835 | 0.845 |
| Two dense FEs | 1,000,000 | 4 | 4.70 | 4.54 | 3.88 | 0.825 | 0.852 |
| Two dense FEs | 1,000,000 | 28 | 3.01 | 2.94 | 2.49 | 0.823 | 0.845 |
| Two dense FEs | 4,000,000 | 4 | 20.30 | 19.93 | 14.90 | 0.734 | 0.755 |
| Two dense FEs | 4,000,000 | 28 | 12.42 | 12.17 | 10.15 | 0.819 | 0.831 |

Against beta.6, coefficients and covariance stay within 2e-5 and 7e-10 of the
§20 envelopes (largest on the sparse design, whose published state is one
Newton step earlier), and sample, status, DoF, drop, separation and FE
fields are exact in every cell. Every arm repeats bit for bit across rounds;
4- and 28-worker lean results agree within 1e-8 and 2e-10 of the envelopes
with exact discrete fields. IRLS iteration counts are unchanged except the
1M-row sparse design (8 instead of 9). The sparse design defers its strict
checkpoint once (1M) or twice (4M) and passes the first one. The earlier
CMG-reuse round had left AKM four-worker cells at 1.048-1.062; all cells are
now faster than both baselines.

The 73-case replay corpus passes its independent dense science targets in
every case with no discrete, repeat or thread differences. All OLS cases are
bit-identical to `7db43e41`; seven PPML cases differ by at most 1.2e-14
relative in coefficients and covariance. New witnesses cover the settling
rule, small and shared-CMG implicit components, the implicit assembly choice,
team-independent joint checks and Gram, refreshes and the three-attempt
bound. The full source-only suite passed at `277c5d11`. A parallel CMG
application, a row-tiled certificate Gram and a data-based IRLS start were
measured and rejected (slower, or no fewer iterations).

### PPML two-FE parity work (development, Mac)

PPML exact-timing protocol: campaign-DGP PPML cases are written by
[`bench/ppml_cases.py`](../bench/ppml_cases.py) (maintained generator, policy
seed, 16 regressors, robust VCE, `separation(fe simplex ir)`, tolerance 1e-8,
iterate 10000) and timed with [`bench/time_exact.py`](../bench/time_exact.py),
which guards exact science between the plugins it compares. The beta.7 lean
record's case generator was never committed; this tool reproduces its protocol
for arithmetic-preserving PPML candidates. Result-changing PPML candidates
still need an extension of `bench/qualify.py`, which refuses PPML today.

On an M2 Ultra Mac (development measurement, three repetitions per cell,
medians of repetitions 2-3, arms interleaved, no competing build/test/Stata
process observed), the exact parallel joint sections and the reuse of a
finished canonical probe changed no bit of `e(b)` or `e(V)` in any of 17
cells (campaign-generator AKM, sparse and dense designs at 1M rows with 16
regressors at 1, 4 and 8 workers; synthetic Veneto-shaped Poisson panels of
1.39M and 5.63M rows at 1, 2, 4 and 8 workers), and the 73-case replay was
exact against beta.8. Native time was 0.975-0.986 of beta.8 on the campaign
designs at 4 and 8 workers and 0.994-1.009 on the Veneto panels. Separation
IR is 42-61% of PPML native time on the Veneto panels at 4 and 8 workers;
the joint frames' scalar CMG cycle is 5-8% on the long panel. A team
application of the PPML hierarchy above CMG's 350,000-edge single-RHS
minimum (row-operator plan only) left results bit-identical but changed the
long panel's native time by 0.980-1.013 (1-8 workers) against the same build
without it, short of the 5% acceptance margin; it was not retained.

## CMG admission level cap (development, Mac)

CMG `5c5b5b2b` caps its default hierarchy depth, and with it every pre-build
admission bound, at 32 levels instead of 128 (Johannes's decision; not part of
the beta.9 qualification). On the synthetic AKM panels of 1.39M and 5.63M rows
(`fereg y <controls>, absorb(worker firm)`, 1 and 4 workers) the
[memory record](../bench/results/cmg-admission-level-cap-2026-10-27.json)
shows the admitted serial CMG construction bound falling from 640 MB to
166 MB and from 1,759 MB to 446 MB; the parallel bound and the retained charge
of two-FE OLS Auto fall by the same 474 MB and 1,313 MB. Both hierarchies have
3 levels, and `e(b)`, solver rows, hierarchy traces and every reported peak
(`e(fereg_memory)`, the routing and cell traces) are identical before and after.
Those reported peaks do not include the CMG admission stage: they are the same
when CMG is refused. Under `FEREG_MEMORY_LIMIT_BYTES` the short panel's CMG
install threshold fell by 473-475 MB (predicted 474 MB); at a 1.25 GB limit the
parent refused CMG (Schur PCG, 52 iterations) and the head installed it (17
iterations, the unconstrained result). Replay is exact against the parent's
plugin for all 73 cases without a scope; against beta.8 only the existing
eager-cells scope applies. CMG's full suite and `./scripts/run_tests.sh
--source-only` passed; no installed package was built or checked.
### PPML separation routing (development, Mac; not activated)
### PPML separation routing (development, Mac; activated for beta.10)

The iterated rectifier was 42-66% of PPML native time on the Veneto-shaped
Poisson panels. On both panels the separation chain's IR phase is one graph
proposal followed by a single rectifier projection that certifies no further
separation at iteration 1: a weighted finite-plus-FE projection of 3 (short) or
12 (long) columns with zero-outcome weights near 1e-21, solved by
retained-product CG (60-61 iterations, no preconditioner hierarchy). The
hidden `inject=ppml_separation_auto` arm solves admitted two-intercept detector
projections with the OLS two-FE Auto route instead; the detector's weights,
tolerances, iteration limit and decision rules are unchanged and the public
default is not changed. Measured on an M2 Ultra (three repetitions per cell,
medians of repetitions 2-3, arms interleaved, no competing process; the
[record](../bench/results/ppml-separation-routing-2026-10-27.json) keeps every
cell):

| Panel | Workers | Public native s | Hidden native s | Ratio | Public IR s | Hidden IR s | Ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Short (1.39M) | 1 | 14.03 | 6.25 | 0.446 | 9.02 | 1.24 | 0.137 |
| Short (1.39M) | 4 | 6.43 | 4.29 | 0.666 | 3.23 | 1.04 | 0.321 |
| Short (1.39M) | 8 | 5.01 | 3.68 | 0.735 | 2.11 | 0.79 | 0.374 |
| Long (5.63M) | 1 | 112.08 | 46.17 | 0.412 | 74.09 | 8.70 | 0.117 |
| Long (5.63M) | 4 | 43.48 | 23.91 | 0.550 | 26.50 | 7.10 | 0.268 |
| Long (5.63M) | 8 | 29.64 | 18.92 | 0.638 | 15.60 | 5.00 | 0.320 |

On both panels at 1, 2, 4 and 8 workers, on four campaign-generator
separation designs (dense, AKM strong and weak mobility, probability weights)
and on the replay corpus, `e(b)`, `e(V)`, the separated rows, drop counts and
`e(feppml_separation_sequence)` are identical between the arms, and
`e(feppml_separation)` differs only in the rectifier's interior certificate
magnitude; the separation trace digest in the receipt hashes that magnitude
and therefore also differs on `ppml-separation-ir`. Each arm repeats bit for
bit. The public replay is exact (73 of 73 cases) with the arm compiled in.

The witnesses do not hold on every case. A seeded census
(`crates/fereg-oracle/tests/ppml_separation_routing.rs`: 18 small chain,
sparse-AKM and dense designs, some with induced finite-regressor separation,
three separation configurations, public tolerances) is identical in 32 of 54
cells. Where both arms succeed the separated set never differs, but in 9 cells
the hidden arm certifies the terminal rectifier at iteration 1 where CG needs
2, which changes RSEP and RSEQ iteration counts. In 9 cells retained CG fails
a projection certificate and the public fit errors while the hidden arm
completes; in 2 cells the Auto route exhausts its iteration budget where the
public fit succeeds; 2 cells fail in both arms with different errors. Under
`fe simplex ir` the divergences are 5 iteration-count cells and 3 public
failures. The hidden arm is deterministic at fixed workers and its decisions
agree across 1, 2, 4 and 8 workers wherever it succeeds. In an isolated unit
test, a Break at every host poll inside a rectifier projection returns
`Interrupted` on both arms.

The public failures were a detector defect, now fixed (SPEC §14, the
[record](../bench/results/ppml-separation-routing-2026-10-27.json)'s
`post_fix_census`). Retained CG solves the rectifier projection on the
weighted normal equations, which square the zero-row weight
`tolerance^2 / zeros` (about 1e-18): on six cells the proposed direction left
the model span (independent least-squares span distance 0.14-0.19 on seeds 11
and 17, 2e-8 on seed 5) and failed the deletion certificate, r(430). On three
`fe simplex ir` cells the span check of an exact simplex direction lying in
the FE span failed its finite score certificate, normalized by FE-solver
noise, r(459). A detector step that fails such a certificate now runs once
more with LSMR under the same rules and certificates. All 9 cells complete
and retain exactly the sample of an independent HiGHS linear program
(maximal recession-cone separation alternated with singleton pruning), which
every completing arm of all 54 cells also matches. Every other cell, in both
arms, is unchanged bit for bit (full separation report, `e(b)`, `e(V)` and
error messages). Seed 3 (2 cells) still fails in both arms: its CG direction
has the right rows but span distance 1.8e-10 over the 1e-10 tolerance, and
LSMR ends 15% over its FE certificate. The replay corpus is exact against the
published beta.9 plugin (73 cases). Through Stata, the four campaign
separation designs (4 and 8 workers) and both Veneto-shaped Poisson panels
(1, 2, 4 and 8 workers) give identical `e(b)`, `e(V)`, separation matrices,
drop counts and dropped rows with no re-execution; run through Stata, the
seven fixed census cells that return r(430) or r(459) under beta.9 complete
with the LP sample (seed 3 stays r(430)). Both arms repeat
bit for bit at fixed workers and agree across 1, 2, 4 and 8 workers.

After the fix the census is identical in 35 of 54 cells and differs in 15
iteration-count cells (equal sample and fit), 2 hidden-route failures and the
2 seed-3 cells; under `fe simplex ir` only 5 iteration-count cells differ.
The two hidden failures (seed 9 without simplex) occur in the first rectifier
projection, not through the rectifier's iteration limit: the Auto route's
two-FE PCG accepts the final residual absorption at iteration 0 on its
recurrence proxy, the FE certificate (the same as CG's) rejects it, final
residual absorptions have no PCG correction, and the MAP fail-safe exhausts
its 16,000 iterations; retained CG certifies the same residual in 8
iterations.

The residual was accurate; the certificate's arithmetic was not. The
categorical FE certificate (shared by two-FE PCG, retained CG and MAP) forms
each level mean as the first member's value plus a compensated sum of
weighted differences. When that member is a zero-outcome row with a residual
of order one, the positive rows' residuals (about 1e-18) vanish in their
differences and the mean carries one ulp of the shift, which against the
rectifier's weighted residual norm (9.1e-9) is of the order of the 1e-8
tolerance: exact relative moments at most 2.7e-10, computed 1.06e-8, and MAP
corrections below one ulp leave every bit unchanged for 16,000 sweeps; retained
CG's later iterates certified (8.5e-21). A stop scaled like the certificate
would not remove that floor. Since 2026-10-28 an Auto-routed detector step
that fails a numerical certificate takes no MAP continuation and runs once
more as retained CG under the same budget and rules, then continues exactly
as a retained-CG step. Both seed-9 cells complete with the LP-witness sample
and the seed-3 cells report the retained-CG error in both arms; every other
census arm-cell is bit-identical at 1 and 4 workers (full report with
certificate magnitudes, `e(b)`, `e(V)`, errors). The census is then 39 same
and 15 iteration-count cells.

#### Activation (beta.10 candidate, 2026-10-28)

Johannes authorized activating the Auto route for beta.10 with a declared
scope for the iteration-count class (SPEC §14): the separated set, sample,
`e(b)` and `e(V)` are unchanged on every witness; `RSEP` iteration counts and
certificate magnitudes, `RSEQ` iterations and the receipt's separation trace
digest may change. `inject=ppml_separation_cg` keeps the retained-CG path.
Witnesses, all on the activated arm64 profiling candidate against the
published beta.9 plugin with the same ado (the [record](../bench/results/ppml-separation-routing-2026-10-27.json)'s
`activation`):

- Census (`ppml_separation_routing.rs`, CG arm vs default): all 54 cells
  agree in sample and fit (39 identical, 15 iteration-count cells, no
  failure class); every completing arm retains the LP-witness sample;
  fixed-worker repeats and cross-worker decisions hold at 1, 2, 4 and 8
  workers.
- Stata: the four campaign separation designs (4 and 8 workers) and both
  Poisson panels (1, 2, 4 and 8 workers) give `e(b)`, `e(V)`,
  `e(feppml_separation_sequence)`, drop counts, `e(feppml_fit)` and the
  dropped rows identical to beta.9 at every worker count (1,794, 430, 1,078,
  430, 7,498 and 1,536 dropped rows); `e(feppml_separation)` differs only in
  the rectifier row's interior certificate magnitude and the receipt only in
  the separation trace digest. Each worker count repeats bit for bit; dropped
  rows, drop counts, the separation sequence and the discrete separation
  columns agree across worker counts (coefficients within 2.1e-12 relative).
  No detector re-execution occurred. The CG arm reproduces beta.9 exactly,
  separation matrices and receipt included, on all six designs.
- Replay (`scripts/run_plugin_replay.sh`, 73 cases): 72 exact; only
  `ppml-separation-ir` differs, in `RSEP` zero-based 22 (the rectifier row's
  interior certificate, 2.69e-20 to 2.39e-20) and `RREC` 28-29. The replay
  harness now declares this class (development workflow, plugin replay):
  `RSEP` certificate magnitudes and iteration counts are scope leaves by row
  stride, and the separation trace digest is accepted only beside a declared
  `RSEP` leaf with a separation reason. With the
  [declared scope](../tests/replay/scopes/2026-10-28-ppml-separation-activation-macos.json)
  and the [within fused LSMR scope](../tests/replay/scopes/2026-10-28-within-fused-lsmr-macos.json),
  replay of the integration head (`d64fdc0d`, arm64) against the published
  beta.9 plugin passes 74/74 (15 scoped cases, no science, exact or discrete
  failures outside the declared leaves).
- Break: an isolated unit test cancels at every host poll inside a rectifier
  projection of the activated policy (Auto, no MAP continuation, retained-CG
  re-execution) and of retained CG; every stop returns `Interrupted` and is
  not re-executed.

Timing (frozen rule: before each cell, no other Stata, cargo, rustc or Python process
and no other agent's timing driver for 60 s, a 10-s monitor during the cell, discard and
repeat once on a competitor; one cell, short at 1 worker on the candidate, was discarded
for a rustc process and repeated cleanly). arm64 profiling builds of the activated tree and
of v0.5.0-beta.9, Stata 19/MP on the M2 Ultra, three repetitions per Stata process, medians
of repetitions 2-3, arms alternating first per cell, `FEREG_PHASE_TIMING=1`:

| Panel | Workers | beta.9 native s | Activated native s | Ratio | beta.9 IR s | Activated IR s | Ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Short (1.39M) | 1 | 14.72 | 6.93 | 0.471 | 9.32 | 1.38 | 0.148 |
| Short (1.39M) | 4 | 6.50 | 4.38 | 0.674 | 3.27 | 1.05 | 0.322 |
| Short (1.39M) | 8 | 5.03 | 3.67 | 0.729 | 2.13 | 0.77 | 0.363 |
| Long (5.63M) | 1 | 87.88 | 38.11 | 0.434 | 57.01 | 7.29 | 0.128 |
| Long (5.63M) | 4 | 34.19 | 20.55 | 0.601 | 19.81 | 6.23 | 0.314 |
| Long (5.63M) | 8 | 23.81 | 16.23 | 0.682 | 12.03 | 4.35 | 0.361 |

The rectifier's share of native time falls from 42-65% to 19-30%.

Seed 3 (cells (3,1) and (3,2)) was a documented limitation. With a
60-digit reference projection the rectifier's exact direction flags the 20
LP-witness rows and passes every deletion certificate (span 6e-19). Retained
CG's fitted values are wrong by up to 2.3e-10 on zero-outcome rows only
(4.9e-17 on positive rows; `u - r` formation is exact), which the weighted
FE certificate cannot see at weight 1e-18 but the unit-weight span check
(1e-10) rejects (1.82e-10). LSMR's and Auto's directions are accurate (span
3.3e-17 and 1.7e-12, same 20 rows) but their residuals fail the FE
certificate's first-member-shift floor (computed 1.149e-8 and 4.25e-8, exact
6.3e-23 and 1.2e-18; independent of summation order).

Johannes chose the narrow re-certification (2026-10-28, SPEC §14): when an
Auto detector step's retained-CG and LSMR re-executions also fail, the Auto
step runs once more and its two-FE PCG certificate miss re-certifies the
rejected candidate with each level's mean formed about its largest-weight
member, at the same scales and tolerance
(`prepared_separation/recertify.rs`); the shared FE certificate is unchanged.
Witnesses at the integration head plus this change, against the integration
head (`d64fdc0d`):

- Census (`ppml_separation_routing.rs`): full fingerprints of all 108
  arm-cells (separation report with certificate magnitudes, drop reasons,
  `e(b)` and `e(V)` bits, errors; elapsed seconds excluded) at 1 and 4
  workers are identical except the Auto arm of (3,1) and (3,2), which now
  completes with the LP-witness sample (220 rows) after the last attempt
  instead of the retained-CG error. The retained-CG arm keeps that error, so
  the census is 37 same, 15 ir-iterations and 2 cg-fails; fixed-worker
  repeats and cross-worker decisions hold at 1, 2, 4 and 8 workers.
  Seed 9, whose Auto miss retained CG certifies, is unchanged.
- Unit tests: the re-check removes a constructed first-member floor that the
  shared certificate rejects and matches compensated level sums about zero,
  while a genuine 1e-6 moment fails both; an injected two-FE PCG miss reaches
  the re-check only on a miss, a certified PCG result never consults it and is
  bit-identical; the detector policy arms it only after the Auto, retained-CG
  and LSMR attempts all fail numerically, reports the retained-CG error when
  it fails, propagates Break and allocation failures and never arms it for a
  retained-CG detector.
- Replay: all 77 cases are bit-identical in the exact view between arm64
  builds of the integration head and of this change; against the published
  beta.9 plugin with the two beta.10 scopes, 75 pass and the one- and
  three-FE derived-residual cases differ as described in the derived-residual
  section.

## Package checks

### Beta.12 interface packaging

Beta.12 changes the Stata interface only; runtime `864960f1` keeps beta.11's
arithmetic, routing, iteration budgets and certificates. It lowers the minimum
release to Stata 15, adds replay (so `estimates replay`, `bootstrap:` and
`jackknife:` work), posts `e(fereg_version)`, states the passed accuracy check
in the OLS header, quiets saved-output placeholders and replaces the failed
score-certificate message with one that names the loose `tolerance()`.
The [native record](../bench/results/beta12-native-20261031.json) binds the
three plugins and checks below.

| Platform | Beta.12 checks |
| --- | --- |
| macOS | Rust 1.85.1 universal build offline from the vendored `864960f1` source archive (`b0f6aaf4...`) with an empty Cargo home and path remaps; binary audit and actual ABI/load/panic checks passed. All 77 replay cases are exact against the beta.11 Mac plugin (`dfb50318...`), with the same three repaired reference defects. All 68 fixtures of the full Stata 19/MP profile and both Break rollback fixtures passed; the Stata 15-syntax replay fixture passed on Stata 15.1 and 18. Scratch copies of the 67 earlier full-profile fixtures, with `version 15.0` and Stata 15 matrix syntax in their assertions only, passed 66 of 67 on Stata 15.1; the remaining fixture's own `poisson` oracle rejects negative factor levels in Stata 15, after its `fereg` fits passed. The local Stata 17 license has expired. |
| Linux x86-64 | SCC build job 8370549 and check job 8370550 (both `failed=0`, `exit_status=0`): ELF audit and actual ABI/load/panic checks passed, all 68 fixtures of the full Stata 19/MP profile passed, and the Stata 15-syntax fixture passed on Stata/MP 15.1, 16.1, 17 and 18. |
| Windows x86-64 | Run `win-20261031T095017Z-ec6e9c7d` at `864960f1` passed the Rust 1.85.1/MSVC build, PE archive-path and source-manifest checks, the native loader and the licensed Stata/MP 19 `stata-do` driver, whose ten mapped fixtures now include the Stata 15-syntax replay fixture; the controller's execution-identity, source-hash and shutdown-backstop checks passed and the instance stopped (receipt `4eb0539b...`, plugin `5f97813d...`). |

Estimates are the same across Stata releases: a feature check on Stata 15.1
and 19 agrees to `4e-16` relative difference, and 26 OLS/PPML statistics are
bitwise equal to beta.11.

The [package record](../bench/results/beta12-release-package-20261031.json)
binds all 50 release assets. Packaging source `8bd119ad` differs from the
offline-built `864960f1` archive only in its records and generated metadata
(3,471 identical files). The source ZIP is
`a4717974377419e6bcd7828441176cc08a32ecfb06c8995a9bbe38c811bb034b`; the
binary ZIP is `cf508943de83bc5cb233becde22eb68ace6e4b518817fe0aee5a9100a7e232fd`.
The dependency audit reports no advisories. The exact binary archive passes
all twelve staged-package fixtures (the eleven earlier ones plus the replay
fixture) on Mac and on Linux (job 8370604). Fresh local `net install` sessions
of the root package on Stata 19 and Stata 15.1 verify all 35 installed files,
the selftest and independent OLS/PPML comparisons with `regress` and `poisson`
(relative differences `7e-15` and `7e-11`). The strict default
`./scripts/run_tests.sh` passes with the refreshed plugins.

Beta.12 was published with archives only (binary and source ZIPs, checksums
and Mac build notes). The community `github` helper had failed since beta.11
because it reads the whole release list as one Stata line, truncated at
`c(macrolen)` (645,200 characters on SE/MP, 264,392 on BE); per-file assets had
made that list 906,445 characters. With Johannes's approval the 479 per-file
assets of beta.1 to beta.11 were removed after each was verified byte-identical
to its copy in that release's binary ZIP; the list is now 148,286 characters.
Versioned installations use the release tag, whose `dist/` carries exactly the
released plugins from beta.2 onward. Fresh installations from the beta.12 tag,
from `main` and with `github install johannes-schmieder/fereg` pass on Mac in
Stata 19 and 15.1 (35 of 35 files byte-identical, selftest, OLS/PPML checks);
see the [publication record](../bench/results/beta12-publication-20261031.json)
for these and the Linux installations.

### Beta.11 migration packaging

Beta.11 retains the selected migration runtime `65500be6` and within pin
`8d401d82`, including counts, gather, Fold and sparse back-substitution.
No optimization or scientific tolerance changed during release qualification.
The large companion panel passes all 36 native and eight memory cells on each
of Mac and Linux. The accepted eight-/sixteen-FE regressions and the inconclusive
performance settings above remain part of the release decision.

The [Mac release record](../bench/results/beta11-macos-release-20261030.json)
passes all 77 exact replay cases (693 native calls across one, two, four and
eight workers), all 67 licensed Stata fixtures, native ABI/load/panic checks
and the universal binary audit. Its plugin was built with Rust 1.85.1 offline
from the vendored `6c2231c2` source archive and an empty Cargo home.
The [Windows release record](../bench/results/beta11-windows-release-20261030.json)
passes the MSVC build, numeric and native-loader checks, actual local package
installation and ten mapped Stata fixtures at the same source revision.
Two preceding Windows failures are retained: a diagnostic-only driver localized
the failure to the slope fixture's source-location assertion, and using an
absolute ado path fixed the harness without changing the fixture or runtime.
The qualified Linux plugin retains its original `65500be6` build and unchanged
native, replay, Break/rollback and large-companion evidence.

All production and dependency inputs in packaging source `b58474bc` match the
offline-built source. The archive comparison checks every file: its differences
are documentation, completed validation records and generated metadata only.
The source ZIP is `b8b6f33e3f1ad581658bd6748805a73afd40a2309ca26736551ac5105eadd558`;
the binary ZIP is `5c4e551877efc0f023cf3de81de54cec87334a6f18a707e70daeff9ba60b300e`.
The dependency audit reports no advisories. The strict default
`./scripts/run_tests.sh` passes with all three refreshed native plugins,
including the installed-package binding, complexity, FFI and allocator checks.
The current [native metadata](../dist/fereg-native.json) binds each actual
build commit, plugin checksum and the corresponding-source archive.

The [package qualification record](../bench/results/beta11-release-package-20261030.json)
binds all 50 release assets and the completed checks. The exact binary archive
passes all eleven staged-package fixtures on both Mac and Linux. Fresh local
installations verify all 35 main-package files on Mac and all 34 flat-package
files on Linux, native selftest and independent OLS/PPML examples. Linux job
8362394 on `scc-222` has successful terminal accounting, 15 seconds wall time
and 2.804 GB maximum virtual memory. Its 32-slot request covers the existing
`threads(32)` installation test; no host or processor was selected. The job
waited about 42 minutes before execution. Switching the queued job to the
general `omp` environment and requesting a scheduler reservation changed no
resource quantity, test, input or limit.

Native Intel-host Stata execution remains unqualified; both Mac processor
slices are supplied. Existing source and native releases remain immutable.

The [public verification record](../bench/results/beta11-publication-20261030.json)
confirms all 50 published asset digests and sizes and fresh Stata 19/MP
installations from the versioned beta.11 URL and raw GitHub `main` URL.
All 34/35 installed files match, native selftest passes, and independent OLS
and PPML coefficient checks pass (relative differences `3.17e-14` and
`1.12e-10`). The OLS covariance check also passes; PPML covariance is covered
by the maintained independent-sandwich fixtures.

The actual community `github install` command is a documented limitation:
it downloads all 35 correct files but then returns `r(132)` inside its
release-metadata parser, before the smoke estimates. A standalone query
reproduces that error; the official current helper is identical. A bounded
instrumented offline diagnostic returns a different parser error (`r(133)`)
and does not establish the exact cause. Neither failure is relabeled as a
successful command. Two earlier attempts stopped before installation because
the isolated bootstrap omitted the helper's `PLUS/g` directory; both are
retained, including the first unsuccessful repair command. The tested built-in
`net install` route is now the primary installation instruction. This changes
no runtime, package file, scientific assertion or published asset.

### Beta.10 integration packaging

Beta.10 ships the integration round described in
[Beta.10 integration qualification](#beta10-integration-qualification). The
qualified Linux binary is the `3348fe45` SCC build (job 8313475), Windows was
built at `7674cf06` (run `win-20261028T183336Z-bc5a4e6b`), and universal macOS was built offline from
the corresponding-source archive at `7674cf06`. Runtime production inputs are
unchanged across these revisions (`scripts/check_complexity.py
--compare-production 3348fe45`: 225 runtime modules, 58 artifact inputs). The
[beta.10 package metadata](https://github.com/johannes-schmieder/fereg/blob/v0.5.0-beta.10/dist/fereg-native.json)
records each actual build commit, binary checksum, runtime identity and
corresponding-source hash.

| Platform | Beta.10 checks |
| --- | --- |
| macOS | Rust 1.85.1 universal build from the vendored source archive with an empty Cargo home and offline mode; binary privacy/export audit and actual ABI/load/panic checks passed. Replay of the archive-built plugin (`aa453ba7...`) against the qualified Mac plugin (`3e8f88de...`) is exact on all 77 cases. All 67 fixtures of the full Stata 19/MP profile passed (69 of 69 profile steps) on a fresh universal build at `7674cf06` with the path-remap environment. All 11 staged-package fixtures passed from the exact binary archive (`1a4de469...`) in a fresh Stata 19/MP session. |
| Windows x86-64 | Run `win-20261028T183336Z-bc5a4e6b` at `7674cf06` passed the Rust 1.85.1/MSVC build, the project tests, the PE archive-path and source-manifest checks, the native loader and licensed Stata `stata-do` smoke with its authoritative status file, and the execution-identity, source-hash and shutdown-backstop controller checks (receipt `039f3245...`, source archive `f709dca1...`). |
| Linux x86-64 | The qualified binary from build job 8313475 retains its SHA-256 (`a7964116...`). Job 8318613 passed the ELF audit, actual ABI/load/panic checks and all 67 fixtures of the full Stata 19/MP profile (0 failed); job 8318967 passed all 11 staged-package fixtures from the exact binary archive (`1a4de469...`) in Stata 19/MP. |

The dependency audit found no advisories. The strict default
`./scripts/run_tests.sh` passed with the refreshed package. The source archive
SHA-256 is `2ca4fa49566c462bb24d045ea4b878bba5ab80952c3c410eb7924ae2990e365b`;
the binary archive SHA-256 is `1a4de46983daaea92643dd0adba682007fd2dce6b8410219079e9e1ba94dc9cf`. Native Intel-host Stata testing remains
outstanding; both macOS slices are supplied. One tooling mistake preceded the
accepted runs and changed no artifact: the first archive-build attempt looked
for the source ZIP in a directory while `package_release.py` writes the ZIP at
the given output path.

After publication, fresh Stata 19/MP sessions installed beta.10 from the public
[beta.10 URL](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.10)
and from updated `main` into isolated PLUS directories. All 34 package files
installed from the release matched the staged package bytes, and the 35 files
installed from `main` matched them too apart from `dist/fereg-native.json`, the
root package's own manifest, which bound the runtime to `7674cf06`. Native
selftest passed on both routes, and `fereg`/`feppml` on the `auto` data agreed
with Stata's `regress` and `poisson` dummy regressions to rounding (coefficient
relative differences 2e-16 and 6e-14). GitHub's recorded SHA-256 and size
matched every one of the 50 published assets. A first `main` installation
before the notices refresh (`d894f630`) differed only in
`fereg-THIRD-PARTY-NOTICES.txt`, which main's copy had not carried forward.

### Beta.9 two-FE Auto round-two packaging

Beta.9 ships the round-two changes described above. The qualified Linux binary
is the `d6975ec2` SCC build (job 8305425), Windows was built at `8b2f38a4`
(run `win-20261027T235540Z-45d74d45`), and universal macOS was built offline
from the corresponding-source archive at `8b2f38a4`. Runtime production inputs
are unchanged across these revisions (`scripts/check_complexity.py
--compare-production`). The [package metadata](../dist/fereg-native.json)
records each actual build commit, binary checksum, runtime identity and
corresponding-source hash.

| Platform | Beta.9 checks |
| --- | --- |
| macOS | Rust 1.85.1 universal build from the vendored source archive with an empty Cargo home and offline mode; binary privacy/export audit and actual ABI/load/panic checks passed. All 67 fixtures of the full Stata 19/MP profile, the 73-case replay against the beta.8 plugin under the eager-cells scope, public OLS/PPML Break/rollback and eleven staged-package fixtures passed (the five remaining installation fixtures need the intentionally excluded development adapters or a Windows host). |
| Windows x86-64 | Run `win-20261027T235540Z-45d74d45` at `8b2f38a4` passed the Rust 1.85.1/MSVC static-CRT build, exact-FMA and fused-kernel tests, C numeric guards, PE audit, native loader checks, checksum, Stata smoke and single-runtime stages; the controller stopped the machine, deleted transient objects and released its lock. The collected plugin's SHA-256 matches the receipt. |
| Linux x86-64 | The qualified binary from build job 8305425 retains its SHA-256. Job 8308581 passed the ELF audit (glibc 2.28 maximum), actual ABI/load/panic checks and all 67 fixtures of the full Stata 19/MP profile; job 8308686 passed the same eleven staged-package fixtures from the exact binary archive (job 8308667 stopped before any fixture because the runner needs an absolute Stata executable path). |

The dependency audit found no advisories. The strict default
`./scripts/run_tests.sh` passed with the refreshed package. The source archive
SHA-256 is `7f1ab939328f94c338ceb126cb404185439f0b1bbe0857a508963c9e7547decd`;
the binary archive SHA-256 is `ae409717f30440191e1ce633695d14079714f8e74f02e6d834f2dc2de5ae7fac`. Native Intel-host Stata testing
remains outstanding; both macOS slices are supplied. Two tooling mistakes
preceded the accepted runs and changed no artifact: the first source-archive
attempt used an exported snapshot without Git metadata and the first staging
attempt lacked the plugin checksum files.

After publication, fresh Stata 19/MP sessions installed beta.9 from the public
[beta.9 URL](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.9)
and from updated `main` into isolated PLUS directories. All 35 installed files
matched the staged package bytes on both routes, the manifest bound the runtime
to `8b2f38a4`, native selftest passed, and `fereg`/`feppml` on the `auto` data
agreed with Stata's `regress` and `poisson` dummy regressions to rounding
(coefficient relative differences below 1e-13). GitHub's recorded SHA-256 and
size matched every one of the 50 published assets.

### Beta.8 two-FE Auto packaging

Beta.8 ships the two-FE Auto changes described above. The qualified Linux
binary is the `18b47675` build, Windows was built at `37f1ab89`, and universal
macOS was built offline from the corresponding-source archive at `6b97e504`.
Runtime production inputs are unchanged across these revisions. The
[package metadata](../dist/fereg-native.json) records each actual build commit,
binary checksum, runtime identity and corresponding-source hash.

The strict default `./scripts/run_tests.sh` passed with the refreshed package:
319 Python tests, Rust unit and independent numerical integration tests,
Clippy, FFI allocation audits, portable checks, mock SPI transactions,
the complexity ratchet and the root-package identity check. An independent
high-effort source review found no blocking defects in the parallel operators,
gate v2, derived-residual certification or publication validation; it did not
replace the executed checks.

| Platform | Beta.8 checks |
| --- | --- |
| macOS | Rust 1.85.1 universal build from the vendored source archive, with an empty Cargo home and offline mode; binary privacy/export audit and actual ABI/load/panic checks passed. All 44 mapped Stata 19/MP fixtures, 73 exact replay cases against the qualified Mac candidate, public OLS/PPML Break/rollback, and ten staged-package fixtures passed. |
| Windows x86-64 | Run `win-20261027T153054Z-e4e3b51f` at `37f1ab89` passed the Rust 1.85.1/MSVC static-CRT build, FMA/hardware tests, C numeric guards, PE audit and actual ABI/load/panic checks. A fresh local `net install` verified plugin bytes and passed installation smoke, single-runtime repeatability and four OLS/PPML fixtures. The controller stopped the machine, deleted transient cloud objects and released its lock. |
| Linux x86-64 | The qualified binary from build job 8298656 retains its SHA-256. Job 8299903 passed ELF audit (glibc 2.28 maximum), actual ABI/load/panic checks and 42 Stata fixtures. Two development fixtures required the intentionally excluded `_fereg_estimate_ols_test` adapter; after restoring it only in the test workspace, job 8300025 passed both. No runtime or assertion changed. Job 8300111 passed ten staged-package fixtures and a fresh local `net install`, verifying all 34 versioned files and independent OLS/PPML comparisons. |

A fresh local Mac installation of the root package verified all 35 installed
files, native selftest, exact OLS/PPML repetition and rollback, and independent
full-dummy OLS and Poisson/covariance comparisons. The dependency audit found
no advisories. The initial Mac build without the documented path remaps failed
the personal-path audit; the remapped offline rebuild is the distributed binary.
An initial local source-test invocation selected Python 3.9 and failed on missing
Python modules; the required strict suite passed in the corrected environment.
No performance observations were repeated during release preparation.

The source archive SHA-256 is
`8e89db60ba8b9111b000c68ea81d71e0bea48b5f6421f421a00e205592546c62`;
the binary archive SHA-256 is
`aafc8ce74dbe0df876a07c4375f6e34f0c860fbcd44f76d29bb7ec67a7ebd843`.
Native Intel-host Stata testing remains outstanding; both macOS slices are supplied.

After publication, fresh Stata sessions installed from the public
[beta.8 URL](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.8)
and through `github install johannes-schmieder/fereg` from updated `main`.
All 34 versioned files and all 35 main-package files matched their expected
bytes; native selftest, repeatability/rollback and the independent full-dummy
OLS and Poisson/covariance checks passed on both routes. GitHub's recorded
SHA-256 and size matched every one of the 50 published assets.

### Beta.7 PPML packaging

The beta.7 package shipped the lean PPML two-FE protocol and CMG reuse.
Its [versioned release](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.7)
records each binary's build revision, checksum and corresponding-source identity. The Linux plugin is the qualified
`277c5d11` build measured above. Windows was built at `7a33fc20`, and the
universal macOS plugin was built offline from the corresponding-source archive
packaged at `7a33fc20` with a passing dependency audit (no advisories). The
runtime identity is unchanged between `277c5d11` and `7a33fc20`.

The default `./scripts/run_tests.sh` passed at `8506a65a`: 319 Python tests,
Rust unit and independent numerical integration tests, Clippy, FFI allocation
audits, portable checks, mock SPI transactions, the complexity ratchet and the
root-package identity check.

| Platform | Beta.7 checks |
| --- | --- |
| macOS | The offline archive build passed its binary audit and actual ABI/load/panic checks. 40 Stata 19/MP fixtures passed: the 18 beta.6 release fixtures plus 22 PPML route, canonical, joint, separation, inference and parity fixtures. Public PPML and OLS Break/rollback passed, and ten installed-package fixtures passed from the staged archive. |
| Windows x86-64 | Run `win-20261026T181228Z-1bcb165d` at `7a33fc20`: Rust 1.85.1/MSVC static-CRT build, exact-FMA and hardware-kernel tests, C numeric guards, PE audit and actual ABI/load/panic checks passed. A fresh `net install` verified the installed plugin bytes, then passed the installation smoke, single-runtime and OLS/PPML fixtures in Stata 19/MP. The machine was stopped afterwards. |
| Linux x86-64 | Qualification job 8287434 built the plugin offline with Rust 1.85.1 and GCC 12.2.0; ELF audit and actual ABI/load/panic checks passed. Job 8289216 repeated the audit (glibc 2.28 maximum) and load checks and passed the same 40 Stata 19/MP fixtures; job 8289358 passed ten installed-package fixtures from the staged archive. |

The root package passed a fresh local Mac `net install`: all 35 installed files
matched, and selftest, OLS/PPML examples, a three-FE OLS comparison and a
two-FE PPML comparison with explicit dummies passed. After publication, a fresh
Stata 19/MP `net install` from the
[beta.7 release](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.7)
matched all 34 versioned package files and passed the same checks. All 50
release assets matched their local SHA-256 checksums. After the merge,
`github install johannes-schmieder/fereg` from `main` matched all 35 installed
files and passed the same checks in a fresh session.

### Beta.6 many-FE geometry packaging

The beta.6 package shipped the many-FE geometry policy. The Linux and macOS
plugins were the qualified `99f0134d` builds; Windows was built at `07d4da6d`, and the runtime
identity is unchanged between them. The corresponding source was packaged from
`07d4da6d` with a passing dependency audit (no advisories). An offline Cargo
build from that archive produced a universal Mac plugin that passed its binary
audit and ABI/load/panic checks.

The default `./scripts/run_tests.sh` passed at `78dfeca0`: 319 Python tests,
Rust unit and independent numerical integration tests, Clippy, FFI allocation
audits, portable checks, mock SPI transactions, the complexity ratchet and the
root-package identity check.

| Platform | Beta.6 checks |
| --- | --- |
| macOS | Binary audit and actual ABI/load/panic passed. The 14 focused Stata 19/MP fixtures above passed, public OLS Break/rollback passed again at `07d4da6d`, and eight installed-package fixtures passed from the staged archive. |
| Windows x86-64 | Run `win-20261026T003403Z-d67fa2b4` at `07d4da6d`: Rust 1.85.1/MSVC static-CRT build, exact-FMA and hardware-kernel tests, C numeric guards, PE audit and actual ABI/load/panic checks passed. A fresh `net install` verified the installed plugin bytes, then passed the installation smoke, single-runtime and four OLS/PPML fixtures in Stata 19/MP. The machine was stopped afterwards. |
| Linux x86-64 | SCC job 8277741 built the plugin offline with Rust 1.85.1 and GCC 12.2.0. ELF audit (glibc 2.28 maximum) and actual ABI/load/panic checks passed. Job 8280233 passed 18 Stata 19/MP fixtures, including the 14 focused many-FE fixtures; job 8280278 passed eight installed-package fixtures from the staged archive. |

The root package passed a fresh local Mac `net install`: all 35 installed files
matched, and selftest, OLS/PPML examples and a three-FE comparison with
explicit dummies passed. After publication, a fresh Stata 19/MP `net install`
from the [beta.6 release](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.6)
matched all 34 versioned package files and passed the same checks. All 50
release assets matched their local SHA-256 checksums. After the merge,
`github install johannes-schmieder/fereg` from `main` matched all 35 installed
files and passed the same checks in a fresh session.

### Beta.5 platform packaging

The beta.5 packages added Linux and Windows binaries with the method-reporting
correction; their runtime production code was unchanged by the packaging work.
The default `./scripts/run_tests.sh` passed for the reporting correction: 316
Python tests, Rust unit and independent numerical integration tests, Clippy,
FFI allocation audits, portable checks, mock SPI transactions and the complexity
ratchet. Packaging changes reuse that runtime evidence and add focused tooling,
native and installed-package checks. No new speed claim is attached to them.

| Platform | Checks |
| --- | --- |
| macOS | Universal plugin built offline with Rust 1.85.1; binary audit and actual ABI/load/panic containment passed. All 12 mapped Stata 19/MP fixtures passed on Apple Silicon, plus public OLS Break/rollback at the pre-preparation checkpoint. |
| Windows x86-64 | Rust 1.85.1/MSVC static-CRT build; exact-FMA and eight hardware-kernel tests, C numeric guards, PE audit and actual ABI/load/panic checks passed. A fresh `net install` verified the installed plugin bytes, then passed the installation smoke, single-runtime and four OLS/PPML fixtures in Stata 19/MP. |
| Linux x86-64 | Offline Rust 1.85.1 build; ELF audit (glibc 2.28 maximum), actual ABI/load/panic and C numeric guards passed. Six Stata 19/MP fixtures passed, followed by a fresh local installation, selftest and four OLS/PPML fixtures. All 30 installed files matched. |

The scientific fixtures cover CG/CMG reporting, zero-step certificates, mixed-block
MAP fallback, independent OLS dummy-variable comparisons and PPML full-dummy
Poisson. The broader Mac set also covers weighted/wide OLS and explicit CMG-Schur.

Linux build job 8269530 produced the plugin and passed its audit/load checks,
then failed because the default compiler's sanitizer library was missing.
Its dependent installation job 8269613 stopped before testing. Job 8269823 reused
the same plugin with GCC 12.2.0: C guards and all Stata checks passed, but the file
checker rejected Stata's lowercase ancillary filenames. Exact-byte verification
with case-normalized names passed in job 8269843 (`failed=0`, `exit_status=0`).
The focused verification reused the existing installed files and test results.
These environment/checker failures required no numerical changes or estimation
retries. The failed jobs remain failed.

The Windows build harness required two corrections: it assumed exactly three
hardware-kernel tests although the suite now has eight, and its installed-file
shell call used Stata compound quotes instead of OS quotes. Native compilation,
exact-FMA/hardware checks and ABI/load/panic containment passed before the latter
failure. Short inert-file probes isolated argument quoting; the corrected probe
verified installation and exact bytes without building or loading native code.
A later cold build exceeded the controller’s 15-minute limit during the C bridge
build. The fereg build/test profile now has a fixed 30-minute allowance; other
profiles and cleanup safeguards are unchanged. Failed build/install attempts
remain failed; these were tooling repairs. Final run
`win-20261024T170546Z-5b24267c` passed at source `5b381a2c` in 884.843 seconds.
Its actual build revision and plugin checksum are recorded in the native
package metadata and release manifest. Transient objects were deleted, the lock released and the
Windows machine stopped.

The assembled three-platform main package passed a fresh local Mac installation,
selftest and four OLS/PPML fixtures; all 35 installed files matched exactly.
After publication, actual `net install` from beta.5 and `github install
johannes-schmieder/fereg` passed in fresh Stata 19/MP sessions. All 34 versioned
package files and 35 main-package files matched, including Linux and Windows
binaries; selftest and the four OLS/PPML fixtures passed in both installations.
All 50 GitHub release assets matched their local SHA-256 checksums and sizes.

## Limits

- Linux performance evidence applies to Linux. The practical Mac ownership/cell
  check reports paired medians, not confidence bounds or a universal speed claim.
- Many-FE designs that need the additive preconditioner from the start can be
  slower than beta.5: the near-confounded third FE is about 12% slower at one
  worker on Linux. This is an accepted exception, not a passed comparison.
- The Mac plugin contains Apple Silicon and Intel slices. Licensed installation
  testing is on Apple Silicon; native Intel-host Stata testing is outstanding.
- PPML CMG reuse mainly helps weakly connected two-FE designs; AKM and dense
  two-FE PPML gain 16-27% from the leaner passes and settled strict entry.
  The PPML joint frames still apply their CMG hierarchy with one scalar
  cycle per step, and separation IR dominates PPML time on Veneto-shaped
  panels; the October 27 parity work measured these on the Mac only. In
  the beta.7 SCC comparison fereg was faster than default fixest in every
  eligible PPML configuration, but PyFixest's `within` and MAP demeaners were
  faster on dense two- and three-FE PPML.
- PyFixest's `within` default was also faster than fereg on dense three-FE OLS.
  After the two-FE Auto speed changes it remains 3% faster on the short Veneto
  panel at two workers; fereg is faster in the other nine Veneto cells measured.
- The two-FE Auto speed qualification accepted 11 instead of 12 distinct
  physical hosts per Linux cell (waived by Johannes); its Mac check is
  descriptive.
- Explicit `solver(cmg)` was about 50 times slower than `solver(cmgschur)` on the
  real Veneto panels, and explicit `solver(lsmr)` returned `r(909)` on the long
  panel in a 64 GB allocation where Auto completed. Auto does not use either
  route there.
- Package comparisons are descriptive: one seed, three repetitions, and
  same-model SCC nodes that differed by up to 2.4 times in absolute speed.
- GitHub Actions remain disabled; checks run locally or on the authorized SCC
  and private Windows hosts.
