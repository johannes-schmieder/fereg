# Why fereg loses to PyFixest `within` on two-way (AKM) designs

Investigation notes, 2026-10-26, measured at `ee60ff55` before any change on
the `codex/two-fe-auto-speed` branch. All numbers below
are Mac M2 Ultra measurements on synthetic panels unless they are quoted from
the SCC campaign records. Linux remains the performance host; the Mac numbers
reproduce the SCC ratios closely (see the consistency check at the end) and are
used to explain them, not to replace them.

## Short answers

1. **fereg's two-FE solver core is not slower than `within`.** Explicit
   `solver(cmgschur)` absorbs the short panel in 0.36 s against `within`'s
   0.38 s at four threads, with 14 PCG iterations against 20 LSMR iterations
   and a cheaper preconditioner build (0.10 s against 0.18 s). CMG as a
   preconditioner is at least as good as `within`'s approximate Cholesky here.
2. **The loss comes from the public Auto policy and from fixed extra passes.**
   Auto runs a 16-iteration diagonal Schur pilot before it builds CMG, and it
   then applies the CMG cycle on one thread (`Install::Auto` is "scalar,
   team-free"). On the long panel at 8 workers that is 1.35 s of serial
   preconditioner time inside a 3.3 s fit. Explicit `cmgschur` on the same
   fit takes 1.77 s and beats PyFixest's 2.9 s.
3. **Tolerances and the certificate are not the issue.** The certificate costs
   3-5% of native time. `within`'s default output passes fereg's 1e-8
   certificate (6e-10 on the short panel, 9.9e-9 on the long one), so the
   two packages solve to the same accuracy; loosening fereg to 1e-6 saves
   nothing and currently fails with r(459) (side finding).
4. **Core scaling.** fereg's Auto has two serial components that `within`
   does not have: the scalar CMG application and the Stata input copy. Those
   cap Auto at about 2x from 1 to 16 workers while `within` reaches 4.8x.
   Explicit `cmgschur` scales 2.9x; the remaining gap is the input copy
   (0.23 s, inherent to the SPI) and the membership-indirect operator.
5. **PyFixest's saved FE are slow because they are recomputed from scratch**
   with scipy's unpreconditioned, single-threaded LSQR on the sparse dummy
   matrix, discarding the coefficients `within` already has. Estimation speed
   is unrelated: fereg recovers both channels from its PCG solution for
   +0.2 s on the short panel; PyFixest needs 22 s on the Mac (224-532 s on
   SCC for the real panel).

## Setup

- Plugin: worktree source at `ee60ff55`, `cargo build --profile profiling`
  (release + symbols), arm64 only, ad-hoc signed; Stata/MP 19 licensed for
  8 cores. `FEREG_PHASE_TIMING=1` and `FEREG_SOLVER_TRACE=1` supplied the
  phase and route traces; Instruments (`xctrace`) supplied function profiles,
  because samply cannot attach to Stata's hardened runtime.
- PyFixest main `263cf2d3` (the campaign's `pyfixest-within` arm) built from
  source with within-py 0.4.0 (PyPI wheel for timing, a source build with
  line tables for the samply profile). Default `LsmrDemeaner`: tol 1e-8,
  `adaptive` preconditioner (Jacobi LSMR, escalating to additive Schwarz
  after four stalled contractions).
- Panels shaped like the Veneto samples, connected, no singletons:
  - short: 1,386,980 rows, 693,490 workers x 2 years, 39,700 firms, 209,305
    movers, one year indicator;
  - long: 5,626,943 rows, 1,143,748 workers (2-6 years), 83,113 firms,
    441,758 movers, two age polynomials and five year indicators.
- Each cell: two or three repetitions after a warm run; medians of the last
  repetitions are shown. Stata timer seconds include the ado layer; the
  native phases come from the plugin's own clocks.

## Timings

Seconds. `pf` is `pf.feols` end to end; `within` is the raw solver on the
same codes and columns.

| Panel | Threads | fereg auto | fereg cmgschur | fereg lsmr | pf (within default) | raw within solve |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| short | 1 | 1.17 | 0.91 | 1.25 | 0.85 | 0.81 |
| short | 2 | 1.00 | 0.81 | 0.84 | 0.59 | |
| short | 4 | 0.80 | 0.56 | 0.63 | 0.48 | 0.38 |
| short | 8 | 0.69 | 0.45 | 0.59 | 0.45 | |
| short | 16 | | | | 0.46 | 0.34 |
| long | 1 | 6.46 | 5.00 | r(909) | 12.0 | |
| long | 4 | 4.36 | 2.84 | r(909) | 4.1 | 3.57 |
| long | 8 | 3.61 | 2.06 | | 2.9 | 2.22 |
| long | 16 | 3.25 | 1.72 | | 2.5 | |

Consistency with SCC: the short-panel route diagnostic there gave auto 1.80,
cmgschur 1.36, lsmr 1.45 and PyFixest 1.00 (ratios 1.8/1.36/1.45); the Mac
gives 1.67/1.17/1.31. The long panel at 16 cores gave 15.6 against 11.7 on
SCC (0.75) and 3.25 against 2.5 here (0.77).

### Where fereg's time goes (long panel, 8 workers)

Native seconds from the phase clocks; "build" is the CMG setup inferred as
projection minus the recurrence phases.

| Phase | auto | cmgschur |
| --- | ---: | ---: |
| input copy from Stata (serial, SPI) | 0.23 | 0.23 |
| categorical prepare + moments | ~0.10 | ~0.10 |
| Schur init (group RHS, left projection) | 0.23 | 0.08 |
| Schur operator applications | 0.78 (25 it.) | 0.41 (13 it.) |
| recurrence incl. CMG cycles | **1.35** (serial cycles) | 0.30 (parallel plan) |
| CMG build | ~0.37 | ~0.33 |
| certificate | 0.08 | 0.08 |
| residual absorption (second certified solve) | 0.19 | 0.19 |
| finite solve, inference setup, finalize | 0.14 | 0.14 |
| native total | 3.32 | 1.77 |

The recurrence phase is 1.35 s at 1, 4 and 8 workers in Auto: it does not
scale because `CmgSchurPreconditioner::cycle` runs `Install::Auto` with
`team: None`. The explicit route's recurrence goes 0.80 / 0.43 / 0.30 at
1 / 4 / 8 workers. The Instruments profile confirms it: in Auto one worker
thread holds 26% of all samples (`cmg::graph::Laplacian::matvec_lanes`, 11%
self) while the other seven hold about 7% each.

### Where `within` spends its time

`solve_batch` on the long panel at 8 threads: 2.22 s = 0.51 s Schwarz build
(serial approximate Cholesky of the firm Schur complement, after the Jacobi
rungs stalled) + 1.5-1.7 s of LSMR per column, 24-26 iterations, all eight
columns solved concurrently. PyFixest adds about 0.5 s of pandas/formulaic
work on top (0.11 s on the short panel). `within`'s own serial floor
(preconditioner build plus Python) is what limits its scaling, not the
iteration itself.

Structurally the two solvers are the same idea with different outer loops:
both eliminate the larger effect (workers) and precondition the firm Schur
complement, fereg with a CMG hierarchy inside PCG on the Schur system,
`within` with approxChol (clique-tree sampled Schur, randomized Cholesky)
inside LSMR on the full design. Per iteration both make two passes over the
observations plus one preconditioner application.

## Why Auto is slower than explicit cmgschur

1. **The diagonal pilot.** The qualified two-FE gates build CMG only at local
   iteration 16 (`routing_gates.rs`, "feature16 <= 0 -> Build"). On these
   graphs the Jacobi Schur recurrence makes no useful progress, so the 16
   iterations are sunk: 0.23 s on the short panel at four threads (about 30%
   of Auto's projection phase) and about 0.6 s on the long panel at eight
   workers. `within` escalates adaptively after four consecutive contraction
   ratios above 0.7 instead of at a fixed count.
2. **Scalar CMG application.** After the handoff Auto keeps the "team-free
   scalar cycle with no row operators" (`cmg_schur/bounded.rs`,
   `Install::Auto`); explicit `cmgschur` builds a `ParallelCmgPlan` with 5-6
   parallel level operators and interleaves 3-4 columns per cycle. This is
   the single largest item: 1.0 s of 3.3 s on the long panel at eight
   workers, and it is the reason Auto gains nothing from 4 to 16 workers.
3. **Deferred cells help a little.** Auto does switch to the aggregated
   (worker, firm) cell operator after the handoff (1.68M cells for 5.6M rows),
   which makes its operator 50 ms per application against 60 ms for the
   explicit route; this does not offset the two points above.

## Fixed costs that `within` does not pay

- **Residual absorption.** After the fit, fereg projects the residual seed
  through a second certified two-FE solve (`project_residual`, 2-4 CMG
  iterations plus a certificate): 0.09-0.11 s on the short panel, 0.19 s on
  the long one, 12-15% of native time without saved FE. PyFixest forms
  residuals as the linear combination of the already-demeaned columns. The
  combination of certified projections is itself certified by linearity (the
  group moments of `ỹ - X̃b` are the same combination of the certified group
  moments), so the second pass is a design choice, not a scientific
  requirement; keeping it only when saved FE or final-residual outputs are
  requested would be a scoped change.
- **Input copy.** 0.23 s for 5.6M rows x 11 columns through `SF_vdata` on the
  main thread, 0.03 s on the short panel. Inherent to the SPI; it is the one
  serial component explicit `cmgschur` still carries.
- **Certificate.** 0.03 s / 0.08 s (short / long at 4-8 workers): 3-5% of
  native time. Not a lever.
- **Stata ado layer.** 0.02-0.05 s.

## Tolerances and certification

`within`'s stopping rule is the LSMR normal-equation estimate at 1e-8 with no
independent check; fereg stops its Schur recurrence at a level-moment proxy of
5e-9 and then certifies the reconstructed residuals at 1e-8 against the
original design. Applying fereg's certificate to `within`'s demeaned output
gives 6.3e-10 (short) and 9.9e-9 (long): PyFixest's default output would pass
fereg's gate, narrowly on the long panel. Changing `within`'s tolerance to
1e-6 or 1e-10 moves its iteration count only from 17 to 22 and its time by
+-10%, so neither package is tolerance-bound. Both packages agree on the first
coefficient to 1e-12 relative. Conclusion: neither tolerances nor the
certificate explain the gap. Side finding: `fereg ..., tolerance(1e-6)` with
Auto or `cmgschur` fails with r(459) "prepared iid covariance or fit-statistic
certificate did not pass" on both panels, while `solver(lsmr)` accepts it.

## Is CMG worse than LSMR/within?

No, on these graphs it is better per iteration: 13-14 PCG iterations against
19-26 LSMR iterations, and the CMG build (graph 0.05 + hierarchy 0.04 s short;
0.19 + 0.12 s long, on the team) is cheaper than approxChol (0.18 / 0.51 s,
serial). The explicit route's per-iteration cost (operator 7.6 ms + cycle
5.7 ms on the short panel at four threads) matches LSMR's (about 9.5 ms).

Two caveats about the hierarchy itself. On the firm Schur graph the CMG
forest aggregation stagnates: short panel 39,700 -> 11,797 -> 10,948 ->
10,755 -> 9,966 -> 5,963 -> 23 vertices with cumulative nonzeros 4.3x the
finest level; long panel 83,113 -> 21,558 -> 17,953 -> 17,789 -> 17,770 ->
17,762 vertices with 1.16M nonzeros on every level, terminated by
`StagnatedFill` with no direct terminal factor. Each cycle therefore costs
about five finest-level matvecs, and the coarse end is handled by smoothing
only. Heavy-edge forest aggregation is designed for image-like graphs; firm
Schur complements of AKM panels are expander-like with hub firms. A hierarchy
that coarsened geometrically would cut the cycle cost by 2-3x and might lower
the iteration count. Second, with real Veneto mobility the SCC diagnostic
showed explicit `solver(cmg)` (fused-FWL CMG) 50x slower than `cmgschur`, so
only the Schur route is competitive; Auto never uses `cmg` there.

## Core scaling

| Long panel | 1 | 4 | 8 | 16 | 1->16 |
| --- | ---: | ---: | ---: | ---: | ---: |
| fereg auto | 6.46 | 4.36 | 3.61 | 3.25 | 2.0x |
| fereg cmgschur | 5.00 | 2.84 | 2.06 | 1.72 | 2.9x |
| PyFixest within | 12.0 | 4.1 | 2.9 | 2.5 | 4.8x |

Amdahl explains the pattern. Auto's serial parts at 8 workers are the CMG
cycles (1.35 s), the input copy (0.23 s), the Stata layer and parts of the
build: about 1.9 s of 3.6 s. Explicit `cmgschur` keeps 0.23 s input plus about
0.3 s of poorly scaling work. `within` is poorly parallel at one thread
(12 s, 2x slower than fereg) but has only the 0.5 s approxChol build and the
0.5 s Python layer as serial work, so it keeps improving to 16 threads.

Two further observations from the profiles:

- fereg's level-grouped operator (`apply_schur_parallel` via
  `parallel::group_blocks`, 22% self time) walks membership lists and gathers
  rows out of order on the firm side; `within` sorts observations by the
  dominant factor once and streams rows (`SortedCoalesced` scatter,
  `gather_apply` in 4096-row chunks). The explicit route's operator went
  0.81 -> 0.78 -> 0.41 s at 1 / 4 / 8 workers, so the 1 -> 4 step buys
  almost nothing; the row-streaming layout is the likely reason `within`'s
  operator scales more smoothly.
