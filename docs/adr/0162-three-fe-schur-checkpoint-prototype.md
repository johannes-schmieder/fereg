# ADR 0162: select centered Schur checkpoints for production integration

Status: experimental design selected; production adoption pending, 2026-09-24.

## Decision

Advance a three-categorical-FE checkpoint solver that eliminates the largest
partition, constructs the remaining current-weight Schur matrix once, and solves
each RHS independently with diagonally scaled CG. Reconstruct from original
inputs and FE coefficients, then require the existing full original-weight
certificate. Initial and final canonical rank, finite solve, score, objective,
reconstruction and Hessian authority remain with their existing owners.

The [source-bound experiment](../../benchmarks/reports/development/ppml-three-fe-schur-prototype-local-2026-09-24/README.md)
supports this organization: strict frozen checkpoints improve about three- to
sixfold, and three public development targets improve 13–33%. This is a
selection for further implementation, not an accepted automatic route or a
completed performance goal. Production remains the computation at `f6ed8db6`.

Reject the competing coefficient-space GS/Irons–Tuck prototype. It ties the
unbalanced case, loses on the other dense cases and exhausts its 256-sweep probe
cap on the sparse case. Do not tune its acceleration thresholds. Its evidence
does not reject coefficient methods generally: the Schur alternative changes
the operator's computational cost, not just its recurrence.

## Mathematical organization

With A the eliminated indicators and B the other two partitions, prepare

```
S = B' W B - B' W A (A' W A)^-1 A' W B
r = B' W y - B' W A (A' W A)^-1 A' W y.
```

Each group contributes a weighted covariance to S. Assemble those contributions
locally. Subtracting accumulated global matrices manufactured a small negative
diagonal for a structurally zero direction in the sparse probe. The successor
recognizes group-constant indicators from membership and accumulates their exact
zero covariance, with stable `t * ((mass - t) / mass)` diagonals elsewhere.
It does not clip negative diagonals or impose an artificial identifying gauge.
Additional null directions remain permissible; only the projected span matters.

Iteration operates on the reduced matrix rather than observations. Weighted
couplings and diagonal scales are immutable within a call; RHS sums and Krylov
state are column-specific. No weighted state has cross-frame authority. Current
original-input residual certification remains independent of the matrix and its
recursive residual. Fresh certificates and two separate explicit weighted-design
oracle implementations validate the successful frozen solves.

## Requirements before adoption

The isolated public prototype deliberately lacks production memory admission,
checked size arithmetic and fallible reservations for all buffers. Its work cap,
error classification, rejected-work accounting and interruption behavior also
need integration with the existing contracts. It must not be distributed or
installed as the production plugin.

Admit the full overlap of the quadratic reduced matrix, its construction scratch,
sparse couplings, worker state and output banks before allocation. Use structural
work/memory estimates to refuse unsuitable large reduced systems; do not fit
guards to the benchmark cases. Preserve affordable existing checkpoint solving
after optional refusal and untouched inputs after numerical rejection. Allocation,
domain and interruption failures keep their established fatal behavior.

Protect the existing component-scaled path. The public prototype only attempts
the new route when the existing component-scaling helper returns no alternate
weights; disconnected/extreme-weight integration requires its own evidence.
Test changing weights, structural null directions, zero RHSs, tiny tolerances,
work caps, refused budgets and transactional failure. Then run independent
Poisson/Hessian/sandwich fixtures and predeclared fresh public rotations.

Excluded controls do not yet establish universal non-regression. Narrow two-FE
medians lose 41→48 ms in the pilot and 45→48 in a five-pair follow-up; disconnected
two FE improves 69→68. OLS exports are exact, but one categorical run takes 143 ms
against the usual 17–18 ms. Preserve those observations and resolve the relevant
performance boundary with the production integration. No threshold tuning or
claim of a repaired OLS regression follows from this experiment.
