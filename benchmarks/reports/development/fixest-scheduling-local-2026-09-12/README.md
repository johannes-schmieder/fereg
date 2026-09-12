# Wide-OLS scheduling: local fereg/fixest follow-up, September 12

The retained changes improve sixteen-thread complete-command medians by **7.4%
on degree three and 4.6% on degree six**. Fereg is now 5.1% and 18.6% slower
than fixest on those two designs, respectively. At four threads it remains
32.2% and 23.5% faster than fixest. One-FE performance is essentially unchanged
at four threads and improves slightly at sixteen; its Stata boundary remains
substantial. These are local development measurements, not universal parity,
SCC scaling evidence, or release qualification.

## Complete-command results

The baseline is the retained implementation from the
[numeric-pass report](../fixest-passes-local-2026-09-12/README.md). Each primary
cell has 983,040 observations, 50 controls, IID covariance, three seeds
(104729, 8675309, 20260819), and rotated baseline/candidate/fixest order.
Processes run sequentially, without concurrent compilation or testing. Each
fresh process performs a small untimed warm-up. Import, process startup and
result extraction are excluded; automatic routing and complete estimation
are included. Phase tracing is off for these medians, in seconds.

| Graph | Threads | Previous fereg | Current fereg | fixest | Current / fixest |
|---|---:|---:|---:|---:|---:|
| One FE | 4 | 0.930 | 0.947 | 0.802 | 1.181 |
| Well-connected, degree 3 | 4 | 1.307 | 1.286 | 1.897 | 0.678 |
| Well-connected, degree 6 | 4 | 1.242 | 1.187 | 1.552 | 0.765 |
| One FE | 16 | 0.700 | 0.687 | 0.448 | 1.533 |
| Well-connected, degree 3 | 16 | 0.951 | 0.881 | 0.838 | 1.051 |
| Well-connected, degree 6 | 16 | 0.875 | 0.835 | 0.704 | 1.186 |

[Ranges and RSS](primary-summary.csv), [individual calls](measurements.csv),
and [command decomposition](command-parts.csv) preserve all observations.
The machine is an Apple M2 Ultra with 16 performance and eight efficiency
cores. macOS chooses placement; there is no CPU affinity or isolated node.
Stata 19 MP is licensed for eight Stata processors, while Rust and fixest
receive the requested native 4/16-thread teams. Fixest is the same pinned
0.14.2 source build. Small differences on this host can reflect scheduling
and load. [Environment and reference identities](environment.json).

The original four-thread one-FE median is 17 ms slower. Three additional,
order-alternated pairs at seed 104729 give previous/current times of
0.927/0.919, 0.979/0.927 and 0.946/0.953 seconds. The direction does not recur
consistently; these repeats do not replace the primary observations or prove
that every one-FE workload improves.

## What changed

1. **The admitted wide-OLS recurrence keeps parallel state updates above seven
   workers.** The old cutoff was tuned on difficult chains in the shared
   iterative kernel. That shared cutoff remains intact for PPML. Scalar CG
   reductions retain their canonical order, as do convergence readiness,
   iteration caps and fallback rules.
2. **The three-column routing probe prepares group statistics in parallel.**
   Workers own disjoint groups and preserve each group's row order. The
   probe, contraction threshold and selected route are unchanged.
3. **Wide one-FE/Schur OLS certificates use fixed group partials.** Group means
   and every absolute/relative maximum retain their arithmetic. The maximum
   relative FE moment still determines acceptance at the same tolerance.
   Fixed 1,024-group partials remove the serial two-norm accumulation; the
   diagnostic two-norm can differ in its last bits. Existing norm scratch
   holds the partials, adding no workspace allocation. Narrow, PPML and
   existing weak-graph solver paths retain their certificate reductions.

No Stata transaction, SPI loader, covariance formula, sample/weight rule,
rank policy, scientific tolerance, or public option changes.

The final seed-104729 traces localize the sixteen-thread gains:

| Phase, milliseconds | Degree 3 previous | Degree 3 current | Degree 6 previous | Degree 6 current |
|---|---:|---:|---:|---:|
| Routing probe | 47.9 | 21.3 | 46.2 | 20.0 |
| Schur recurrence | 30.2 | 16.7 | 13.3 | 10.1 |
| Fresh FE certificate | 55.3 | 32.1 | 41.0 | 31.7 |
| Schur matrix operations | 72.8 | 74.8 | 55.8 | 57.9 |

These are diagnostic traces, not paired command medians: the previous traces
come from the initial cutoff screen and current traces from the final plugin.
All use the same seed and input. Matrix operations are unchanged, so their
small differences should not be interpreted as algorithmic changes.
[All phase traces](phase-timings.json).

## What did not work

Seven source-bound prototypes were screened. The cutoff-only arm has two
paired seeds; the later prototype screens have one seed per graph/thread cell.
Their single observations identify promising directions, rather than provide
precise rankings. All completed prototype calls pass the numerical checks.

| Prototype, sixteen threads | Degree 3 seconds | Degree 6 seconds | Decision |
|---|---:|---:|---|
| OLS recurrence cutoff only, median of two seeds | 0.937 | 0.871 | Retain with other changes |
| Independent four-variable Schur batches | 1.164 | 0.982 | Reject |
| Independent single-variable Schur jobs | 1.219 | 1.124 | Reject |
| Cutoff plus parallel probe | 0.905 | 0.848 | Retain |
| Also parallelize scalar reductions by RHS | 0.925 | 0.858 | Reject this addition |
| Parallel certificate ratios, serial two-norm | 0.909 | 0.854 | Insufficient benefit |
| Fixed certificate partials | 0.882 | 0.829 | Retain; final build measured above |

