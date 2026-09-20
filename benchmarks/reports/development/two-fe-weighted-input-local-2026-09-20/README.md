# Weighted OLS input overlap: local development evidence

Measured on 2026-09-20 on the local M2 Ultra Mac, licensed Stata/MP 18 and the
pinned stock fixest reference. Ordinary two-intercept OLS, sixteen requested
native workers, N=983,040 unless listed otherwise. These are development
measurements, not release or cross-platform qualification.

## Source and mechanism

Incremental parent: `3a02a7dae07fa8824da705d7be81775508d7d601`, artifact
`weightedfinish`, plugin `732ec4b95ed7a37aea70abfd65485b443a5788ba7e8218d28de43ead689cf90e`.
All 514 captured source entries match that commit; see
`incremental-parent-provenance.json`. The separately copied
`baseline-provenance.json` identifies the earlier full-milestone baseline
`e9d593bd`, not this incremental parent.

Candidate: `weightedinput2`, that parent plus the recorded `estimate.rs`
overlay, plugin `7adc6377c38d280007de94985fb05478456459def6263ebf6cfd6941ff21cfdf`.
All 514 captured entries match the working source at capture. The isolated
artifact build uses a fresh Cargo target, actual-plugin ABI/panic check and
code signing. The build helper embeds its September 19 build date; measurement
and this source capture occurred September 20.

Weighted ordinary-intercept OLS may now use the existing bounded numeric
pipeline and speculative raw RHS owner. The weight occupies a numeric tile
column. SPI stays on the main thread; preparation retains all domain, sample
and normalization rules. The prepared RHS must match exactly before use.
Unit-weight raw moment overlap explicitly excludes supplied weights. See
[ADR 0078](../../../../docs/adr/0078-weighted-native-input-overlap.md).

## Timings

Three fresh-process pairs per row below; each process warms the command once.
Command medians in milliseconds, with unchanged scientific comparisons.

| Request | K | Parent | Candidate |
|---|---:|---:|---:|
| Ordered AW IID | 6 | 330 | 325 |
| Ordered AW IID | 50 | 937 | 851 |
| Ordered AW full outputs | 6 | 480 | 475 |
| Ordered AW full outputs | 50 | 1112 | 1033 |
| Shuffled AW IID, fresh seed | 6 | 372 | 366 |
| Strong-d3 AW IID, N=245,760 | 6 | 92 | 90 |

Shuffled and strong-d3 narrow AW fitting remain slower than stock fixest.
Strong-d3 reference median is 83 ms. No universal win is claimed.
Separate phase calls place K=50 input at 282.70 -> 225.98 ms and RHS setup at
30.06 -> 16.11 ms; initial absorption is unchanged (380.75 -> 384.71 ms).
Total fit changes 503.83 -> 491.43 ms.

Single-pair controls show PW two-way clustered saved FE at 630 -> 617 ms (K=6)
and 1253 -> 1175 ms (K=50), and FW full outputs at 485 -> 473 and 1123 -> 1022
ms. One-FE AW IID controls improve 201 -> 194 and 538 -> 467 ms; unit K=10
remains 108 ms. Unit two-FE K=10 IID is 262 -> 262 ms; its full-output call is
410 -> 424 ms. The four-worker AW control is 157 -> 154 ms.

The initial low-memory AW K=6 full-output pair at N=61,440 slows 242 -> 258 ms.
The separate three-pair confirmation has medians 244 -> 239 ms and median
paired ratio 0.967611. No outlier is removed. `confirmation/` preserves all six
additional calls and their comparison checks.

## Validation and accounting

- `exact-pairs.json`: 34 native pairs have byte-identical b, V, status, DoF,
  certificates and fit statistics; all 15 saved-output pairs are byte-identical.
  No native allocation peak increases.
- Fifteen independent saved-output audits check group constancy, weighted
  normal equations, component normalization, reconstruction and RSS.
- Three licensed-Stata fixtures pass with application receipts and terminal
  sentinels: narrow weighted Schur, multi-FE input and eclass transaction.
- The final-source weighted loader test covers three weight kinds and nine
  sample/domain/budget/thread cases, comparing complete loaded data,
  preparation errors, samples and robust inference against ordinary loading.
  Six final-source pipeline tests also pass. The earlier two-test loaded-RHS
  run predates the corrected outer public guard; it is retained with that limit.
- Actual plugin ABI/panic checks pass. `source-audit.json`, `identities.json`
  and `implementation.patch` bind the measured source and artifacts.

The first prototype, `weightedinput`, modified inner loading but accidentally
left the public outer weight exclusion in place. Its 39 application calls
cannot establish a public-path optimization and are retained as an inactive
prototype. The corrected initial archive has 65 additional calls; confirmation
adds six, for 110 successful application calls total. All attempts and all
reference results remain. The reference metadata and controller hashes bind
the executed harness; `harness/` is the contemporary source snapshot, including
separate broader graph/science work, not a claim that every newly added option
was exercised here. Machine paths and raw licensed logs are excluded.

The broad milestone remains open: weighted/narrow operator costs, additional
graphs, reference accuracy adjudication, weight-distribution controls and the
predeclared two-block scoreboard still require work. No installed package or
remote state changed.
