# Light OLS/PPML comparison

Owner-approved development benchmark: ten sequential SCC bundles, each on a
28-physical-core E5-2680v4 node, with four- and 28-core estimator affinity,
three fresh-process repetitions, and a five-minute target-command cap.
`common.py` is the exact matrix. Its emitted manifest contains 40 scientific
configurations and at most 1,200 timed fits. It is separate from the stopped
systematic campaign and does not constitute release qualification.

The candidate is clean source `e23f15cad16c2f0b2368326aea579921a069ca9a`.
Comparators reuse the byte-verified pinned systematic closure. The candidate
is built in the new run directory; installed packages and old receipts remain
unchanged. `runtime.json` binds source, harness, executable and reference bytes.

## Measurement

Every timed fit starts in a fresh process, after a small untimed initialization,
target loading, and columnwise IEEE-double import verification. Estimation,
inference and requested FE extraction/materialization are timed. Disk export
and independent diagnostics are not. Each command retains its residual or
mean for scientific auditing; `save_fe` adds individual effect channels. R and
Python already retain their fitted state. This output contract applies to all
repetitions and is disclosed rather than represented as a minimal no-output
command benchmark.

PSS is sampled concurrently, following the fevc paper's definition: the maximum
sum of current process PSS across complete process-tree sweeps, not a sum of
individual process peaks. The observer is excluded. Loaded data and runtime
storage are included. Nominal intervals are 20 ms for synthetic N<=400,000
and 50 ms otherwise. Fewer than ten complete in-command sweeps are under-resolved
and excluded from headline memory ratios. RSS, USS, swap, excluded huge pages,
sampling gaps and observer work remain in receipts. Timings are instrumented;
observer CPU is not an estimate of causal slowdown. No separate memory campaign.

Four-core fits use the same four physical cores for every package. All 28 slots
remain reserved; the observer can use the other cores. At 28 cores it shares
the allocation. Stata's processor license ceiling is separately recorded.
Reghdfe is not described as running on 28 application cores under a four-core
license. Nested BLAS is limited to one thread.

Within each bundle, competitors and core counts rotate. A target failure or
timeout skips that arm's remaining repetitions at that scientific configuration
and core count. Unsupported capabilities are explicit. In particular, gtools
does not supply general two-way clustering or absorbed slopes, and the pinned
PyFixest interface does not supply absorbed varying slopes.

## Execution and evidence

`job.sh` is the only compute entry point. Task 1 builds the candidate, verifies
the reusable comparator closure, and executes a single consolidated startup
check against small explicit-dummy OLS/Poisson oracles. Its startup receipt
releases tasks 2--10. All tasks use at most 12 hours, with an internal cleanup
deadline at 11h45 and per-call checkpoints. No automatic matrix expansion,
walltime increase or additional recovery task is allowed. One narrow operational
repair/retest is permitted, preserving the failed attempt.

Startup requires every supported adapter to execute and capture its result,
and requires every native explicit-dummy oracle to pass. Comparator numerical
disagreements remain failed scientific checks, recorded in the startup receipt
and excluded by the same campaign eligibility rules; they do not silently become
passing checks or force tighter comparator tolerances. Stata-created numerical
output variables use double precision. Gglm's public `predict()` is a link-scale
fit; its IRLS working residual is not a response residual.

Private Veneto inputs are supplied through an owner-only deployment manifest,
not committed paths or data. The short panel contains 1999 and 2001; the full
long panel contains 1996--2001. Raw log wages/positive wage levels and jointly
estimated age/year controls replace the fevc paper's adjusted outcome while
preserving its sample. An unverifiable recovery blocks that empirical cell.

`science.py` checks retained order, published coefficient/predictor binding,
fresh scores, objective replay, saved-FE reconstruction and independent span
feasibility. The two-FE checker is the existing independent spanning-forest /
grounded-LSMR implementation. Audit timeout remains indeterminate. ADR 0026
governs ppmlhdfe's reported objective and public-mean diagnostics. Missing
covariance or sample evidence never implies numerical parity.

Only three completed, scientifically comparable repetitions enter headline
timing ratios. Every error, timeout, unresolved diagnostic and unattempted fit
remains in the report. Private detailed outputs stay owner-only; collection
exports only allowlisted aggregate evidence. The final LaTeX report uses
individual vector panels and the fevc paper's visual conventions.

Focused local check:

```sh
python -m unittest discover -s benchmarks/light -p 'test_*.py' -v
```

Current campaign progress belongs only in `STATUS.md`.

## Owner amendments on September 19

The short Veneto panel is omitted at the owner's request. Its original
unattempted receipts remain historical evidence; do not recover, substitute,
or rerun that sample. The long panel continues unchanged.

The owner permits 30 minutes per PPML regression and up to 12 hours per SGE
task. `followup.sh` runs one dependency-held continuation after the original
campaign ends. `followup_plan.py` selects only recorded PPML command timeouts,
with fresh native controls for paired comparisons, and freezes the selected
inputs and original receipt hashes. It excludes nonconvergence errors and OLS
timeouts. Each selected group gets three fresh repetitions, with later
repetitions skipped after execution failure. The task deadline can leave
explicit unattempted groups; there is no automatic further timeout increase.

The original five-minute campaign and its harness remain immutable. Collect
with `collect.py ORIGINAL OUTPUT --omit-short --followup CONTINUATION` after
both are terminal. The report presents longer-allowance results separately,
so original failures remain visible and timings from different hosts or
attempts are never paired together.
