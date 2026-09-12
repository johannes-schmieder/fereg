# Systematic OLS and PPML benchmark

This additive harness implements the September 10, 2026 benchmark design. It
does not modify estimators or supersede release qualification manifests.
`manifest.json` fixes the scientific grid; `references.json` fixes acquired
source archives. Candidate production source is the clean `d44e36b` revision.

## Experiment

Both estimator families cross eight graph designs, five observation counts up
to 3,932,160, and three views: maximum capacity, four-core budget, and the
1/2/4/8/14/28 core ladder at 983,040 observations. Ten controls define the main
grid. Four/50 controls and zero-outcome Poisson cases have separate bounded
appendices. The 304 unique configurations have one cold and six initialized
fresh-process comparisons, or 2,128 comparison groups before smoke/pilot extras.
Overlapping figure cells are measured once. Multiple estimators execute
sequentially in each group, and multiple groups are packed into each SCC task.

The strong/bottleneck membership construction is ported from
`fevc/fevc/benchmarks/fevc_matlab_2026/generate_input.do` in the sibling
varcomp_hdfe checkout. The paper topology, including its shallow hub-tree
construction, is preserved; response/control construction is specific to these
OLS and PPML benchmarks. Econometric designs use five observations per worker,
20%/2% mobility and a crossed county effect. PCG64 seeds and separate control
streams make input regeneration deterministic. All programs consume the same
IEEE-double-preserving DTA file, with exact columnwise binary import hashes.

The positive-count PPML response adapts the existing solver-development
generator. The zero appendix uses unmodified Poisson draws and native sample
selection. Neither inputs nor solver tolerances are repaired after observing
results. Main samples must be singleton-free and strictly positive for PPML.

## Timing, memory and science

Each fit runs in a fresh process, after an untimed 7,680-row initialization
using the same API and column types. No target-dataset model state is reused.
Cold diagnostics omit initialization. Timers surround the public estimator,
including its requested covariance calculation. Parsing, import checks,
initialization, extraction and serialization are untimed.

The parent records absolute process-tree RSS at the invocation boundary and
every 10 ms during the call. A sampler handshake precedes the command timer,
so even a sub-10-ms command has a boundary observation. Shorter transient
peaks can still be missed. No baseline subtraction is used. CPU-affinity
budgets and library limits prevent oversubscription; process thread counts
are diagnostic, not claims about useful parallel work.

OLS requests IID and PPML robust covariance. R/Python PPML request HC0 through
their public small-sample options; extracted covariance is multiplied by
N/(N-1), the existing Stata PPML contract. Native algorithms and stopping
criteria remain at defaults. The PPML rank comparison uses total finite plus
absorbed rank, following ADR 0022: the normalized constant is assigned to
different reported rank fields in feppml and ppmlhdfe. All original fields and
the applied covariance multiplier remain visible.

An independent bounded SciPy explicit-dummy oracle checks candidate slopes
and standard errors. Reference comparisons check exact input/sample identity,
control names, coefficients, covariance, appropriate DoF identities and
reported objectives. Per-call candidate certificates remain active. Gtools'
restricted exposed state is labelled diagnostic. A missing field or failed
command cannot become a passing scientific comparison.

Only six complete, valid repetitions enter a finite figure point. Ratios are
formed within each seed/node comparison first, then summarized by median and
IQR. Raw times, failures, accuracy disagreements, 2,400-second command timeout
bounds and omitted points remain in machine-readable results. No timeout or
scientific error automatically triggers a solver change.
An initialization error yields no warm target timing and must not be described
as evidence that the larger target fit fails. The independent cold invocation
is still attempted. `runs.csv` distinguishes failures before target timing from
failures during invocation/extraction; the underlying logs retain the exact error.

## Workflow

Before submitting jobs, export `FEREG_SCC_RUNS_ROOT` as the absolute parent
allowed to contain `RUN_ROOT`. Setup also requires `FEREG_SCC_RUST_ROOT`, the
isolated Rust installation containing `cargo/` and `rustup/`. These replace
local deployment paths in the original captured controllers. Historical
controller receipts describe their original bytes; no campaign is resumed by
this configuration change.

1. Acquire exact archives with `acquire.py`, freeze the candidate archive,
   and deploy into a new SCC run root with `input/` and `controller/`.
2. Submit `setup_job.sh` with `RUN_ROOT`. It builds the candidate with Rust
   1.85.1, stages the digest-checked Stata closure, and installs isolated R and
   Python environments. Preserve setup accounting and dependency inventories.
3. Generate the `smoke` plan with `campaign.py`; submit `job.sh` with `PLAN`
   and `RUN_ROOT`. Validate application records and oracle comparisons.
4. After smoke acceptance, run `provenance.py freeze RUN_ROOT` against the final
   deployed controller. The inventory binds actual candidate and comparator
   runtime bytes. Generate/run the bounded `pilot` plan. Inspect accuracy, timings, RSS,
   28-core placement and scheduler accounting before production.
5. Generate the `production` plan from pilot results. It packs measured work
   into target-eight-hour bundles, at most twelve hours per task. It selects
   memory from observed peaks plus 25% headroom. Use at most twelve concurrent
   array tasks. Keep the core ladder together where measured budgets permit.
6. Capture each job with `accounting.py RUN_ROOT JOB_ID TASK_COUNT`, then audit
   terminal tasks. Report finalization requires accepted accounting for every
   measured arm as well as consistent runtime identities. Resume only
   checkpointed, unstarted groups; completed
   records are immutable. Technical failures retain their original evidence.
7. Once complete, run `report.py RUN_ROOT OUTPUT_DIRECTORY`. It exports each
   panel as a vector PDF and includes panels in 2x2 LaTeX `subcaption`
   composites. Maximum and fixed-capacity composites share core panels.
   The final PDF must be rendered and inspected before delivery.

`--allow-incomplete` is for explicitly labelled development previews only.
It must never be used to represent an incomplete campaign as a completed
report. Scientific failures remain terminal outcomes rather than missing work.
Current progress belongs exclusively in the repository's `STATUS.md`.
