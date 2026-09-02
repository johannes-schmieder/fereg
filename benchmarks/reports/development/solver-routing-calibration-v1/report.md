# FE-solver routing calibration (development only)

This is development/prototyping evidence, not qualification or release
evidence.  No production routing threshold or policy was changed.

## Snapshot and method

- Repository HEAD: `8bec14e72c59472269361d0a83b14f43c45a2e23`, with the dirty-worktree
  state shown by `git status` at handoff.
- Tested plugin SHA-256:
  `91dc70d31c1742f1a0d5f7458cbb325de99603ec12406fbd971fd151d39bce26`.
- macOS 26.6.2, licensed Stata/MP 18, Apple Silicon universal plugin.
- 114 generated datasets, 550 solver cells, and 1,650 recorded runs.  Every
  cell has exactly three recorded repetitions.  There was one unrecorded
  warm-up per arm, arm order rotated across repetitions, and data generation
  was outside all timings.  Each case generated its data once and reused it
  across solver arms.
- Primary settings were tolerance `1e-8`, 16,000 maximum iterations, four
  controls, and eight threads.  The requested focused tighter-tolerance,
  preconditioner, RHS, and thread checks are exceptions.
- 1,599 commands succeeded and certified.  The 51 unsuccessful commands were
  retained in the raw file and were excluded from eligible wins: 33 MAP, nine
  CG, six Schwarz-LSMR, and the three expected explicit-CMG failures at 65
  nuisance columns.

Small scientific checks passed: fresh Schwarz-LSMR matched the sealed decimal
oracle; explicit CMG with an absorbed nuisance effect matched the independent
full multi-effect CG comparator; and a focused topology-selector test verified
that the two largest effects are chosen as CMG cores in all six absorb orders.

## Main result

The old 15k--25k auto cliff did not reproduce on the current plugin.  Across
the exact requested 10k--30k grid, auto selected Schwarz-LSMR after four MAP
pilot sweeps at every size, the internal `1e-10` LSMR continuation certified at
every size, and no fallback occurred.  The current-source 25k median was 0.048
seconds, not about 0.471 seconds.  Explicit Schwarz-LSMR at `1e-8` and `1e-10`
certified throughout; CMG was slightly faster in this topology.

| N | auto | MAP | CG | LSMR 1e-8 | LSMR 1e-10 | CMG | auto route/fallback |
|---:|---:|---:|---:|---:|---:|---:|:---|
| 10,000 | .021 | 10.750 | .132 | .017 | .018 | .015 | LSMR/no |
| 12,500 | .027 | 9.298 | .149 | .022 | .022 | .019 | LSMR/no |
| 15,000 | .031 | 17.728 | .207 | .024 | .025 | .021 | LSMR/no |
| 17,500 | .033 | 14.812 | .218 | .027 | .028 | .023 | LSMR/no |
| 20,000 | .038 | failed | .335 | .029 | .031 | .026 | LSMR/no |
| 22,500 | .043 | 22.924 | .312 | .033 | .035 | .032 | LSMR/no |
| 25,000 | .048 | 41.765 | .418 | .036 | .038 | .037 | LSMR/no |
| 27,500 | .051 | 29.515 | .408 | .039 | .041 | .040 | LSMR/no |
| 30,000 | .056 | 32.403 | .457 | .044 | .047 | .047 | LSMR/no |

Times are median command seconds.  At 25k, the trace attributed 0.0016 seconds
to pilot setup, 0.0158 to the four pilot sweeps, and 0.0171 elapsed seconds to
the certifying 19-iteration LSMR continuation.  The corresponding values vary
smoothly over the grid.  Diagonal and off LSMR were not substitutes for
Schwarz: at 25k they took 36.745 and 36.785 seconds and fell back to MAP, versus
0.036 seconds and no fallback for Schwarz.

## Common-topology calibration

Each cell below is `best certified explicit / auto route / auto regret`; `*`
marks an LSMR-to-CG fallback.  The full run-level and median tables contain the
certificates and timings behind each entry.

