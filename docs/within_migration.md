# Upstream within compatibility

The baseline audit compares fereg `7f56d06ee4c004741f32d4692bc0e40b898a51d7`
with upstream `py-econometrics/within` main frozen at
`32dde1bdd604f33012e592270781129e09e82a92`. The fereg
[dependency declarations](../crates/fereg-core/Cargo.toml) and lockfile pin both
`within` and `schwarz-precond` to fork revision
`3b5b9b9ab9b79e4a8cac2841f7c1d41bb2276104`. The unpublished migration candidate
is beta.11. The patch evidence below is current through October 29, 2026;
the combined fereg candidate does not yet compile. This report establishes
neither performance acceptance nor readiness for adoption or publication.

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
| Generic `PairReplacement` and reduced-inverse factory | Experimental general-FE developer routing only. Not required by public Auto, PPML, explicit LSMR or correction solves. Retirement requires approval; see below. |
| Sparse back substitution and other kernel changes | Independent performance candidates. Keep worthwhile changes in separate patches; optional speedups do not become API prerequisites. |
| Typed allocation failures | Preserve the public failure contract and admission. Frozen upstream lacks fork allocation variants; removing match arms is not evidence that the contract survives. This does not require porting every historical allocation hook. |

### Controlled starts suffice

In [general routing](../crates/fereg-core/src/fe/borrowed_map/routing_lsmr.rs),
`solve_serial_windows` finishes Jacobi sessions before `escalate` releases the
trial solver, constructs additive Schwarz and starts from each returned `x`
with its remaining allowance. No Krylov recurrence crosses that replacement.
The public `geometry_many` preset cannot build a generic pair replacement:
[`general_pair_checkpoint`](../crates/fereg-core/src/fe/routing.rs) returns
`usize::MAX` for every prefix policy.

[`general_lsmr_attempt`](../crates/fereg-core/src/fe/borrowed_map.rs) also uses
only returned coefficients for its one tighter correction after independent
certificate rejection. `FeCoefficientBatch` documents the original term/channel/
level layout. Each correction keeps the original RHS and subtracts prior work.
The adapter must preserve iteration consumption on failed attempts as well:
the current escalation error return precedes merging continued-session counts,
so this boundary deserves an explicit failure-accounting witness.

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
the pinned fork. The fork's `new_with_screening(..., Disabled)` still performs
per-term slope whitening with the `1e-10` relative Gram rank rule. A new raw
constructor is therefore not an established migration requirement. Upstream
uses compensated two-pass level moments instead of the fork's Welford moments,
and reconstructs weights from retained square roots. Weighted/sloped low bits
can change; boundary witnesses must check that the model span and discrete rank
decisions survive. Any changed rank requires Johannes's decision.

### Experimental branch requiring approval

The narrow retirement proposal is the general categorical, more-than-two-FE
pair-upgrade branch selected by the twelve `routing_h{4,8,16,32}_k{0,1,2}`
developer arms. Their two-FE CMG implementations and selectors remain useful
and would stay. The general-FE interpretation of `routing_builder_break`
(h4/k1) also needs an explicit disposition; its documented two-FE Break witness
stays. Prefix, portfolio, geometry and bounded-window selectors do not require
generic pair factories and need no retirement for this reason. No removal is
authorized by this report.

The pair factory, component CSR view and replacement owner occur only in
[general routing](../crates/fereg-core/src/fe/borrowed_map/routing_lsmr.rs).
There is no public PPML `PairReplacement` consumer. Exact continuation of a live
recurrence after optional setup is refused is tested by
`refused_pair_build_resumes_exact_incumbent_recurrence`; accepted replacement
and reuse have their own test. That historical requirement is what needs full
recurrence retention. Upstream's prepared operator and pair builders are
private, so reconstructing an equivalent factory locally would introduce a
new preconditioner implementation and still would not preserve exact
refusal/resume behavior without recurrence access. It is not a small adapter.
Useful independent projection, coefficient reconstruction, memory and dense-QR
witnesses remain even if this experiment is retired.

