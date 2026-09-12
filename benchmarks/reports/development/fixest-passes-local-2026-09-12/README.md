# Numeric data passes: local fereg/fixest follow-up, September 12

This round reduces wide-model data movement and dispatch overhead. The
baseline is the **retained Schur implementation from the earlier September 12
report**, including its one-FE direct path and guarded weak-graph fallback.
The original pre-Schur solver baseline is not used in the new timing table.

The changes are retained in the local source and normal universal macOS
plugin. Scientific tolerances, solvers, route guards, samples, weights, rank,
DoF, covariance formulas and the guarded Stata transaction are unchanged.
This is local development evidence, not release qualification or a claim of
parity across all graphs. No SCC workload was submitted or resumed.

## Complete-command comparisons

Each cell uses 983,040 observations, 50 controls, IID covariance, three
seeds (104729, 8675309 and 20260819), and rotated baseline/candidate/fixest
order. Timings include automatic routing and the full estimation command.
Import, process startup, a small warm-up and extraction are outside the timer;
phase tracing is off. Every process runs sequentially, without concurrent
compilation or testing. Values below are medians in seconds.

| Graph | Threads | Previous Schur | Current | fixest | Current / fixest | Reduction |
|---|---|---|---|---|---|---|
| One FE | 4 | 1.013 | 0.951 | 0.798 | 1.19 | 6.1% |
| Well-connected, degree 3 | 4 | 1.396 | 1.326 | 1.927 | 0.69 | 5.0% |
| Well-connected, degree 6 | 4 | 1.331 | 1.222 | 1.578 | 0.77 | 8.2% |
| One FE | 16 | 0.781 | 0.704 | 0.470 | 1.50 | 9.9% |
| Well-connected, degree 3 | 16 | 1.030 | 0.960 | 0.835 | 1.15 | 6.8% |
| Well-connected, degree 6 | 16 | 0.949 | 0.878 | 0.692 | 1.27 | 7.5% |

The [summary](primary-summary.csv) includes minima, maxima and RSS. The machine
is an Apple M2 Ultra (16 performance and eight efficiency cores, 192 GiB RAM).
Stata 19 MP is licensed for eight Stata processors; the native Rust/fixest
teams receive the stated 4/16-thread requests. Fixest is the same pinned 0.14.2
source build as before. macOS chooses CPU placement, and the host is not an
isolated SCC node. Three seeds give a bounded local comparison; small timing
differences and changes against earlier reports may reflect scheduling and
machine load. Input hashes, launch load and process observations are retained.

The one-FE/sixteen-thread candidate observations are 0.700, 0.704 and
**1.337 seconds**. The 1.337-second observation remains in the primary range
and median. Its native input time was 0.735 seconds, versus 0.296 seconds for
the matching baseline; source changes in this round do not touch that loader.
Two additional traced, order-reversed checks of the same seed gave:

| Repeat | Previous Schur | Current | Current input |
|---|---|---|---|
| 1 | 0.810 | 0.729 | 0.310 |
| 2 | 0.787 | 0.711 | 0.304 |

The delay did not recur, but its cause is unresolved. These checks are kept
separate and do not replace the original run. The median gains do not
establish a guarantee for worst-case latency on this shared host.

## Changes retained

The fast numeric path binds up to 64 generated columns once and processes
128-row tiles. It applies to numeric/intercept models with 8–64 generated
columns and at least 4,096 active rows. It reads each column's recipe outside
the observation loop and keeps a small residual tile in cache while applying
all controls. Generated-column order and each observation's fused
multiply-add sequence are preserved. Syntax omissions use the prepared model's
source mapping; numerical omissions remain skipped. Factor recipes, narrower
or larger generated widths and low-memory column blocks keep their established
evaluators.

RHS initialization reuses the allocation already reserved with the fallible
allocator. Indexed Rayon collection initializes that capacity in parallel;
it introduces neither unsafe code in the core nor a second N-by-K matrix.
Tests check that the destination address and capacity stay unchanged. Wide
finite-value checks also use the installed pool. General preparation remains
serial when no worker pool is installed, preventing validation from creating
an ambient global team. Every source and output check remains in place.

