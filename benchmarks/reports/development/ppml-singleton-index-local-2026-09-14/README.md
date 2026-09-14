# Prepared PPML singleton queues — local development checkpoint

The singleton fix now covers every production OLS and PPML pruning entry path.
The OLS fix is already in parent `73aef645edc2`. This incremental change replaces
production prepared PPML's separate repeated-membership-pass implementation.
Fast, low and automatic memory modes, solver selection, covariance, supported
weights, interactions and slope-bearing partitions use this same PPML boundary.
No Stata SPI call or observation allocation before PPML admission is introduced.

These are local Apple Silicon development results, not release or cross-platform
qualification. The universal plugin was audited for both slices and executed on
ARM. Installed dist was not changed.

## Implementation and contracts

The immutable generation already owns CSR membership and partition codes. The
new pruner counts exact `u128` frequency multiplicity, or physical rows for unit
and probability weights. It queues each deleted row once, updates every affected
level, and scans a level's CSR slice for its survivor only when its mass reaches
one. That happens at most once per level. It emits sorted compact-row proposals
by reusing the queue in a final selection-mask scan. Each singleton fixed-point
invocation is O(D*N + L). The complete multi-method separation/rebase sequence
is not asserted to have that complexity.

Initial activity, exact frequency arithmetic, interactions, policy order,
separation-induced cascades, ledger generations, final ordering, interruption
boundaries and transactional publication remain authoritative. No-drop samples
skip the queue and selection-mask allocations. OLS retains its existing no-drop
exit and allocation envelope.

PPML now budgets N words for the reused queue/output, two N-byte masks, 16 bytes
per level, D+1 word offsets, and borrowed source-view metadata. This can increase
the pruning peak on high-cardinality graphs; the planner explicitly charges it
before generation-zero admission for both memory routes and cascade overlap.

## Source and validation

- Parent source: `73aef645edc26d2477492ed357d5e612a48aaf81`.
- Baseline plugin: `253bf9eb6e406331f101c3354ceff560ad2a36916e6c97a7582208cc275cf5f2`.
- Candidate plugin: `312c75468e53e77f63d8c04c727651684968592d27a200f4543bc797ad3f5b74`.
- `source.patch` contains the incremental implementation, focused fixture and contracts.
- `built-identities.json` identifies all 177 compiled inputs. The copied main
  sources match every entry (`main-patch-replay.json`).
- 302 PPML core tests pass, including memory admission, preflight rejection,
  weights, separation/rebase transactions and certificates.
- 640 deterministic unit/frequency cases match an independent simultaneous-pass
  oracle exactly, across one to five effects and initial exclusions. Additional
  cascading tests protect frequency-pinned rows. A scale-doubling test bounds
  membership visits by 2*D*N and retains an independent stable cycle.
- Licensed fixtures `feppml_singleton_index`, `feppml_native_preparation` and
  `ols_singleton_index` pass. The new fixture makes 14 native fits across fast/low,
  unit/FW/PW, clustering and interactions, including a cascade induced by exact
  FE separation. It checks exact masks/reasons, rank/status, DoF, normalization,
  covariance and original order. An explicit-dummy Poisson oracle checks
  coefficients and fitted means independently.
- Universal static and actual ARM ABI/self-tests pass. No cluster work ran.

## Measured pruning and command costs

The standalone driver constructs membership before timing, then includes the
working activity mask and pruner allocations. It compares the retained old
prepared-pass algorithm with the actual new helper. Three serial alternating
repeats per shape/size give 54 final calls; every retained mask is identical.

| Shape, rows | Old kernel, seconds | New kernel, seconds | Old/new peak temporary bytes |
|---|---:|---:|---:|
| Cascade, 8,196 | 0.119107 | 0.000124 | 147,528 / 213,136 |
| Cascade, 16,388 | 0.473284 | 0.000244 | 294,984 / 426,128 |
| Cascade, 32,772 | 1.893556 | 0.000476 | 589,896 / 852,112 |
| 50% singleton, 32,772 | 0.000221 | 0.000256 | 589,896 / 589,952 |
| No singleton, 1,048,580 | 0.001732 | 0.000806 | 18,874,440 / 1,048,668 |

