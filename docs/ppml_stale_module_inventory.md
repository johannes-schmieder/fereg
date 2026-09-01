# Stale PPML module disposition

This inventory binds the pre-migration PPML source at F0 candidate
`2dee5d8c4cd99b51bbaca600348f66f325b58ab4`. It prevents an old implementation
detail from becoming public policy merely because it already compiles. The
governing replacement contracts are ADRs 0015--0019.

## Disposition labels

- **Retain after audit:** the algorithm and implementation shape may remain
  after focused correctness, allocation, and independent-oracle tests.
- **Adapt:** preserve a narrow mathematical idea or data type, but change its
  inputs, ownership, storage, or certification to the new architecture.
- **Oracle-only:** keep as a differential or negative-test implementation; it
  cannot execute in a production estimation route.
- **Rewrite:** replace the production implementation while preserving useful
  fixtures and separately verified formulas.
- **Delete after replacement:** remove the duplicated production path once its
  shared replacement is exact-green.
- **Quarantine:** keep unreachable from default/public estimation until the
  named scientific and operational gates pass.

No stale module is accepted as public evidence by source presence or its
existing unit tests.

## `ppml` estimator modules

| Source and symbols | Disposition | Replacement and gate |
|---|---|---|
| `ppml/mod.rs`: `PpmlInitialization`, `PpmlConfig` | Rewrite | Replace arbitrary bounds and OLS-owned policy fields with the ADR 0016 initialization, convergence, PPML rank/DoF/VCE, route, and generation contracts. |
| `ppml/mod.rs`: `PpmlStart` | Oracle-only | The pre-prepared warm-start carrier is now `cfg(test)` and crate-private, with no crate-root re-export. Any future production warm start must instead bind the final sample, topology, basis, offset, and weight semantics. |
| `ppml/mod.rs`: iteration, diagnostic, fit-statistic, and result structs | Adapt | Preserve useful diagnostic concepts, but separate certified weighted-linear products from PPML results and align public fields with the independent PPML wire. Do not expose stale OLS DoF/VCE types as policy. |
| `ppml/mod.rs`: `fit_ppml`, `fit_ppml_with_start` | Rewrite | Replace orchestration with preparation -> certified separation -> fresh session -> buffered IRLS -> strict final solve -> PPML inference/output. The old final profiling pass and accepted-state mismatch are forbidden. |
| `ppml/family.rs`: `PoissonObjective`, deviance and log-likelihood formulas | Adapt | Preserve independently checked Poisson identities. Reimplement with reusable buffered/compensated stable eta-form sums, exact zero handling, fweight replication, and normalized pweights. |
| `ppml/family.rs`: `PoissonBounds`, `means_from_eta` | Rewrite | Delete the fixed `[-29.9,29.9]` acceptance window. Compute representable positive means without clipping and let line search reject nonfinite states. |
| `ppml/family.rs`: `offset_from_exposure` | Retain after audit | Keep the positive-domain/log transform only after it consumes prepared missing/domain decisions, reports the registered error, uses fallible output storage, and has offset/exposure invariance tests. |
| `ppml/family.rs`: `pairwise_sum`, `log_gamma`, scalar objective helpers | Adapt | Reuse only formulas verified against the independent oracle. Replace per-call temporary vectors and unbounded recursion with the shared deterministic reduction/buffer discipline. |
| `ppml/certificate.rs`: `PpmlCertificate`, `score_certificate`, `add_relative` | Adapt | Keep the finite and FE score equations and explicit norms. Route basis access through prepared recipes/topology, use reusable scratch and shared deterministic reductions, and certify the exact final score `q(y-mu)` independently of production moments. |
| `ppml/irls.rs`: `validate_inputs` | Rewrite | Validation belongs to PPML preparation and the stable sample ledger, before session construction. It must implement ADR 0016 source order, weight kinds, offset/exposure behavior, structural constant, and original-row errors. |
| `ppml/irls.rs`: `validate_start`, `initialize` and log-weighted helpers | Rewrite | Implement the offset-aware log-sum-exp intercept start and generation-bound warm-start validation. Do not detect an arbitrary constant column as a substitute for the structural constant. |
| `ppml/irls.rs`: `profile_irls_step` and `ProfileStep` | Rewrite | Replace owned N-by-(K+1) construction, repeated column copies, legacy absorption, and legacy rank solve with explicit borrowed response/Hessian-weight frames in the reusable certified-linear session. |
| `ppml/irls.rs`: `accept_step` and `AcceptedStep` | Adapt | Retain the idea of joint eta/coefficient backtracking. Use preallocated candidate buffers, the ADR 0016 objective rule, no clipping, registered slack, and strict representability/certificate failure. |
| `ppml/irls.rs`: `scheduled_inner_tolerance` | Adapt | Make scheduling depend deterministically on outer deviance/score progress and the PPML route contract. It cannot weaken the final strict tolerance. |
| `ppml/irls.rs`: `multiply`, weighted norm helpers, `matrix_without_first_column` | Delete after replacement | Use prepared recipes, shared moment/projection kernels, and reusable output buffers. These helpers copy dense data or duplicate certified reductions. |
| `ppml/irls.rs`: `weighted_outcome_sum` | Adapt | Fold into checked PPML sample/fit summaries with fweight effective-N semantics and normalized-pweight invariance. |
| `ppml/vce.rs`: `PpmlVceInput`, `compute_ppml_vce`, meat, cluster, sandwich, selection, and symmetry helpers | Delete after replacement | Feed ADR 0016 PPML Hessian and score multipliers into the shared prepared sandwich kernels. The stale code squares fweights in robust meat, limits/handles clusters differently, duplicates dense products, and uses the wrong correction policy. Preserve only negative regression fixtures that demonstrate those defects. |