Independent-variable scheduling did not transfer successfully to our current
implementation. It loses shared graph traversal and accesses the existing
observation matrix with a wider stride; the screens do not isolate each
mechanism. Both versions preserve full b/V exactly, but substantially worsen
runtime, including at four threads. They have been removed from the working
source. This does not prove that every column-oriented design would lose;
it establishes that these straightforward adaptations lose to the existing
batched operator. Fixest's column-oriented layout and scheduling should be
considered together, rather than copying its worker assignment alone.

## Scaling and remaining gap

Four-to-sixteen-thread speedup changes from 1.37x to 1.46x for the degree-three
command, versus 2.26x for fixest. Native fereg computation changes from 1.62x
to 1.83x. Degree-six command speedup is 1.42x both before and after because
both thread counts improve; native scaling changes from 1.75x to 1.81x.
Fixest's corresponding command speedup is 2.20x. Different scopes matter:
native computation excludes fereg's substantial Stata boundary.

A separate single-seed eight-thread check of the changed crossover gives
previous/current/fixest times of **1.065/1.009/1.250** seconds on degree three
and **1.010/0.927/1.011** on degree six. These checks use tracing and remain
separate from the primary medians.

At sixteen threads, current input/native/outer medians are:

| Graph | Input load | Native computation | Outer command | Total |
|---|---:|---:|---:|---:|
| One FE | 0.292 | 0.234 | 0.161 | 0.687 |
| Degree 3 | 0.299 | 0.419 | 0.163 | 0.881 |
| Degree 6 | 0.300 | 0.371 | 0.165 | 0.835 |

Independent medians need not sum exactly. About 0.45–0.46 seconds still lies
in input and surrounding Stata work. Degree three needs another roughly
43 ms to match fixest in this comparison; degree six needs 131 ms, and one FE
239 ms. Eliminating small scheduling costs alone will not close all three
gaps. The next useful targets are the native data/normalization passes and
the Stata staging/input boundary, while retaining the batched graph operator
unless a better layout wins a controlled comparison. Parity remains an
objective, not an established result.

## Weak graphs and scientific checks

The following paired holdouts use seed 59047 and tracing. They are bounded
checks, not repeated performance estimates.

| Graph | Rows | Threads | Previous | Current |
|---|---:|---:|---:|---:|
| Bottleneck ring | 61,440 | 4 | 0.127 | 0.125 |
| Cycle | 61,440 | 4 | 0.222 | 0.223 |
| Disconnected blocks | 61,440 | 4 | 0.091 | 0.091 |
| Unbalanced | 61,440 | 4 | 0.096 | 0.097 |
| Weak degree 3 | 61,440 | 4 | 0.152 | 0.141 |
| Weak degree 3 | 61,440 | 16 | 0.183 | 0.156 |
| Dense two FE | 245,760 | 4 | 0.391 | 0.382 |
| Dense two FE | 245,760 | 16 | 0.320 | 0.310 |

One- and ten-control degree-three/weak holdouts also pass. These comparisons
retain the earlier independent adjudications of native fixest discrepancies
on bottleneck/disconnected designs; they do not turn those disagreements
into equal-accuracy timing claims.

The [science archive](science.json) records **140 completed measured calls**:
40 screen calls and 100 final calls, including 54 primary calls. All pass
their worker/application checks. All 24 final candidate/fixest comparisons
pass. The [full-result audit](extra-science.json) checks 35 candidate/baseline
pairs: full b/V files are byte-identical, as are samples, coefficient status,
DoF, public fit/certificate tables, solver acceptance maxima, iterations,
probe contractions and fallback. The largest relative change in the two
permitted diagnostic norm fields is **7.3e-14**.

Native reported peak allocation is exactly unchanged in all 18 primary pairs;
modeled fast-memory budgets agree in all audited pairs. One unchanged
weak/ten-control holdout reports a 3,796,248-byte lower peak in the candidate.
That decrease is not explained or claimed as a structural memory improvement.
No new cross-command RSS ranking or leak claim is made.

The retained source passes **34 focused Rust FE tests** and **four licensed
Stata fixtures**. Tests include the independent MAP oracle, exact projection
and probe agreement across 1/4/16 workers, weighted shared PCG, memory/budget
failures, forced continuation, and canonical certificate/error comparisons.
The 50-control Stata explicit-dummy oracle covers one/two FE, weights, robust
covariance and 1/4/16 native threads; saved outputs, memory modes and PPML
smoke also pass. The universal macOS build passes binary audit and the actual
arm64 ABI/self-test. [Validation](validation.json), [fixture receipt](fixture-summary.json).

## Provenance and reproduction

[Source identities](identities.json) bind the baseline, seven prototypes and
retained plugin to replayable complete source patches against base
`d44e36bfa5ad7e41a06752d1008f0b9683f8c8dd`. Every patch was applied to an
archived base and all source hashes checked. The
[incremental patch](patches/incremental-from-passes.patch) isolates this round's
Rust change from the retained numeric-pass source. The final plugin SHA-256 is
`20289553c62d5280ece81a07dc39abeecaa573430c34a280cfb3c7cc1b2df763`.

[Reproduction entry points](../../../local_fixest_gap/README.md),
[harness identities](harness-identities.json), [patch checks](patch-checks.json),
and the [numerical evidence archive](evidence.zip) accompany the report.
The initial sandbox-denied process sampler interrupted one worker before
estimation timing; its [interruption receipt](sandbox-interruption.json)
is separate from the 140 completed calls. Raw licensed logs, generated data
and binaries remain outside the repository. Historical reports are unchanged.
No SCC job, commit, push or release action was performed for this work.
