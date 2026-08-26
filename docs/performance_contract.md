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

- default `solver(auto)` peak RSS no greater than 1.30 times the matched frozen
  `df53822` fereg MAP baseline for every case/thread pair;
- observed native allocation within 25 percent of the registered model;
- no repeated-call leak; and
- a constrained-memory case where `memory(low)` succeeds while
  `memory(fast)` fails preflight.

Explicit solver modes are subject to correctness and allocation-safety checks,
not the candidate RSS threshold. Solver development first uses the five-family
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

The release-grade freeze uses at least five warm runs after one cold run.
Quick, matrix, and milestone development profiles use their registered shorter
warm cadence and cannot substitute for freeze or exact-source qualification.
Acceptance requires Stata sentinels, logs, scientific hashes, dependency and
plugin identities, and output schemas to agree. Cluster runs additionally
require their scheduler/accounting receipts. Exploratory Phase 0–1 thresholds
are superseded by ADR 0007; the solver and memory policy is superseded by ADR
0008.

## Current local development evidence

The uncommitted 0.5.0 candidate identified in `STATUS.md` clears the primary
macOS solver-development gates: every registered 100k auto case is faster than
reghdfe, all eight selected 1m auto cases are faster, and the four-case 10m
freeze subset has 12 timed arms, four passing preflights, five warm runs per
arm, and no failures. The 10m auto/reghdfe ratios are 0.901 for one FE at eight
threads, 0.692/0.425 for standard AKM at one/eight threads, and 0.358 for
low-mobility AKM at eight threads. This does not yet satisfy the stronger full
ADR-0008 aggregate, RSS, leak, constrained-memory, packaging, or exact-source
receipt gates.
