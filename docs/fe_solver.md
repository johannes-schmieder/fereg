# Fixed-effect kernel

Status: implemented in the public 0.5.0 OLS path; exact-SHA platform and
performance qualification remains pending.

The safe core provides:

- queue-based singleton deletion to a fixed point;
- multipartite connected-component labeling;
- one-/multiway weighted projections;
- intercept, slope-only, intercept-plus-slope, and multiple-slope local blocks;
- deterministic batched symmetric MAP, genuine weighted CG, and explicit
  `within`/IT/SQUAREM/LSMR support;
- normalized absorbed-sum and saved-FE recovery; and
- fresh basis-invariant weighted FE-span projection certificates.

Each effect stores compressed level-to-observation membership. MAP allocates
and reuses bounded projection workspaces. Rank-deficient local slope blocks use
pivoted elimination; unidentified parameter directions may be normalized, but
fitted projections must remain identified.

A solver recurrence is never sufficient evidence. For each effect-level
block `B`, the final gate computes the projection of `sqrt(W) r` onto
`col(sqrt(W) B)` with scaled, reorthogonalized rank-aware QR and measures it
relative to the outcome/residual scale. Columns are normalized before this
check, making the certificate invariant to absorbed-channel units.

Hitting an iteration limit or failing the fresh certificate is fatal. Solver
threads operate only on Rust-owned storage; no Stata SPI call occurs off the
calling thread. Independent explicit-dummy fits validate small designs,
identified combinations, residuals, absorbed sums, and normalization rules.

## Comparator-equivalent MAP and CG

The comparator-equivalent transform is one complete symmetric Kaczmarz sweep.
Unaccelerated `solver(map)` applies that transform until the independent
certificate passes. `solver(cg)` uses the positive-semidefinite complement
operator `Pi(x) = x - M(x)`, where `M` is the symmetric residual sweep:

1. `r = Pi(y)` and `u = r`;
2. `v = Pi(u)`;
3. for each RHS, `alpha = <r,r>_W / <u,v>_W`;
4. update `y = y - alpha u` and `r = r - alpha v`;
5. use the Fletcher–Reeves ratio for the next search direction.

Outcome and regressor RHSs are processed as one batch but have independent
recurrences. Categorical-only designs borrow finalized codes and membership
directly; they do not copy observation codes merely to use CG. Slope-bearing
effects use the shared owned projection design.

Nonfinite curvature, invalid recurrence values, iteration exhaustion, or an
uncertified terminal value fail closed. A finite nonpositive denominator from
loss of conjugacy restarts that RHS from its true residual; a broken RHS that
already passes its own certificate is deactivated while other RHSs continue.
The recurrence stop matches reghdfe's Hestenes energy rule: each RHS tracks the
recent `alpha * <r,r>_W` improvement relative to its remaining improvement
potential. Publication requires both Hestenes convergence and the joint fresh
FE certificate. A certificate alone cannot terminate an ordinary CG path, and
the recurrence alone can never authorize publication.

Coefficient recovery uses the same recurrence but adds a stricter terminal
condition: `max(abs(target - D alpha)) / max(abs(target)) <= 1e-10` (or the
tolerance-derived stricter value). If the ordinary Hestenes condition fires
first, recovery continues from the current CG state; it does not throw that
work away and start a second solver. This additional scan and stop rule are
used only for saved-effect recovery and do not change ordinary CG estimation.

## Current automatic policy

`solver(auto)` implements the ADR-0009 policy:

- one FE: direct one-pass demeaning and immediate certification;
- multiple FEs: at most four symmetric MAP pilot sweeps;
- return MAP if a pilot sweep certifies;
- otherwise select direct Schwarz-preconditioned LSMR when pilot contraction is
  at least 0.008, with internal tolerance
  `max(1e-14, requested_tolerance * 1e-2)`;
- use genuine CG for the remaining multiway cases or when the complete LSMR
  execution/workspace plan does not fit; and
- if auto-selected LSMR fails or does not certify, perform one visible fallback
  from the saved MAP-pilot state to certified CG within the remaining iteration
  and memory budgets.

