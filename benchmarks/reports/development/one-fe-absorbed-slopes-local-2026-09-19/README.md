# One-FE absorbed-slope performance investigation

Fereg's automatic OLS slope route spends most of its time in repeated,
serial FE certificates. This is a missing specialization for one partition,
not a difficult convergence problem or primarily a Stata input bottleneck.
Fixest precomputes the local slope systems, projects each RHS directly, and
parallelizes across RHSs. No estimator change is adopted by this investigation.

The related one-FE profiling thread's 10/50-control cases use intercept-only
absorption. Those controls are ordinary estimated coefficients, not absorbed
group-specific slopes. This investigation explicitly tests `absorb(g1##c.z)`
against fixest's `g1[z]`. The light campaign's slope variants instead involve
multiple partitions; these findings do not measure that separate graph problem.

## Matched local measurements

Unweighted IID OLS, 245,760 observations, 49,152 groups of five rows, a varying
continuous slope, no requested saved effects, and the frozen one-FE generator
with seed 104729. The extra slope is standard normal with seed 719; the outcome
adds `0.3*sin(g1)*z`. Literal input bytes are audited in both applications.
Times exclude dataset import and use fresh processes with a small warm-up fit.

| Controls | Requested native threads | fereg auto, ms | fixest, ms | fereg / fixest |
|---|---:|---:|---:|---:|
| 10 | 1 | 368 | 95 | 3.87 |
| 10 | 16 | 342 | 54 | 6.33 |
| 50 | 1 | 1,041 | 666 | 1.56 |
| 50 | 16 | 864 | 193 | 4.48 |

Each slope entry is the median of three fresh-process measurements; ratios
are ratios of medians. Execution order alternates between applications.
The matched intercept-only controls at 16 threads take 53/49 ms with ten
controls and 145/206 ms with fifty controls (one observation per application).
These controls localize the large regression to slope handling; their single
measurements are not a general intercept-only speed claim.

In the stock slope calls, initial absorption plus final residual absorption
takes approximately 258 of 342 ms for ten controls and 673 of 864 ms for fifty.
The finite regression takes only 2.6 and 11.5 ms respectively. Initial slope
absorption barely changes from one to sixteen threads: 182/189 ms for ten
controls and 599/605 ms for fifty. No slow-convergence explanation is needed:
the initial and residual solvers each finish after one MAP projection.

## What the source and detailed timers establish

1. `PreparedCategoricalOlsData::fit_absorb_initial_matrix` admits the fast
   initial one-FE route only for intercept-only effects. Slope auto dispatch
   goes through `fit_absorb_owned_matrix`, copying the owned FE representation
   and materializing unit weights.
2. Generic `absorb_auto` computes raw norms and an initial certificate, then
   calls `absorb_map`. MAP recomputes raw norms and the same initial certificate,
   projects once, and computes a third certificate. The generic MAP routine
   does not return early when its initial certificate passes. The final
   residual projection repeats this pattern. The traces show six certificates
   and two projections per command.
3. Each generic certificate serially visits every group, constructs normalized
   slope/intercept columns, allocates and computes a rank-aware reorthogonalized
   QR basis, scans every RHS against that basis, and separately computes raw
   moment diagnostics with further scans. These checks preserve scientific
   guarantees, but their current organization repeats substantial work.
4. The projection and certificate implementations have no parallel group/RHS
   traversal. Increasing the native thread count helps other phases but barely
   changes absorption. Fixest's pinned `demeaning.cpp` precomputes the local
   slope systems in `FEClass`, selects `demean_single_1` for one partition,
   and executes the RHS loop with OpenMP.

Two instrumented runs per width preserve all arithmetic and routing:

| Detailed measurement, 16 threads | 10 controls | 50 controls |
|---|---:|---:|
| Six certificates combined | about 241 ms | about 650 ms |
| Two actual projections combined | about 14 ms | about 33 ms |
| Owned FE construction, both calls | about 4 ms | about 4 ms |
| Complete instrumented command | about 356 ms | about 917 ms |
| Certificate share of instrumented command | about 68% | about 71% |

