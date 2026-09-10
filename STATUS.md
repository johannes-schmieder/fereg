# fereg 0.5.0 beta status

**Last updated:** 2026-09-10
**Branch:** main
**Mode:** prototype/development
**Release state:** public OLS and PPML beta surfaces; no qualified release,
tag, or Windows scientific qualification.

This is the sole mutable progress/handover record. Historical receipts remain
unchanged under benchmarks/reports/. This compact handover supersedes the
completed optimization/calibration investigation chronology formerly here.

## Current checkpoint: OLS performance and LSMR correctness

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
