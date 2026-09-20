# Weighted independent statistics beside fitting

Local development measurements on 2026-09-20 on M2 Ultra, Stata/MP 19.0
(8 licensed processors) and pinned stock fixest, with 16 requested native/R
threads unless specified. The embedded build date is September 19. These
focused measurements do not close the broad milestone or qualify a release.

Candidate `weightedstatistics` is base `70a79e8c` plus the three recorded OLS
overlays; all 516 captured source files match. Plugin SHA-256:
`6456f0911703da8c16db50692ae9705459ded36074edd39e81b8a26b1d354561`.
Parent is accepted `weightedboth`. The separate diagnostic candidate uses the
same source plus isolated statistics timers. Its one-file instrumentation
replays exactly; the other 515 files match. Production and diagnostic plugin
ABI/panic checks pass. No installed package or remote state changed.

The existing bounded overlap now includes independently computed weighted
source facts, reference sums and outcome statistics. Only immutable borrows
and scalar results survive beside fitting. Exact source and fitted-scale
checks protect reuse, including the difference between the direct normalized
sum and a rounded raw-sum round trip. Invalidated speculation follows the
original scans. All original error boundaries and certificates remain.
[ADR 0084](../../../../docs/adr/0084-weighted-independent-statistics-overlap.md)
records the scope and safeguards.

## Timing

Milliseconds are separate medians of three paired runs. All observations,
pair order, individual ratios and dispersion are retained in the receipts.

| AW cell | Parent | Candidate | Default fixest |
|---|---:|---:|---:|
| Shuffled K=6 IID | 305 | 292 | 302 |
| Shuffled K=6 full outputs | 484 | 443 | 562 |
| Ordered K=6 IID | 292 | 272 | 482 |
| Ordered K=6 full outputs | 441 | 415 | 710 |
| Ordered K=50 IID | 748 | 721 | 1975 |
| Ordered K=50 full outputs | 927 | 908 | 2162 |
| Degree-three K=6, N=245760 | 89 | 84 | 83 |
| Degree-six K=6, N=245760 | 83 | 79 | 72 |
| Unbalanced K=6, N=245760 | 88 | 82 | 79 |
| One FE K=1 | 139 | 119 | 86 |
| One FE K=6 | 188 | 167 | 131 |
| One FE K=50 | 480 | 459 | 569 |

The shuffled IID candidate beats fixest in all three pairs, but narrow small
graphs and weighted narrow one-FE cases still lose. These are measured local
comparisons, not broad or universal wins. The full-output reference has the
accuracy discrepancies below.

PW/FW K=6 robust, crossed cluster, two-cluster/saved and other saved/full-output
checks pass. Constant, dispersed and group-concentrated AW, tiny weighted
bridges, low memory, four/eight threads, N=16383/16384 and weighted one-FE
controls pass. No cell's median paired native ratio exceeds 1.05; the maximum
is 1.03455 in the single-pair dispersed-weight output check. The low-memory
and four-thread ratios are 1.01493 and 1.01364. Three-pair shuffled unit K=5
and unit K=50 ratios are 0.99225 and 1.00768. Differences on ineligible paths
are not attributed to overlap. Limited control repetitions remain diagnostic.

The isolated large-shuffled profile measures 22.170 ms of independent source
work beside fitting. Post-fit weight reporting falls from 10.192 to 0.001 ms;
weighted RSS remains 4.252 ms. The parent's outcome mean/squares were
6.516/4.212 ms. Degree-six source work takes 5.441 ms beside fitting;
post-fit weight reporting falls 2.652 to 0.001 ms and RSS is 1.067 ms.
Profile timers are diagnostic and their total application times do not enter
the production medians above.

## Science, memory and evidence

All 170 application calls succeed with explicit result/sentinel checks.
There are 65 production native pairs and two diagnostic pairs. Every pair
preserves all six captured scientific files bitwise (b, V, status, DoF,
certificate and fit) and raises no native allocation peak. All sixteen saved
output pairs are bitwise exact and pass independent weighted normal-equation,
component-normalization, RSS and reconstruction audits.
Nine default-fixest saved-output disagreements remain explicit; all finite
coefficient comparisons pass. Frequency controls use the native parent and
independent frequency semantics, not ordinary fixest weights.

Nine focused Rust tests pass in a fresh target. They exercise AW/PW/FW,
one/two/three FE, ordinary and categorical inference, exact results at the
row/thread boundary, common scales 1e-100 through 1e100, direct-versus-round-trip
scale differences, one-bit scale invalidation, distinct source allocations,
invalid weights and preserved fit/DoF/weight/TSS error order. The earlier
seven-test prototype log and its hash are retained separately. Three
final-artifact Stata fixtures pass with fixture and terminal sentinels,
including independent `areg` plus explicit second-FE dummies and atomic
transaction checks. This is focused development validation.

Remaining work is to reduce duplicate Schur probe/initialization and restore
left means during reconstruction, then run the 111 native controls, sixteen
fresh-seed holdouts and two final five-pair primary blocks. The entire broad
milestone remains active.
