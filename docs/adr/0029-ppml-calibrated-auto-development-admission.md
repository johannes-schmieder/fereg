# ADR 0029: calibrated PPML auto development admission

**Status:** Accepted for development; formal qualification remains separate
**Date:** 2026-09-08
**Decision owner:** Johannes Schmieder

## Context

Johannes requested a fully implemented automatic PPML path after the frozen
calibration and the independent validation of the weighted-assembly repair.
The pre-holdout policy selected MAP for one effect and LSMR for admitted
multi-effect designs, with no MAP or CMG exception region. The repaired-source
bounded campaign certifies every selected route, but 21 thread scopes on nine
datasets lack completed MAP/reference comparisons. That evidence does not
retroactively pass the original calibration or adjudicate its known
coefficient discrepancy.

## Decision

1. Auto selects LSMR for exactly two or three categorical intercept-only FE
   dimensions when fast-memory execution and the complete specialized workspace
   are admitted. Zero or one effect, more than three effects, absorbed slopes,
   low-memory execution and the warmed allocation-audit surface select MAP.
   Categorical interactions remain eligible categorical dimensions. No CMG
   region or holdout-derived threshold is introduced.
2. The memory execution route remains fixed by generation-zero preflight.
   Before any specialized solver construction, the final topology's complete
   ordinary and checkpoint workspace must fit that plan. Auto selects MAP if
   this additional modeled workspace does not fit; an explicit LSMR request
   still fails. A modeled admission veto is not numerical fallback. Actual
   allocation, interruption, invalid-data and internal errors remain fatal.
3. Once admitted, auto executes the same ordinary engine, active-basis policy,
   canonical checkpoint projections and exact final-Hessian synchronization as
   explicit LSMR. K=0 uses the existing fused engine identity; positive active
   finite width uses the joint engine. Separation and Mu work remain MAP-owned.
   Auto selection changes no publication or rank tolerance. The common PPML
   checkpoint refinement described below strengthens internal projection
   accuracy after a demonstrated publication-profile rejection.
4. Requested auto, selected/effective family, concrete engine, checkpoint,
   retries and fallback remain visible through the existing result protocol.
   Both native and Stata publication checks reject auto-to-CMG and unresolved
   auto identities. Explicit solver behavior is unchanged.
5. Development admission separates unavailable comparator timing from
   independent scientific evidence. A prospective process timeout supplies no
   per-fit timing bound and no successful comparator witness. The original
   development adjudicator, ADR 0026 objective policy and all historical
   receipts remain unchanged. A new admission receipt must distinguish full
   comparator parity from independent candidate validation; neither is called
   formal qualification.
6. Hard-case independent validation must bind the published coefficient vector
   to the returned predictor, replay the objective and full finite/per-FE
   score, preserve the entire generated sample, and test exact-source
   stability under tighter outer/score convergence. The existing free-beta
   model-space projection alone cannot establish the coefficient binding.
   Small independent explicit-dummy rank, covariance and sample oracles remain
   mandatory. Additional independent Hessian/projection/covariance evidence is
   required before accepting an alternative to a missing comparison.
7. A tighter outer tolerance is not itself a tighter checkpoint solve:
   the public 1e-8 and 1e-10 outer tolerances share the initial 1e-12 strict target.
   Stability and runtime stationarity do not provide a conditioning-independent
   coefficient-error bound. Known completed scientific disagreements require
   explicit adjudication, not timeout reclassification.
8. Activation requires focused routing, memory, protocol, atomicity,
   prediction/saved-effect and independent science tests, followed by bounded
   exact-candidate validation of the real automatic route. Until those receipts
   exist, this implementation is a development candidate, not an accepted
   calibrated default or release. No old measurement is rebound to this source.
