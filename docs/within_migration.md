# Upstream within compatibility

The baseline audit compares fereg `7f56d06ee4c004741f32d4692bc0e40b898a51d7`
with upstream `py-econometrics/within` main frozen at
`32dde1bdd604f33012e592270781129e09e82a92`. That fereg baseline used
[historical fork revision `3b5b9b9`](https://github.com/johannes-schmieder/within/commit/3b5b9b9ab9b79e4a8cac2841f7c1d41bb2276104)
for both `within` and `schwarz-precond`. The active source for the unpublished
beta.11 counts-only test candidate pins both dependencies through its
[dependency declarations](../crates/fereg-core/Cargo.toml) and
[lockfile](../Cargo.lock) to
`534ea64f7b0e26b772f93ca087f43837c1e187e4`: the tested migration stack
`d3407a16` plus the dense categorical-count optimization. Johannes approved
advancing this counts-only candidate to testing after the bounded diagnosis;
scatter and back-substitution remain held. Fereg `c1f4d5ad` now has actual
counts-only Mac/Linux plugins, completed native correctness checks and the
[bounded main comparison](../bench/results/within-counts-qualification-20261029.json).
Mac has 55 passed cells, two inconclusive cells and one regression; Linux has
five valid hosts, leaving all 58 cells below the declared six-host minimum.
Beta.11 is not performance-qualified; installed-package admission also remains
outstanding. Earlier `d3407a16` and combined `4beaa8af` results remain historical,
unaccepted evidence below. No result authorizes acceptance, adoption or a
production release.

Johannes's current instruction is to develop and push these patches only in
`johannes-schmieder/within`. Any upstream push or pull request requires prior
discussion with him. No migration patch has been submitted upstream. This
supersedes the initial authorization to submit qualified contributions.

The authority remains [SPEC](SPEC.md), [architecture](ARCHITECTURE.md) and
[development requirements](development_workflow.md). Supported models,
scientific tolerances, original factor slots, sample/rank/omission/separation
decisions, public memory controls and transactional failure remain requirements.

## Comparison identities

The protected beta.10 tag is `435f42ce97d090e23c94625efac5fd48daa9b8fb`.
[Its native metadata](../dist/fereg-native.json) records these actual plugins:

| Platform | Source revision | Plugin SHA-256 |
|---|---|---|
| Linux | `3348fe4597a872c5981d646b49346ff62688c737` | `a79641166c17e5c193dc5f86a7fd487265d6978076e60ab7ca72f32f240f9f08` |
| macOS | `7674cf06a6786aed08d282bc27b6ae1e71ed26fb` | `aa453ba70c90a605877c1c85383f574ea6cbd5e4ba3a2875b61b451e9d7ceae0` |
| Windows | `7674cf06a6786aed08d282bc27b6ae1e71ed26fb` | `a2b18138dbde76dc31b0065ad2ea949ad650f90b65d01b7e3156d82fc06ad3b2` |

The shared production-input identity is
`c33649a542ab888e873fb3e867b3dd7a596a94804a24f4fd179f9b5ca2e87535`;
the corresponding-source archive hash is
`2ca4fa49566c462bb24d045ea4b878bba5ab80952c3c410eb7924ae2990e365b`.
[Beta.10 validation](VALIDATION.md#beta10-integration-packaging) describes the
matching production inputs and installed-package checks. Upstream patch evidence
must compare the frozen upstream base with that base plus the patch; combined
fereg acceptance separately compares the actual beta.11 plugin with beta.10.

## Supported execution paths

| Consumer | Required behavior and source |
|---|---|
| Public many-FE categorical OLS Auto | [FE session selection](../crates/fereg-core/src/fe/session.rs), `select_ols_routing`, selects GeometryMany. The bounded MAP pilot, private recurrence target, requested independent certificate and original-input MAP fallback remain fereg policy. [General routing](../crates/fereg-core/src/fe/borrowed_map/routing_lsmr.rs), `JacobiTrialRun`, predicts remaining Jacobi work from two recent contractions and escalates at its existing threshold. |
| Explicit OLS LSMR and general owned designs | [LSMR adapter](../crates/fereg-core/src/fe/lsmr.rs), `absorb_lsmr_certified_at`, accepts weights, slope-bearing terms and multiple RHSs. [Owned Auto](../crates/fereg-core/src/fe/auto.rs), `absorb_auto`, can also attempt this backend before certified MAP fallback. |
| PPML explicit LSMR | [Validation](../crates/fereg-core/src/ppml/prepared_estimator.rs), `validate_public_solver_request`, supports at least two categorical-intercept FEs, including more than three, in the supported memory modes; absorbed slopes remain unsupported for this explicit request. [Joint solver](../crates/fereg-core/src/ppml/categorical_lsmr.rs), `ReusableCategoricalJointLsmr`, solves the augmented finite-plus-FE operator, including zero finite regressors. |
| PPML canonical checkpoints | [Borrowed projection](../crates/fereg-core/src/fe/borrowed_map.rs), `CheckpointLsmrBatch`, independently certifies, optionally tightens, and falls back from untouched inputs with remaining work. Explicit LSMR and CMG frames reach this acceleration through [prepared IRLS](../crates/fereg-core/src/ppml/prepared_irls.rs). |
| PPML Auto separation fallback | [Retry policy](../crates/fereg-core/src/ppml/prepared_separation/retry.rs), `run`, can retry an admitted categorical numerical failure through retained CG and LSMR. Ordinary PPML Auto selecting MAP does not make this dependency irrelevant. |
| Two-FE OLS/PPML CMG and slope-only MAP paths | These retain their existing implementations, explicit support rules, independent certification and cancellation. General within migration is not authorization to replace these routes. |

## Extension classification

| Capability | Classification and migration boundary |
|---|---|
| Initial coefficients, progress/early stop, returned coefficients and stop reason | Essential. Expose existing low-level solve controls through the high-level interface, with documented coefficient coordinates. Keep fereg's Jacobi-to-additive rule. |
| Work accounting, including numerical failure | Essential. Return consumed iterations when a solve fails so retries and corrections debit the original allowance. A successful result alone is insufficient. |
| Original-response convergence reference | Essential. A coefficient seed solves a correction while stopping remains relative to the original response. Independent fereg validation remains authoritative. |
| Cancellation | Essential separate patch. Coarse construction checks and one check per iteration are sufficient; dispatched work must join and a distinct cancellation must propagate to Interrupted. |
| Fixed-worker repeatability | Essential adoption prerequisite. Upstream work must be validated on the actual integration; a reduction mode name is not evidence of the guarantee. |
| Shared prepared design | Existing upstream API and useful optimization. Upstream accepts a shared `Design`; do not require a new generic workspace ownership API. |
| Fork `Solver::with_preconditioner` | Optional unused optimization. No current fereg caller uses it. |
| `SlopeScreening::Disabled` | Replaceable through upstream's ordinary per-term preparation. The fork option disables cross-term diagnostics, not whitening or rank decisions. Check peak storage and slope arithmetic. |
| Generic `PairReplacement` and reduced-inverse factory | Experimental general-FE developer routing only. Not required by public Auto, PPML, explicit LSMR or correction solves. Johannes approved retirement of this branch; the two-FE selectors remain. |
| Sparse back substitution and other kernel changes | Independent performance candidates. Keep worthwhile changes in separate patches; optional speedups do not become API prerequisites. |
| Typed allocation failures | Preserve the public failure contract and admission. Frozen upstream lacks fork allocation variants; removing match arms is not evidence that the contract survives. This does not require porting every historical allocation hook. |

### Controlled starts suffice

In [general routing](../crates/fereg-core/src/fe/borrowed_map/routing_lsmr.rs),
the baseline finished Jacobi sessions before releasing the trial solver,
constructing additive Schwarz and starting from each returned `x` with its
remaining allowance. No Krylov recurrence crossed that replacement. The
candidate's `solve_controlled_windows` and `escalate` express this behavior
directly through coefficient results and the controlled API. The public
`geometry_many` preset retains its existing policy and never needed the retired
generic pair factory.

[`general_lsmr_attempt`](../crates/fereg-core/src/fe/borrowed_map.rs) also uses
only returned coefficients for its one tighter correction after independent
certificate rejection. `FeCoefficientBatch` documents the original term/channel/
level layout. Each correction keeps the original RHS and subtracts prior work.
The adapter must preserve iteration consumption on failed attempts as well.
The candidate debits successful and failed additive continuations before
propagating an escalation failure. The focused
`failed_escalation_keeps_prior_work_in_the_off_retry_budget` witness covers a
counted failure at the adapter boundary, an actual Off retry with the remaining
allowance and budget exhaustion. Upstream failing-operator witnesses separately
establish the returned iteration count; the adapter test does not depend on
accidental roundoff to enter fallback.

PPML's joint solver already calls low-level `mlsmr` with `MlsmrOptions`
containing `warm_start`, `escalation` and `local_size`. It retains accepted
coefficient vectors across IRLS frames, scales them by the accepted step and
invalidates them on canonical synchronization; it retains no Krylov session.
These low-level controls and `Solver::preconditioner()` exist upstream.

### Defaults and coordinates

The baseline [`additive_schwarz`](../crates/fereg-core/src/fe/lsmr.rs) helper
started from `PreconditionerConfig::default()` and changed the reduction only
if that value is Additive. Frozen upstream defaults to Adaptive. A mechanical
pin update would silently substitute upstream's routing policy and miss the
reduction override. The candidate now constructs Additive explicitly with the
intended local solver settings and upstream's ownership reduction. This also applies to
PPML's `rebuild_schwarz`, which extracts the factor and applies it inside
fereg's own augmented operator; an adaptive getter can return the diagonal
ladder before Schwarz exists.

There is no `Solver::new_raw` call in current fereg and no such constructor in
the historical fork baseline. Its `new_with_screening(..., Disabled)` performs
per-term slope whitening with the `1e-10` relative Gram rank rule. A new raw
constructor is therefore not an established migration requirement. Upstream
uses compensated two-pass level moments instead of the fork's Welford moments,
and reconstructs weights from retained square roots. Weighted/sloped low bits
can change; boundary witnesses must check that the model span and discrete rank
decisions survive. The [weighted slope-boundary witness](../bench/results/within-slope-boundary-20261029.json)
finds a concrete dependency rank difference. Johannes accepted upstream's
whitening correction for continued migration qualification; the original
fork/candidate parity failure remains recorded.
Its [independent rational oracle](../bench/compare_within_slope_boundary.py) compares
the actual IEEE inputs with the [public API results](../bench/within_slope_boundary.rs),
including original coefficient coordinates. The fixed Mac matrix contains 60
fixtures: five perturbations around the `1e-10` Gram rule, four shifts/scales,
three row orders, unequal nonsquare weights, one/four workers and two repeats
per pin, for 480 records. Neither fixtures nor tolerances changed after freezing.

The candidate passes every exact-rank, projection and retained-span certificate
check; maximum projection error is `6.16e-12` and normalized certificate is
`3.39e-12`, against unchanged bounds `1e-8` and `1e-9`. The fork differs in
observed kept/drop identities on 23 fixtures: 13 change retained counts and
the exact retained span; three change counts on exactly aliased slopes without
changing the exact span; three keep the same count but select a different
near-aliased slope, changing the exact span; four only rename the dropped
exactly redundant slope. These are 49 per-level identity differences across
240 levels, including 25 retained-count and 28 exact-span differences.
All differences occur with shifted/scaled loadings; zero-offset fixtures and
all `delta=2e-5` fixtures agree. The largest fork projection error is `0.200018`.

For example, with shift `2^24` and `delta=1.01e-5`, the exact residual-variance
fractions are `1.01995e-10` to `1.02014e-10`, above the threshold. The candidate
keeps both slopes at all four levels. The fork drops the first slope at two
levels and fails both the projection and independent certificate. Below the
threshold at `delta=0.99e-5`, the reverse-row fixture instead finds the fork
retaining both slopes at three levels where the exact rule retains only the
second slope. Baseline parity therefore cannot be substituted for the oracle.

Input bit digests, original layouts, all empty fork slots, fixed-worker repeats
and cross-worker agreement pass. Returned original-coordinate coefficients also
pass the predeclared conditioning-aware reconstruction bound; their absolute
reconstruction error can reach `1.22e-4` under large offsets, separately from
the strict returned-projection checks. This isolates dependency whitening;
it does not yet establish a change in fereg's published rank, omission or
estimates. Johannes's approval permits continued qualification with upstream's
whitening correction. The native checks below validate selected published
results, original factor slots/base flags, PPML and replay behavior, while
preserving the documented near-boundary rejections. The candidate must still
pass beta.10 performance qualification. Scientific
tolerances remain unchanged; this is not production-adoption approval.

### Approved retirement of the general-FE experiment

Johannes approved retirement of the general categorical, more-than-two-FE
pair-upgrade branch selected by the twelve `routing_h{4,8,16,32}_k{0,1,2}`
developer arms and by `routing_builder_break` (h4/k1). The candidate must reject
these explicit requests with InvalidArgument on more-than-two-FE categorical
designs. Their two-FE CMG implementations, selectors and documented Break witness
remain supported. Prefix, portfolio, geometry and bounded-window selectors do
not require generic pair factories and remain supported. This approval does
not change public Auto, explicit LSMR or PPML policy.

In the audited baseline, the pair factory, component CSR view and replacement owner occur only in
[general routing](../crates/fereg-core/src/fe/borrowed_map/routing_lsmr.rs).
There is no public PPML `PairReplacement` consumer. The historical
`refused_pair_build_resumes_exact_incumbent_recurrence` test required exact
continuation of a live recurrence after optional setup was refused; accepted
replacement and reuse had their own test. That historical requirement needed full
recurrence retention. Upstream's prepared operator and pair builders are
private, so reconstructing an equivalent factory locally would introduce a
new preconditioner implementation and still would not preserve exact
refusal/resume behavior without recurrence access. It is not a small adapter.
Useful independent projection, coefficient reconstruction, memory and dense-QR
witnesses remain requirements after retirement. Implementation-specific tests
of the retired factory and recurrence machinery do not establish public-policy
correctness; retained controlled-start and independent witnesses must cover the
supported routes.

## Cancellation, failure and memory

Stata continues polling Break on its main thread; numerical workers observe
the host signal. All work joins before Interrupted is returned, and partial
estimates remain private. The new control must cover cold batch solves,
Jacobi trials, additive solves, construction and PPML's direct `mlsmr` calls.
The baseline [`categorical_lsmr`](../crates/fereg-core/src/ppml/categorical_lsmr.rs)
error mapper converted non-allocation errors to NonConvergence, which strict-final
IRLS may catch for a canonical retry. The candidate now calls the cancellable
constructor and low-level solve and maps Cancelled to Interrupted before that
catch. The actual-iteration Mac/Linux Break witnesses below verify propagation,
joined work, rollback and successful reuse for both OLS and PPML.

PPML's joint policy has a separate existing work convention: initial,
staleness and certificate continuations each receive the configured per-run
maximum and their counts are summed. OLS/checkpoint adapters instead debit a
shared allowance. Migration must not silently change PPML's existing convention.

The candidate [categorical Schwarz estimate](../crates/fereg-core/src/memory/categorical_schwarz.rs)
replaces fork pool and scatter assumptions with upstream ownership/prolongation,
shared workspace and per-subdomain output-cache bounds. The shared workspace
bound accounts for concurrent RHSs and nested yielding components rather than
multiplying a complete independent pool by every RHS. [Recurrence estimates](../crates/fereg-core/src/memory.rs)
now include audited-restart overlap, data-selected Fold/Owner scratch and
controlled-result metadata. The adapter retains the historical workspace
high-water charge when later RHS batches narrow. PPML separately charges active
design, retained vectors, correction overlap and checkpoint storage.

A pinned-toolchain allocation/science witness compared frozen main and the
reproducibility stack at one and four workers: 120 records covering 72
recurrences and 48 weighted, sloped and categorical batch cases passed, with
maximum planted-residual error `4.222e-10` and exact repeats on those cases.
The maintained [allocation witness](../bench/within_allocation.rs) and
[runner](../bench/measure_within_allocations.py) also passed 60 records against
the fallible-scratch patch `7b4fa1e4e88fa1d8b3d90b5ed08706fb924bcd86` on
draft #478. Numerical summaries and iteration counts matched the unpatched
stack exactly; six four-worker heap peaks differed with bookkeeping/pool
scheduling, which is not a scientific-bit target. Independent source review
found no underbound or checked-overflow mismatch in the candidate formulas.
The mapped fereg memory/admission checks passed 97 distinct tests, including
retained LSMR storage, exact versus coarse admission, constrained PPML memory,
coefficient/certificate handoff and an analytic weighted CMG cycle. After the
helper refactor, all 85 memory tests and four retained-storage tests passed
again. The retained yielding-component witness exercises a nonzero yielding
count and RHS width 64 to 1: the exact bound admits the solve, while one byte
less is rejected before the native batch. These are mapped source tests;
complete plugin admission validation remains necessary.
Preserve admission and allocation-error behavior without importing unrelated
allocation interfaces.

## Upstream work and evidence limits

[Compensated accumulation #464](https://github.com/py-econometrics/within/pull/464)
is already merged in the frozen main; do not duplicate it. The older
[reproducibility PR #465](https://github.com/py-econometrics/within/pull/465) was
closed in favor of upstream-led work. Its large-working-set regressions are a
reason to test outside cache, not to discard repeatability.

At this audit, upstream's [draft #474](https://github.com/py-econometrics/within/pull/474)
was `638f96f9180d2581dbb0620afb6ea62ae8cfd2f3`, based on frozen main;
[draft #478](https://github.com/py-econometrics/within/pull/478),
`6f2ead13bc494f7b6507b40a894c6f277aa21679`, adds adjoint work on that integration.
The stack includes fixed LSMR reductions, Schwarz ownership and data-selected
adjoint strategies. Neither draft is merged in frozen main. Frozen main
guarantees bitwise repeatability only for one thread and lacks the fork's
Ordered reduction. fereg requires repeatability at every fixed worker count
and seed, scientific agreement across counts and stable discrete decisions.

### Performance inventory

This source-only followup compares fork `3b5b9b9ab9b79e4a8cac2841f7c1d41bb2276104`,
frozen main `32dde1bdd604f33012e592270781129e09e82a92`, the reproducibility stack
`6f2ead13bc494f7b6507b40a894c6f277aa21679` and combined candidate
`41c53760e233ed6b0766af74f55951a4a0d51067`. The candidate retains the draft's
design adjoints and bidiagonalization kernels. These findings identify optional
work for review; they establish no timing benefit or regression.

| Fork improvement | Upstream equivalent or remaining gap |
|---|---|
| Fused observation pass | The fork's [optional operator hook](https://github.com/johannes-schmieder/within/blob/3b5b9b9ab9b79e4a8cac2841f7c1d41bb2276104/crates/schwarz-precond/src/lib.rs#L58-L75) combines `u = Av + c*u`, its squared norm and selected adjoint terms. [Modified LSMR](https://github.com/johannes-schmieder/within/blob/3b5b9b9ab9b79e4a8cac2841f7c1d41bb2276104/crates/schwarz-precond/src/lsmr/bidiag.rs#L341-L379) reuses that adjoint. The hook is absent from frozen main and both later stacks; [candidate steps](https://github.com/johannes-schmieder/within/blob/41c53760e233ed6b0766af74f55951a4a0d51067/crates/schwarz-precond/src/lsmr/bidiag.rs#L478-L498) still execute separate forward and update passes. This remains an optional performance candidate. |
| Fixed-lane adjoints | The fork's [lane plan](https://github.com/johannes-schmieder/within/blob/3b5b9b9ab9b79e4a8cac2841f7c1d41bb2276104/crates/within/src/operator/design/fused.rs#L1-L107) combines sorted runs and buffered categorical terms, then handles slopes and large unsorted terms through coefficient ownership. Draft #478 supplies the reproducibility purpose through [data-selected Owner/Fold plans](https://github.com/py-econometrics/within/blob/6f2ead13bc494f7b6507b40a894c6f277aa21679/crates/within/src/domain/adjoint_sum.rs#L1-L29) and [fixed adjoint trees](https://github.com/py-econometrics/within/blob/6f2ead13bc494f7b6507b40a894c6f277aa21679/crates/within/src/operator/design/scatter.rs#L75-L179). Those sums have different arithmetic order from the fork. Porting the old lanes is not required merely to obtain repeatability. |
| Per-step norm and normalization work | Upstream already fuses the affine vector update with its norm. The [current implementation](https://github.com/py-econometrics/within/blob/6f2ead13bc494f7b6507b40a894c6f277aa21679/crates/schwarz-precond/src/lsmr/bidiag.rs#L25-L59) uses eight accumulators per 4,096-row leaf, a fixed reduction tree and range-safe norm fallback. The fork's scalar norm helpers are superseded. Upstream also retains the optimization of leaving `u` unnormalized, now bounded to beta in `[1e-64, 1e64]` to protect extreme-scale solves. |

The old fusion was active on supported paths. The fork constructed
[`StepKernel::Fused` by default](https://github.com/johannes-schmieder/within/blob/3b5b9b9ab9b79e4a8cac2841f7c1d41bb2276104/crates/within/src/solver.rs#L479)
and fereg did not override it. Within-based many-FE Auto Jacobi/Schwarz attempts,
explicit weighted or sloped OLS LSMR, owned Auto LSMR attempts and PPML canonical
checkpoint or separation LSMR projections therefore used it when executing
modified LSMR. Cold Off solves use ordinary Golub-Kahan and bypass the fused
forward step, but their design adjoints still followed the fork's lane order.
PPML's [augmented joint solver](../crates/fereg-core/src/ppml/categorical_lsmr.rs),
`ReusableCategoricalJointLsmr::run_once`, instead supplies its own
`AugmentedCategoricalOperator` with only forward and adjoint actions. It never
used this within-specific fusion; common low-level norm changes do affect it.
Evidence must distinguish these paths from MAP/CMG work that never executes the
kernel under review.

The smallest proposed optional boundary is to fuse the forward gather and
affine observation update while preserving the candidate's current adjoint
kernel, norm arithmetic, solver policy and public controls. It must retain the
per-row `xi + scale * yi` expression, current eight-lane leaves and reduction
tree, finite checks and scaled-norm fallback. Full forward/adjoint fusion is a
separate arithmetic and storage change: old lane boundaries and run grouping do
not reproduce the current per-level and per-term trees. The current design also
uses `u32` ownership rows and shares row work across slope columns, whereas the
fork's ownership fallback used `usize` rows and separate coefficient passes.
The old algorithm is not necessarily faster on these designs.

If raw-adjoint fusion is explored later, beta outside `[1e-64, 1e64]` must still
trigger normalization and recomputation of the adjoint; scaling an already
rounded adjoint is not equivalent. Speculative raw-adjoint overflow must not
block that fallback. Restoring fusion also does not automatically eliminate an
observation vector: [current gradient audits](https://github.com/johannes-schmieder/within/blob/41c53760e233ed6b0766af74f55951a4a0d51067/crates/schwarz-precond/src/lsmr/bidiag.rs#L512-L525)
use the forward scratch buffer. A different buffer lifetime needs its own
allocation and admission check.

Before timing, any such patch needs exact candidate-versus-patch updated vectors,
norms, solver fields and stop decisions, together with independent weighted and
sloped projection certificates. Cover sorted Owner, sparse indexed Owner and
dense Fold designs; zero weights; lane/leaf boundaries; extreme magnitudes;
warm starts, correction audits, cancellation and counted failures. Retain the
existing range, determinism and independent projection witnesses. A bounded
Mac/SCC comparison must include both cache-resident and beyond-cache working
sets with traces establishing execution of the changed path. No new fusion is
implemented in beta.11, and it is not a prerequisite for migration. The separate
sparse back-substitution patch remains subject to its own evidence and review;
combined beta.11 qualification against beta.10 remains a separate gate.

### Patch evidence and unresolved results

The separate API patch is `cf228c412fdadac89a3ff28b12f53b7e1580f3de` and the
separate cancellation patch is `b596c9854ea078dc40e903300d66a54d6062fcf0`, each
on frozen main. Both patch suites passed on Apple Silicon with Rust 1.85.1;
the cancellation log records 548 passing tests including four doctests, with
one existing ignored test. The latter
adds nine cancellation tests covering joined work and distinct cancellation,
without changing existing `MlsmrOptions` struct literals. The combined upstream
stack `41c53760` on draft #478 passed 574 Mac tests with one ignored test and
independent high-effort review. Final API review found no remaining blocker.
These are upstream patch checks; they do not establish beta.11 plugin readiness.
The downstream source review against fereg `0bcd10fb` and dependency `41c53760`
also found no remaining source blocker after the wrapper and cancellation fixes;
its final nine-file diff digest was
`17244ad11ab07a6659c8a51bbeddf80f1081ad0023919e79011075890bd03e47`.
That review identified the rank-boundary witness above as a qualification
requirement. Its subsequent parity failure is retained; Johannes accepted the
independently supported upstream whitening correction for continued qualification.
The source review does not replace runtime or timing evidence.

The independent reproducibility witness at
`43348fd7329fc1093ec3aa6442696ce1950485e9` on draft #478 passed with Rust 1.85.1
on Apple Silicon: 18 trajectory cells across four pools (432 RHS solves), plus
24 converged weighted three-slope analytic projection solves on 12,586 rows
across Off/Diagonal/Additive and 1/2/3/8 workers, with a `1e-9` residual bound.
Four upstream LSMR, one weighted-adjoint and four Schwarz-owner tests also passed.
The same baseline diagnostic exposed frozen-main fixed-worker nonrepeatability
at eight workers: 10,386 coefficient entries and 55,568 demeaned entries
changed in low bits, with maximum differences `9.71e-16` and `4.44e-16`;
iteration count and discrete decisions stayed fixed. This establishes a real
baseline defect, not failure of the stated scientific tolerance.

An October 29 source review also inspected the three newer draft #478 commits
through `4516668612dbdb8bf3019eda6703fd573bcbe0f9` against frozen `6f2ead1`.
They change the unsorted slope scatter threshold, increase by-level reduction
leaves from 4,096 to 16,384 rows, and move scatter indices into each prepared
design. These changes can alter arithmetic and construction/overlap storage;
the previous evidence cannot qualify them. In particular, locality sorting
adds an `8L` transient cursor and per-term index construction is concurrent.
The fallible Schwarz scratch patch is source-disjoint, but does not cover
these within allocations. No required correctness fix was identified in this
delta. The candidate retains `6f2ead1`; newer upstream performance claims are
motivation for later evaluation, not transferred evidence. Fixed-worker
repeatability remains a production prerequisite.

SCC correctness jobs completed with success sentinels, terminal exit 0 and no
test failures. Each suite had one existing ignored test; these commands omitted
doctests.

| Source | Job | Host | Passing tests |
|---|---|---|---:|
| Frozen base | `8325710` | `scc-md7` | 535 |
| Controlled API | `8325725` | `scc-md7` | 545 |
| Cancellation | `8325726` | `scc-ke3` | 544 |
| Reproducibility | `8325752` | `scc-222` | 548 |
| Allocation | `8325753` | `scc-222` | 549 |
| Back substitution | `8325754` | `scc-eb1` | 538 |
| Combined stack | `8325803` | `scc-222` | 574 |

These used 12-slot correctness allocations, not verified performance isolation.
Combined-stack job `8325803` also has a verified success sentinel and terminal
exit 0. These correctness checks do not establish combined beta.11 performance acceptance.

The separate back-substitution patch is `3303a6e`; its common benchmark harness
is frozen at base `846d84d` and patch `5f31a1a`. The
[compact evidence record](../bench/results/within-backsub-20261029.json) retains
source, binary, harness and plan hashes, every paired ratio, per-host statistics
and the exact Linux report hash. On Mac, all six large-case science-only
base/patch comparisons agreed exactly for unit/weighted direct application at
one/four workers and complete solves at one worker. The design has 8,388,608
rows and a 360,718,788-byte factor/RHS lower bound; maximum independent fitted
error was `9.094502928519432e-12`. Four-worker complete solves were excluded
before timing because the frozen base failed repeatability there; that negative
result remains relevant to adoption.

The Linux release build completed in job `8325815` on `scc-eb1`, with source,
compiler and binary identities checked. Science-only preflight `8325890` on
`scc-222` passed all 24 calls across 12 cells. The reused Python and Julia pins
matched the campaign declaration. Timing array `8326747` then completed 1,872
calls on six distinct, verified whole-node allocations, with no exclusions or
retries. Accounting, scientific signatures, iteration counts, independent
witnesses and replayed isolation checks passed. Six mirrored blocks per cell
and host gave 432 paired comparisons. All 12 host-median nonregression intervals
passed the declared max(5%, 1 ms) allowance; pooled descriptive median ratios
were 0.932–0.979 in cache and 0.959–0.987 outside cache. With only six independent
hosts, the exact median bounds are the minimum and maximum host scores; these
are not simultaneous bounds across cells or proof of a speed improvement.

One observation still triggers the frozen review rule: weighted memory direct
application at four workers, `scc-yk3`, zero-based block 4, took 0.328317 seconds
for the patch versus 0.310275 for the base, a 5.815% increase and 2.528 ms beyond
the allowance. It remains in the report. Johannes reviewed this result and
instructed us to hold the optional upstream PR. The patch remains outside the
combined candidate and does not replace beta.11 qualification against beta.10.

The completed practical Mac comparison contains 24 science preflights and 288
timed calls, giving 72 mirrored paired blocks across the same 12 cells. Exact
science, schedule, binary and observer checks passed. All 12 descriptive
intervals and all 72 individual blocks stay within max(5%, 1 ms). Eleven cell
medians improve by 0.28–5.92%; the cache/unit complete-solve median is 0.45%
slower. All 15 unfavorable point comparisons remain recorded; the largest
relative increase, 7.84%, is 0.119 ms and below the absolute allowance.

Four earlier Mac observer attempts remain separate, including 32 completed
timings and eight paired blocks. The first failed before execution because
process observation was sandboxed; the second stopped after four science
calls when Claude used CPU. Two later attempts exposed overly narrow Claude
identity rules; their gated children were stopped before native execution.
Johannes authorized operational retries and retaining Claude as ordinary
desktop noise. The final prospective declaration binds five verified Claude
executable identities, including two search helpers. Across 391 samples they
used 5.87 sampled CPU-seconds, with peak simultaneous sampled CPU of 11.8%
and summed RSS of 2,475,936 KiB. These counters do not prove idleness or explain
any individual timing. Other competing compute remained screened. No timing
was excluded or pooled across attempts, and unfavorable native results do
not authorize resampling. Mac evidence is descriptive; Johannes's hold on
submitting this optional optimization remains in effect.

The common cold-solve fingerprint driver (SHA-256
`f546648c5d7982cfb0299d723663c33f1e0a27ca7293f0e42dd13282071fad42`) found
40 of 42 cells exact across frozen main, API and cancellation. The two exceptions
were four-worker Additive three-slope cases, with and without weights, where
the baseline itself failed fixed-worker repeatability. Separately, the large
weighted three-slope Off case was exact across all three arms but failed the
declared planted-residual bound: `2.136169907629437e-8` against `1e-8`. At 13
iterations every arm reported normal-equation ratio `1.5652502967804625e-10`.
Read-only diagnosis found no contradiction with the native backward-error
stopping rule: it differs from the driver's maximum-row forward-error witness.
The internal operator-norm estimate and stop reason were not exposed, so the
exact stopping branch cannot be reconstructed. The forward witness remains a
failure; equality to the baseline is not a scientific pass. Johannes authorized
one stricter diagnostic at tolerance `1e-12` on the same failed cell, all three
arms and two repeats. Exactly six fresh-process calls passed the unchanged
`1e-8` witness without retries: every call used 16 iterations, with maximum
planted error `8.477663016037695e-13` and normal-equation ratio
`2.6044044562489446e-16`. Common scientific bits and iterations matched across
arms and repeats; inputs and all ten original source/configuration/result hashes
were unchanged. This diagnostic changes no public default and does not erase
the original tolerance-`1e-10` failure or establish beta.11 acceptance.

Focused fereg adapter checks on Rust 1.85.1 passed 79 main tests and four
subprocess witnesses: LSMR/batch 16, controlled routing 5, retired selectors 1,
bounded CMG 24, analytic Schur 1 and PPML categorical LSMR 32. The approved
retirement removes the obsolete pair/session consumers. The
[complexity ratchet](../tests/reference/complexity.json) reduces fereg-core
production lines from 113,243 to 113,029 and dispatch candidates from 1,458 to
1,451. Essential host cancellation and the sanctioned actual-iteration Break
witness add 148 plugin production lines and two injection dispatch candidates.
The combined runtime decreases by 66 lines; large-file/function and chunk
counts do not increase. The new selector's parser and host entrypoints remain
below 150 production lines through meaningful request/guard helpers.

One original test failed its expected fallback-route assertion, not its
scientific certificate. Read-only diagnosis of the unchanged extreme-weight
fixture found Auto/Diagonal converged in four iterations with certificate
`5.2752027773493125e-12` and analytic residual error
`2.220446049250313e-16`; Off gave certificate `5.275202776979238e-12` and error
`1.1102230246251565e-16`. With Johannes's approval,
[`extreme_weight_tree_matches_analytic_projection_with_auto_and_off`](../crates/fereg-core/src/fe/lsmr.rs)
preserves the historical rows and weights, requires the original `1e-10`
certificate and checks the independent analytic projection within `1e-14`.
Failure-accounting tests use explicit counted errors at adapter boundaries,
actual retry solves and independent upstream failure witnesses. This changes no
Auto policy and retains the original failed expectation as evidence.

The full source suite stopped at a core-test failure after 1,535 passes and
three ignored tests:
[`automatic_structural_map_uses_admitted_shared_operations`](../crates/fereg-core/src/ppml/prepared_estimator/tests.rs)
expected `solver_workspace_bytes > 0`. Diagnosis found a test assertion on the
wrong charge field, not a missing allocation: with 12 rows, FE levels 6/2/3,
three RHSs and 24 workers, explicit LSMR needs 57,465 base bytes plus 80,524
solver bytes, while the test supplies 137,988 usable bytes, one byte less.
Auto legitimately admits 408 certificate bytes and 55,416 reusable
coefficient/joint bytes for a 113,289-byte total. Its optional component
checkpoint would add 57,864 bytes and exceed the cap at 171,153, so that
checkpoint is refused and the separate solver-workspace charge is zero.
The test-only correction requires both actual reusable charges to be positive
and total selected bytes to fit the cap; policy and numerical comparisons stay
unchanged. This test and `public_lsmr_checkpoint_workspace_is_required_before_canonical_work`
passed focused reruns. The original full-suite failure is retained. The rerun
passed all 1,536 core tests (three ignored), then failed the pinned PPML
separation routing census at seed 17, configurations 1/2: the retained-CG
developer arm's LSMR retry fails its residual certificate. Public Auto's
sample and separation decisions remain exact against the incumbent in all
three diagnosed configurations, including the independent 228-row LP witness.

Causal isolation captured the final 260-row, two-FE residual solve with 49
coefficients and minimum weight `5.74712643678161e-19`. With identical
coefficients and one solver iteration, weighted-residual unscaling gives
fereg's FE certificate `1.0478754742976134e-8`, exceeding this call's actual
`1e-8` tolerance. Direct unweighted model subtraction gives
`3.689858156465186e-11`, matching the incumbent; the vectors differ by at most
`5.551115123125783e-17`. An independent 80-digit evaluation of the same
floating-point coefficients gives maximum reconstruction errors of
`5.57e-17` and `3.52e-19`, respectively. Independent stable mathematical
moments pass both outputs; this identifies a caller-unit reconstruction
compatibility issue, not a coefficient error. Isolated API revision `773372c`
adds an opt-in `ResidualOutput::PreparedModel` to the new controlled interface,
reusing upstream's existing gather and measured residual buffer before
coefficient back-transformation and row restoration. The default output and
weighted convergence audit remain unchanged. The combined revision `d3407a16`
passed 579 unit/integration tests and four doctests, plus focused strict Clippy;
the isolated API passed 549 tests and three doctests. Independent source review
found no blocker. Tests cover the captured input, weighted group means,
sloped rank loss, zero weights, original row order, warm handoff, storage reuse
and cancellation after reconstruction with exact consumed work.

SCC follow-up job `8326940` on `scc-wm3` passed both updated suites with the
same counts, plus two isolated subprocess executions per suite. It used Rust
1.85.1, GCC 12.2.0, 12 reserved slots, four build jobs and eight Rayon workers;
tests ran serially. Terminal accounting reports failed/exit status 0, 626
seconds wall time and 3.876G maximum virtual memory. This was a correctness
allocation, with no isolation or performance claim. The
[compact record](../bench/results/within-output-option-20261029.json) preserves
source, script, manifest and result identities, including earlier diagnostics.

A fresh Mac science panel for coherent API `773372c` passed all 48 declared
processes and 384 RHS solves, including warmups, against frozen upstream
`32dde1b` and inactive cancellation `b596c`. Proposal SHA-256
`020a2a81b31ace8e11daf76748b38eaf04f707ce4091da21703bfce5d5878675`
binds six cells at one worker: 8,192 and 8,388,608 rows for unweighted
categorical Off, weighted categorical Diagonal and weighted three-slope
Additive designs. The same API binary supplies default and PreparedModel
output modes. All common named fields match exactly across base, default API
and cancellation. Between API output modes, coefficients, dimensions, layout,
unidentified directions, warnings, convergence, iterations, recurrence-residual
bits and explicit stop reasons remain exact; only demeaned values, their
planted-residual errors and the full-output digest differ. Each arm repeats
exactly across warmup/retained groups and two fresh processes. The independent
dyadic projection witness passes the unchanged `1e-8` absolute bound, with
maximum error `4.001599052116944e-11`. The same compact record preserves the
three actual binary hashes and source, proposal and preflight identities.
This panel contains no timed invocations and makes no fereg adoption claim;
historical `cf228c` performance evidence remains separate.

A matching SCC science panel for coherent API `773372c`, upstream `32dde1b`
and inactive cancellation `b596c` passed all 48 processes, 384 RHS solves
including warmups and 24 arm/cell references under the same proposal, using
Rust 1.85.1 and GCC 12.2.0. The
same six cells, one-worker configuration, exact scientific comparison scope
and fixed-worker repeatability checks passed. All controlled stops were
`NormalEquationTolerance`; the independent projection witness again had
maximum error `4.001599052116944e-11` against the unchanged `1e-8` bound.
Source/artifact binding, clean checkouts and unchanged helper sources were
verified. Job `8328473` ran once on `scc-eg1` in `qphys-pub`, with actual
granted PE `omp16` and 16 slots. Terminal accounting reports failed/exit
status 0, 684 seconds wall time and 2.455G maximum virtual memory. The
[compact record](../bench/results/within-output-option-20261029.json) retains
all three Linux binary pins and the collected source, preflight, accounting
and result hashes; the summary SHA-256 is
`b9c76d6d0f451307544e299b231fcd808115d245f478dc8cbfdb636efaf950bc`.
This was a shared correctness allocation with no retries, timing, isolation
or fereg qualification/adoption claim; historical `cf228c` performance remains
separate.

Fereg explicitly selects this output mode in both ordinary batch and controlled
routing helpers. Its numerical scope includes their weighted/sloped OLS and
PPML checkpoint, separation and fallback consumers. The returned residual may
affect later independent certification or estimation steps; final estimation
bits are not presumed unchanged. The within solve's coefficients, convergence
audit, stop reason and work stay unchanged for a given input. No new retained
observation buffer is allocated, but the extra gather needs fresh performance
qualification. No census expectation or scientific tolerance has changed.
The previously failing PPML separation census and the fixed/cross-worker
agreement test now pass with their original expectations and tolerances.
The next full run passed 1,535 core tests and found two adapter witnesses still
comparing the explicitly selected output against upstream's default weighted
output. Their test-only correction preserves exact full-batch coefficients,
convergence, work, diagnostics and failure checks, and independently verifies
every returned residual bit as the original response minus its two categorical
coefficients. Output-bank shape checks remain explicit. Both focused witnesses
passed; no scientific tolerance changed. The original failure is retained.
The final Mac `./scripts/run_tests.sh --source-only` completed with exit status
0 against this exact pin: 273 Python tests (12 skipped), 1,537 core unit tests (three
ignored), workspace scientific/integration tests, plugin/FFI tests, isolated
allocation audits, formatting, fuzz compilation, strict Clippy, portable/C
guards and the complexity ratchet passed. The source-run log SHA-256 is
`9f24d2a9cf823362a5cc79e11cd1e8eb396e8338ba59160dc17406a67c6776dd`.
This run used Rust 1.85.1, Python 3.12.14, four build jobs, eight Rayon workers
and one BLAS worker on the development Mac. It explicitly did not perform
installed-package admission. Historical `cf228c` entrypoint evidence does not
qualify the new API revision.
The native checks below cover existing memory fixtures, transactions and the
declared scientific replay scope. The new controlled-solve, cancellation and
fallible-allocation entry points still need patch-specific performance evidence.
Mac/SCC beta.11 performance against beta.10 and adoption review remain separate
requirements. No upstream PR
submission or production acceptance is claimed by these checks.

### Native candidate checks

The [native evidence record](../bench/results/within-native-20261029.json)
binds fereg `d96eccfc`, within `d3407a16`, both actual plugin hashes and all
declared inputs. The universal Mac build passed binary audits and actual
ABI/load/selftest and panic checks; runtime tests used Apple Silicon. SCC job
`8327053` on `scc-gd4` passed the source-only suite and Linux build, ELF audit
and actual ABI checks with Rust 1.85.1 and GCC 12.2.0. Both platforms passed
all 67 licensed Stata 19/MP fixtures, including the existing memory controls.

The Mac replay made 693 native calls across 77 inputs at one/two/four/eight
workers. Science, 308 fixed-worker repeats and 231 cross-worker checks passed.
The [original declaration](../tests/replay/scopes/2026-10-29-within-migration-macos-original.json)
failed exact comparison on PPML `RCERT[10]` in three inputs. This retained-bread
asymmetry magnitude can be nonzero because ordered rescaling rounds its two
divisions differently. The source-reviewed
[correction](../tests/replay/scopes/2026-10-29-within-migration-macos.json)
classifies that numerical field for all ten eligible PPML inputs and removes
five OLS `RCERT[39]` structural-zero allowances. All 77 comparisons pass a
read-only reassessment of the same transactions; the original failure remains
recorded. No scientific tolerance, model, route or protected discrete criterion
changed. Johannes authorized diagnosed harness repairs and continued
qualification; agents and the lead reviewed this declaration correction.

SCC job `8327417` passed the independent fixture and Break stages but failed
before its first replay native call: `--replay` rejects the inherited ABI-only
`FEREG_TEST_THREADS` setting. Replay-only repair `8327539` on `scc-b24` removed
that setting and passed all 693 calls under the frozen
[Linux declaration](../tests/replay/scopes/2026-10-29-within-migration-linux.json).
All 77 scientific/exact-field comparisons, repeats and cross-worker checks
passed; 58 cases remain wholly exact and 19 differ only in declared leaves.
The original failed job is retained; passed fixture/Break stages were not rerun.

Each platform passed four explicit-LSMR Break runs: OLS/PPML at one/four
workers, with two interruptions per run after a completed continuing iteration.
Actual upstream `Cancelled` traces, populated/null estimate rollback, absent
outputs, processor restoration and exact same-process follow-up fits passed.
These are small weighted categorical witnesses, with no construction-latency
or performance bound.

The separately frozen public slope fixture `3d61d4e0` passed on both unchanged
plugins; Linux job `8327553` ran on `scc-b23`. Cases 1/4 produce eight oracle
fits per platform at one/four workers with two repeats. Cases 2/3 retain eight
atomic `r(459)` rejections; four explicit-base requests retain `r(498)` refusal.
The original syntax failure and failed case-2/3 success expectations remain
recorded, including bounded diagnostics showing beta.10 also rejects both.
Johannes instructed us to preserve those rejections. The original three-case
inputs, oracles and tolerances are unchanged; the appended orthogonal fourth
case is a positive control, not an accepted estimate for either rejected case.

Two additional fixtures pass on both platforms outside the full profile:
four weighted OLS fits exercise low-memory pool limits 1/5/10 and fast mode;
eight offset PPML fits agree with Poisson references at one/four workers.
Linux job `8327607` on `scc-pg8` records 52 OLS RHS results and 28 augmented
joint-PPML frames, including eight successful strict final results, matching
the Mac path counts. This does not establish high-level PPML checkpoint,
wide-RHS Within batching, continuation or predictor coverage. The same job
compiled the source-bound C timing harness with GCC 12.2.0 and made zero native
performance calls. The record preserves exact hashes, configurations, bounds
and limitations. All successful SCC jobs have verified zero scheduler and
application exit statuses and a 16-CPU affinity cap; they make no performance
isolation claim. Independent Intel Mac runtime, Windows and package admission
remain outstanding; the completed main beta.10 comparison is described below.

### Combined candidate performance

The [compact qualification record](../bench/results/within-qualification-20261029.json)
retains all 4,988 main-panel calls, including every warmup and adverse timing,
with exact identities, order, solver rows, scientific fingerprints and decoding
instructions. Mac's 1,508 calls produce 51 passing, six inconclusive and one
regressing cell. Linux's 3,480 calls on six distinct whole-node SCC hosts produce
55 passing, two inconclusive and one regressing cell. Both panels cover the same
58 cells and use the declared max(5%, 1 ms) rule. There are no native errors,
retries or discarded timings; the scientific and discrete gates pass for the
declared scope. Six many-FE families actually execute Within; the dense three-FE
control remains on exact MAP and supplies no Within cost evidence.

Mac's sparse three-FE one-worker cell has paired median ratio `1.12666`
(raw medians 5.59002 versus 6.30906 seconds). Linux's near-confounded third-FE
four-worker cell has ratio `1.11520` and positive excess above the allowance
of 1.58–8.60 ms. Their unchanged recorded work is respectively 19 RHS/390
iterations and five RHS/71 iterations, both using additive Schwarz without
fallback. The Mac difference lies in recorded solve duration; the Linux
difference lies in setup duration. The retained traces contain no per-RHS timing,
and these observations do not attribute either regression to a specific kernel.

The Mac observer retains 47.95 sampled Claude CPU seconds as authorized desktop
noise; no measurement is excluded on that basis. Linux uses physical hosts as
independent units; Mac uses six mirrored blocks on one desktop. The one-sided
`.025` intervals are conditional on the declared unit assumptions and have no
simultaneous-coverage guarantee. Both reports remain `needs_discussion`.
[Validation](VALIDATION.md#upstream-migration-candidate) records the limitations;
neither these main panels nor independent patch improvements authorize adoption.

Johannes authorized exactly one function-level profile per frozen plugin for
each platform's regressing cell and a justified fix in the fork only. The four
completed calls pass the independent oracle, their own frozen scientific
fingerprint and recorded solver-work checks. Their summaries and profile/source
identities are in the same compact qualification record. Mac's selected
adjoint/fused/gather/scatter stack union rises from 1,661 to 2,414 samples;
the hot categorical accumulation loop retains two avoidable row bounds checks.
A private slice specialization
can preserve its reduction tree, arithmetic order and allocation sizes. Linux's
cross-tab accumulation stack count rises from 29 to 60, out of 1,722 and 1,745
total samples. Source inspection gives a correction-array allocation total of
20.88 MB for this unweighted categorical fixture; a dense-only exact-integer
path can omit them while preserving the existing dense/sparse choice. Weighted and sloped sums
still require compensation. The two fork patches and their completed Mac and
Linux public science checks are described below.

Single instrumented calls identify investigation targets, not causal effects
or performance acceptance. Mac samples include harness work and retain desktop
activity; Linux's user-space event excludes kernel and page-fault costs. The
held back-substitution patch remains excluded. A revised candidate requires a
separately declared comparison before acceptance; both completed reports remain
`needs_discussion`.

### Independent fork optimization checks

The [patch evidence](../bench/results/within-optimizations-20261029.json)
retains each frozen base, patch commit, compiler/artifact/driver hashes, full
scientific summaries and streamed field hashes. Both patches preserve arithmetic
and were tested independently; neither is in the `d3407a16` beta.11 plugin.
They are published on Johannes's fork as `codex/categorical-unit-counts` and
`codex/categorical-scatter-slices`, respectively. No upstream push or pull request
has been made; performance acceptance and actual-plugin integration remain pending.
Source review finds both deltas apply separately and together to `d3407a16`;
the existing fereg memory bounds remain conservative. This is not an integrated
artifact check or evidence that either patch recovers the recorded regression.

| Patch | Exact base / fork patch | Production scope |
| --- | --- | --- |
| Dense categorical counts | `32dde1bd` / [`ba409ada`](https://github.com/johannes-schmieder/within/commit/ba409ada2ffb27e90d19b22532c4d830a161f964) | Omit the correction vector only for dense structural Unit × Unit pairs without weights and with at most `2^53` observations. Every partial count is an exactly representable integer, so the existing compensation is zero. The dense/sparse dispatch rule and weighted, sloped and sparse arithmetic stay unchanged. |
| Categorical scatter slices | `6f2ead13` / [`1f16b437`](https://github.com/johannes-schmieder/within/commit/1f16b4376d08ebd1b99f5120b868db7262af6b84) | Check row-slice bounds once per leaf for unweighted categorical Fold terms. Preserve leaf sizes, the fixed reduction tree, addition order and allocation sizes; weighted terms, slopes and Owner terms use the existing path. |

Both Mac public panels used Rust 1.85.1 with `target-cpu=generic`, fixed inputs,
four RHSs per group and one internal repeated group, without timers or retries.
The count panel passed 24 processes: 12 exact base/patch comparisons and eight
exact one/four-worker apply comparisons. It covers cache, large dense, sparse
memory and weighted-cache designs; the largest dense table has 2,499,995 cells
and omits 19,999,960 correction bytes without changing dispatch. Full solves use
one worker because frozen base `32dde1bd` already has a recorded four-worker
LSMR repeatability failure. This limitation does not relax the combined
candidate's repeatability requirements. Per four-RHS group, both arms use
4/4/8/4 iterations for those designs; maximum independent error is `7.20e-14`.

The scatter panel passed 40 processes, 20 exact paired comparisons and eight
exact one/four-worker full-solve comparisons. Cache and 4,194,304-row cases
include weighted controls; separate weighted entrypoint cases cover zero/three
slopes and Off/Diagonal/Additive preconditioners. Maximum independent error is
`2.995e-12` in the two-FE harness and `1.231e-9` in the weighted/sloped harness,
against an unchanged `1e-8` witness bound. Their solver tolerances are `1e-11`
and the previously approved `1e-12`, respectively. The entrypoint records retain
SHA-256s for coefficients, residuals, layout, convergence and work; the two-FE
harness uses FNV64 vector signatures plus independent witnesses. Read-only
review reconstructed every comparison without native reruns.

The same public schedules passed on Linux: jobs `8339222` and `8339223` completed
24 and 40 calls, respectively, with all paired and worker comparisons exact.
Maximum independent errors match the Mac bounds reported above. Both used Rust
1.85.1, generic CPU builds and GCC 12.2.0 on `scc-tb4`, with four slots and 3 GB
per slot in shared correctness allocations. Scheduler failure/exit status was
0/0 for both; these allocations establish no timing or isolation claim.

Private tests retain independent integer/dyadic witnesses, compensation checks,
leaf-boundary cases and mixed weighted/sloped solves. The evidence distinguishes
the Mac full suites, final test-only edits and Clippy's existing lint exception.
Linux successor `8339224` passed all eight cross-tab and three scatter witnesses
on the same host, with scheduler failure/exit 0/0; it ran focused filters, not
the full suite. The independent performance results and combined source checks
below extend this evidence. These standalone
results neither establish recovered performance nor replace combined OLS/PPML,
fallback, memory, cancellation and beta.10 qualification checks.

### Independent performance and combined source candidate

The [independent performance record](../bench/results/within-optimization-performance-20261029.json)
retains all 840 Linux calls: 168 untimed host preflights and 672 timed calls,
their exact order, scientific fields, every block and host aggregate, and
source/binary/configuration/environment/accounting hashes. Array `8340554` used
six distinct whole-node hosts, `omp28` with 28 slots and `num_proc=28`; every
task ended with scheduler failure/exit 0/0. Both patch reports verified science
and isolation without native errors, retries or discarded timings. Each cell
has two mirrored blocks per host. Build and solve/apply time are protected
separately by max(5%, 1 ms); six host medians produce exact min/max bounds for
the median at one-sided alpha `.025`, without simultaneous coverage.

| Independent patch | Passed endpoints | Inconclusive endpoints | Positive lower-bound regressions | Decision |
| --- | ---: | ---: | ---: | --- |
| Dense counts | 11 | 1 | 0 | `review_required` |
| Scatter slices | 12 | 4 | 0 | `review_required` |

Dense construction's median host ratios are `0.554–0.568` in the three large
dense cells. Its sparse construction control remains inconclusive, with an
upper bound of 10.47 ms beyond the max(5%, 1 ms) allowance. Scatter's unweighted
memory solve ratios are
`0.98976` at one worker and `0.99665` at four; these do not establish recovery
of the earlier Mac regression. Three cache construction controls and the
weighted memory four-worker solve remain inconclusive; the latter's host-median
ratios range from `0.97744` to `1.11216`, with an upper bound 238.56 ms beyond
the allowance. All adverse blocks remain in the record, including
those in passed host-level intervals. Read-only reconstruction reproduced all
28 decisions from the retained timings. Neither patch is performance-accepted.

Mac completed 28 untimed science calls before each of two operational launch
failures, for 56 retained calls and zero native performance timings. The first
stopped at a denied hardware query; the second stopped at host admission after
observing three competing Stata processes. The unrelated Monte Carlo work has
not been stopped. Bound Claude activity remains part of the declared policy;
it is not an exclusion for adverse observations. Mac timing remains blocked,
and neither attempt is a performance result.

The historical [combined source record](../bench/results/within-optimized-integration-20261029.json)
binds fereg `379bf7c8` to fork commit [`4beaa8af`](https://github.com/johannes-schmieder/within/commit/4beaa8afcdc56ddee6f29b3cbd5bf151f0f2d5c5)
on `codex/beta11-candidate-optimized`. Both dependency crates passed 588
top-level tests and two isolated subprocess tests with all features on Mac
Rust 1.85.1; one fixture-regeneration test remains ignored. The first attempt
failed before tests because the new private scatter witness lacked the
existing `Cancellation` argument. A test-only default argument fixed that
integration; both production patches still match their independent versions.
The failed compile and an earlier formatter PATH repair remain recorded.
fereg's source run passed 324 Python tests, 1,537 core tests, 189 non-FFI and
219 FFI plugin tests, scientific integration and independent oracles, strict
Clippy and portable checks. The original script exited 1 at stale complexity
locations after a comment-only change; its first follow-up then rejected ten
unused historical growth entries. The approved metadata repair refreshed seven
position/comment-length values and retired those entries without changing any
measured total or limit. The final size/ratchet, mock-SPI and numeric-guard checks
passed. Both failed invocations remain recorded; no broad rerun was needed.
That combined candidate remains unaccepted. Fereg `65449fd6` restored the pin
to `d3407a16`; no optimized plugin was built at that stage. The manifest and both
lockfiles are byte-identical to tested fereg `d96eccfc`. Shipping Rust, frontend,
bridge and scripts differ only in an explanatory memory comment, so the earlier
source/native correctness evidence is reused. Locked offline metadata checks
passed for the workspace (59 packages) and fuzz workspace (64 packages) with
Rust 1.85.1; no plugin rebuild or new fereg native test was performed for the
restoration. The bounded diagnosis is complete; scatter remains held
with the cause unresolved. The standalone scatter weighted-control concern and the original `d3407a16` versus
beta.10 regressions are distinct comparisons; restoring the pin resolves neither.
At that stage, the additional strict-reference-fit budget remained unapproved.
The later approved extension is recorded below. The held back-substitution
patch remains excluded. There has
been no upstream push or PR, and these source checks do not authorize adoption.

Johannes subsequently approved testing the counts-only stack `534ea64f`.
Its parent is exactly `d3407a16`, and its two-file diff adds 18 production lines
and 124 independent test lines. The unweighted Unit-by-Unit dense path omits
compensation only when every integer count is exactly representable; weighted,
sloped and sparse accumulation and the conservative memory-admission estimate
are unchanged. Scatter is absent. Standalone patch `ba409ada` remains the
separate upstream-base comparison; its inconclusive sparse construction control
is retained and is not waived as a performance pass.

The fereg pin update changes only the manifest and two lockfiles relative to
`65449fd6` (patch SHA-256
`f97a5b31f52710cd88ac677433fbfd04fa227035c15a872a9cdbbc35454e4972`).
One `./scripts/run_tests.sh --source-only` invocation passed on Mac Rust 1.85.1:
324 Python tests, 1,537 core tests (three ignored), 189 non-FFI and 219 FFI plugin
tests, isolated allocation audits, scientific integration/oracles, strict Clippy,
fuzz compilation, portable/C checks and the complexity ratchet. The source log
SHA-256 is `23944c13168735406fbf560d9b456303fdfd5a1b598301573bbeee0dda762f38`.
The run used Python 3.12.2, four Cargo jobs, serial Rust test threads, eight
Rayon/fereg workers and single-threaded BLAS. The subsequent native refresh and
fixed 58-cell comparison are recorded below, including every previously
nonpassing cell. The large companion panel is separate from this main
comparison. A separate gather-zeroing investigation is excluded from this
fixed counts candidate.

The exact dependency stack also passed focused Mac Rust 1.85.1 checks:
325 top-level tests and one isolated subprocess, with no failures or ignored
tests. These cover cross-tab witnesses, controlled solves, cancellation, slopes,
residual output, repeatability, Schwarz and iteration budgets. Both changed
files match standalone `ba409ada` byte for byte; formatting and clean-tree
checks passed. The dependency test log SHA-256 is
`bcda6d90ef8dbf1013b9a1b39dd71f7aa4ee31644278f00ac28b7315faab0e60`.

### Counts-only native and main comparison

The [counts-only record](../bench/results/within-counts-qualification-20261029.json)
binds fereg `c1f4d5ad`, within/schwarz `534ea64f`, the actual Mac plugin
`20344bd1` and Linux plugin `226181ae`. On each platform, all 67 licensed
fixtures, four OLS/PPML iteration-Break checks, mapped memory/reuse/slope
fixtures and the 77-case, 693-call replay pass. Replay scientific results and
solver work match protected `d3407a16` exactly, with 308 fixed-worker and 231
cross-worker checks. The 24 targeted calls per platform also pass: eight
explicit unweighted Schwarz calls exercise dense counts, eight retained-CG
calls reach the generation-2 LSMR retry, and eight public Auto controls agree.
The independent LP sample remains 228 original rows. The record preserves
operational build/host-name guard failures and their narrow repairs.

Both platforms then pass the unchanged 28-call science preflight against
beta.10, with seven families, two actual binaries and one/four workers. Linux's
first preflight stopped before native execution on an environment-version
mismatch; restoring the frozen environment preserved all input and science
thresholds. These results do not waive the separately declared timing gate.

| Counts-only main panel | Recorded calls | Report-eligible calls | Unique-cell verdicts |
| --- | ---: | ---: | --- |
| Mac | 1,508 | 1,508 | 55 passed, 2 inconclusive, 1 regression |
| Linux | 2,981 | 2,900 | All 58 insufficient hosts |

The compact record retains all 4,489 calls: 713 warmups and 3,776 timed calls,
with full ordered result objects, scientific/work fingerprints, frozen plans,
source/artifact/environment bindings and report hashes. There were no actual
retries or discarded attempts. Original and incumbent again alias beta.10;
comparison rows are counted once in the table.

Mac `campaign-three-sparse-t1` has median paired ratio `1.130373`, a 13.0373%
regression. Across all 12 timed calls per arm, complete-call medians increase
from 5.294788 to 5.983382 seconds; summed solver-row solve medians increase from
4.527583 to 5.218120, while setup medians decrease from 0.151976 to 0.150099.
Both arms' warmup traces retain 390 iterations and 19 RHS solves. All six paired
block ratios lie between `1.126548` and `1.136977`. This locates the increase
in the solve phase but does not identify its cause. The two inconclusive cells
are `campaign-akm-k64-t4` and `wide-rhs-t1`. Bound Claude activity is retained
desktop noise (70.19 sampled CPU seconds); no adverse timing is excluded.

Linux array `8343686` completes five 580-call hosts. Task 1 stops operationally
after 81 recorded calls when the initial synchronous census plus the first
five-second observer wait produces a 7.843798-second process-observation gap,
exceeding the frozen 7.5-second limit. No competing process or native/scientific
failure is recorded. All 81 calls remain retained, including the last unpaired
warmup; the unchanged report excludes the entire incomplete host. Report job
`8345728` ends with scheduler failure/exit 0/0. Every five-host cell is within
the allowance descriptively, but one-sided `.025` bounds are undefined with
five independent hosts and the six-host requirement is unmet. All 116
reference-labelled rows are `insufficient_hosts`; no Linux pass is claimed.

Johannes also approved one bounded extension for the three strict large
references. Job `8342499` passes all three independent references with unchanged
scientific thresholds: sparse three-FE OLS at ten million rows, sparse three-FE
PPML at one million, and AKM PPML at ten million. It uses four workers, process
caps of 40, 30 and 20 minutes respectively, and no retries; accounting records
2,876 seconds wall time and 12.742G maximum virtual memory. Earlier timeout
attempts remain failures. These references do not execute the large companion
native or eight-call memory-admission panels, which remain outstanding.
The confirmed Mac regression and Linux uncertainty still block acceptance.

The subsequent [Mac solve diagnosis](../bench/results/within-mac-solve-diagnosis-20261029.json)
retains all four sampled call graphs from the actual beta.10 and counts-only
plugins in fixed ABBA order. Every call passes the unchanged independent oracle,
its own main-panel scientific fingerprint and discrete signature, and the
trace records 390 iterations over 19 RHS solves. Sampling is at 1 ms, with a
120-second native-call cap and zero retries. The record binds the source,
artifacts, arm64 image UUIDs, parser and full scientific result heads.

Disjoint ancestry attribution places 1,472–1,488 samples in current categorical
Fold, including descendants. Gather has 550–566 versus 545 samples; explicit
reorthogonalization has 134–140 versus 284–303. Historical fusion excluding
gather has 1,026–1,036 samples but combines update, norm and adjoint work, so it
is not a directly comparable Fold timing. Counts are samples, not milliseconds;
blocked waiting and input parsing are separated. Coalesced PC offsets have no
individual weights, both small FE terms share the same specialization, and no
hardware counters establish cache traffic or dependency stalls. These two
calls per arm localize investigation without establishing causality or recovery.

The profile led to an isolated private Fold running accumulator: initialize
from the existing coefficient sum, perform each original row addition, flush
on a level change and reload on revisits, preserving leaves and merge order.
It differs from historical run subtotals, which changed arithmetic. The second
Fold term has 215,878 leaf-bounded runs per million rows; the third has 999,052,
so frequent-transition overhead remains a required control. This experiment was
absent from the profiled binaries. The earlier Astra source review predates
these four profiles and did not review this implementation.

The separate [Fold correctness record](../bench/results/within-fold-register-checks-20261030.json)
checks commit `303ba33e` on `d3407a16`. Mac and Linux each passed 587 top-level
tests plus two nested subprocess repeats, with one existing ignored test, and
strict library Clippy. Four focused Mac witnesses also passed and are included
in the full suite. Linux job `8346716` completed on `scc-tb4` with accounting
`failed=0` and `exit_status=0`. Independent coefficient-order witnesses cover
all fixed arities, weighted/sloped and scalar fallback dispatch, run revisits,
leaf boundaries, numerical edges and panic behavior. These checks establish
arithmetic preservation; performance is evaluated separately below. Counts,
held scatter slices and gather zeroing remain separate. Correctness alone
neither demonstrates recovery of the Mac regression nor approves adoption.

The independent [gather comparison](../bench/results/within-gather-performance-20261030.json)
tests only chunk-local zeroing, `de67f402` against `d3407a16`, using matched
plugins from fereg `c1f4d5ad`. Counts, scatter slices, back-substitution and the
Fold accumulator are absent. All 516 calls are retained: 156 on Mac and 60 on
each of six isolated 28-core SCC hosts. The six cells cover the actual
one-million-row sparse three-FE public Auto fit at one/four workers, plus
4,194,304-row weighted categorical and three-slope controls at both worker
counts. Both platforms first pass the 16-process scientific preflight; measured
calls preserve those scientific fingerprints and solver work. There are no
native, scientific or observer failures, timing exclusions or timing retries.

| Primary endpoint | Mac change | Linux change |
| --- | ---: | ---: |
| Actual sparse three-FE, one worker | -0.26% | -0.45% |
| Actual sparse three-FE, four workers | +2.28% | -1.35% |
| Weighted categorical solve, four workers | -1.56% | -15.45% |
| Weighted three-slope solve, four workers | -1.59% | -15.24% |

Mac percentages are medians of six paired-block ratios; Linux percentages are
medians of six host-median ratios. The report also retains the different
12-block descriptive medians. Inference uses six physical hosts' median excess
scores, with no simultaneous guarantee across cells. All six Linux primary
endpoints satisfy the allowance, but three secondary construction endpoints
remain inconclusive and six adverse construction blocks remain included.
Mac's four-worker native fit is inconclusive, including a +5.46% block and an
upper excess bound of +8.76 ms. Its other eleven primary/secondary endpoints
satisfy the allowance. Bound Claude activity remains measurement noise.

The original Linux report retains six missing-accounting errors: each accounting
query exceeded its ten-second cap. One bounded 90-second array query recovered
all six successful records. An independently reviewed accounting-only supplement
revalidates the unchanged observations and scoring; it repeats no native calls
and leaves the original report intact. Array `8347704` ran on `scc-wf1`,
`scc-wf4`, `scc-wk2`, `scc-wk4`, `scc-xe2` and `scc-xe1`, with scheduler
failure/exit 0/0 throughout.

The paired point estimate is about 16 ms faster on the affected Mac single-worker
fit and does not resolve the earlier roughly 0.69-second increase against beta.10. Its large
Linux weighted-control gains warrant separate consideration, but do not clear
the Mac concern or demonstrate beta.10 non-regression. Gather remains outside
the combined candidate. The separate Fold accumulator comparison follows.
Beta.10 remains accepted, and these results authorize neither migration adoption
nor upstream publication.

The independent [Fold comparison](../bench/results/within-fold-performance-20261030.json)
tests only `303ba33e` against `d3407a16`, with fereg `c1f4d5ad` and no counts,
gather, held scatter-slice or back-substitution patch. Both platforms rebuild
matched arms at the same private source path with Rust 1.85.1 and identical
release flags. Mac uses Apple clang 21; Linux uses GCC 12.2.0. Both platforms
pass 16 scientific preflight processes, and all 516 comparison calls complete:
156 on Mac and 60 on each of six isolated 28-core SCC hosts. Every call preserves
its scientific fingerprint and solver work. No scientific or observer failure,
timing retry or exclusion occurs. The refreshed reference environment verifies
all 26,018 pinned files before timing.

| Primary endpoint | Mac change | Linux change |
| --- | ---: | ---: |
| Actual sparse three-FE, one worker | -14.89% | +2.96% |
| Actual sparse three-FE, four workers | -10.57% | +1.87% |
| Weighted categorical solve, one worker | -50.36% | -5.05% |
| Weighted categorical solve, four workers | -44.23% | -3.85% |
| Weighted mixed three-slope solve, one worker | -45.52% | -10.69% |
| Weighted mixed three-slope solve, four workers | -41.23% | -7.10% |

Mac changes are medians of six paired-block ratios; Linux changes are medians
of six host-median ratios. The record also retains all 12 Linux block ratios.
Linux inference uses six host-median excess scores and has no simultaneous
six-cell guarantee. Mac intervals remain descriptive, with ordinary desktop
activity and all five bound Claude processes retained as noise.

For the affected Mac T1 fit, all six blocks improve by 14.60–15.04%. Across all
12 timed calls per arm, complete-fit medians fall from 5.956652 to 5.073316
seconds, solve medians from 5.201078 to 4.313869, and setup medians remain
0.149674 versus 0.149621. Both arms retain 390 iterations over 19 RHS solves.
The 0.887208-second solve saving exceeds the historical 0.690537-second loss
as context; these separate comparisons do not establish beta.10 non-regression.
All twelve Mac primary/construction intervals pass, with no adverse blocks.

Linux does not fully qualify this generic patch. All six hosts slow the actual
T1 fit by 2.24–4.41%, although its interval remains inside the 5% allowance.
Five primary intervals pass. Weighted categorical T1 is inconclusive: five
host medians improve by 4.79–5.46%, while `scc-zj4` worsens by 11.15%, with
both blocks adverse (+11.47% and +10.82%). That host uses Gold 6132; the other
five use E5-2680v4. This association is descriptive and does not establish a
processor-level cause or justify exclusion. Its host-median excess interval is
[-0.337550, +0.180489] seconds. Sparse T4 also retains adverse blocks of
+5.05% on `scc-wo4` and +6.28% on `scc-xa2`, despite its passed host interval.
Four construction intervals pass; categorical T1 and mixed-sloped T1 remain
inconclusive. All four adverse primary and four adverse construction blocks
remain in the record. No slower host is excluded or replaced.

SCC array `8353171` runs on `scc-wo4`, `scc-zj4`, `scc-we4`, `scc-wj3`,
`scc-wf4` and `scc-xa2`; report `8353172` runs on `scc-gr4`. All scheduler
records have failure/exit 0/0. One bounded whole-array accounting query supplies
all six records, followed by one report invocation. Maximum virtual memory
ranges from 1.922G to 2.119G. Mac observation records 22.98 sampled Claude CPU
seconds, 12.8% peak summed sampled CPU and 3,428,352 KiB peak summed RSS;
these counters do not establish the absence of unsampled interference.

These six cells execute the changed Fold kernel only at arity one. The slope
term in the mixed design uses Owner; its categorical terms use Fold. The
independent correctness suite covers other arities, but Fold arities two through
four still lack performance evidence for this generic implementation. The Mac
result supports the accumulator diagnosis, while the Linux slowdowns and
inconclusive controls require discussion before proceeding. The patch remains
separate from the combined migration candidate. No integration, beta.11
acceptance or upstream publication is authorized by this comparison.

The [bounded diagnosis](../bench/results/within-scatter-diagnosis-20261029.json)
retains the restoration checks, existing control observations, generated-code
audit and exactly two profiled calls. All 60 original weighted-control calls
have identical science and work; no known competing compute was found. The
adverse hosts show temporal variation, which does not identify a cause or justify
excluding observations. Static inspection finds unchanged weighted right-Fold
instructions but real caller stack, register and layout changes; it does not
establish full compiled equivalence. This weighted categorical path supports
weighted OLS and some PPML projections, but the standalone benchmark does not
measure all weighted fereg fits or the PPML joint solver.

SCC `8341332` stopped before native work on a software-counter label mismatch.
A parser-only repair retained that failure; `8341371` then completed one call
per frozen binary on `scc-zj4`, with scheduler failure/exit 0/0 and exact science.
The base/patch totals of 15,541/15,389 userspace samples include setup, warmup
and eight RHS solves per process. Weighted Owner samples were 691/658 and Fold samples 723/657;
some parallel-helper work remains unresolved. There were no lost samples.
Instrumented final-group times were 3.15062/3.11007 seconds, so this pair on a
previously stable host did not reproduce the slowdown. It neither explains nor
clears the adverse hosts. User-filtered context-switch/migration zeros are not
proof of no scheduling activity; native resource reports show nonzero switches.
Scatter remains held, the counts control remains inconclusive, and the original
beta.11 regressions remain unresolved. No qualification or acceptance follows.

### Final bounded slowdown diagnosis

Johannes requested one final diagnostic attempt, then consolidation of existing
general improvements with clear gains and only minor regressions. The
[final diagnosis](../bench/results/within-final-slowdown-diagnosis-20261030.json)
uses the retained profiles, source and exact profiled plugins. It adds no native
calls, timing rounds or architecture-specific implementation. An independent
Astra audit and lead reproduction agree on the selected disassembly.

Loss of FMA does not explain the inspected Mac hot loops. Beta.10's 802-instruction
fused lane function contains no FMA instructions: both its affine update and
squared norm use separate multiply/add instructions. The current categorical
Fold leaf contains no multiplication at all. Historical "fusion" combined
operator passes; it did not imply fused multiply-add arithmetic. This is a
statement about the selected functions, not every instruction in either plugin.

The strongest supported Mac explanation is changed categorical accumulation.
The current leaf loads, adds and stores the coefficient for every row; the old
buffered route retained sums across repeated levels. The separate Fold patch
restores register accumulation while preserving the current addition order,
leaf boundaries and merge tree. On the affected second factor it avoids 784,122
logical coefficient loads and stores per adjoint, because one million rows
contain 215,878 leaf-bounded runs. These are source-derived accesses, not measured
cache misses or DRAM traffic. The third factor changes levels almost every row,
so it receives little traffic reduction and adds transition bookkeeping.

The Fold intervention saves 0.887 seconds in the Mac solve while leaving the
full observation vector and separate operator passes intact. This supports the
accumulation explanation without proving an exact decomposition of the original
0.691-second loss. Branching, compiler layout and lost tile locality can also
contribute; existing samples cannot separate them. Restoring the historical
arithmetic or adding processor-specific dispatch is not justified by this audit.

The original Linux setup increase was 14.9–16.7 ms across six hosts, with the
same 71 iterations over five RHS solves. Upstream's dense construction adds a
correction allocation, compensated updates and a finishing sweep even for exact
integer counts. Counts removes that unnecessary work; its independent large
dense construction ratios are 0.554–0.568. The five completed counts-only main
hosts have setup ratios 0.943–0.964 and complete-call ratios 0.967–0.981 versus
beta.10. That is descriptive recovery, not a six-host qualification pass. The
missing host and inconclusive sparse construction control remain unchanged.

Diagnosis ends at this implementation-level attribution. The retained Linux
Fold and gather regressions still govern inclusion; independent benefits cannot
be added together to claim combined beta.10 non-regression.

Independent tests to preserve include the dense QR and weighted/disconnected
families in [routing_portfolio](../crates/fereg-oracle/tests/routing_portfolio.rs),
the sealed [weak-graph decimal oracle](../crates/fereg-testkit/tests/weak_graph_decimal_oracle.rs),
[slope oracles](../crates/fereg-testkit/tests/intercept_slope_oracle.rs),
and explicit three-FE LSMR in
[prepared robust oracles](../crates/fereg-testkit/tests/prepared_robust_oracle.rs).
[Routing tests](../crates/fereg-core/src/fe/borrowed_map/routing_tests.rs)
cover Jacobi completion/escalation, seeded correction, memory refinement and
fixed-worker repeats. [PPML joint tests](../crates/fereg-core/src/ppml/categorical_lsmr/tests.rs)
cover independent augmented products, dense core-pair inverses, changed weights,
warm starts and numerical extremes; [IRLS tests](../crates/fereg-core/src/ppml/prepared_irls/tests.rs)
cover strict final truth and actual memory refusal. Main-thread callback tests
alone do not establish interruption inside LSMR; the actual plugin witnesses
above supply the completed-iteration and rollback evidence for their stated scope.

Patch-specific science and bounded performance comparisons must precede a
combined beta.11 qualification. Include large sparse and dense weighted/sloped
working sets, explicit PPML and fallback traces, Mac behavior and verified
isolated SCC allocations. Protect beta.10 under the current qualification rule;
report adverse and inconclusive results to Johannes rather than resampling or
accepting a small regression without discussion.