An initial counter draft regressed the balanced kernel. It was rejected; the
final unit-weight counter avoids per-row checked `u128` additions. Its earlier
diagnostic results and build remain in local evidence. The one-pass singleton
control retains a roughly 0.03 ms kernel overhead; this is reported, not hidden.

Complete-command checks use six small synthetic PPML cases, three alternating
artifact pairs each (36 commands), with no concurrent builds or other tests.
They include native loading and Stata command work; input generation is outside
the timer. The cascade has a stable 4,096-row crossed-panel component.

| Case | Old command, seconds | New command, seconds | Old/new singleton-plus-FE-separation phase, seconds |
|---|---:|---:|---:|
| Cascade, 12,288 rows, low | 0.326 | 0.297 | 0.152934 / 0.000511 |
| Cascade, 20,480 rows, low | 0.653 | 0.294 | 0.539387 / 0.000674 |
| Cascade, 36,864 rows, low | 2.123 | 0.298 | 2.004218 / 0.001000 |
| 50% singleton tail, 36,864 rows, low | 0.497 | 0.494 | 0.001929 / 0.001944 |
| Balanced, 135,168 rows, fast | 0.262 | 0.265 | 0.000804 / 0.000732 |
| Balanced, 135,168 rows, low | 1.848 | 1.882 | 0.000786 / 0.000762 |

The largest cascade improves command time 7.1x; its kernel gain must not be
presented as an estimator-wide multiplier. Small-case non-pruning costs are
material. Balanced command medians are 1.1% and 1.8% slower despite slightly
lower singleton/separation phase time; three repeats do not establish those
small changes as systematic. The 50% tail control is effectively unchanged.

The paired slope coefficient and its variance, sample counts, rank, DoF, deviance,
iteration counts, step halvings and fallback counts are exactly equal. All
solver and generic fallback counts are zero. The largest cascade's peak native
allocation delta rises 24,318,782 to 24,364,958 bytes; the other five case peaks,
selected memory models and scratch amounts are unchanged. Balanced fast/low
native peaks are 126,218,802/108,448,996 bytes; low scratch is 1,081,344 bytes.
These benchmark memory figures are native allocation measurements, not process
RSS. The complete phase, memory and per-run records are in the JSON files.

## German OLS follow-up and remaining work

The parent plugin was screened on three progressively larger worker-ID subsets
before any further full-data run. Every native and pinned reghdfe fit succeeds;
retained masks and DoF agree exactly. The input reader selected the six required
fields; estimation controls and options match the original specification.
These are single diagnostic timings, not paired performance qualification.

| Input / retained rows | Fereg low / fast, seconds | Reghdfe, seconds | Maximum coefficient difference |
|---|---:|---:|---:|
| 75,350 / 65,503 | 1.676 / 1.366 | 1.468 | 1.78e-14 |
| 151,029 / 132,691 | 10.204 / 9.771 | 8.048 | 5.58e-13 |
| 378,124 / 338,054 | 57.512 / 56.901 | 31.613 | 1.11e-11 |

The largest subset is a useful weak-graph performance control; fereg is slower
there. It does not reproduce the full model's bread-inverse failure. The earlier
full preparation result of **88,876,264 retained rows** remains valid, but full
OLS estimation still fails its bread certificate. No tolerance was relaxed.
German PPML outcome selection remains pending; no empirical PPML success is
claimed. Solver repairs and full comparator comparisons remain active work.

`validation.json` distinguishes the accepted results from setup failures.
`external-evidence-hashes.json` binds locally retained raw logs without committing
licensed logs or private data. The minimal comparator closure was insufficient
for a direct reghdfe runtime; successful OLS screens use the full offline,
lock-pinned reghdfe/ftools/require checkouts. PPML synthetic oracles are Stata's
built-in Poisson and the independent mask algorithm.

For standalone replay, `replay_ppml_scale.py` takes an isolated candidate source,
its freshly built core rlib, target/host dependency directories, a compiler and
an unused output directory. `replay_command_bench.py WORKSPACE STATA_EXECUTABLE`
expects isolated `baseline` and `candidate` packages beneath WORKSPACE and writes
fresh run directories. Replay helpers were syntax-checked; the original harness
runs, rather than another redundant campaign, are the measured evidence here.
