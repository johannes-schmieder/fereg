# P5 centered-column primitive: unadopted probe

Against accepted `436fe2a` (numerical source `257f4ce`), the test-only prototype
borrows original column slices and finalized one-FE membership. It stores G-by-K
stable group means and fills caller-provided row tiles by direct subtraction.
It reuses the existing scaled/compensated group-mean routine, including 64-column
blocks at larger widths, without storing a full within matrix. Each constructor
uses one fixed weight frame. No production estimator dispatch changes.

Four focused Rust tests pass on the final formatted source:

- Exact tile equality with the existing certified projector at 1/2/17/64/65/129
  columns, unit/dispersed weights, 1/4/16 workers and 1/17/128-row tiles.
- A separately calculated 2^40-offset case retains within sum of squares 10,
  exact dependence and an absorbed constant; subtracting raw cross-products
  loses that small variation.
- Pre-allocation mean-budget rejection, injected allocation failure, bounded
  capacity, invalid source rejection and unchanged output on an invalid tile.
- Fresh construction under changed weights matches direct group-mean answers.

[The receipt](probe.json) binds 180 inputs, both successful test logs and the
[prototype patch](prototype.patch). The heap limit checked here concerns group
means only. It is not full estimator memory admission; tiles, workers, moment/
QR storage and fallback overlap still require integration. Large G can erase
memory savings. No new plugin, licensed fixture or performance comparison ran.

The existing finite kernel offers deterministic moments, guarded Cholesky and
retained streaming QR, but ordinary OLS currently requires a materialized within
matrix. Next connect this representation to moments/norms and the established
OLS fallback without importing PPML rank policy, then original-row residuals/
inference and saved outputs. Full command/memory evidence must decide adoption.
PPML needs a separate current-weight/checkpoint evaluation. Every P5 checkbox
remains open. Installed dist and unrelated Veneto work are preserved.
