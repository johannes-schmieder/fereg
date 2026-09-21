# Narrow clustered reduction checkpoint, 2026-09-21

The isolated `ord6` plugin is
`5e1dda46536c8a675ef617be64a4b4a3d661b5ca7edecae131cd8051e3dbc823`:
`ffa3bde1` plus `sandwich/qualified_cluster.rs`. It includes neither the loader
nor Schur fallback drafts. The production prefix matches the checkpoint; an
additional test is bound separately. ABI and panic containment pass.

The existing ordered parallel cluster reducer replaces serial dense-table
accumulation in the narrow guarded OLS envelope of
[ADR 0090](../../../../docs/adr/0090-narrow-ols-ordered-cluster-reduction.md).
Three alternating fresh-process pairs on degree-six, N=245,760, K6, 16 workers,
seed 8675309 give the following two-way clustered, full-output medians:

| Weight | Prior ms | Candidate ms | fixest ms |
|---|---:|---:|---:|
| Unit | 141 | 123 | 134 |
| Analytical | 151 | 132 | 153 |
| Probability | 156 | 135 | 157 |
| Frequency | 156 | 133 | — |

FW inference uses the native baseline and independent frequency-weight oracle;
ordinary fixest weights are not its scientific comparator. Weighted one-way
clustering with saved effects and nested two-way clustering without outputs
also improve. `summary.json` retains each repetition, all ten cells, paired
ratios and medians. These are screens, not final-block acceptance.

All 81 applications complete successfully. Every native pair passes scientific
comparison, all 81 direct graph inference checks pass, and all 21 candidate
saved/full-output bundles pass independent reconstruction and normal equations.
Default fixest output discrepancies remain recorded in the completion receipts.
For the unit full-output cell, the native measured peak falls from 77,750,133
to 65,362,880 bytes; this does not establish complete memory-model coverage.

Ten existing OLS clustered tests pass. A new test exercises dimensions six and
seven at 32,771 rows and 1/8/16 workers. It compares every one-/two-way subset
against independently grouped integer-arithmetic score sums and outer products,
using dyadic weights, noncontiguous labels and unbalanced groups. Production
and certificate meats are exact, and reservation records prove the ordered
route actually runs on the threaded arms.

The portable archive contains numerical exports, identities, workers, input
hashes and output audit receipts. Large output vectors and licensed logs stay
local, with explicit vector byte counts and hashes. Protected controls, final
blocks, allocation review and Linux confirmation remain open; nothing is
installed, pushed or qualified for release.