The report retains requested solver `auto` and records used solver, pilot
iterations, pilot contraction, total iterations, stop reason, setup/solve time,
fallback, and certificate norms. There is no FE-coefficient cap. The prepared
categorical route passes the already finalized codes and memberships directly
to `within`, avoiding the former observation-scale owned-code and membership
copies. Schwarz construction and LSMR reuse those prepared structures.

The frozen choice followed calibration of direct requested/tight tolerances,
off/diagonal/Schwarz preconditioners, forced restarted CG refinements of
4/8/16 iterations, and the prior coarse-LSMR/ordinary-CG hybrid. Direct tight
Schwarz-LSMR was the surviving lowest-runtime arm; CG refinement is therefore
not a production stage. The forced-refinement test path bypasses CG's initial
certificate return and proves that the comparison arm really executes its
registered number of restarted iterations.

## Saved fixed-effect recovery

Saving absorbed effects is treated as an optional solver product, not as a
second estimation problem. In `memory(fast)`, categorical intercept routes use
the cheapest coefficient-space product available:

- LSMR retains the coefficient vectors it already solved for.
- CMG converts its exposed worker--firm PCG potentials to effect coefficients
  and combines them with the nuisance-effect Schur solution. A nonzero CG
  polish invalidates that product and deliberately triggers the fallback.
- CG carries compact FE coefficient vectors through the same alpha/beta
  recurrence as its observation-space directions.
- Irons--Tuck applies every accepted extrapolation to the parallel compact
  coefficient states.
- `solver(auto)` records its four-sweep MAP pilot and adds the coefficient
  product of the selected LSMR or CG continuation.
- unaccelerated intercept-only MAP records the group coefficients from its
  original batched `y`/`X` sweeps, then forms
  `alpha_y - sum_j beta_j alpha_Xj` after the finite coefficients are known.

This MAP coefficient accumulation removes the recovery solve completely and
was the fastest certified route at both the 30,000- and 300,000-row AKM
checkpoints. The updates are cache-local and parallel for large level tables.
Routes without a retained product, including `memory(low)` and SQUAREM, first
run one reconstruction-aware CG recovery capped at 256 iterations. If that
does not certify, intercept-only recovery performs four forward-Kaczmarz
sweeps with exact steepest-descent line searches while accumulating level
coefficients, then runs coefficient-producing LSMR only on the remaining
target. One-effect intercept recovery is direct. Slope-bearing recovery uses
strict coefficient-producing LSMR. Certified symmetric MAP remains the final
fail-closed fallback if the bounded coefficient solvers fail.

Every retained or recovered coefficient product is normalized by connected
component, scattered only for requested channels, checked against the absorbed
target in observation space, and independently subjected to the unchanged FE
moment certificate. Solver retention never relaxes the publication tolerance.

For an LSMR-routed residual, bounded fixtures must match the sealed 100-digit
oracle and large fixtures must remain within the registered envelope against a
100-times-tighter solve. Reghdfe residual distance remains a diagnostic for
this route; sample, singleton, coefficient/status, VCE, DoF, RSS, fit, FE-score,
and OLS-score contracts remain hard publication gates.

## Residual reuse and projection cutoff

Fast OLS always reconstructs the within-regression residual from the QR state.
That residual is submitted directly to the selected FE certificate/solver: an
already-within residual returns at iteration zero, while a failed certificate
triggers refinement. The raw and within residual vectors are not retained
simultaneously. The initial certificate reuses the column norms computed over
the same unchanged residual during setup, removing one redundant full-matrix
scan without reusing or weakening the independent FE-moment calculation.

Matched cutoff profiling at 32,768, 65,536, 131,072, and 262,144 rows selected
65,536 observations for parallel borrowed projections. It improved the
registered eight-thread standard CG median by more than 10 percent without a
measurable one-thread regression. Small batched RHS projections use a fused
per-group kernel, CG reuses its projection matrix, and large weighted dot
products use deterministic 32,768-row partial reductions. Final publication
still requires the complete independent certificate.