The numeric bindings are shared by the legacy and categorical prepared
representations and by independent raw-residual reconstruction. They also
support changing weighted-linear working responses, which matters for the
PPML seam. All large allocations retain the existing reservation and
failure policy; invalid dimensions, nonfinite inputs and overflowing residual
arithmetic remain fatal before any public result is posted.

## Where the savings occur

The traced profiles below use seed 104729. Baseline profiles are from the
paired initial screen with the same retained Schur artifact; final candidate
profiles use the final universal plugin. Profiles are diagnostic and are
separate from the untraced primary timing table.

| Graph | Threads | Previous RHS (ms) | Current RHS (ms) | Previous bread/raw residual (ms) | Current bread/raw residual (ms) |
|---|---|---|---|---|---|
| One FE | 4 | 108.2 | 49.1 | 45.2 | 15.8 |
| Well-connected, degree 3 | 4 | 106.9 | 51.3 | 49.8 | 18.1 |
| Well-connected, degree 6 | 4 | 109.4 | 48.9 | 39.1 | 17.8 |
| One FE | 16 | 67.2 | 28.9 | 38.5 | 9.9 |
| Well-connected, degree 3 | 16 | 67.6 | 30.8 | 42.2 | 10.7 |
| Well-connected, degree 6 | 16 | 71.3 | 31.2 | 38.3 | 11.2 |

The largest, consistent saving is independent raw-residual construction:
we stop revisiting the complete residual vector once per control. RHS gains
come from binding column recipes once, parallel initialization and parallel
validation. Initial absorption, solver iterations, routing probes and finite
regression are unchanged; differences in their recorded times are timing
variation rather than intended algorithmic improvements.

## Weak graphs, smaller models and memory

These are individual diagnostic holdouts at seed 59047; tracing is enabled.
The narrow-control checks also use 61,440 rows and four threads.

| Graph | Rows | Threads | Previous Schur | Current | fixest |
|---|---|---|---|---|---|
| Bottleneck ring | 61,440 | 4 | 0.123 | 0.122 | — |
| Cycle | 61,440 | 4 | 0.228 | 0.223 | — |
| Disconnected blocks | 61,440 | 4 | 0.093 | 0.093 | — |
| Dense two FE | 245,760 | 16 | 0.337 | 0.310 | 0.439 |
| Dense two FE | 245,760 | 4 | 0.403 | 0.393 | 1.069 |
| Unbalanced | 61,440 | 4 | 0.100 | 0.096 | — |
| Weak degree 3 | 61,440 | 16 | 0.193 | 0.188 | 0.173 |
| Weak degree 3 | 61,440 | 4 | 0.156 | 0.156 | 0.403 |

| Graph | Controls | Previous Schur | Current |
|---|---|---|---|
| Well-connected, degree 3 | 1 | 0.047 | 0.048 |
| Well-connected, degree 3 | 10 | 0.115 | 0.112 |
| Weak degree 3 | 1 | 0.042 | 0.041 |
| Weak degree 3 | 10 | 0.108 | 0.114 |

The initial weak/10-control observation was 6 ms slower. Three additional
paired runs at seeds 59047, 59059 and 59063 gave medians 0.103 versus
0.101 seconds. The original observation remains in the table; the repeat
does not establish a systematic slowdown. Millisecond differences in these
holdouts are too small for a strong ranking from one call.

Native reported peak allocation is **exactly unchanged in all 18 primary
pairs**, as are the modeled fast-memory budgets. That supports no added
observation-scale memory cost. The external RSS sampler missed the peak in
two sixteen-thread degree-three candidate calls (about 0.477 GiB sampled
versus 1.734 GiB reported inside the plugin). Other sparsely sampled calls
are flagged in [memory-quality.json](memory-quality.json). Raw RSS values
remain in the archive and summary; the anomalous median is not evidence of a
memory reduction. This round makes no new cross-command RSS ranking.

Every current/baseline holdout checks the full coefficient and covariance
matrices, sample, omissions, rank and DoF. The custom graph timing comparisons
are against the same Schur baseline. We do not rerun or recast the earlier
native fixest discrepancies on bottleneck/disconnected graphs as successful
comparisons; their independent adjudications remain in the
[earlier report](../fixest-gap-local-2026-09-12/README.md).

## The remaining boundary cost

The complete-command decomposition is retained in [command-parts.csv](command-parts.csv).

