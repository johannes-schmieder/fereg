# Original rank-deficient saved-output failure

The completion batch stopped after slope and three-FE pairs passed exactly.
The original rank-deficient AW request (N30720/K50/T4, x2=x1 exactly, seed31415927)
returns r(0) in both arms but fails saved-residual parity by 2.82195686263087e-9.
No original input, seed or tolerance is replaced. Explicit-CG and the three
current-artifact primary verification pairs remain unexecuted.

An independent weighted dummy projection grounds one FE per component and solves
the reduced system with SuperLU plus one residual correction, then uses LAPACK
SVD for the finite rank. The full 50-column SVD has rank49; dropping the known
exact duplicate gives the same residual within 1e-12. Separate full sparse weighted
LSMR and LSQR agree with the direct residual within 4e-14. Full canonical b/V,
normalized constant and DoF match both fits within original tolerances.

The baseline passes all original pointwise output tolerances. The candidate fails
13 residual rows, 48 first-FE rows and 160 second-FE rows (maximum tolerance ratios
1.923/1.713/1.154). This is a candidate regression, not a baseline exception.
Its 0.496→0.406 second timing and 89.07→47.61 MB native peak are failed-science
diagnostics, excluded from passing performance aggregates. The five-file
prototype cannot be adopted on the current evidence.

Both final residual projections take zero iterations. The saved target is built
from y-xb-r, so an approximate residual's remaining FE component propagates into
the saved channels even though subsequent recovery reconstructs that target
strictly. The new saved-Schur route must polish its final residual to the existing
saved-recovery precision before constructing that target. Apply this to the newly
deferred saved recovery route, preserve user-requested stricter tolerances,
original reference scales, certificates, budget and iteration/fallback limits.
Add a closed-form regression and repeat the same failing request before resuming
the original controls/primary protocol. Earlier failed comparisons remain archived.
