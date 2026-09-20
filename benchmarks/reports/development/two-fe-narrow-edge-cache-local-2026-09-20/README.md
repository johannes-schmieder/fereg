# Narrow one-sided Schur edge cache: local development evidence

Local M2 Ultra Mac, licensed Stata/MP and pinned stock fixest, 2026-09-20.
Ordinary unit-weight two-intercept OLS, sixteen requested native workers.
This is a bounded development increment, not completion of the broad milestone.

## Change and artifact

The capped unit Schur operator can cache its right gather when existing idle
scratch holds N words but not 2N. That gather reads the larger left-level state.
The left gather retains its original loop. Both-sided and insufficient-scratch
routes remain as before. A bounded sample of at most 64 evenly spaced left
levels and 32 members per level must show at least one quarter repeated neighbors
before admitting the new one-bank cache. It allocates no storage and changes
only a cost decision. Codes/counts retain the existing packed representation;
certificates, iteration caps, continuation and memory allowances remain.

Parent `weightedinput2` is the production source committed as `329bc17c`.
First prototype `narrowedges` plugin:
`b576805ce33521d676abcc1366c8d3662cc5ba6f0806719034402abf65033a31`.
It admitted a one-bank cache without the repetition screen. The final
`narrowedges2` plugin is
`8ec5df3a1686c9f476dcf2a83b63318ee252f789b1e49b75c624d5300d76efb2`,
built from `329bc17c` plus the recorded core overlay. All 514 captured source
entries match the final working source. Actual-plugin ABI/panic checks pass.
The isolated helper embeds September 19 as its build date; these measurements
and source captures occurred September 20.

## Measurement and rejected prototype

The first prototype improves the dense cells but regresses the degree-six
control by 5.9% in three paired calls (68 -> 72 ms medians). Its distinct edges
provide too little repetition to repay cache construction. All 93 original
calls remain in the top-level archive. The final guarded candidate's five-pair
control has parent/candidate medians 67/69 ms and median paired ratio 1.029851,
below the declared 5% regression threshold. Its scientific values are exact.
The reference median is 68 ms; the paired candidate/reference ratio is 0.985915.
Median ratios need not equal ratios of separate medians.

Final three-pair dense command medians in milliseconds, N=983,040:

| Order | K | Parent | Candidate | Default fixest | Paired candidate / fixest |
|---|---:|---:|---:|---:|---:|
| Ordered | 3 | 241 | 224 | 351 | 0.632479 |
| Ordered | 5 | 270 | 257 | 368 | 0.687166 |
| Ordered | 6 | 264 | 242 | 370 | 0.645777 |
| Shuffled, fresh seed | 3 | 266 | 243 | 238 | 1.008403 |
| Shuffled, fresh seed | 5 | 300 | 279 | 247 | 1.137652 |
| Shuffled, fresh seed | 6 | 284 | 265 | 254 | 1.031496 |

All shuffled cells still miss the speed target. The first prototype's separate
K=5 shuffled profile reduces operator work from 87.93 to 68.70 ms; setup changes
9.36 -> 10.08 ms. Its complete fit is 215.25 -> 195.35 ms. The final guarded
profile has command time 303 -> 287 ms. These profiles remain separate from
paired headline calls.

The first prototype also tests unit K=1/6/10/50 at N=245,760, AW full outputs
and shuffled unit K=5 full outputs. The unaffected controls stay within 5%; the
saved-output request improves 433 -> 417 ms. A single weighted full-output pair
is 479 -> 486 ms. No outlier or slower call is removed.

## Validation and scope

The final operator test covers no bank, exactly one bank, just under two banks
and two banks, several RHS widths and 8/16 workers. It compares to the direct
observation operator and tests repeated versus distinct-neighbor sampling.
Projection tests exercise one- and two-bank widths against a separately
converged MAP solve and verify across-thread deterministic results. Both tests
pass on the final source, as do three final-artifact Stata fixtures for narrow
weighted Schur, multi-FE input and eclass transaction.

The 36 prototype native pairs and 24 final native pairs all preserve exact
status/DoF and pass the unchanged numerical comparisons. None raises the
native allocation peak. Two independent prototype output audits pass. The
[separate graph/science report](../two-fe-broad-graph-science-local-2026-09-20/README.md)
contains nineteen additional small native/reference calls using that prototype
for FW literal-expansion and one-FE output harness checks; they are not counted
in this archive's 164 calls. Final guarded output/transaction checks use the
completed artifact's Stata fixtures.

`final/` retains all 71 guarded-candidate calls, identities, comparisons, tests
and source audit. The two source patches distinguish the rejected unconditional
admission from the retained repetition guard. The broad 77-cell screen and
repeated acceptance blocks remain pending. No installed package, remote state
or release qualification changed. See [ADR 0079](../../../../docs/adr/0079-narrow-one-sided-schur-edge-cache.md).
