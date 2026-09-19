# Reuse of the private OLS sample marker

This local Mac development step retains the unrestricted candidate marker as
the result sample. The unused candidate input slot binds the numeric outcome;
the ordinary outcome slot and every scientific source remain read-only. Rust
already receives the all-candidates flag and never reads that unused slot.
Restricted calls retain the original separate-marker transaction. This avoids
both copying and later deleting an observation-sized Stata variable.

A separate native shortcut replaces compensated accumulation of unit weight
mass with exact N, bounded by 2^53. Scaled value arithmetic, compensated
numerators, observation order and supplied-weight paths remain unchanged.

The source-bound archive contains 173 accepted calls: a three-pair primary
screen, three pairs for all eleven protected controls, and five fresh primary
pairs. Native coefficient, covariance, sample, status, DoF, fit-statistic and
certificate exports are bit-identical. Five-pair confirmation medians are:

| Variant | K=10 | K=50 |
|---|---:|---:|
| Committed native baseline (`factor`) | 188 ms | 477 ms |
| Sample reuse only (`sampleowner`) | 182 ms | 447 ms |
| Sample reuse plus unit means (`samplemean`) | 183 ms | 444 ms |
| Pinned fixest | 140 ms | 459 ms |

The preceding three-pair screen gave 189/185/182/141 ms and
481/447/443/471 ms respectively. The wide lead does not consistently reach
5%; neither screen nor confirmation reaches the narrow target. The native
mean shortcut is small: it does not show a narrow command gain in confirmation.
All protected controls pass. Sample-only median ratios have maximum 1.01695
and geometric mean 0.95888; combined ratios have maximum 0.98305 and geometric
mean 0.94638. Native allocation peaks remain 233,169,535 and 925,859,855 bytes.

Two native tests compare unit means against explicit frequency-unit weights,
including threshold sizes, extreme scales and error ordering. Ten licensed
fixture executions across the three artifacts and a strengthened source-cell
invariance rerun pass. The native-input fixture covers storage types, extended
missings, supplied weights, residual outputs and transaction failures; its
explicit-dummy checks and the candidate oracle remain independent authorities.
Ten public/hidden transaction metadata cases also pass. `validation.json`
binds the receipts; `identities.json` binds exact sources and artifacts.

The confirmation additionally includes the experimental `outcomefit` artifact,
which moves the unchanged independent outcome mean/TSS scan alongside fitting.
Its medians are 177/445 ms. Those calls establish exact scientific comparison
and an initial narrow gain; protected-control evidence for that scheduling
change belongs in its separate report. This archive does not claim its adoption.

The associated lifecycle diagnostic and rejected plain-post experiment are in
`../one-fe-stata-lifecycle-2026-09-19/`. Raw licensed logs and machine paths stay
outside Git. This is focused local arm64 development evidence, with macOS core
placement uncontrolled, and is not release or cross-platform qualification.
