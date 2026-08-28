# Performance contract

Performance is an acceptance gate for the 0.5.0 OLS beta, not a
marketing claim. Timing or memory comparisons count only after exact sample,
omission, coefficient, VCE, DoF, fit, output, and certificate parity passes
against the pinned `reghdfe` closure. PPML is outside this contract.

Required benchmark reporting:

- wall-clock median and dispersion across warm runs;
- peak resident memory;
- effective sample and singleton/separation deletion counts;
- FE/IRLS iteration counts and final certificates;
- thread count, CPU model, OS, Stata edition/version, and plugin hash;
- comparison against the pinned reference versions in `references.lock.toml`.

Approved performance cases contain at least 10 million observations and cover
dense and sparse two-way FEs, low mobility, absorbed slopes, clustered VCE, and
moderate regressor counts. Comparator and candidate arms run on the same node
and in the same job with registered one- and eight-thread comparisons; 16/32
thread scaling is supplemental where useful work justifies it.

The governing runtime gates apply to default `solver(auto)`:

- geometric-mean candidate/reference runtime ratio no greater than 0.40
  (at least 2.5x faster);
- at least 80 percent of approved cases at least 2x faster;
- registered single-thread comparisons at least 1.5x faster; and
- no unexplained case more than 25 percent slower.

The governing memory gates are:

- observed native allocation within 25 percent of the registered model;
- clean constrained-memory solver fallback or pre-allocation failure;
- a constrained execution-mode case where `memory(low)` succeeds while
  `memory(fast)` fails preflight; and
- no repeated-call leak.

Process-RSS and old comparator/MAP ratios remain diagnostic, including cases
where a faster certified route deliberately spends more resident memory.
Solver development first uses the five-family
dense/sparse/slope/cluster/moderate-K matrix with shortened repetition counts;
the frozen release-grade matrix is rerun after `solver(auto)` routing is fixed.

The local solver-development cadence is intentionally separate from release
qualification:

- `quick`: the four decision-relevant arms on every 100k topology after each
  substantive solver change;
- `matrix`: every registered 100k topology and algorithm arm at a phase
  boundary;
- `milestone`: selected one-million-row cases; and
- `freeze`: selected ten-million-row cases with one cold and five warm runs.

Generated datasets are created before timing and identified by generator and
dataset SHA-256. Scientific preflights request residuals and samples separately
from timed fits. Every timed reghdfe/fereg repetition is rechecked against the
tight preflight oracle for N, coefficients, VCE, RSS, R2, and absorbed DoF.
Failed diagnostic arms may be recorded and continued only when the exploratory
continuation option is explicitly requested; the default remains fail closed.

The pinned gregress/gtools arm is a diagnostic speed target. Its distance is
reported by case and algorithm, but it is not a 0.5.0 beta acceptance oracle
because the beta scientific and runtime contract is defined against reghdfe.
Normal PLUS installations may be used for daily development, but qualification
continues to stage the exact reference closure in isolation.

CMG automatic routing has an additional route-local gate defined by
`benchmarks/manifests/ols_cmg_auto_calibration_v1.json`: hierarchy setup is
included; no selected cell may be more than five percent slower than explicit
Schwarz-LSMR; and the selected aggregate must be faster. The matrix crosses
panel length, mobility, threads, RHS width, weights, connectedness, core
cardinality ratio, nuisance dimension, and fast/low memory mode at 100k, with
fresh 1m and selected 10m holdouts. Saved-output overhead remains bounded by
the larger of 0.05 seconds or five percent.

The release-grade freeze uses at least five warm runs after one cold run.
Quick, matrix, and milestone development profiles use their registered shorter
warm cadence and cannot substitute for freeze or exact-source qualification.
Acceptance requires Stata sentinels, logs, scientific hashes, dependency and
plugin identities, and output schemas to agree. Cluster runs additionally
require their scheduler/accounting receipts. Exploratory Phase 0–1 thresholds
are superseded by ADR 0007; the solver and memory policy is superseded by ADR
0008.

The registered Stata 19 command-comparison matrix is defined by
`benchmarks/manifests/ols_command_comparison_stata19_v1.json`. It covers 13
one- and two-FE datasets at 100,000, one million, and ten million observations;
fit and separate saved-FE output; and fereg, Greg, pinned reghdfe 6.14.1, and
Stata 19 `areg, hdfe`. macOS records one/eight application threads and SCC
Linux records one/four because that Stata/MP license exposes four processors.
Each deterministic large dataset is generated and saved once, then reloaded
outside timing by fresh component processes; regeneration and sorting therefore
cannot be mistaken for estimator runtime. Fresh Stata processes isolate
case/thread shards, and the merger fails closed
unless every available command has one cold plus five registered records and a green
scientific preflight. fereg saved effects additionally require an independent
relative-infinity reconstruction certificate; comparator reconstruction
distances remain visible even when their publication rule is weaker.

Large two-way `areg, hdfe` measurements use one timed invocation per fresh
Stata process because repeated calls retain substantial process state. Each
such component has a registered 2,400-second cap. A capped command is retained
as an explicit DNF cell with an elapsed-time bound rather than omitted,
fabricated, or allowed to block the core matrix indefinitely. The fereg/Greg/
reghdfe scientific preflight remains mandatory for that cell; an areg parity
preflight is mandatory only when all registered areg timings complete. DNF
cells do not enter geometric-mean timing ratios. The five large-areg records,
when available, are independent fresh-process repetitions rather than
retained-process cache-warm repetitions; that distinction is carried in the
evidence limitations.

## Current direct-LSMR qualification evidence

ADR 0009 freezes direct Schwarz-LSMR for slow-mixing automatic routes. The
complete 100k matrix has 148 timing records and ten green preflights; only two
quarantined explicit-IT diagnostics fail closed. Fresh-seed 1m and untouched
10m holdouts pass the independent stability, hard scientific, runtime,
allocation-model, and leak gates. At 10m, routed low mobility reduced warm
runtime by 76.9 percent at one thread and 50.7 percent at eight threads against
the exact pre-change CG plugin. Unrouted standard cases did not regress. The
final plugin and receipt hashes are sealed in
`benchmarks/expected/direct_lsmr_qualification_v1.json`.
