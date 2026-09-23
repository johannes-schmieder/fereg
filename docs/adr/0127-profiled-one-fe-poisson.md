# ADR 0127: concentrated one-intercept-FE Poisson steps

Status: accepted for local development, 2026-09-23.

## Decision

After the canonical initial sample, separation and finite-rank solve, publication
PPML with one ordinary categorical intercept FE may use concentrated Poisson
Newton steps. The route belongs to the MAP owner and requires fast execution
and admission of its complete optional workspace. Synthetic no-FE constants,
slopes, multiple effects, low memory, explicit other engines and mu separation
retain their existing paths. No sample-size, benchmark-name or seed guard is used.

For prior weights q, offset o and finite coefficients b, optimize each group
intercept analytically at every trial:

    alpha_g(b) = log(sum_g q*y) - log(sum_g q*exp(o + X*b)).

Both sums use stable log scaling. Prior weights and outcome group totals belong
to the immutable prepared sample; the current mean, q*mu, centered design
moments, Hessian and Newton direction are recomputed for each step. The weighted
centered Hessian is assembled directly, avoiding subtraction of large raw
cross-products. Fixed group blocks and an ordered compensated merge make
arithmetic independent of worker scheduling. Centering first subtracts one
observed group row to limit loss from large group-specific finite components.

Solve the standardized finite Hessian system with a conservative pivot guard
and independently multiply its unfactored matrix to check the direction.
This is a step check, not a replacement for canonical rank or final acceptance.
Rank loss or another numerical refusal abandons the optional engine and refreshes
the canonical current-weight session. Domain, interruption and other errors
remain fatal. Every backtracking trial reoptimizes group intercepts and uses
the existing objective slack, bounds, step floor and halving cap. No trial is
published before successful acceptance; finite means are never clipped.

## Independent acceptance and exact final state

The canonical initial rank/status decision, full outer scores and two consecutive
convergence passes remain mandatory. The ordinary concentrated steps do not
claim to have produced a fully certified projected control matrix. Strict
canonical WLS reconstructs and independently verifies the converged point at
the current Hessian weights, including its rank and model space.

When that strict solve confirms the accepted concentrated point, retain the
accepted eta and identified finite coefficients. Do not advance to an equivalent
strict point merely because it was reconstructed by a different recurrence.
This keeps q*mu exactly unchanged, enabling the existing same-weight control
reuse at final-Hessian synchronization. The final predictor-response WLS is
still freshly solved and all projection, rank, finite solve, bread, reconstruction,
objective, score and bitwise q*mu conditions remain required.

Consistency compares canonical identified finite slots; an absorbed structural
constant is a normalization coordinate. The entire predictor must still agree.
This extends the existing ordinary specialized-engine convention to the
concentrated final checkpoint; it does not excuse movement in identified
coefficients or any observation's eta. Public normalized coefficients and saved
effects come from the existing independently certified output boundary. A changed
strict basis revokes the concentrated engine. Errors retain transactional rollback.

The public MAP row continues to identify the projection owner. On this path an
ordinary inner unit is one finite-dimensional concentrated Newton step, whereas
canonical checkpoints report their projection work. Counts from these algorithms
are not comparable to MAP sweeps without operation-level diagnostics.

## Ownership, memory and allocator evidence

The optional owner retains a selected N-by-K finite-design copy, N prior log
weights, N trial predictors, N trial means, group totals/shifts, active coordinate
identities, finite system buffers and fixed group-block moment banks. It admits
all payloads and descriptors before construction, including storage retained
through strict solving. Size or budget refusal preserves the canonical route.
Actual capacities are checked and no ordinary step grows a buffer.

An explicitly requested allocation audit also reserves a second cold owner.
After the unchanged sixteen complete-iteration measurement window, that owner
reconstructs immutable inputs and verifies the current direction, means,
gradient and Hessian against the warm owner. This is a storage-reuse witness;
independent dummy-design mathematical oracles remain separate. Capacity checks
and real allocator events both matter: equal capacities do not prove no churn.

An initial real audit detected an allocate/free event during the numerical
preparation. Diagnostic phase counters localized it; pinned Rayon/crossbeam
source supports an external task-queue block allocation explanation. Grouping
dependent numerical phases into one submitted job reduces ordinary external
submissions from seven to two. Nested phases use the same installed team.
Interrupt and SPI polling remain on the calling thread. The unchanged real
sixteen-iteration audits pass at T1 and T4 on the final artifact.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-one-fe-profile-local-2026-09-23/README.md)
records fresh public gains of 7.9–20.0%, four licensed fixtures, 134 focused Rust
tests, independent WLS and Poisson/Hessian oracles, excluded-path controls and
all rejected or corrected attempts. Extra tracked peak ranges 0.20–18.89 MB.
Every final one-FE cell still loses to strict stock fixest: ratios are 1.30–2.63.
The retained finite-design copy and duplicated canonical final work are material
remaining costs. This is an accepted foundation, not closure of the broader
0.90-times-stock objective, the known allocation inventory drift, the obsolete
hidden receipt assertion, constrained-fit failures or platform qualification.
