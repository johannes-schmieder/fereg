# Guarded OLS Schur route: local results, September 12

The implemented route beats pinned fixest on both large well-connected,
50-control designs at four threads. It cuts those fereg command times by
about 81%. At sixteen threads it cuts fereg time by about 74%, but remains
21–36% slower than fixest. One-FE time falls by 18–23%; this remains the largest
relative gap. The weak and dense holdouts improve, and a difficult cycle
returns to the existing solver with a small measured routing cost.

These changes are now in the local development source and the normal built
macOS plugin. They are governed by [ADR 0035](../../../../docs/adr/0035-guarded-wide-ols-schur.md).
The work stayed local. It does not resume the stopped SCC campaign, change
frozen qualification manifests, or establish parity on every graph.

## Repeated complete-command comparisons

All cells have 983,040 observations, 50 controls and IID covariance. Entries
are medians over the same three seeds, with estimator order rotated within
each cell. The timer includes the complete estimation command and automatic
routing, with import, process startup, the small warm-up and extraction outside
the timer. Phase tracing is off. Every candidate/baseline/fixest group passed
its numerical comparisons at the unchanged tolerances.

| Graph | Threads | Baseline fereg | New fereg | fixest | New / fixest | Reduction from baseline |
|---|---:|---:|---:|---:|---:|---:|
| One FE | 4 | 1.260 | 1.034 | 0.795 | 1.30 | 18% |
| Well-connected, degree 3 | 4 | 7.630 | 1.404 | 1.916 | 0.73 | 82% |
| Well-connected, degree 6 | 4 | 6.962 | 1.297 | 1.548 | 0.84 | 81% |
| One FE | 16 | 0.995 | 0.769 | 0.447 | 1.72 | 23% |
| Well-connected, degree 3 | 16 | 3.946 | 1.009 | 0.835 | 1.21 | 74% |
| Well-connected, degree 6 | 16 | 3.692 | 0.936 | 0.686 | 1.36 | 75% |

Times are seconds. Both degree-three and degree-six designs have two FE
dimensions; degree is observations per worker. This block contains 54
invocations and 18 paired scientific checks. The [summary](primary-summary.csv)
contains all ranges and median RSS. Three seeds provide useful local evidence,
not a precise estimate of performance across machines and datasets.

Hardware and references are unchanged from the preceding investigation:
Apple M2 Ultra, 16 performance plus 8 efficiency cores, 192 GiB RAM, Stata 19
MP and source-built fixest 0.14.2. Stata's own processor limit is eight; the
Rust and fixest teams receive the stated 4/16-thread requests. macOS does not
provide controlled CPU placement for this experiment. Launch loads, actual
thread observations and input hashes are retained. See [environment](environment.json).

Memory remains a relative strength. Median absolute process-tree RSS for
new fereg is 1.39 GiB for one FE, 1.73 GiB for degree three, and 1.54–1.61 GiB
for degree six. Fixest uses 1.80–1.99 GiB across these cells. Schur adds some
workspace relative to baseline fereg, especially degree three at four threads
(1.63 to 1.73 GiB). Sampled RSS is diagnostic; the new route also has an explicit
pre-allocation workspace bound and reserves room for continuation.

## What changed, and why it matters

**The solver now works on the smaller FE coefficient system.** Fereg's existing
Schur kernels are reused for full-width OLS. The larger FE dimension is
eliminated analytically; the smaller system is solved without constructing a
dense FE matrix or an explicit Schur matrix. This avoids expensive multigrid
work on well-connected, wide models. Prepared group memberships support
parallel initialization and reconstruction while preserving each group's
observation order.

**The recurrence now stops on the scale used by the true certificate.** The
old prototype checked a residual group mean divided by the raw RHS norm. The
fresh FE certificate also includes the square root of the group's weight.
Large groups could therefore pass the cheap recurrence check too early,
then fail fresh certification and require expensive observation-space MAP
polishing. The old code still certified every published answer; the issue was
wasted work. Adding the missing scale makes the representative degree-three
solve take 12 useful Schur steps instead of stopping at 11 and polishing.
Degree six takes 11. This explains much of the improvement over September 11
and overturns the earlier inference that the weak holdout intrinsically
favored the old backend over Schur.

**One-FE OLS reaches its exact initial-projection path.** The outer planner had
sent it through the generic direct-LSMR wrapper, bypassing the intended direct
initial entry point. It now performs the existing exact projection immediately.
The fresh certificate's residual norms move with the projected matrix into OLS,
avoiding another full scan. Fixed-row LASSQ reductions also parallelize raw and
residual norms without changing their partitioning across worker counts.
Weights, samples, rank decisions, tolerances and covariance formulas are intact.