- Stata's own OpenMP and OpenBLAS worker threads spin (`kmp_flag_64::wait`,
  `blas_thread_server` polling `gettimeofday`) for about 16% of all sampled
  CPU time while the plugin runs. On this 24-core machine `set processors 1`
  against `8` with `threads(8)` changed nothing (2.04 vs 2.02 s), so it is
  not a cause here; it could matter when workers fill the node.

## PyFixest's saved fixed effects

`Feols.fixef()` (`feols_.py:1794`) rebuilds the sparse dummy matrix `D`
(n x (workers + firms)), forms `uhat - X beta` and calls
`scipy.sparse.linalg.lsqr(D_w, uhat, atol, btol)`: unpreconditioned LSQR on
an ill-conditioned two-way design, single-threaded in scipy, converging in
thousands of iterations. It never reuses the coefficients that `within`'s
`solve_batch` already returns in `BatchSolveResult.x`. On the short panel the
Mac took 21.8 s for `fixef()` at scipy's default 1e-6 after a 2.2 s fit; the
SCC diagnostic recorded 224-532 s. This has no bearing on estimation speed:
PyFixest's estimation path never computes or stores FE, and fereg's does so
cheaply when asked (short panel at four threads: Auto 0.80 -> 1.01 s,
cmgschur 0.56 -> 0.72 s, almost all of it in the final residual absorption
that retains the coefficient banks). "Faster without saving FE" is simply
PyFixest not doing the work, not an advantage of its estimation path.

