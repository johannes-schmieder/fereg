# Memory model

The `estimate_phase2_memory()` function is a conservative preflight, not an
allocator and not a proof of current or future peak RSS.

For `N` observations, `K` finite regressors, `G` absorbed dimensions, `C`
cluster dimensions, and pool size `P`, it includes outcome, weights, working
vector, observation map, FE codes, cluster codes, and a `K x K` cross-product.

- Fast mode additionally budgets resident raw and transformed `N x K` doubles.
- Low-memory mode budgets raw and transformed `N x min(P,K)` blocks; when
  `K=0`, the finite-column pool is exactly zero rather than one.

All arithmetic uses checked `u128`; overflow becomes an allocation error. The
future estimator must add solver, FE-level, score, PPML working-array, and saved
output allocations before treating this estimate as a user-facing preflight.

## Numeric sample-preparation allocation discipline

Numeric preparation checks aggregate output dimensions before allocation and
uses `try_reserve_exact` for every output vector before pushing any elements:
the original-length reason mask, active-row map, outcome, outer regressor table,
each compact regressor, and any supplied weight or offset/exposure column.
Absent weights and adjustments allocate no pseudo-columns. Zero-length buffers
take the same fallible reservation path.

Every reservation failure maps to `ErrorCode::Allocation` and Stata return code
909. An injected reservation gate tests every reservation point and ensures that
no partial result is returned. The public retained-row and drop-reason accessors
borrow existing storage and do not allocate.

This proves fallible allocation behavior for this bounded numeric output only.
It does not certify the existing Phase 2 memory estimate, observed peak RSS,
SPI-loading buffers, factor/interaction storage, FE or cluster encodings, or any
general solver/estimator workspace. Those remain later memory gates.

## Borrowed intercept MAP allocation discipline

The core-only strict borrowed MAP path adds no observation-scale FE storage and
does not allocate an unweighted pseudo-column. It consumes the caller's N x K
RHS matrix and borrows prepared codes and level membership. Its six O(K)
buffers hold reference norms, residual norms, RHS scales, value scales,
compensated group sums, and corrections. Every buffer uses
`try_reserve_exact`, checks capacity before resize, and is allocated before the
first numerical mutation. Injected failures and undersized reservations cover
all six points and map to `ErrorCode::Allocation`/Stata 909. Projection and
final certification allocate nothing.

This does not yet cover fallible construction of the estimation RHS matrix,
general slope-capable solvers, OLS rank/VCE workspaces, or SPI result buffers.
