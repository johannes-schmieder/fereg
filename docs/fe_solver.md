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

## Current automatic policy

`solver(auto)` implements the frozen bounded policy:

- one FE: direct one-pass demeaning and immediate certification;
- multiple FEs: at most four symmetric MAP pilot sweeps;
- return MAP if a pilot sweep certifies;
- otherwise select LSMR only when pilot contraction is at least 0.008 and the
  total FE coefficient count is at most 65,536;
- use genuine CG for all other multiway cases; and
- if auto-selected LSMR fails or does not certify, perform one visible fallback
  to certified CG within the remaining iteration budget.

The report retains requested solver `auto` and records used solver, pilot
iterations, pilot contraction, total iterations, stop reason, setup/solve time,
fallback, and certificate norms. The coefficient cap is a scientific guard:
on larger weak graphs, current LSMR residual representations can pass FE
moments while missing the frozen observation-residual parity gate. Explicit
LSMR and its preconditioners remain available as diagnostic/escape-hatch
choices. At 100k the registered low-mobility case selects LSMR; its 1m and 10m
counterparts exceed the cap and select CG.

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
