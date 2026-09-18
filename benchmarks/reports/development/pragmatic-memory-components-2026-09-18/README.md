# Explicit CMG components and partial whole-action memory reporting

Development evidence, 2026-09-18. **Adopt explicit-CMG components; do not accept
the whole-action memory accuracy claim.** This is not a release candidate or
formal performance qualification.

## Implemented decisions

Explicit `solver(cmg)` selects the pinned component engine with coarse-isolate
pruning, preserved unpruned stopping and terminal component factors. Automatic
routing stays ordinary. The private `FEREG_CMG_ORDINARY` seam permits same-binary
comparisons. Existing certified CG correction/fallback remains; there is no new
ordinary-CMG retry, tolerance change or dependency modification.

OLS and PPML publish separate 11-by-2 presence/value memory tables. These
preserve legacy runtime layouts and distinguish missing baselines, partial
estimates and calibrated practical coverage. Stata allocated storage is read at
entry and immediately before the native call. Used storage is unavailable with
multiple frames because the Stata counter is frame-local. Allocated storage is
not RSS and must not be subtracted again from OS available memory.

The native observer now covers argv decoding through error handling and ABI
return without another per-allocation atomic. A fixed-size phase forecast keeps
fast OLS projection parents separate from later finite/inference banks. PPML
uses its generation/phase inventory. Publication growth counts an e(sample)
copy, numeric matrices and an explicit 64-KiB descriptor estimate. These are
uncalibrated engineering estimates. They never use the measured allocator peak
as their prediction. Conservative admission, runtime headroom and strict
coverage bits remain separate. Combined peak and additional reserve remain
unavailable; status is partial.

Action/OLS-result/PPML-request/PPML-result versions are 1.6/1.5/1.6/1.3;
model wire stays 2.1. Both matrix payloads are validated before publication and
participate in rollback. Public `e(*_action_protocol)` strings were corrected
from 1.5 to 1.6 after the licensed runs; the static protocol/packager check covers
that metadata-only correction. The tested Rust sources match the r6 snapshot.

## Source and validation identities

- Clean baseline: `8dc40a24`; `baseline-source.sha256` binds its isolated build.
- Component pairs: preserved memory overlay plus reporting r5, bound by
  `component-source.sha256`. Artifact and input hashes are in every arm receipt.
- Final memory pairs: r6, bound by `native-r6/source.sha256`, its identity and
  artifact receipt. This includes earlier uncommitted memory work; the clean
  baseline comparison cannot isolate the cost of the report alone.
- CMG remains `1d39c113f0c397f29653d92672090e5a42f80921`.
- Fixed cases: `pragmatic_memory_development_v2.json`. V1's unsupported
  continuous product was replaced with supported numeric factor expansion;
  the original failure remains recorded. No selection based on timing occurred.

SCC jobs reserved 28 slots on 28-core nodes; native compilation used four jobs,
fixtures used one or eight native threads. Stata's license limited its own
worker team to four. No unrelated job was touched.

Job 7632442 passes 148 plugin tests, 10 core protocol tests, 41 CMG tests and six
prepared PPML CMG tests. Linux build, export audit and actual C SPI loader pass,
including injected errors, allocation/store failures, panic containment and
whole-ABI observation reset checks. Zero-match filters in an earlier attempt
are not counted. Job 7632448 builds/audits the clean baseline.

Local Stata 19 passes `FEREG_MEMORY_BASELINE_PASS`: numeric/string storage,
multiple frames, held estimates and malformed memory shadows. The 59-test
Python run passed 55 checks before four in-process inventory-identity errors;
the four pass on the focused stable-inventory rerun. Four shared-memory audit
and wire-metadata tests pass. Repository checks and reviewed OLS v34/action v19
structural inventories pass. These inventories are not capacity proofs.
Mac linking remains blocked by the unaccepted Xcode license.

## Paired results

Each case has three alternating fresh-process pairs. Samples/order, coefficient
status, DoF, FE/cluster metadata and applicable separation ledgers agree; b/V,
certificates and saved outputs satisfy the fixed comparison tolerances. Focused
native CMG tests retain their independent oracle checks.

