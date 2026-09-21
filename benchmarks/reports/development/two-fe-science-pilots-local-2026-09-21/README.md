# Two-FE science coverage pilots, 2026-09-21

The corrected pilot completes all 44 graph/weight cells without a process
failure. Baseline and candidate coefficients, covariance and five requested
output vectors are identical. All 88 native explicit-dummy checks pass,
including literal observation expansion for frequency weights. These are
concurrent local science checks; their runtimes are not performance evidence.

Each pilot uses eleven graphs, four weight kinds, six regressors, two-way
cross-clustering and all saved/predicted outputs. The unchanged baseline is
`0efa78bd`, plugin
`bc9e5c6c522af7292757c6d20753a7671e9728baa75f8ee6ee67c6cb09c8453e`;
the candidate is the [direct slope recovery checkpoint](../slopes-recovery-local-2026-09-21/README.md),
plugin `bef34c8c45052866f2bde2b52e0041183013f58e089ba86995f7b246cf3c9c41`.
Categorical two-FE production behavior is unchanged by that checkpoint.

| Pilot | Attempted cells | Process failures | Native oracle passes | Default fixest oracle passes / mismatches |
|---|---:|---:|---:|---:|
| Original, N=960 | 44 | 4 | 80 | 1 / 29 |
| Corrected, N=1,920 or 7,680 | 44 | 0 | 88 | 2 / 31 |

The original four weak-graph inputs failed generation before estimation:
N=960 falls outside the generator's registered branch envelope. Its degree-six
inputs also exposed a zero modulus in the offset generator at that size;
those shared-data science checks cannot establish coverage of the intended
registered graph shape. The corrected pilot uses N=1,920 for ordinary graphs
and the minimum registered N=7,680 for the weak graph. All attempts remain
archived. No frozen large benchmark input or production estimator changed.

For the larger weak input, a separate full sparse explicit-dummy oracle grounds
one level per connected component and solves the weighted normal equations
with SuperLU, then checks fresh group moments. It independently computes finite
coefficients, inference and normalized output vectors. Frequency checks refit
physically expanded observations. A preliminary cross-check against all forty
completed original dense-SVD cases passes. The new sparse oracle is independent
of the native projector and of the existing Schur graph oracle.

Default-reference discrepancies remain failures under the unchanged tolerances.
They include saved-effect/reconstruction accuracy and variance-correction
differences. They require separate matched-accuracy and DoF adjudication; no
claim of universal reference parity or a reference defect is made here.
Ordinary fixest weights are excluded as an FW inference comparator.

The portable archive contains 231 application receipts, generated workers,
plans, every controller attempt, numerical exports and small output vectors.
Raw licensed logs and input data files remain local. `archive-hashes.json`
maps original files to path-redacted exports. `controller-sources.zip` includes
both executed controller versions: the original files were reconstructed and
their SHA-256 hashes match the original recorded plan exactly. ZIP integrity
checks pass. The [summary](summary.json) gives machine-readable counts.

This checkpoint prepares the approved 1,518-cell small matrix and 168-cell
large expansion. Neither complete-matrix acceptance, final timing blocks,
new solver routing, Linux confirmation nor release qualification is claimed.
