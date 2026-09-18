# CMG expected heap separated from admission

Development evidence, 2026-09-18. Implementation commit `290a2ffb` corrects the
CMG forecast attribution identified in the [preceding investigation](../cmg-memory-attribution-2026-09-18/README.md).
**Adopt the separate native-heap channel; retain partial whole-action status.**
This report does not establish a 10% RSS forecast, a hard memory bound, or a
qualified production candidate.

## Change and invariants

Solver diagnostics now carry conservative admission and expected heap separately.
The expected channel uses actual retained capacity unions, constructed hierarchy
dimensions, component factor fill and selected workspace banks. It adds an
engineering estimate for the largest construction scratch stage. Shared prepared
owners count once; sequential attempts use maxima; simultaneous parents add.
Finest-edge spare capacity still counts after duplicate aggregation.

Runtime heap allowances and requested worker stacks remain admitted at the
unchanged policy. They do not masquerade as predicted Rust heap. Unknown opaque
failed-constructor temporaries make the expected channel unavailable. OLS
polishing/fallback and PPML prepared parents, checkpoints, retries and basis
rebuilds carry both channels without mixing them.

No numerical arithmetic, tolerance, sample, weight rule, solver routing,
component default, dependency pin, memory admission limit or public matrix schema
changes. Explicit CMG continues to default to components; automatic routing
remains ordinary. Private forecast traces use revision 2. Expected heap is a
post-construction engineering forecast, not a replacement for preallocation
admission. It never reads the allocator peak to manufacture its prediction.

## Source and validation

Fresh isolated snapshots/builds bind baseline `e7ad6501` and the candidate in
`native-r2/*-source.sha256`; artifact hashes and the dependency pin are in
`identities.json`. The 194 compiled Rust/Stata/ABI/configuration files checked
against that candidate match the local implementation. Inventory/documentation
updates are later, noncompiled changes; this is not exact-final-SHA qualification.
Both builds use independent Cargo target directories.

Native SCC job 7639913 passes 205 focused Rust checks (5 lifetime-carrier, 42 CMG,
6 reusable PPML CMG, 4 PPML frame/retry and 148 plugin tests). Linux export audit
and actual plugin/mock-SPI loading pass for both artifacts, including deliberate
panic containment. The existing allocation/interruption/rollback checks and
independent numerical CMG oracles remain. The new forecast oracle covers ordinary
and component engines on one and 256 independent blocks. The failed r1 assertion
is preserved in `native-r1`, with its correction explained in
`failed-attempts.json`.

Four focused Python memory/audit checks, three allocation-inventory checks,
repository checks and clean action v20/OLS v35 inventory audits passed locally.
The obsolete uncompiled CMG draft remains untracked and untouched. Clean committed
inventories omit it; working overlays include it. `inventory-snapshots.json`
records both identities. Structural inventories are not byte-completeness proofs.
Mac linking remains blocked by the unaccepted Xcode license. The installed Mac
plugin is unchanged; no installed artifact, upstream CMG source, remote branch,
release or stopped campaign was changed.

## Native-memory calibration

All twelve input and column-hash bindings equal the previous fixed calibration.
Job 7639948 passes every native-memory case under ADR 0074's criterion:
absolute error at most max(10% of the observed native peak, 64 MiB).
The independently observed measure covers ABI entry through return.

| Case | Previous forecast MiB | New forecast MiB | Observed MiB |
|---|---:|---:|---:|
| Connected CMG OLS | 162.05 | 22.85 | 14.17 |
| Tiny components | 396.36 | 18.65 | 13.98 |
| Mixed components, clustered | 290.40 | 29.80 | 19.95 |
| Tiny weighted bridges | 54.50 | 4.93 | 3.18 |
| Low-memory CMG inference | 178.24 | 30.77 | 14.68 |
| CMG PPML, saved outputs | 140.38 | 13.81 | 9.45 |

The new CMG errors are approximately 1.8–16.1 MiB. The absolute allowance is
material: these are **not** 10%-relative results. No global multiplier was fitted.
The narrow, wide, three-FE, slope, one-FE PPML and separation controls also pass;
all native errors and process RSS observations remain in `calibration/`.

Three independent holdouts in job 7639985 and the corrected wider-control holdout
in job 7639998 pass their scientific and native-memory checks:

| Holdout | Forecast MiB | Observed MiB | Error MiB |
|---|---:|---:|---:|
| Double connected-CMG rows | 45.66 | 30.23 | 15.43 |
| 65 controls plus two factor indicators | 108.88 | 71.22 | 37.66 |
| 75% tiny-component observations | 29.81 | 20.85 | 8.96 |
| Eight separated PPML groups | 16.52 | 10.01 | 6.51 |

