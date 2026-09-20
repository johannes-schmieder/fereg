# Raw-moment preparation overlap and exact unit mass — local Mac development

Five fresh rotated primary pairs reduce the narrow command median from 164 to
155 ms, versus pinned fixest at 144 ms. Wide medians are 439 to 431 ms, versus
fixest at 467 ms. The overlap-only intermediate measures 160/432 ms. The narrow
5% win target remains unmet; the wide result is one block, with substantial
fixest variation (439–476 ms), not completion of the repeated-win gate.

The archive contains 190 accepted calls: 18 initial primary measurements,
66 overlap-only controls, 40 confirmation primary measurements and 66 combined
controls. Every native scientific export is bit-identical to its paired
baseline. Overlap-only control median ratios have maximum 1.04517 and geometric
mean 0.99131. Combined ratios have maximum 1.01527 and geometric mean 0.98350.
All eleven protected cells pass. The primary observed native peaks remain
233,169,535 bytes (K=10) and 925,859,855 bytes (K=50).

After SPI loading has finished, eligible unit one-FE fits compute the existing
raw-moment cache from their already loaded row-major matrix alongside native
sample/FE preparation on the same resolved team. This does not overlap matrix
construction with source-column moment scans, which previously lost time.
The private row owner and its statistics travel together. Shape, recipes and
every prepared value must match before the cache can be consumed. Missing-row,
singleton, changed-recipe or stale-value mismatches discard the speculative
cache and retain ordinary construction. Nonunit frames also discard it.
Moment errors are deferred to the original moment boundary, after fit-input
validation; preparation errors still take precedence.

The optimization requires at least 16,384 rows, 8–64 RHS columns and eight
workers. The preparation admission includes both the loaded matrix and the
three result vectors plus the fixed partial bank: 3*K*(chunks+1)*8 bytes,
where chunks is the existing deterministic count capped at 64. Actual vector
capacities are checked against this allowance. Insufficient headroom keeps
ordinary fit-time computation. No new N-by-K matrix, thread pool or SPI access
is introduced. All reduction orders and numerical checks remain unchanged.

Separately, inference verifies unit reference mass using the independently
validated physical observation count. It retains the exact-f64 integer limit
and the independent comparison with the bread's claimed weight sum. Explicit
weights keep their compensated scan. This removes another full row pass.

Three focused native tests cover complete fit identity, row/worker thresholds,
stale inputs, insufficient admission, deferred errors, explicit unit weights,
malformed weight storage and the exact-integer boundary. Both artifacts pass
the actual-plugin ABI and three licensed fixtures each, including the dummy
oracle and e-class transaction. Adjacent identities and validation receipts
bind the evidence. The combined source also renames the retained preparation
allowance to describe both rows and moment workspace and checks width addition.

This is bounded local arm64 development evidence. No installed package,
remote job, publication or release qualification changed.
