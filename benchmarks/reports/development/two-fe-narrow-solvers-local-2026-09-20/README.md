# Narrow well-connected graphs: forced-solver diagnostics

Thirty successful local Mac calls compare auto, MAP, CG, LSMR and CMG on the
three remaining well-connected graph families. N=245,760, K=6, sixteen native
workers, shared seed 8675309 and either unit or moderate analytic weights.
Each entry is one warmed command in a fresh process, so these are diagnostic
measurements, not repeated acceptance evidence. Source is `c9635415`, artifact
`weightededges2`, plugin
`337deed4ecdae50f490f84cd8731f1be68beb69b3151bdde7a0ce2a5be4edb9f`.

| Graph | Weight | Auto ms | MAP ms | CG ms | LSMR ms | CMG ms |
|---|---|---:|---:|---:|---:|---:|
| Degree three | Unit | 77 | 240 | 156 | 344 | 203 |
| Degree three | AW | 89 | 285 | 177 | 365 | 255 |
| Degree six | Unit | 69 | 151 | 133 | 283 | 153 |
| Degree six | AW | 88 | 170 | 155 | 285 | 173 |
| Unbalanced | Unit | 72 | 258 | 159 | 284 | 282 |
| Unbalanced | AW | 94 | 276 | 179 | 307 | 363 |

All coefficient, covariance and sample comparisons against the corresponding
auto call pass. The independent direct-oracle evidence for these auto designs
is in the [broad screen](../two-fe-broad-screen-local-2026-09-20/README.md).
The forced family is recorded in `narrow-solver-diagnostics.json` and each
request, although the common measurement-cell schema does not contain it.
`profiles.json` retains native phase traces. No forced result substitutes for
an auto benchmark or changes the production routing policy.

Auto already selects the fastest measured path in every cell. The next
optimization should remove duplicate work inside that path, especially the
separate three-column Schur probe and repeated group initialization. The
weighted degree-six profile in the cache report spends about 7.6 ms probing
before a roughly 24.4 ms initial absorption; shuffled dense narrow probing
costs about 23 ms. A combined probe/solve needs its own capacity, rejection,
iteration-accounting and independent-certificate checks before admission.

The artifact's original build date is September 19 and the measurements were
made September 20, using Stata/MP 19.0. These local development diagnostics
change neither installed packages nor remote state or release qualification.