The original `wider_k` receipt is invalid as a width holdout: it generated 65
controls but inherited a 50-control RHS. It is preserved, never relabeled, in
`holdouts/`; `holdouts-r2/` contains the single corrected retest and requires
model DoF 67. The German job was held while queued until this correction passed.
No other holdout was rerun. Single holdout pairs establish scientific checks,
not a three-pair performance claim.

## Paired cost and science

Job 7639948 runs three alternating fresh-process baseline/candidate pairs for
each of nine predeclared cases, using the same component policy in both arms.
All 27 comparisons pass: exact retained sample/order and structural diagnostics,
plus b/V, certificates and saved outputs at the frozen comparison tolerances.
No command or comparable phase exceeds both +5% and +10 ms.

| Case | Baseline seconds | Candidate seconds |
|---|---:|---:|
| Narrow OLS | .210 | .209 |
| Wide/missing/generated OLS | .730 | .739 |
| Connected CMG | .315 | .315 |
| Tiny components | .294 | .295 |
| Mixed components, clustered | .341 | .350 |
| Weighted bridges | .248 | .250 |
| Low-memory inference | .500 | .503 |
| One-FE PPML | .249 | .259 |
| CMG PPML, saved outputs | .358 | .357 |

OLS native allocation medians are unchanged. Saved-output CMG PPML increases by
3,072 bytes; sampled RSS varies in both directions. Mixed and low-memory OLS
retain one certified correction/fallback in both arms, and low-memory scratch
remains 9,437,184 bytes. Other paired cases report zero fallbacks. Full-command
cost includes discarded work. Phase times, native peaks, RSS, scratch and fallback
counts are preserved in `calibration/phase-summary.json` and individual receipts.
These bounded comparisons do not make a universal speed claim.

SCC jobs reserve 28 slots on 28-core nodes. Compilation uses four jobs; synthetic
fixtures use one/eight native threads and the licensed Stata team is at most four.
Native and paired jobs have terminal sentinels and scheduler failed=0/exit=0;
calibration job 7639948 lasts 87 seconds with 1.926G scheduler maxvmem. This metric
is distinct from native allocation and sampled Stata RSS.

## Unchanged German 5% check

The single approved native OLS run (job 7639994) completes in 285.070 command
seconds, 290.152 process seconds. Input hash remains
`6498d6051aac8cce1ff545c1a0228f655fcf7d6e88845b0c6f1ce564e6e7f59e`.
It uses the unchanged controls, `solver(auto)`, four native threads,
`memory(fast)`, saved residuals and 40-GiB native limit. This empirical check
exercises the general forecast; it is not an explicit-CMG performance test.

All seven scientific tables match the preserved native result byte-for-byte:
b, V, status, fit, DoF, FE and certificate. All ten saved columns are numerically
exact, including retained mask/order, residual and fitted outcome. Retained rows
are exactly 4,361,733. No independent/reference/full-sample estimation was rerun.

The forecast is 3,705,301,141 bytes versus independently observed native peak
3,570,575,744 bytes: **+134,725,397 bytes, or +3.77%**. This passes the relative
criterion without relying on the 64-MiB allowance. Whole-process peak RSS is
4,347,969,536 bytes and is kept separate. Scheduler accounting is
failed=0/exit=0, 307 seconds and 5.222G maxvmem; the terminal sentinel is retained. The run establishes neither a paired
speedup nor new adjudication of the historical scientific reference discrepancy.

Stata allocated storage increases from 718,866,956 bytes before the command to
820,192,832 after it. Its used-storage increase is 41,644,475 bytes. The public
pre-plugin baseline is 719,627,084 bytes and the narrow publication-growth
estimate is 4,628,641 bytes. These boundaries are different: the before/after
change includes persistent saved output, e(sample), metadata and allocation
segment effects. This is useful evidence for the remaining Stata lifetime/
publication calibration, not evidence that the publication estimate covers
all of that growth. Combined peak and additional reserve remain unavailable.

## Evidence use

`diagnostics.zip` contains fixed-size public memory/runtime/solver tables and
native stderr traces, excluding licensed Stata logs and observation-level data.
Display paths are normalized in text and harness copies; original input/source/
artifact hashes remain unchanged. Original raw harness hashes bind remote bytes,
not the path-normalized display copies. Raw failed and successful runs remain in
isolated SCC directories. Replay the sixteen accepted synthetic native-memory
checks, the German audit and paired gates without estimation:

```sh
python3 benchmarks/reports/development/cmg-expected-heap-2026-09-18/audit.py
```

Whole-action acceptance still requires calibration of Stata publication growth
and explicit treatment of allocator retention, opaque runtime and requested
stacks. Public combined peak and additional reserve remain unavailable. Strict
construction/dependency proof limitations remain separate from this pragmatic
native forecast.
