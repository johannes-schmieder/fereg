# Low-memory OLS score-cache reads, 2026-09-21

Clustered low-memory inference can visit rows in cluster order. The previous
scratch cache decoded up to one megabyte on every miss, even when the next
request jumped elsewhere. A five-second sample of the cycle/K6/AW/full-output
fit places 3,761 active samples in scratch score-row reads. The final FE
recovery in the public phase trace takes only 6 ms of a 24.966-second command.

The candidate decodes one row after a jump and resumes bounded read-ahead when
the next miss continues the previous block. Allocation sizes, arithmetic order,
weight and inference semantics, certificates and output publication are
unchanged. The change is confined to ordinary OLS scratch-score storage;
dense/strided storage and retained PPML score adapters keep their existing code.

## Screen and science

The isolated candidate is `sc1`, plugin
`76f216103849051d2d234c8e6f3b937ced9003f2d620a4ddb520b7e15a1d33dd`.
The comparator is the earlier `m5` development artifact. The full source
identities include other, separately evaluated graph and slope drafts; these
are not an acceptance claim about those drafts or the eventual commit SHA.
The two-FE fits have identical native numerical exports and allocation peaks.

| Design and request | Previous native | Candidate | Default fixest |
| --- | ---: | ---: | ---: |
| Cycle, N245760, K6, AW, two-way clustering/full outputs | 23.121 s | 0.753 s | 3.268 s |
| Cycle, N245760, K50, AW, two-way clustering/full outputs | 37.150 s | 9.261 s | 12.009 s |
| Dense, N983040, K6, unit, robust/no outputs | 1.919 s | 1.887 s | 0.378 s |

All commands explicitly request low memory and sixteen native/reference
threads. Each row has three rotated pairs and no detected external-process
overlap. Paired candidate/previous-native medians are 0.032568, 0.250854 and
0.988015. These are exploratory screens, not the milestone's final blocks.
Explicit low-memory dense fitting still has a substantial reference gap.

All 27 calls complete. Six native independent inference checks, eighteen exact
within-variant repeat checks and six independent saved-output audits pass.
Coefficients, covariance, fit statistics and outputs are also bitwise equal
between the two native artifacts in all nine pairs. Native peaks match exactly:
147,326,351, 241,724,433 and 420,861,353 bytes for the three rows respectively.
The default reference passes the dense oracle and fails both cycle oracles;
those scientific discrepancies are preserved and its times are not silently
replaced with tighter reference runs.

The bit-preservation/read-volume fixture covers sequential, permuted and
resumed sequential access at ranks 1, 6 and 50, including signed zero and
extreme finite values. Both existing low-memory clustered fixtures also pass,
including independent legacy group influences and allocation-failure checks.
The isolated plugin passes its actual ABI containment fixture.

## Rejected product-retention direction

The archive retains the preceding `m6`/`m7` low-final-product experiments.
The first core draft passes its reconstruction fixture but the public adapter
still disables products, so its public screen is ineffective. The corrected
draft enables only the final public two-FE automatic projection and passes its
32-case fixture and public independent checks. It still takes 24.966 seconds.
Sampling identifies scratch reads instead. Both drafts were removed from the
working candidate; their exact source snapshots, tests, patch and diagnostics
remain available. Earlier short unconstrained timeouts are not solver-stall
evidence: later K50 low-memory calls complete around 38–39 seconds.

## Evidence boundary

`evidence.zip` preserves four follow-on roots and their requests/results,
numerical exports, generated workers, input hashes and comparisons. Large
output vectors remain local with explicit sizes and hashes. `sources.zip`
binds all four native artifacts to their base revisions and captured overlays.
`checks.zip` preserves focused tests, independent audits, paired timings,
selected sampled stacks and the public phase trace. Hash maps distinguish
original bytes from portable path substitutions. Licensed logs and binaries
are excluded. The sampled command is diagnostic, not a headline timing.

The broader protected matrices, expanded graph coverage, final repeated
blocks, remaining numerical drafts and SCC/Linux confirmation remain open.
No package was installed, pushed or released.