**Selection is guarded.** Eligible fast, unit-weight, two-FE fits with 16–64
actual RHS columns and at least 16,384 rows run an eight-step probe on three
RHSs. Only sufficient contraction selects Schur. Other cases retain the
existing CMG/LSMR/CG routes. Saved-FE requests, weights, narrow models,
low-memory execution and additional FE dimensions are outside the new route's
current envelope. Selected solves have a 64-step cap and a certified CG
continuation that shares the original iteration budget. The probe and any
failed work are charged to the report. The guard is based on observed
convergence, not benchmark design names.

The probe costs about 45–58 ms in the large traced fits, approximately 4–5%
of the resulting complete command. That overhead is already in the main table.
A three-column probe cannot guarantee that every other RHS is equally easy;
the cap and continuation address that failure mode. This is not a proof of
uniformly small routing regret on arbitrary graphs.

## Protecting weak-graph performance

These are individual diagnostic holdouts at seed 59047, separate from the
large repeated block. Phase tracing is enabled. A timing difference of a few
milliseconds should not be interpreted as a confirmed speed difference.

| Holdout | Rows | Threads | Baseline | New | fixest | Route |
|---|---:|---:|---:|---:|---:|---|
| Weak degree 3 | 61,440 | 4 | 0.359 | 0.145 | 0.401 | Schur |
| Weak degree 3 | 61,440 | 16 | 0.303 | 0.196 | 0.169 | Schur |
| Dense two-FE panel | 245,760 | 4 | 0.994 | 0.417 | 1.045 | Schur |
| Dense two-FE panel | 245,760 | 16 | 0.623 | 0.312 | 0.438 | Schur |
| Bottleneck ring | 61,440 | 4 | 0.303 | 0.122 | — | Schur |
| Disconnected blocks | 61,440 | 4 | 0.316 | 0.094 | — | Schur |
| Unbalanced FE counts | 61,440 | 4 | 0.302 | 0.097 | — | Schur |
| Cycle | 61,440 | 4 | 0.224 | 0.229 | — | Probe rejected; previous route |

The first four candidate/fixest comparisons pass. Every row passes full
candidate/baseline comparisons, including the constant and covariance matrix,
exact sample, statuses, rank and DoF. The three custom-graph baselines before
the cycle are reused from the earlier same-day diagnostic block; other
baselines here were measured alongside the final candidate.

The cycle's probe contraction is 0.100, versus the 0.01 selection threshold;
its probe costs 3.1 ms. The weak graph's contraction is 0.00451 and the dense
panel's 0.00321, so both select Schur and improve over baseline. Four separate
1/10-control strong/weak checks retain their previous routes and exactly match
exported full coefficients and covariance. Their timings are essentially
unchanged. The 16-thread weak fit is slower than the four-thread fit: using
more workers for a small problem remains an improvement opportunity.

These observations protect the particular strengths tested here. They do not
establish non-regression for every weak graph, and the probe/cap design still
needs broader holdouts before qualification.

## Scientific disagreements remain visible

The bottleneck and disconnected native fixest comparisons failed the existing
parity envelope. They are excluded from equal-accuracy speed claims. Separate,
column-equilibrated SciPy LSMR solves of the full explicit control-plus-dummy
model used all 61,440 observations and did not call either estimator's absorber.

On the bottleneck ring, baseline and final fereg agree with that independent
solution to 8.9e-15 in the largest coefficient difference. Native default fixest
has a largest coefficient error of 1.68e-6 and a relative RSS difference of
3.42e-6. Its recorded runtime, 3.875 seconds, remains a diagnostic result; we
have not timed a tightened, equal-accuracy fixest fit. See the
[bottleneck adjudication](explicit-final-bottleneck_ring.json).

On disconnected blocks, coefficients agree, but default fixest reports 40,399
residual degrees of freedom versus the independently verified 40,430 in fereg.
The graph has 32 components, explaining the difference of 31. Rescaling the
reference covariance solely for that DoF difference reduces the largest
covariance discrepancy from 4.95e-9 to 1.54e-19. This is a native default DoF
convention difference, not evidence of an incorrect coefficient solve. The
original timed fit and failed covariance comparison remain unchanged. See the
[disconnected adjudication](explicit-final-disconnected_blocks.json).

## What still keeps fixest ahead