| Graph, 16 threads | Input load | Native computation | Outside native/preflight | Total |
|---|---|---|---|---|
| One FE | 0.291 | 0.258 | 0.155 | 0.704 |
| Well-connected, degree 3 | 0.304 | 0.496 | 0.156 | 0.960 |
| Well-connected, degree 6 | 0.298 | 0.418 | 0.159 | 0.878 |

These independently computed medians need not sum exactly. Preflight is
about 0.0004 seconds.

Two isolated Stata profiles split the surrounding transaction further:
syntax/candidate-marker/parser about 12–13 ms, bindings/shadows/estimate hold
12 ms, complete-input marker and missing-value precheck 41–42 ms, and result
validation/publication inside the guarded adapter 18 ms. The remaining outer
wrapper and program-return work is not separated by these stage timers.
[Stage definitions and timings](ado-timers.json) preserve what was measured.
Timers were confined to a disposable ado copy and are absent from the retained
source.

I also tested two C-bridge alternatives on the same one-FE input:
hoisting callback/pointer validation, and copying through a small row tile
before scattering into native columns. Neither established a useful
complete-command gain. The latter increased input time. Both were rejected;
the retained C bridge and Stata ado files are unchanged.

| Diagnostic variant | Runs | Median command (s) | Median input (s) |
|---|---|---|---|
| tiles | 3 | 0.735 | 0.295 |
| bridge-hoist | 3 | 0.732 | 0.289 |
| bridge-tile | 3 | 0.743 | 0.310 |

This C experiment used the earlier tiles-only Rust artifact, not the final
parallel-initialization/validation candidate. Three seeds and rotated order
were used for each variant.

These experiments give no evidence that simple C-loop tuning will remove the
input bottleneck. They do not prove a hard lower bound. The remaining useful
work is to reduce independent passes still inside native computation, improve
small-model scheduling at high thread counts, and investigate whether private
Stata staging variables can be reused to reduce setup/cleanup without changing
sample semantics or transaction guarantees. Any loading/computation overlap
must keep all SPI callbacks on Stata's main thread and use Rust-owned data on
workers. These are next investigations, not measured improvements here.

## Validation and reproducibility

139 completed measured invocations are preserved: 41 exploratory calls and
98 final calls. The final block contains 54 primary commands, 20 wide
holdouts, eight initial narrow-control commands, six additional narrow
repeats, four outlier checks and six profiles. All workers passed. All 22
final candidate/fixest comparisons passed the unchanged numerical envelope.
All 35 final candidate/baseline pairs have byte-identical exported full
b/V files, identical fit and certificate tables, identical reported fit
statistics, and identical solver diagnostics apart from timing fields.
See [extra-science.json](extra-science.json) and [science.json](science.json).

The final source passes **7 RHS tests, 60 prepared-fit tests and four licensed
Stata fixtures**. The normal universal macOS build passed binary audit and
the actual arm64 ABI/self-test. The x86-64 slice was built and audited; it
was not scientifically tested in Stata.

The focused Rust tests cover exact per-observation arithmetic at 1/4/16
threads, partial tiles, 8/16/51/64 generated-column boundaries, the 65-column
general fallback, intercepts, omissions, working responses, categorical
preparation, finite checks, insufficient reservation and overflow. Existing
prepared-fit tests supply independent mathematical and explicit-dummy oracles.
The licensed 50-control fixture compares one/two FE and weighted/unweighted
robust fits against Stata's explicit-dummy regression. Memory-mode and saved
output parity fixtures and PPML smoke pass. No full or cross-platform
scientific campaign was run.

[Validation](validation.json) binds the final artifact, source patch and
fixtures. [Source identities](identities.json) include both a complete patch
against `d44e36bfa5ad7` and an incremental patch against the previous Schur
candidate. Every patch was applied to an archived source snapshot and the
resulting file hashes checked. The [numerical archive](evidence.zip) contains
results, full b/V, certificates and input identities; licensed logs, data,
binaries and machine-specific filesystem paths are excluded. Local raw logs
remain available in the task's temporary workspace.

The [local harness](../../../local_fixest_gap/README.md) documents the existing
`route_compare.py` entry points and the new `passes_profile.py`. Historical
reports and qualification manifests remain unchanged.