Function times are nested: MAP and auto totals must not be added to the
certificate/projection totals. Instrumented runs are separate diagnostics,
not replacements for the stock timing table. Their complete commands are
roughly 4–6% slower than the corresponding stock medians.

The diagnostic fixest runs spend about 28/71 ms in demeaning, including
workspace and output handling, and about 4/41 ms in finite fitting. Thus fereg
already has a fast finite-regression stage on these cases; absorption is the
clear priority. Explicit `solver(map)` takes 258/771 ms in one measurement
per width. It uses the borrowed slope implementation and saves work, but it
retains serial slope processing and does not close the gap.

## Recommended implementation boundary

Add a certified direct route for one partition with absorbed slopes. First
remove duplicated initial certification by handing the initial state through
the solver, and let already-certified final residuals return without an extra
projection. Preserve one independent terminal certificate and the authoritative
original reference scales. Then reuse stable group-local slope geometry and
schedule independent group/RHS work on the existing bounded team. Prefer
borrowed finalized topology over constructing another owned design.

Retain rank-aware handling of constant/zero/collinear slopes, weight semantics,
finite-value/error precedence, memory admission, independent FE and OLS
certificates, and failure atomicity. A centered intercept-plus-one-slope kernel
with guarded rank fallback is a sensible first implementation. Generic
multiple-slope and multi-partition paths should remain available. Do not infer
that removing all certificates is acceptable or that this investigation proves
the achievable end-to-end speed of a future implementation.

## Evidence and reproduction

There are 38 successful application calls: two small oracle calls, 24 repeated
slope timings, four intercept controls, two explicit MAP diagnostics, four
instrumented fereg calls, and two instrumented fixest calls. All require the
application return code, terminal sentinel and imported-byte audit. Fifteen
stock fereg/fixest pairs pass coefficient, covariance, RSS, sample and DoF
comparisons at existing tolerances. Both small fits pass a separately computed
explicit-dummy least-squares oracle (360 rows, rank 146, residual DoF 214).
All six instrumented calls reproduce their stock scientific exports exactly;
both MAP diagnostics agree with auto within the existing tolerance.

- [Measurements](measurements.csv), [application receipts](calls.json), and
  [detailed function/phase profiles](profiles.json).
- [Independent and comparator checks](science.json) and
  [instrumentation checks](diagnostic-science.json).
- [Exact source/artifact/input identities](identities.json),
  [source comparison](source-check.json), and
  [diagnostic-only source patch](instrumentation.patch).
- [Measured driver](driver.py). Supply `FEREG_SLOPE_ROOT`,
  `FEREG_SLOPE_R_DEPS` and `STATA_MP_PATH`; make `Rscript` available on PATH.
  Copy the four workers/helpers from `benchmarks/local_fixest_gap/` into
  `$FEREG_SLOPE_ROOT/controller/`. The driver uses the isolated residual-overlap
  source/artifact and pinned R library locations from the preceding one-FE
  campaign. Run `driver.py smoke` and `driver.py primary` with the existing
  benchmark Python environment. Rebuilding the diagnostic requires an isolated
  copy of the baseline, the archived patch, a distinct Cargo target, the normal
  release compiler settings, and the actual-plugin ABI test. Run the same
  slope cells against that artifact and compare complete scientific exports.

Baseline plugin SHA-256 is
`cebf82abba46ecde7947dfe7f5a1143acf5d8bdf241d7821611cbfef3dcd92f0`.
The five inspected dispatch/solver files match committed `e868808d` exactly;
the full snapshot manifest, including historical base/overlays, is retained.
Fixest is archive-pinned 0.14.2, SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
The diagnostic plugin passes the actual-loader ABI test.

This is bounded local macOS arm64 development evidence on an uncontrolled
scheduler. Stata itself has an eight-processor license; the Rust and fixest
kernels receive the recorded 1/16-thread requests. Other project activity may
affect short timings, so no universal speed, Linux/Windows result, or release
qualification is claimed. Production code, installed packages and remote jobs
were not changed. Raw licensed logs, local data and binaries remain outside
the tracked report.
