# Performance contract

Performance is an acceptance gate for the 0.5.0 OLS beta, not a
marketing claim. Timing or memory comparisons count only after exact sample,
omission, coefficient, VCE, DoF, fit, output, and certificate parity passes
against the pinned `reghdfe` closure. PPML is outside this OLS comparator
contract and has a separate additive gate under ADR 0018.

Shared-engine and PPML-development checkpoints also require the direct
candidate-versus-F0 gate in `fereg_regression_contract.md`. That gate uses the
same `fereg` command in both arms, a scientific-first nine-pair AB/BA design,
and stricter 1.02 aggregate, 1.05 confirmed-cell, allocation-class, leak, and
RSS limits. It supplements rather than replaces the external comparisons here.

## PPML solver performance gate

PPML timing is inadmissible until the same cell passes the frozen candidate,
exact pinned `ppmlhdfe`, and independent-oracle scientific checks. The PPML
matrix, memory model, route calibration, warm/fresh equivalence, leak checks,
and constrained-memory behavior are governed by ADR 0018 and
`benchmarks/manifests/ppml_memory_performance_v1.json`. The exact candidate
source must also pass the direct candidate-versus-F0 `fereg`
gate so shared-engine reuse cannot hide an OLS regression.

The PPML runtime threshold is evaluated only from source-, artifact-, dataset-,
comparator-, and environment-bound receipts on the registered cells. Profile
definitions, benchmark scripts, or successful unit tests are not performance
evidence. Public availability is not a performance claim; qualification still
requires the complete scientific, transaction, platform, and `fereg`
non-regression matrix to be exact-green on the same source.

Required benchmark reporting:

- wall-clock median and dispersion across warm runs;
- peak resident memory;
- effective sample and singleton/separation deletion counts;
- full-model simplex width, checked peak-cell bound, executed/skipped status,
  and any same-generation comprehensive-IR closure;
- FE/IRLS iteration counts and final certificates;
- requested and selected solver family, concrete and effective engine,
  checkpoint route, per-engine frames/retries, and any fallback reason;
- thread count, CPU model, OS, Stata edition/version, and plugin hash;
- comparison against the pinned reference versions in `references.lock.toml`.

Approved performance cases contain at least 10 million observations and cover
dense and sparse two-way FEs, low mobility, absorbed slopes, clustered VCE, and
moderate regressor counts. Comparator and candidate arms run on the same node
and in the same job with registered one- and eight-thread comparisons; 16/32
thread scaling is supplemental where useful work justifies it.

The governing runtime gates ultimately apply to default `solver(auto)`. Auto
remains MAP while explicit MAP, LSMR, and CMG are optimized and compared; no
automatic selection rule is inferred before the separate calibration campaign.

- geometric-mean candidate/reference runtime ratio no greater than 0.40
  (at least 2.5x faster);
- at least 80 percent of approved cases at least 2x faster;
- registered single-thread comparisons at least 1.5x faster; and
- no unexplained case more than 25 percent slower.

The governing memory gates are:

- observed native allocation within 25 percent of the registered model;
- bounded full-model simplex execution within its joint explicit-design,
  construction-scratch, null-space, tableau, and certificate estimate;
- an over-cap simplex case that allocates neither its N-by-P design nor its
  tableau and is closed by comprehensive IR on the same ledger generation,
  plus simplex-only `r(909)` behavior;
- clean constrained-memory solver fallback or pre-allocation failure;
- a constrained execution-mode case where `memory(low)` succeeds while
  `memory(fast)` fails preflight; and
- no repeated-call leak.

Process-RSS and old comparator/MAP ratios remain diagnostic, including cases
where a faster certified route deliberately spends more resident memory.
Solver development compares the three public families with the pinned default
`ppmlhdfe` comparator. It starts with the versioned local science and smoke
profiles. When an SCC development campaign
is explicitly authorized, it uses small diagnostic cells on comparable
28-core nodes with roughly 10--20 independent tasks in parallel. Those runs
are not qualification evidence. Large-sample and broad route-calibration work
requires separate authorization, and the frozen release-grade matrix is rerun
only after `solver(auto)` routing is fixed.

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

The original registered Stata 19 command-comparison matrix is defined by
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

Optimization round V2 is separately frozen in
`benchmarks/manifests/ols_command_comparison_stata19_v2.json`; it does not
rewrite the captured V1 evidence. V2 excludes `areg`, crosses 0, 2, 10, and 30
controls with one-, two-, and three-FE designs (the third dimension is an
explicit ten-level year effect), and records requested native threads
1/2/4/8/16 at one million and ten million observations. Core one-FE,
standard-AKM, and standard-AKM-plus-year cells use the complete scaling ladder;
low-mobility and remaining control-count coverage use the one/16-thread
endpoints. Each scope retains one cold plus five warm calls for fereg, Greg,
and pinned reghdfe in fit and saved-FE modes.

V2 distinguishes requested threads from effective comparator capacity. fereg
always receives the requested native value, including 16. Stata-side work is
capped at `min(requested,c(processors_lic),c(processors_mach))`. Greg exposes no
documented per-call thread control, so it is repeated on every requested row
but labeled uncontrolled rather than assigned an invented effective count.
The harness records both the cap and fereg's reported native thread count and
fails if fereg does not honor the request.

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
