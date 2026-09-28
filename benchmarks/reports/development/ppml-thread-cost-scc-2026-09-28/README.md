# Accepted-source PPML thread-cost attribution

Development diagnostic, 2026-09-28. This profiles retained package **14c753ed…**
(main 1383d0bc; computational code and Linux d007491a… unchanged). It supplies
attribution, **not headline timing or stock-reference rankings**.

## Boundary and results

SCC **7763785**, 28 isolated E5-2680v4 slots, completes 36 applications: six
repetitions each of one-FE N30,720/K6 T1/T2/T4, its T2 low-memory alternative,
one-FE K24 T2, and dense two-FE K6 T2. The known diagnostic seed is 2026092817.
Canonical warm-up, columnwise input audit and START/START.CHECKED remain intact.
An isolated runner attaches `perf` immediately before releasing the command and
stops recording on first observed DONE. Attach settling and a short extraction
monitor tail remain in the profile. No production instrumentation or source edit.

All application, repeated-fit and identical-input cross-thread/memory science
checks pass. Every recording has positive samples and zero lost samples; all 36
requested effective native counts are confirmed. Accounting has failed=0,
exit_status=0, 89 seconds, maxvmem 1.013G; both terminal sentinels are present.
Raw sampled binaries and licensed logs remain in the remote run directory named
in submission02.json. Compact output hashes, science and process-stop receipts
are in applications02.json; aggregate CPU and runtime rows are in
profile-summary02.json. Aggregated symbol percentages are cycle weighted and
merge identically demangled monomorphizations; they are not operation counts.

| Design | Samples | Diagnostic public median ms | Finite ms | Strict ms |
|---|---:|---:|---:|---:|
| One FE T1 | 596 | 232.0 | 44.3 | 59.6 |
| One FE T2 | 1,182 | 259.4 | 55.2 | 69.9 |
| One FE T4 | 863 | 213.0 | 46.3 | 56.9 |
| One FE T2 low | 1,376 | 280.7 | 77.4 | 82.5 |
| One FE K24 T2 | 5,272 | 616.2 | 231.3 | 134.8 |
| Two FE T2 | 2,591 | 433.7 | 45.6 | 71.5 |

Phase clocks can nest and must not be added. These instrumented clocks repeat
the direction of the previously accepted T1-to-T2 loss; stock ratios remain those
of the separate unprofiled host-scope report.

## Interpretation and next experiment

Demangled Rayon helper self samples are 12.8% at one-FE T1, 31.0% at T2, and
52.3% at wide T2. They **do not establish scheduling overhead**. The T2 first
recording's largest individual helper (22.89%, mangled suffix
`h26b6a3a18f2de99a`) has a caller chain through
`solve_full_retained_rank -> fill_finite_wls_moments -> fill_retained_partitioned
-> fill_retained_dense_standardized_rows`. Its worker-side stack has the same
helper. Most code in such a helper can be the inlined numerical closure.

Source inspection identifies per-observation writes to disjoint Gram rows and
RHS entries that can nevertheless occupy shared cache lines. In particular a
six-column RHS occupies only 48 bytes and can make independent partitions share
a cache line. The narrow dense kernel is therefore a
coherent next experiment: private bounded accumulators, unchanged ordered FMA
recurrences, followed by one copy to the existing output. This is a testable
false-sharing hypothesis, not measured cache-coherence-event attribution. A
positive whole-command result must still survive independent oracles, paired
baseline/stock rotations, fresh cases and excluded-path controls. PPML already
has a scoped native owner; another outer team scope is not justified by this
profile. Rejected checked-helper inlining and general OLS host scoping stay closed.

## Failed attempt and collection

Attempt **7763777** exits 1 after the diagnostic controller rejects perf's
intentional SIGINT return (-2). The first sample file is readable, but no completed
application is accepted for that attempt. One narrow controller fix accepts
SIGINT only when it actually requested the stop, verifies nonempty output, and
ensures Stata cleanup even on profiler failure. Five stop-boundary checks pass;
the retest is 7763785. Both attempts and hashes are preserved. The controller's
sample header check was supplemented at collection with positive integer sample
counts for every file. A collection-only rsync mode error was resolved by omitting
remote permission preservation; no fit was repeated for collection.
