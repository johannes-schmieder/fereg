# Narrow numeric loading and RHS reuse

Local Mac development on September 20, 2026, using Stata/MP 19.0 and the pinned
stock fixest library. The retained change extends existing transport and unit
moment overlap to narrow ordinary-intercept OLS at eight or more workers,
including a dedicated loaded-RHS consumer binding. Exact prepared-value
matching and all scientific/memory admission rules remain. Ordinary predictor,
residual and RHS-construction fast-path guards are unchanged.
[ADR 0081](../../../../docs/adr/0081-narrow-ols-loaded-rhs-reuse.md) defines the scope.

## Runtime results

Three-pair command medians, milliseconds, N=983,040 and sixteen native workers:

| Design/request | K | Parent | Candidate | Default fixest |
|---|---:|---:|---:|---:|
| Ordered unit IID | 1 | 206 | 204 | 337 |
| Ordered unit IID | 3 | 225 | 218 | 353 |
| Ordered unit IID | 5 | 253 | 247 | 370 |
| Ordered unit IID | 6 | 237 | 236 | 375 |
| Shuffled unit IID | 1 | 227 | 222 | 224 |
| Shuffled unit IID | 3 | 244 | 237 | 230 |
| Shuffled unit IID | 5 | 285 | 273 | 249 |
| Shuffled unit IID | 6 | 269 | 258 | 258 |
| Shuffled AW IID | 6 | 342 | 338 | 300 |
| Shuffled AW all outputs | 6 | 498 | 484 | 553 |
| One-FE unit IID | 1 | 78 | 67 | 69 |
| One-FE unit IID | 5 | 114 | 100 | 102 |

At N=245,760 and K=6:

| Graph/weight | Parent | Candidate | Default fixest |
|---|---:|---:|---:|
| Degree three/unit | 72 | 73 | 78 |
| Degree three/AW | 90 | 87 | 85 |
| Degree six/unit | 69 | 66 | 67 |
| Degree six/AW | 83 | 82 | 71 |
| Unbalanced/unit | 75 | 73 | 72 |
| Unbalanced/AW | 94 | 88 | 81 |

These separate medians do not substitute for the paired ratios retained in
`final/summary.json` and `final/native-pairs.json`. No cell's median native
paired ratio exceeds 1.05. Some remain slower than fixest, and near-ties need
the planned independent five-pair blocks. Ordered AW gains are small. The
single-pair robust, clustered/saved and full-output checks at K=1/5 all win
against default fixest; their numerical output disagreements remain explicit.

The unit shuffled K=5 profile reduces RHS work 13.90 -> 9.25 ms and fit-time
raw moments 4.18 -> approximately 0 ms; the moments now run during preparation.
Preparation is 28.42 -> 27.60 ms. The AW K=6 profile reduces RHS work
13.79 -> 8.30 ms but retains its weighted moment scan; other phase variation
leaves that one command at 340 ms in both arms. Profiles localize work and are
separate from the three-pair timing evidence.

## Science, memory and retained failures

`initial/` contains 107 calls from a prototype whose inner numeric binding
still rejected narrow RHS reuse. It often rebuilt the matrix after loading.
The modest initial results are retained, not treated as evidence for the final
consumer. A newly expanded Rust fixture initially used noncontiguous source
IDs and failed before loading; the corrected fixture remaps IDs explicitly.
Both failure and correction are preserved.

`final/` contains 226 successful application calls. All 77 native pairs retain
identical b, V, status, DoF, certificate and fit files; combined with the first
screen, all 114 pairs are exact. Native output files also remain exact. Seven
independent output audits pass. Seven default-reference output mismatches stay in
`series.json`; they do not become scientific passes or replace default timing
with a tightened reference. Prior weighted and dense reports retain separate
matched-accuracy comparisons.

Three final-artifact Stata fixtures pass with process status and both terminal
sentinels. Focused core tests pass allocation-pointer reuse and stale/malformed
fallback, negative zero/extreme finite values, retained-row and worker
boundaries, moment admission and deferred errors. Six transport tests cover
missing classification, capacity and failure shutdown. The weighted plugin
test covers 108 width/weight/sample/domain/budget/thread combinations. Actual
plugin ABI and panic containment pass.

Two K=1 output cells raise native allocation peaks by about 4.04 MB: two-way
cluster/saved FE rises 145.44 -> 149.48 MB (2.8%); full outputs rises
121.84 -> 125.87 MB (3.3%). Robust K=1 adds 72 bytes. These peaks remain below
the unchanged early fast-memory estimates. Earlier activation of the existing
RHS/moment owners is a measured lifetime cost, not a zero-memory-cost claim.
The other 74 final pairs do not increase native peaks. Two unchanged
low-memory/four-thread controls exceed their early fixed estimates in both
artifacts; their measured peaks are identical. The complete model also accounts
for solver workspace and declared coverage gaps. `memory-plan-audit.json` and
`memory-plan-adjudication.json` preserve this distinction; this is not a full
memory qualification.

## Identity and remaining work

Final plugin `narrowinput3` is
`ed58adbeb34ef6ff34a56b3ad90b98ea9962f387b939f554d4896f9add30a24b`,
built from `c9635415` plus the five source overlays in `final/implementation.patch`.
All 515 captured source entries match. The helper embeds a September 19 build
date; measurements are September 20. The timed preliminary artifact is
`narrowinput2`, plugin
`06bea1324b6e3a3930cf04798361d258322dc287f2cc7f6db4c7b66799cf10ee`.
The first `narrowinput` build was not timed. Parent is `weightededges2` from
the [weighted cache checkpoint](../two-fe-weighted-edge-cache-local-2026-09-20/README.md).

The broad milestone remains open. The next work is repeated-edge operator
coverage on both gathers, weighted finishing scans and duplicate probe work,
followed by all 111 controls, sixteen declared fresh holdouts and two final
five-pair blocks. No installed package, remote state or qualification changed.
