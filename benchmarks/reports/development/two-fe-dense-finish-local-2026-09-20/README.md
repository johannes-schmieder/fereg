# Two-FE strict final projection and output screen

The `finalschur` prototype wins all twenty primary cells in a single-pair local
Mac screen, with fereg/fixest ratios 0.4053–0.7308. This is not final acceptance:
fresh-seed holdouts, regression controls and both five-pair blocks remain open.
The source/artifact-bound archive contains 96 application calls. Default fixest
saved-output disagreements are recorded separately from runtime comparisons;
they are not labeled passing scientific comparisons.

## Changes and incremental measurements

The starting `graphrecover` artifact incorporates multi-FE input/RHS reuse,
narrow unit-weight Schur admission and independently certified spanning-forest
recovery, described in the [initial report](../two-fe-dense-initial-local-2026-09-20/README.md).

The intermediate `finalcg` variant selects ordinary symmetric CG for the final
one-RHS projection after a successful initial unit-weight Schur probe on at
least eight workers. Three-pair narrow medians improve saved FE from 602 to
564 ms versus stock fixest's 647 ms. Full outputs improve 636 to 628 ms versus
651 ms, missing the 10% target.

The retained prototype instead reuses the accepted graph/weight probe for a
fresh final Schur solve. This is the same immutable prepared operator within
one fit. It retains the caller's strict output tolerance, independent residual
certificate, bounded 64-step attempt, memory admission and ordinary-CG
continuation. It charges no invented pilot iterations to the final solve.
The initial probe and its iteration accounting are unchanged. Residual-only
coefficient products are omitted when no initial products exist and final-target
recovery is authorized, since those products cannot form the saved effects.

In a separate three-pair screen, saved-FE medians improve 574 to 450 ms versus
635 ms for default fixest; full outputs improve 616 to 480 ms versus 645 ms.
Every native output comparison passes. Default reference output discrepancies
remain explicit, with the tighter-reference adjudication in the initial report.

## Full primary screen

Each time below is one complete command, in milliseconds. These are individual
screen measurements, not repeated-block medians. The hardware, data, timing
boundaries and comparator pin are unchanged from the initial report.

| Request | K=10 fereg / fixest | K=50 fereg / fixest |
|---|---:|---:|
| IID | 316 / 443 | 793 / 1501 |
| Robust | 330 / 481 | 915 / 1913 |
| FE-level cluster | 338 / 500 | 855 / 1980 |
| Crossing cluster | 343 / 483 | 879 / 1873 |
| Two crossing clusters | 381 / 564 | 955 / 2356 |
| Two clusters, nested FE | 357 / 542 | 972 / 2305 |
| Saved FE | 461 / 644 | 873 / 1678 |
| FE-level cluster + saved FE | 479 / 697 | 1006 / 2197 |
| Two clusters + saved FE | 514 / 767 | 1075 / 2571 |
| Full outputs | 456 / 624 | 949 / 1698 |

The committed `637f2407` baseline is measured alongside every cell. All native
coefficient, covariance, sample/status/DoF and requested-output comparisons pass.
Default fixest coefficients, covariance, samples and RSS pass their comparisons;
its saved-output discrepancies remain in `series.json` with status
`reference_output_mismatch`. No default-reference timing is replaced by a
tighter or slower reference run.

## Independent evidence and remaining work

All eight native saved-output cells pass an independent observation-space audit:
effects are constant within their declared groups, normalized by component,
and reconstruct the returned predictions/residuals. The largest within-FE
residual mean is 2.60e-13; the largest finite normal-equation score per observation
is 1.72e-16. Full-output reconstruction errors are below 5.4e-15. This audit
does not reuse either estimator's demeaning or recovery algorithm.

The final projection passes the five focused Schur tests, including the strict
final certificate and zero new pilot count. Graph recovery passes independent
normalization/reconstruction cases and exact/one-byte-short finishing-budget
admission. Actual-plugin ABI tests and artifact-specific Stata input/oracle and
transaction fixtures pass. An extended fixture adds string-FE fallback and
unrestricted low-memory loading after missingness/singleton removal.

The production prototype is `finalschur`, plugin
`4b005ae743eb4c18d958e2f7915c558a6d48782365f060a6244c8866ecef8c96`.
Later test-only additions and a one-RHS boundary check are being frozen into a
new isolated candidate for the repeated campaign. No installed package,
remote state, release or cross-platform qualification changed.