## What could make fereg win these cases

Estimated from the measured phases; each item is a scoped change to the
qualified Auto policy and would need declaration, independent witnesses and
re-qualification per SPEC, so none was implemented here.

1. **Parallel CMG application in Auto** (use the explicit route's
   `ParallelCmgPlan` and interleaved columns after the handoff): -1.0 s on the
   long panel at 8 workers, -0.05 s short. Largest single lever and the main
   scaling fix. Fixed-worker repeatability is unaffected: the explicit route
   already repeats exactly at a fixed worker count.
2. **Adaptive handoff instead of the fixed 16-iteration pilot**: a
   stall-based rule like `within`'s (four contractions above 0.7) or a
   contraction forecast at iteration 4-8 would save most of the pilot:
   -0.2 s short, -0.5 s long. Dense designs that converge within a few Jacobi
   iterations keep their no-build path.
   With 1 and 2 together Auto would equal explicit `cmgschur`: long 2.06 s
   against PyFixest 2.9 s at 8 workers and 1.72 against 2.5 at 16; short
   0.45 against 0.45 at 8 workers, 0.56 against 0.48 at four.
3. **Skip the second residual absorption when no saved outputs are
   requested** and form residuals from the certified demeaned columns:
   -0.09 s short, -0.19 s long. With 1-3, fereg beats PyFixest on the long
   panel at every thread count measured and on the short panel at 1, 4 and 8
   workers (4 workers: about 0.47 vs 0.48); at 2 workers it stays slightly
   behind (0.65 vs 0.59).