## Cancellation, failure and memory

Stata continues polling Break on its main thread; numerical workers observe
the host signal. All work joins before Interrupted is returned, and partial
estimates remain private. The new control must cover cold batch solves,
Jacobi trials, additive solves, construction and PPML's direct `mlsmr` calls.
The baseline [`categorical_lsmr`](../crates/fereg-core/src/ppml/categorical_lsmr.rs)
error mapper converted non-allocation errors to NonConvergence, which strict-final
IRLS may catch for a canonical retry. The candidate now calls the cancellable
constructor and low-level solve and maps Cancelled to Interrupted before that
catch. Complete fereg Break/rollback validation is still outstanding.

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
This source/allocation evidence is distinct from behavioral tests of fereg's
revised formulas, which remain blocked by the incomplete integration build.
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

### Patch evidence and unresolved results

The separate API patch is `cf228c412fdadac89a3ff28b12f53b7e1580f3de` and the
separate cancellation patch is `b596c9854ea078dc40e903300d66a54d6062fcf0`, each
on frozen main. Both patch suites passed on Apple Silicon with Rust 1.85.1;
the cancellation log records 548 passing tests including four doctests, with
one existing ignored test. The latter
adds nine cancellation tests covering joined work and distinct cancellation,
without changing existing `MlsmrOptions` struct literals. The combined upstream
stack `41c53760` on draft #478 passed 574 Mac tests with one ignored test and
independent high-effort review. These are upstream patch checks, not a working
or accepted beta.11 plugin.

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
exit 0. No performance timing has been collected.

The separate back-substitution patch is `3303a6e`; its common benchmark harness
is frozen at base `846d84d` and patch `5f31a1a`. On Mac, all six large-case
base/patch comparisons agreed exactly for unit/weighted direct application at
one/four workers and complete solves at one worker. The design has 8,388,608
rows and a 360,718,788-byte factor/RHS lower bound; maximum independent fitted
error was `9.094502928519432e-12`. Four-worker complete solves are excluded
prospectively from timing because the frozen base failed repeatability there;
that failure is retained. The fixed microbenchmark schedule has six mirrored
blocks per cell, six distinct SCC physical hosts, no retries and no exclusions.
It reuses the maintained qualification statistics and host observation tools;
it does not replace beta.11 qualification against beta.10. Mac host preflight
failed with zero benchmark calls because the sandbox denied process observation.
Permission for a preflight retry is pending; there are no Mac timing results.
The Linux release build completed in job `8325815` on `scc-eb1`, with source,
compiler and binary identities checked. Its science-only preflight passed in
job `8325890` on `scc-222`: 24 calls across all 12 cells, exact base/patch
agreement, and maximum independent error `9.094502928519432e-12`. The reused
Python environment's PyFixest/within-py versions and PyFixest commit, plus the
recorded Julia pins, matched the current campaign declaration. No Linux timing
array has been submitted.

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
failure; equality to the baseline is not a scientific pass. A single diagnostic
at tolerance `1e-12` has been proposed to Johannes but has not been approved or
run. The `1e-8` witness and original failed observations remain unchanged.

The fereg integration check still reports 21 compile errors for preserved
optional pair/session APIs. The experimental-retirement question remains
pending; no paths or selectors have been removed. Actual beta.11 plugin tests,
full memory-admission checks, combined fereg scientific results, Mac/SCC performance against
beta.10 and adoption review therefore remain outstanding. No upstream PR
submission or production acceptance is claimed by these checks.

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
alone do not establish interruption inside LSMR. Actual plugin replay and
licensed Stata Break/rollback, slope and explicit-solver checks remain necessary.

Patch-specific science and bounded performance comparisons must precede a
combined beta.11 qualification. Include large sparse and dense weighted/sloped
working sets, explicit PPML and fallback traces, Mac behavior and verified
isolated SCC allocations. Protect beta.10 under the current qualification rule;
report adverse and inconclusive results to Johannes rather than resampling or
accepting a small regression without discussion.
