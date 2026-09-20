# Both counted Schur gathers in idle OLS scratch

Local development measurements on 2026-09-20, M2 Ultra, Stata/MP 19.0
(8 licensed processors) and pinned stock fixest, with 16 requested native/R
threads. The embedded build date remains September 19. These are focused
measurements, not final milestone or platform qualification.

The candidate `unitboth` uses base `33b8b7ba` and the recorded core overlay.
All 515 captured source files match; plugin SHA-256 is
`c26b715a42a9c704e26b4d7caf46afee27e626776f4369fd2e6483ad0cc7ee3c`.
The comparator `narrowinput3` is the previous accepted artifact. Identities,
inputs, paired order, all 101 successful application calls, three fixture
receipts and the implementation patch are retained here. The newly added
unit fixture is hashed separately in its receipt. Nothing was installed or
published.

Eligible narrow unit OLS uses idle left-RHS storage for the left counted-edge
gather when group-statistics scratch fits only the right gather. The existing
bounded repeated-edge sample and capacity checks remain; original left means
are restored before reconstruction. Wider two-bank, weighted, small-team and
shared/PPML paths retain their prior behavior. No heap owner or allowance is
added. See [ADR 0082](../../../../docs/adr/0082-two-sided-unit-ols-edge-scratch.md).

## Timing

Milliseconds are separate medians of three paired measurements unless marked
otherwise. Paired ratios, every repetition and failures remain in the receipts.

| Cell | Parent | Candidate | Stock fixest |
|---|---:|---:|---:|
| Ordered unit K=1 | 199 | 202 | 337 |
| Ordered unit K=3 | 214 | 213 | 354 |
| Ordered unit K=5 | 238 | 237 | 366 |
| Ordered unit K=6 | 228 | 231 | 372 |
| Shuffled unit K=1 | 221 | 223 | 222 |
| Shuffled unit K=3 | 233 | 225 | 235 |
| Shuffled unit K=5 | 268 | 258 | 248 |
| Shuffled unit K=6 | 259 | 252 | 261 |
| Degree-six unit K=6, N=245760 | 65 | 65 | 66 |
| Shuffled K=3, two clusters and saved FE, one pair | 469 | 442 | 552 |
| Shuffled K=5, two clusters and saved FE, one pair | 492 | 476 | 572 |
| Shuffled K=5, full outputs, one pair | 398 | 399 | 500 |
| Ordered unit K=50, one pair | 670 | 664 | 1520 |
| Shuffled AW K=6, one pair | 336 | 329 | 305 |

All sixteen native cell median paired ratios are below 1.05; the maximum is
1.00905. The weighted and wide controls do not exercise the new cache and their
small timing differences are not attributed to it. K=1 remains near parity,
and K=5 still loses. The single separate K=5 shuffled profile records operator
68.44 -> 49.69 ms, initialization 9.17 -> 9.99 ms, reconstruction
2.69 -> 7.16 ms, and initial absorption 139.40 -> 128.24 ms. Restoration costs
part of the gain; the unchanged contraction probe still costs about 23 ms.

## Science and memory

All 34 native paired numerical comparisons pass, with exact status/DoF and
sample identities. Eleven pairs preserve all six checked scientific files
exactly; the largest coefficient difference is 9.72e-17 and covariance
difference 1.59e-22. Four saved-output comparisons pass numerically (their bits
change), and all four independent weighted-normal-equation, normalization,
RSS and reconstruction audits pass. All four default-reference saved-output
disagreements remain explicit; no coefficient comparison failed. These
reference output errors are not reinterpreted as native failures or omitted.

There is no native allocation-peak increase in any of the 34 pairs. Thirteen
focused Schur tests pass, including independent observation operators, MAP
oracles, connected/disconnected graphs, cache capacity and authority guards,
8/16-thread determinism and forced capped continuation. Their original absolute
oracle bounds remain. The actual-plugin ABI/panic-containment check and three
licensed Stata fixtures pass with explicit fixture and terminal sentinels.
The new unit fixture compares K=1/3/5/6/12 IID, robust and cluster inference and
saved-output reconstruction against `areg` plus explicit second-FE dummies.

The full broad control matrix, fresh-seed holdouts and two final five-pair
blocks remain open. This change alone does not close the performance goal.