The remaining gap is concentrated in serial loading, data movement and
thread scaling. The [complete-command decomposition](command-parts.csv) gives
these independently computed medians for the final candidate:

| Design, 16 threads | Input load | Native computation | Outside native/preflight | Total |
|---|---:|---:|---:|---:|
| One FE | 0.293 | 0.321 | 0.155 | 0.769 |
| Degree 3 | 0.296 | 0.556 | 0.161 | 1.009 |
| Degree 6 | 0.298 | 0.486 | 0.156 | 0.936 |

Preflight is about 0.0004 seconds. Independent medians need not add exactly.
The outside-native column includes Stata work and publication after the
native pre-store timestamp, not just syntax parsing. Loading plus that
boundary still costs about 0.45 seconds, approximately fixest's entire
one-FE call. This is a limit of the current implementation, not an immutable
Stata lower bound.

Final traced one-FE work at sixteen threads includes 65 ms of RHS construction,
70 ms of initial absorption, 32 ms of the finite solve, and 52 ms of bread/raw
residual work. On degree three, the Schur operator itself takes only 72 ms;
its recurrence takes 32 ms, reconstruction 6 ms, and fresh certification
54 ms. Removing all recurrence time would therefore not close the whole gap.
The [phase records](phase-timings.json) separate warm-up from target fits; the
last record of each phase type is the target. All six final profiles reproduce
the corresponding untraced candidate's exported full b and V exactly.

Two further ideas were implemented and rejected in isolated copies. A
coefficient-space Irons–Tuck variant needed 27/15 operator applications where
PCG needed 12/11, and was slower. Moving missing detection into the native
batch loader, using Stata's authoritative missing-value callback, also made
the tested commands slower. Neither change is retained. Their patches,
artifact identities and measurements are preserved. Fixest's pinned source
already contains specialized coefficient-space iteration and parallelism;
the evidence does not support assuming that it has received little optimization.

The next work should attack the remaining passes and serial costs: bind RHS
recipes once per tile, fuse compatible preparation/moment work, improve the
independent raw-residual kernel, and reduce boundary work while preserving
Stata's sample and missing-value semantics. Small wide models need a cheaper
parallel execution policy. These targets follow from measured costs; they are
not implemented improvements or a promise of universal parity. Reducing the
probe's roughly 50 ms cost is useful, but insufficient on its own.

## Validation and reproducibility

This report preserves 148 completed measured invocations: 63 exploratory and
85 on the final artifact, including 54 primary comparisons, 25 holdout/narrow
calls and six profiles. All workers returned success; native parity failures
against fixest remain separately recorded. All 123 fereg invocations pass the
native FE, rank, score, bread, covariance and determinant certificate checks.
There are 83 recorded full native comparisons. The collection also preserves
all 23 passing fixest comparisons and four disagreement records spanning the
exploratory and final artifacts.

Focused validation passed 33 borrowed-FE tests and 58 prepared-OLS tests.
These include weighted/padded Schur kernels, deterministic worker-count
comparisons, LASSQ extreme scales, independent MAP projection, probe rejection,
workspace capacity, forced continuation, and shared iteration accounting.
Seven licensed fixtures passed their explicit return codes and terminal
sentinels: wide 50-control explicit-dummy robust/weighted comparisons, weighted
OLS, OLS and PPML smoke, saved outputs, memory modes and factor parsing.
The normal universal macOS build passed binary auditing and the actual-plugin
ABI/panic-containment self-test on arm64. The x86_64 slice was built and audited,
not scientifically exercised. See [validation](validation.json) and the two
fixture summaries linked there.

The final plugin SHA-256 is
`ffa5537a0da609888de55ad122973140f86f48700a3368fb2f3098a785d072ac`.
It is an uncommitted local source snapshot based on
`d44e36bfa5ad7e41a06752d1008f0b9683f8c8dd`, identified by the
[full source manifests and patches](identities.json), not by the base SHA alone.
The baseline artifact is bound to the preceding report. Eleven source patches
pass application checks against the base source snapshot. The initial sandbox
RSS failure and corrected shared-Cargo-cache build attempt are recorded in
[investigation notes](investigation-notes.json); neither contributes a timing.

[All measurements](measurements.csv), [scientific checks](science.json),
[numeric evidence](evidence.zip), [source patches](patches/) and the
[local reproduction harness](../../../local_fixest_gap/README.md) retain the
work without licensed logs, datasets, binaries or machine-specific paths.
No release, frozen F0, SCC or cross-platform qualification claim is made.
