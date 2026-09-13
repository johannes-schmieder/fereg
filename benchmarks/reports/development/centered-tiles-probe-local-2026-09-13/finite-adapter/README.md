# P5 centered finite adapter: unadopted local prototype

Against accepted `436fe2a295dfe96ba25f00f8eddb43a9352aa22d` (numerical
checkpoint `257f4ce`), this extends the earlier [primitive](../README.md) to
ordinary OLS finite moments and guarded coefficient solving. The earlier
receipt and patch remain unchanged. The [new receipt](receipt.json) binds 181
inputs, the [complete prototype patch](prototype.patch), test names and logs.
Patch SHA-256 is `536db7b56258e64759a83e89798493b2a38bd99644f0ee583c1914a241b3fa09`.

## Implemented boundary

The test-only adapter borrows original columns and finalized one-FE membership,
retains stable group means and supplies 128-row centered/standardized tiles.
It shares the established finite-candidate classifier, moment accumulators,
Cholesky guard, coefficient/bread calculations and residual construction.
Within each moment, row and multiply/add order are unchanged. At up to 64
active columns, fixed row partitions retain the existing compensated merge.
Above 64, disjoint Gram-row partitions scan canonical rows and retain one tile
each, with no blocks-by-K-squared partial bank. All work uses the ambient team;
there is no SPI or new production routing.

The shared helpers are refactored in ordinary source; the centered adapter and
column source remain under `cfg(test)`. The core check is not evidence that a
release compiler preserves the existing route's performance. No plugin has been
built, no installed artifact replaced, and no command-time gain is claimed.

## Checks and numerical findings

The final source passes **97 distinct focused Rust tests**: ten selected by
`centered_` (nine prototype tests and one existing slope-recovery test), 64
existing prepared-fit tests, and 23 shared finite-kernel tests. A non-test core
`cargo check` also passes. Exact commands and all terminal counts are in the
receipt. The five new finite tests establish:

- Exact reference/current norms, centered values, RHS/Gram, coefficients,
  statuses, bread, residuals and rank certificate against the retained-matrix
  path for N257/K10/unit/one worker, N32777/K3/FW/four workers,
  N257/K64/AW/four workers and N257/K65/PW/16 workers. An exactly absorbed
  interior generated slot forces noncontiguous candidate mappings; a reported
  intercept remains deferred. The FW case crosses a fixed row-partition bound.
- Exact and clearly weak full-rank designs under all four OLS weight kinds
  reject Cholesky, then match the established matrix Householder result exactly.
  Only that fallback decides numerical omissions. The test materializes its
  fallback oracle; a bounded fallback is **not yet implemented** by this probe.
- Independent four-point contrasts retain coefficient 7 and bread 1/640 with a
  large between/within contrast. Equal centered columns with raw offsets 2^14
  and 2^40 retain different authoritative FE-collinearity statuses; within
  normalization must not erase the original reference scale.
- An insufficient moment budget fails before any reserve; each of four reserve
  failures, oversized returned capacities, invalid candidate mappings and a
  nonfinite reference norm fail closed.
- Above 64 columns, requested moment/tile storage is identical at N512 and
  N1024 for four workers. The prototype cap includes all live tiles, partials,
  RHS and Gram capacities. This is a bounded allocation assertion, **not** an
  observed estimator peak or a complete memory-admission claim.

The existing regression checks cover weighted full/weak/deficient fast and low
fits, retained session rank handling, original-row reconstruction, LSMR batch
controls, deterministic moments and covariance primitives. No licensed Stata
oracle, public transaction test or performance pair was run for this prototype.

## Preserved failures and corrections

The first compile used the nonexistent `into_residuals` method; it was corrected
to the existing consuming `into_parts` API. The first three tests then passed.
The expanded suite exposed two test assumptions, preserved in its failed log:

1. The large-outcome fixture asserted residual error below 1e-12. A separate
   diagnostic proves its inherited outcome-centering error is
   **1.16415321826934814e-10**, exactly matching the unchanged projector before
   fitting. The final test requires finite solving within 1e-12 of that centered
   input and retains the documented independent absolute residual envelope of
   1e-9. No projector, estimator tolerance or policy was changed.
2. A virtual oversized wide layout was expected to reject although it holds only
   partition metadata and allocates no square bank. The corrected test exercises
   the consumer's checked square arithmetic; actual moment allocation remains
   checked before reservation.

The initial compile, first passing tests, failed expanded suite and diagnostic
remain separate from the final green source; no old receipt was reinterpreted.

## Remaining estimator work

Production integration must preserve the caller's actual raw-moment/norm cache
and fixed weight-frame identity, independently certify FE projection, and avoid
retaining an N-by-K matrix through the rank fallback. The existing low-memory
Householder implementation uses scratch, but differs in standardization order
(`value * (root / norm)` versus `(root * value) / norm`) and some moment
accumulation. It cannot be substituted while assuming bitwise identity. The
retained streaming QR likewise cannot silently import PPML rank policy.

Original-row residual/inference and saved-FE consumers, full overlapping
means/inputs/solve/fallback/output admission, high group-count costs, isolated
release builds and complete-command paired comparisons remain open. PPML
requires a separate current-weight/checkpoint evaluation. Every P5 checkbox
remains open. Installed dist and unrelated Veneto work remain intact; no
agents, SCC jobs, qualification or push occurred.