## `ppml_separation` modules

| Source and symbols | Disposition | Replacement and gate |
|---|---|---|
| `ppml_separation/mod.rs`: method, status, reason, certificate, and report types | Adapt | Preserve the useful vocabulary but map reasons into the authoritative prepared sample ledger. Add stable original-row proposals, generation identity, no-separation evidence, and certificate norms required by ADR 0017. |
| `ppml_separation/mod.rs`: `SeparatedPpmlResult` | Delete after replacement | Separation is a preparation phase of the new PPML driver, not an estimator wrapper around the stale `PpmlResult`. |
| `ppml_separation/fe.rs`: `zero_only_fixed_effect_levels` | Retain after audit | Retain the exact intercept-FE criterion after moving it to prepared topology, excluding slope-only channels, adding fweight multiplicity tests, and returning sorted stable row IDs without mutation. |
| `ppml_separation/ir.rs`: `IteratedRectifierConfig`, `iterated_rectifier` | Oracle-only, then rewrite | Existing fixtures remain useful. Production requires the ADR 0017 immutable proposal interface, restart/fixed-point semantics, strict nonconvergence policy, reusable projections, and an independently recomputed span/sign/margin certificate. |
| `ppml_separation/projection.rs`: projection types, `project_onto_model`, weighted norms | Rewrite | Use the shared certified weighted-linear projection/session and its memory planner. Remove the separate legacy FE absorption and finite-rank implementation; preserve old cases only as differential tests. |
| `ppml_separation/simplex/mod.rs`: workspace planner, simplex kernel, and certificate helpers | Rewrite | The prepared production coordinator now supplies one bounded explicit design spanning every finite column and finalized FE intercept/slope channel. Checked joint design/kernel preflight precedes its N-by-P and tableau allocations; the detector returns an immutable proposal and the coordinator independently recertifies it against the prepared full-model span. This implementation status does not substitute for the exact-source activation evidence in ADR 0017. |
| `ppml_separation/simplex/nullspace.rs` and `tableau.rs` | Retain after audit | These are now fallible production internals of the bounded full-model detector. Checked dimensions and reservations, scaled complete-pivot null-space construction, deterministic Bland pivots/tie-breaking, nonfinite/zero-pivot rejection, iteration limits, and the joint tableau-memory bound are covered by focused tests. They remain detector internals rather than a general fitting route. |
| `ppml_separation/simplex/tests.rs` | Retain as tests | The primer and full-rank negative cases remain. Production coverage also exercises prepared finite/FE and absorbed-slope construction, overflow and allocation injection, deterministic restart order, and constrained-memory behavior. The complete FE-only/mixed-model, degeneracy, independent explicit-design cone, and pinned-comparator evidence required by ADR 0017 remains an activation gate. |
| `ppml_separation/mu.rs`: `detect_mu` | Quarantine | `mu` remains default-off. Production use requires a fitted generation-bound start, a certified recession direction rather than low-mean evidence, atomic full-session invalidation, and a complete separation/fit restart. |
| `ppml_separation/runner/accounting.rs`: validation, masks, singleton and reason application | Rewrite | Make the prepared sample ledger the only mutator. Singleton pruning uses prepared topology and literal fweight multiplicity. Proposal validation/union, reason precedence, cascade, compaction, and topology rebase occur atomically. |
| `ppml_separation/runner/detect.rs`: `detect_separation` | Rewrite | Replace the one-pass mutable sequence with the authoritative restart-to-fixed-point coordinator in ADR 0017. Every direct drop triggers singleton/exact-FE follow-up, atomic rebase, and a restart from method one. |
| `ppml_separation/runner/fit.rs`: separated fit wrappers | Delete after replacement | The new PPML driver owns the single final prepared sample and starts a fresh certified-linear session after separation. A wrapper that subsets dense inputs and calls the stale fit cannot remain a production route. |
| `ppml_separation/subset.rs`: `SubsetProblem`, matrix/FE/vector copying | Delete after replacement | Replace with stable prepared views and one atomic ledger compaction/rebase. Repeated dense subset copies violate the fit-lifetime memory model. Keep only small test helpers outside production if an independent oracle needs them. |

## Crate exports and removal order

The legacy crate-root exports for direct PPML, `PpmlStart`, separation
detection, and separated-fit wrappers have been removed. Their retained oracle
implementations are exact-`cfg(test)` crate internals. The plugin calls only the
new prepared PPML transaction; two production PPML engines are forbidden.

Removal proceeds in dependency order:

1. establish PPML preparation, oracle fixtures, and the shared linear frame;
2. replace profile/IRLS orchestration and separation projection/coordinator;
3. route inference through the shared sandwich engine;
4. route the hidden plugin action through the new driver;
5. delete dense subset, legacy rank/absorption, legacy VCE, and wrapper paths;
6. keep the exact-SHA structural source contract green. It verifies the exact
   test-only module/item inventory and scans every non-test Rust token in both
   crates, rejecting ungated oracle modules, legacy symbol references
   (including wrapper calls), alternate `#[path]` aliases, or dynamic
   `include!` edges that could conceal a duplicate WLS, VCE, or separated-fit
   route. Negative mutation tests remove `cfg(test)` and add an indirect wrapper.

At every step, stale unit tests may be retained only when their expected value
agrees with ADRs 0016--0018 or is clearly labeled as negative/oracle-only
evidence. An old test expectation cannot override the accepted contract.