4. **Row-streaming Schur operator** (sort once by worker, stream rows, scatter
   with per-level coalescing as `within` does) to remove the 1 -> 4 worker
   plateau in the operator: worth perhaps 0.1-0.2 s at 4 workers on the long
   panel; needs a measurement before committing.
5. **Hierarchy quality on expander-like firm graphs**: experiment with CMG's
   aggregation (or an approxChol-style terminal) for the Schur graph; the
   present hierarchy is 4-5x the finest level and has no direct terminal on
   the long panel. Potential 2x on the recurrence phase; speculative.
6. Not levers: tolerance, certificate, Stata layer, OpenMP interference.

## Other findings

- `solver(lsmr)` returns r(909) "LSMR workspace exceeds the joint execution
  plan" on the 5.6M-row panel on a 192 GB machine (`BorrowedLsmrCallMemory`
  against the solver budget), the same failure the SCC long panel showed in a
  64 GB allocation. The admission bound looks far too conservative.
- `tolerance(1e-6)` fails with r(459) for Auto and `cmgschur` (see above).
- Instruments showed 16% of sampled CPU in Stata's spinning OpenMP/OpenBLAS
  threads during the native call; harmless here.
- PyFixest reported "8758 singleton fixed effect(s) dropped" on a panel that
  has no singletons, yet kept N = 1,386,980; fereg kept the same N and both
  agree on the coefficient. Probably a reporting quirk, not checked further.

## Reproduction

The synthetic panels follow the Veneto shapes above (log-normal firm sizes,
mover share 0.30 short and 0.12 per year long, largest connected component,
singletons pruned). Profiles, logs and the local PyFixest build were working
files and are not retained; the measurements are reproducible from the
description above with `FEREG_PHASE_TIMING=1 FEREG_SOLVER_TRACE=1`.
