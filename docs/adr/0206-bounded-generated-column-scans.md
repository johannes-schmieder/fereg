# ADR 0206: bounded generated-column scans in certified PPML

Status: accepted for local development, 2026-09-25; not qualification.

## Decision

Use one immutable prepared-column range visitor for numeric, constant, factor
and categorical-interaction columns. Resolve interaction source slices once per
128-row Boolean tile, including vanished levels and arbitrary component counts.
The visitor owns no reduction, expanded design or weight statistic. Replace the
duplicated predictor and original-joint-column recipe evaluators with this helper.

Populate existing low-memory RHS blocks through disjoint row panels. Reconstruct
recipe-backed joint residuals through 1024-row panels, preserving finite-coordinate
FMA order within each row. Independently accumulate their fresh finite moments by
column, preserving observation order. Generated reference norms/means can use the
existing parallel scalar banks at the established N-by-K work boundary; direct
numeric eligibility is unchanged.

For generated finite outer scores, reuse the numeric certificate's 64-entry scalar
bank in chunks, permitting widths above 64 without another bank. Each column
reads original values, current means and prior weights, uses the same compensated
arithmetic, and enters the final reduction in canonical column order. Use the
existing numeric path's 4096-row boundary and the resolved team, including T1.
FE scores retain their existing independent calculation.

## Scientific and ownership boundary

This changes computation organization, not the solver recurrence or check
schedule. Every current-weight projection, original-input finite/FE score,
model-space, rank, finite-solve, objective, reconstruction and exact final q*mu
Hessian requirement remains. No authority survives a new weight frame. Prior
guesses, canonical projectors, rank policy, tolerance, sample, separation, DoF,
VCE and transactional publication retain their contracts.

No new heap bank, persistent cache, N-by-K expansion or worker team is introduced.
Existing owners remain immutable while workers use disjoint destinations. The
interaction helper uses 128 Boolean stack entries; predictor reconstruction drops
its former 128-double value tile. This is not a new whole-action allocation or
physical-memory proof. Errors propagate before publication; no worker accesses SPI.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-generated-scans-local-2026-09-25/README.md)
binds `generatedscan1` to `5d84eb9d` plus six Rust overlays. All 565 captured
computational inputs match the accepted source. The original large low-factor
case improves 1077->717 ms against 719-ms stock; fast improves 969->817 ms against
718-ms stock. The former saves 360 ms, but neither clears the 0.90-stock target.
Fresh generated families win all 25 baseline pairs, with median gains 1.9–38.5%.
Fresh source/seed437 are frozen; no threshold tuning follows them.

Sixteen distinct Rust tests, six independent explicit-design Poisson/Hessian/
sandwich checks and three licensed fixtures pass. All 63 pilot/fresh direct-stock
pairs pass. Twelve OLS pairs have exact scientific exports. Losses remain visible:
the million-row pilot median rises 14 ms, fresh four-FE rises four ms, fresh
multiple slopes one ms, and low/generated OLS controls one/two ms. This is not a
universal non-regression or parity claim.

All 63 pilot/fresh tracked native peak and scratch pairs match. Modeled selected
bytes vary by -48 through +112 bytes; no new owner is admitted. No warmed-allocation
campaign, physical cap, Linux/SCC run or qualification was performed. Historical
output/DoF discrepancies, static inventory drift and constrained-fit failures
remain open. The next substantial costs are repeated full-control projection in
fast two-/three-FE ordinary IRLS and large-N traversal/finalization.
