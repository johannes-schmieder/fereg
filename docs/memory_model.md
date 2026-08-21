# Memory model

The Phase 0–1 `estimate_phase2_memory()` function is a conservative preflight,
not an allocator and not a proof of future peak RSS.

For `N` observations, `K` finite regressors, `G` absorbed dimensions, `C`
cluster dimensions, and pool size `P`, it includes outcome, weights, working
vector, observation map, FE codes, cluster codes, and a `K x K` cross-product.

- Fast mode additionally budgets resident raw and transformed `N x K` doubles.
- Low-memory mode budgets raw and transformed `N x min(P,K)` blocks; when
  `K=0`, the finite-column pool is exactly zero rather than one.

All arithmetic uses checked `u128`; overflow becomes an allocation error. The
future estimator must add solver, FE-level, score, PPML working-array, and saved
output allocations before treating this estimate as a user-facing preflight.