| topology | 10k | 25k | 50k | 100k |
|:---|:---|:---|:---|:---|
| worker--firm dense | CMG/LSMR/64% | CMG/LSMR/85% | CMG/LSMR/35% | CMG/LSMR/66% |
| worker--firm sparse | CMG/LSMR/73% | CMG/LSMR/80% | CMG/LSMR/47% | CMG/LSMR/69% |
| worker--firm very sparse | CMG/LSMR/50% | CMG/CG/869% | CMG/CG/1,024% | CMG/CG/560% |
| worker--firm--year (5) | CMG/LSMR/69% | CMG/LSMR/70% | CMG/CG*/1,100% | CMG/LSMR/32% |
| worker--firm--year (20) | LSMR/LSMR/24% | LSMR/LSMR/25% | LSMR/LSMR/33% | LSMR/LSMR/16% |
| worker--firm--county (20) | LSMR/LSMR/24% | LSMR/LSMR/27% | LSMR/LSMR/32% | LSMR/LSMR/15% |
| balanced three-FE | LSMR/LSMR/9% | LSMR/LSMR/13% | LSMR/LSMR/19% | LSMR/LSMR/-1% |

Across all 114 cases, the fastest certified explicit arm was CMG in 60,
Schwarz-LSMR `1e-8` in 53, and CG in one.  MAP won none of its 55 tested cells
and failed in 11.  Median auto regret was 33.6%; it exceeded 25% in 90 cases
and was at most 5% in eight.  Among 91 non-fallback auto-LSMR cells with a fresh
explicit comparator, auto was 1.27 times as slow at the median, consistent
with pilot overhead.  Tightening fresh Schwarz-LSMR from `1e-8` to `1e-10`
cost a median 5.4% in the 37 paired cells.

The `0.008` contraction boundary did not separate winners.  Below it, the
explicit winner was CMG in eight cases and Schwarz-LSMR in two.  Above it, the
winner was Schwarz-LSMR in 51, CMG in 46, and CG in one.  In particular, the
boundary sends very-sparse two-FE designs to CG exactly where CMG was between
6.6 and 11.2 times faster than auto.  Changing the number of RHSs also moved
otherwise similar cases across `0.008`: zero-control designs selected CG,
whereas four- and 16-control designs generally selected LSMR.  One-thread and
eight-thread routes were otherwise similar; the notable exception was the
order-sensitive marginal LSMR certificate described below.

There is no positive evidence here for continuing MAP after the four-sweep
pilot: explicit MAP never won and often exhausted the budget.  A future MAP
continuation rule should therefore require a directly estimated very small
remaining sweep count, rather than treating MAP as a broad competitor.

## CMG cores, nuisance size, and current eligibility guards

The internal selector test confirms that worker and firm are the two CMG core
dimensions and year is the nuisance dimension, independent of absorb order.
With one shared dataset seed per year-level design, all six permutations gave
identical reported coefficients and RSS under every successful solver.  CMG
median runtime varied by no more than 1.8% within a year-level design.

| year levels | nuisance columns | CMG median range | LSMR 1e-8 median range | faster arm | result |
|---:|---:|---:|---:|:---|:---|
| 5 | 4 | .058--.059 | .061--.066 | CMG | all six certify |
| 10 | 9 | .066--.067 | .062--.065 | LSMR | all six certify |
| 20 | 19 | .090--.091 | .065--.068 | LSMR | all six certify |
| 40 | 39 | .175--.177 | .067--.069 | LSMR | all six certify |
| 65 | 64 | .304--.305 | .070--.073 | LSMR | all six certify |
| 66 | 65 | failed (`rc=498`) | .068 | LSMR | focused beyond-limit check |

The current strict auto-CMG region (at least eight observations per largest
level, at least a 10:1 core cardinality ratio, and zero nuisance columns) was
clean: all six boundary cells inside it were CMG wins.  It was also
conservative: it selected only six of the 15 CMG wins in the boundary study.
CMG won all four cells with about four observations per largest level, five of
eight cells below the 10:1 ratio, and four of 16 cells with nuisance columns.
The nuisance results support a small allowance, not the full 64-column range:
four nuisance columns were often favorable, nine were near the crossover, and
19 or more favored LSMR in the controlled permutation design.