9. Johannes authorizes a prospective independently proven reference-accuracy
   exception for finite coefficients and control covariance. Preserve the
   stricter scientific-contract envelopes: coefficients rtol1e-7/atol1e-10,
   covariance rtol1e-6/atol1e-10. A source-bound independent enclosure of the
   mathematical optimum must prove that every candidate coordinate is inside
   its envelope. Every pairwise discrepancy requiring an exception must also
   have a reference coordinate proven outside its optimum envelope, allowing
   printed-reference uncertainty. An unresolved enclosure fails closed.
   Fixed-returned-mean covariance accuracy alone is insufficient: propagate
   certified full-model predictor error to optimum weights, FE projections,
   Hessian and robust meat. This exception changes no production certificate
   or publication tolerance. Sample, rank/status, DoF, objective, routes and
   all other requirements remain mandatory; unavailable diagnostics remain
   explicit. The historical v5 covariance atol1e-9 is not used for this new
   admission. This authorization does not itself establish acceptance evidence.

## Independent optimum-accuracy development adjudication

The common PPML solver now refines its fit-owned checkpoint target after a
certified, unchanged-basis checkpoint fails the publication-profile movement
test. Warm residual certification can otherwise retain the old canonical
profile and repeatedly undo a more accurate ordinary step. The next strict
checkpoint uses a target 100 times smaller, bounded below by 1e-14 without
ever loosening an already-smaller target. Final-Hessian synchronization uses
the same refined target. This strengthens the internal recurrence and FE
projection gate; outer convergence, score, objective, rank and public
certification envelopes remain unchanged. Refinement is counted with the
next executed checkpoint's retries and separately identified in diagnostic
traces. Canonical transactions, specialized seed invalidation and two
consecutive ordinary convergence checks remain required. This is a common
numerical repair, not a new automatic routing region. The focused exact-case
Linux retest and repaired-source guardrails pass.

The bounded two-FE accuracy certificate in `scripts/ppml_coefficient_accuracy.py`
is an additional candidate-accuracy instrument, not a replacement for runtime
certificates, reference objective validation or the old strict-reference
adjudicator. Its mathematical state has exactly the published finite beta and
independently computed FE potentials. Sixty-digit outward Decimal arithmetic
encloses that state's predictor and exponential mean.

For a connected incidence graph, a spanning-tree flow carrying the fresh FE
normal-equation residual bounds its inverse-Laplacian energy by the sum of
squared edge flows divided by edge weights. Apply this both to the full FE
score and to each independently projected control. If their upper bounds are
`L_FE^2` and `epsilon_j^2`, respectively, the true finite within-Hessian is
`S = R' W R - E' W E` with `||E' W E|| <= sum(epsilon_j^2)`.
An outward Gershgorin bound, including Gram arithmetic, mean and residual
rounding uncertainty, supplies a positive lower bound `m_S` for its curvature.
The implementation checks the finite FP64 range assumptions behind its
dot-product envelope; unsupported arithmetic or nonpositive curvature fails.

With an interval bound on `g_hat = R' (y-mu)`, the full Newton decrement obeys
`lambda^2 <= L_FE^2 + (||g_hat|| + epsilon_F*L_FE)^2 / m_S`.
When `lambda / sqrt(min(mu)) < log(2)/2`, exponential curvature and convexity
give `|beta_j-beta_j_optimum| <= 2*lambda/sqrt(m_S)` for every finite control.
The certificate requires this radius below the existing absolute coefficient
tolerance `1e-10`; it never increases a comparison tolerance. The direct
banded solve is only a producer of approximate potentials: the independent
interval residuals certify its errors afterward.

Before using this result to adjudicate a known coefficient difference,
the receipt must bind original data, candidate coefficients, approximate
potentials, tree edges, proof source and arithmetic dependencies. The pinned
reference must be bound to the same original data and its reported objective
pair must remain valid under ADR 0026. Covariance differences require the
separate optimum-covariance proof in decision 9; all other mismatches remain
fail closed. Compare the reference coefficients against the certified optimum
intervals with the registered coefficient envelope. Record unavailable
reference diagnostics explicitly; this cannot turn an unfinished historical
strict-reference fit into a completed fit or make an old campaign pass.

