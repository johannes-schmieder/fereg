# German OLS residual adjudication evidence

The saved native residuals agree with an independent projection on **all
4,361,733 retained rows**, at the unchanged pointwise envelope
`1e-9 + 1e-7 * max(abs(actual), abs(oracle))`. Maximum error is 1.177e-10.
This is a conditional-on-published-beta projection; coefficient, covariance,
sample, ordering and DoF parity pass separately. No native defect is established.
These are development diagnostics, not candidate qualification or an approved
reference exception. [Validation](validation.json) binds sources, inputs,
reference pins, saved artifacts and the receipts below.

The single authorized tighter pinned reference run uses tolerance 1e-14 and
completes in 1,712.825 command seconds, with process and Stata success. Its
[comparison](tighter-empirical-audit.json) still fails: 8,687 residuals outside
the original envelope, maximum difference 3.819e-7. Both arms retain exact
sample/order/DoF and passing coefficients/covariance and prediction feasibility.
The original default-tolerance failure (699,592 rows, maximum 6.366e-5) remains
unchanged. There was no further estimation retry and no full reference restart.

The independent localization uses three increasingly general checks:

- [Complete small components](ols-components-tighter.json): all six selected
  native components pass 100-digit explicit-dummy projection; all six tighter
  reference components fail. The giant component still contains 8,664 pairwise
  failures, so these small examples alone cannot adjudicate the whole sample.
- [Graph cuts](ols-cut-cell-audit.json): after collapsing repeated worker/firm
  cells, a bridge cell's weighted residual sum must vanish. Its individual
  residuals are therefore the within-cell deviations of `y - X beta`.
  An exact edge-deletion oracle checks 50 small graphs, and 100 high-precision
  projections check repeated/unequally weighted observations. Native residuals
  pass on all 3,257,454 rows covered by this exact cut argument; reference
  residuals fail on 6,188. The largest discrepancy is on a single-row bridge.
- [Remaining cycles](ols-cycle-projection-audit.json): remove bridge cells,
  retain their exact within-cell residuals, and independently project the
  remaining cell means with cell multiplicities as weights. Ground one
  highest-degree vertex per connected component and degree-scale the sparse
  design. This reduces 4.36 million observations to 410,862 cyclic cells and
  253,295 coordinates. SciPy LSMR uses atol/btol 1e-15, conlim 1e14 and the
  existing 5,000-iteration limit. Both arms finish in 453 iterations. Fresh
  all-original-row normal errors are below 2.9e-17. Twelve 100-digit dummy
  calibrations, including chains, cycles, bottlenecks, disconnected components,
  duplicates and multiplicities, agree within 4.1e-15. Native residuals pass
  every row; reference residuals fail 8,691. The saved-output diagnostic takes
  34.87 seconds. This is a numerical independent projection, not an outward
  interval enclosure of the joint finite-parameter optimum.

Thus the residual discrepancy is localized to reference projection accuracy;
smaller native scores alone are not the argument. The existing LSMR-specific
reference exception is not applied to native CG. A separate review-only
adjudication is proposed in [draft ADR 0073](../../../../docs/adr/0073-german-ols-review-adjudication.md).
Until approved, the pairwise gate remains failed and candidate admission is
blocked. Memory coverage, exact-candidate checks, Mac toolchain availability,
Windows completion and formal qualification remain separate gates.

The portable [projection helpers](projection_oracles.py) preserve the empirical
algorithms while removing machine paths. Their [local calibration](portable-calibration.json)
passes all 50 cut-graph, 100 cut-projection and twelve cyclic-projection checks.
The original empirical wrapper hashes remain in their receipts.