## Fallback and certification diagnosis

Five common cells had deterministic auto LSMR-to-CG fallback in all three
recorded repetitions:

- `topology_wfy5_n50000`;
- `sensitivity_wfy5_k16_t1`;
- the same shared five-year CMG-order dataset when ordered `firm worker year`,
  `firm year worker`, or `year firm worker`.

The other three permutations of that identical dataset did not fall back.
All successful explicit CMG and Schwarz-LSMR `1e-8` results were scientifically
equivalent across those orders.

This is primarily a tolerance/certificate-scale and pilot-handoff problem, not
evidence that Schwarz-LSMR is unreliable in those cells.  Auto's internal LSMR
uses `1e-10`; the five failed continuations stopped with recomputed relative
infinity norms from `1.0249e-10` to `1.1209e-10`, only 2.5%--12.1% over the
internal target and roughly two orders of magnitude tighter than the requested
public `1e-8`.  In `topology_wfy5_n50000`, a fresh explicit `1e-10` LSMR solve
certified in the same 19 iterations with `2.93e-11` and took .069 seconds,
whereas auto discarded its .036-second attempted continuation and spent about
.614 additional solve seconds in CG.

The code explains the scale difference.  Auto preserves the original raw-RHS
reference norms for fallback CG, but `run_borrowed_lsmr` recomputes its
certificate reference norms from the already-residualized MAP-pilot state.
That makes the same numerical residual face a stricter relative scale.  Small
floating-point differences caused by FE order or thread execution can then
move a value just above or below `1e-10`, producing the discontinuity.

Iteration and timing accounting do not cause the fallback, but they hide its
cost: after an uncertified auto-LSMR attempt, the final public solver row
reports pilot plus CG iterations/times and drops the discarded LSMR attempt.
Total native and command time still include it.  The environment-gated trace
and `fallback_phase_table.csv` expose pilot, attempted LSMR, and fallback CG
separately without adding a public Stata result.

There are two bounded solver-reliability findings to retain.  Explicit
Schwarz-LSMR `1e-8` returned `rc=459` in two extreme 64-nuisance-column boundary
cells, all three repetitions; CG returned `rc=459` in three cells.  These are
real failures and are excluded from wins, but they are distinct from the five
auto continuation fallbacks above.  Schwarz-LSMR certified throughout the
cliff grid and all 28 common-topology cells at both requested tolerances.

## Candidate rules to test next (not implemented)

The simplest evidence-backed candidate is:

1. Sort FE dimensions by retained level count.  Consider the two largest the
   CMG cores, independent of syntax order.
2. Try early CMG for two-FE designs when observations per largest level are at
   least four and the core cardinality ratio is at least 5:1.  Extend the same
   rule to three-FE designs only when the nuisance expansion has at most four
   columns.  Keep a stricter candidate variant with zero nuisance columns as a
   control.
3. Otherwise use the four MAP sweeps only for an early certificate and cost
   measurement.  Continue MAP only when a deterministic extrapolation predicts
   no more than roughly two additional symmetric sweeps; no current cell meets
   an evidence-backed broader MAP condition.
4. If the pilot is not nearly done, choose Schwarz-LSMR whenever it fits the
   memory/size plan.  Do not use `0.008` to select CG.  Reserve CG for cases in
   which LSMR is unavailable or genuinely fails, preserving fallback as a
   correctness mechanism.

Before timing that rule, test two continuation variants separately: certify
post-pilot LSMR against the original RHS reference scale, and allow the same
LSMR state to continue to the public certificate instead of discarding it.
Neither variant should weaken the public `1e-8` certificate.  Also make phase
iterations and times additive in internal diagnostics so any future fallback
is visible.  A second, more conservative candidate is identical except that it
keeps the existing CMG guards and changes only the post-pilot default from the
`0.008` CG/LSMR split to Schwarz-LSMR; this cleanly separates the two policy
questions in the next experiment.