This development adjudication path requires source-bound execution and
admission evidence. A standalone successful coefficient bound does not itself
activate auto or establish formal qualification.

`compare_optimum_intervals` separately verifies coordinate-wise inclusion of
the candidate and exclusion of a discrepant reference. Its result never grants
campaign admission on its own: the caller must validate proof provenance,
common original data, the complete independent candidate audit, and all
non-excepted scientific gates. Comparing two rounded centers or merely finding
both estimators close to a wide interval is not an admissible exception.

The full-model Newton ball also bounds each feasible predictor coordinate by
`2*condition_upper`. Add the outward-enclosed distance between public log means
and the independently reconstructed feasible predictor to obtain `delta`, a
bound on distance from public log means to the optimum. This reconstruction
term is mandatory; the public means are binary observations, not assumed exact
model-space values.

For fixed-public-mean profiled Hessian `H_p`, robust meat `M_p`, and control
residuals `R_p`, let `h >= ||H_p||_2`, `m >= ||M_p||_2`,
`C = max((y-mu_p)^2/mu_p)` and `U = max(mu_p)`. With upward bounds
`t = exp(delta)-1`, `kappa = t*exp(delta)`, weighted minimization gives
`exp(-delta) H_p <= H_opt <= exp(delta) H_p`. FE projection drift obeys
`||sqrt(W_p)(R_opt-R_p)||_2 <= kappa*sqrt(h)`. Therefore add
`t*h` to the fixed-mean Hessian error and `2*sqrt(m)*s+s^2` to the meat error,
where `s = sqrt(h)*(kappa*sqrt(C)+t*(1+kappa)*sqrt(U))`.
The latter includes both projection and outcome-residual drift.

The covariance certificate uses fresh fixed-mean tree energies and guarded
Gram rounding, then outward small-matrix inverse/perturbation calculations.
For nonsymmetric approximate inverses it bounds operator norm by the smaller
of the Frobenius bound and `sqrt(norm1*normInf)`, never by row norm alone.
All logarithms, exponentials, square roots, matrix products and scalar bounds
are outward enclosed. The receipt binds the coefficient/predictor proof to
the identical original inputs, published coefficients and means used by the
optimum-covariance proof. Unsupported topology/arithmetic, unresolved curvature,
input/source changes or an insufficient enclosure are fatal to this exception.

## Development admission evidence

The admission record is
`benchmarks/reports/development/ppml-auto-development-admission-2026-09-08.json`.
It combines the frozen policy, complete baseline actual-auto independent
coverage, the common checkpoint-cycle repair and four repaired-source Linux
scopes, and the repaired universal Mac routing/output/transaction checks.
This is explicitly development admission, not an exact repaired-source rerun
of every historical scope or formal cross-platform qualification.

Eighteen hard scopes pass comparisons with complete pinned reference records.
One has the source-bound optimum-accuracy exception above. Two share a dataset
with no complete reference record; their independent candidate audit and
coefficient/covariance optimum inclusion supply the additional evidence in
decision 6. This does not assert that the unavailable reference was inaccurate.
Truncated matrix rows are unavailable evidence, not numerical discrepancies.
Complete earlier rows in a failed campaign can be prospective scientific
witnesses when bound to preserved inputs and the reference closure; they do
not pass that campaign or become accepted timing observations. The four
subsequent strict-reference timeouts remain failed attempts. No comparison,
publication or runtime-certificate tolerance is relaxed.

## Relationship to prior decisions

This decision supersedes the MAP-only auto
restriction in ADR 0025 decision 2 and ADR 0027 decisions 1 and 3. The latter's
independent canonical checkpoint authority applies identically to admitted
automatic and explicit LSMR. Every other scientific, architecture, explicit
solver, memory and failure-atomicity requirement remains in force.