Component job 7632403: all 18 pairs pass. Command medians (seconds):

| Case | Ordinary | Components |
|---|---:|---:|
| Connected OLS | .311 | .305 |
| Tiny components | .296 | .291 |
| Mixed components, clustered | .329 | .336 |
| Tiny weighted bridges | .245 | .250 |
| Low-memory clustered OLS | .505 | .496 |
| PPML, saved effects/residuals | .362 | .356 |

No command or comparable phase exceeds both +5% and +10 ms. This is bounded
non-regression evidence, not a general speedup claim. Mixed and low-memory OLS
retain one reported certified fallback/correction in both variants; PPML has
zero. Scratch for low-memory OLS is 9,437,184 bytes in both variants. Native
allocation and sampled process RSS, including changes in either direction, are
retained in `components-phase-summary.json` and the arm receipts.

Memory job 7632768: all nine baseline/candidate pairs pass. Command medians are
.255/.262 seconds (narrow OLS), .729/.750 (wide OLS with missing values and
factor-generated columns), and .284/.284 (one-FE PPML). No command or comparable
phase crosses the declared threshold. Allocation, RSS, scratch and fallbacks
are in `memory-phase-summary.json`. These are local development comparisons.

## Accuracy decision: not accepted

The fixed target is absolute error no greater than max(10% of independently
observed native memory, 64 MiB). The final candidate gives:

| Case | Native estimate MiB | Whole-ABI observed MiB | Criterion |
|---|---:|---:|---|
| Narrow OLS | .86 | .41 | pass |
| Wide OLS, missing/generated | 85.90 | 81.70 | pass |
| Connected CMG | 162.05 | 14.17 | fail |
| Tiny components | 396.36 | 13.98 | fail |
| Mixed components | 290.40 | 19.95 | fail |
| Weighted bridges | 54.50 | 3.18 | pass |
| Disconnected three FE | 6.92 | 6.56 | pass |
| Slopes, saved outputs | 6.31 | 3.91 | pass |
| Low-memory CMG inference | 178.24 | 14.68 | fail |
| One-FE PPML | 8.89 | 2.44 | pass |
| CMG PPML, saved outputs | 140.38 | 9.44 | fail |
| PPML separation | 16.52 | 10.07 | pass |

Passing the absolute-error allowance does not mean ten-percent relative
accuracy. The wide OLS difference is about 5.1%; CMG still combines engineering
runtime allowances and worst-case construction reservations with expected
payload. A global fitted multiplier would hide that distinction and is rejected.
The next change must carry a separate CMG expected ownership/lifetime estimate
through construction, retained state, failed attempts and PPML parent overlap,
while keeping conservative admission intact. Stata publication growth and
whole-process/RSS uncertainty also remain to be calibrated.

The calibration gate failed, so the four declared holdouts and German 5% rerun
were not launched. No German full-sample run, campaign expansion, installed
artifact replacement, push, release, issue edit or CMG-repository write occurred.

## Evidence and failures

`components-*.json` and `memory-*.json` contain input/source/artifact identities,
per-arm timings, sample hashes, scientific outcomes and allocation observations.
`diagnostics.zip` preserves fixed-size public matrices and native stderr traces;
it excludes licensed Stata logs and observation-level data. The harness copies
and `harness/analyze_pairs.py` reproduce the audits from those records. Display
paths in text/JSON receipts are normalized; source/artifact hashes are unchanged.
Scheduler receipts show terminal zero exit/failure for accepted jobs (7632403,
7632442, 7632448, 7632768). The last job took 33 seconds with 1.928G job maxvmem;
that scheduler metric is distinct from native allocation and sampled RSS.

`failed-attempts.json` records original failures. Their isolated source and raw
logs remain preserved remotely, and no failed receipt was relabeled as passing.
The corrected final audit first failed on an inactive Stata matrix placeholder,
not estimation; its regression test now selects the declared native estimator.

The obsolete uncompiled `fe/cmg_container_memory.rs` draft remains untouched
and untracked. Clean committed inventories exclude that absent file; preserved
worktree inventory overlays include it. Both pass independently; their hashes
are in `inventory-snapshots.json`. No compiled code differs for this reason.
