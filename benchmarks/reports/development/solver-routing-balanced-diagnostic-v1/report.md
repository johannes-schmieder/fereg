# Balanced AKM solver-routing diagnostic

This is development/prototyping evidence, not qualification. The benchmark
does not change the production routing rule.

## Snapshot and design

- Repository HEAD: `8bec14e72c59472269361d0a83b14f43c45a2e23`, with the dirty
  worktree preserved.
- Tested plugin SHA-256:
  `04cde2f5854e513567dd9551286090d42b01268e5748711c6dc7ab8a351e6be5`.
- macOS 26.6.2, licensed Stata/MP 18, Apple Silicon universal plugin.
- 408 generated datasets, 1,656 solver cells, and 4,968 recorded runs. Every
  cell has exactly three repetitions. Data were generated once per cell,
  solver arms shared the same data, each arm received one unrecorded warm-up,
  and arm order rotated over the recorded repetitions.
- The primary matrix used four controls, eight threads, tolerance `1e-8`, and
  16,000 maximum iterations. It crossed 3/4/5/6/8/12 observations per worker,
  worker/firm cardinality ratios 2/3/5/7.5/10, sparse and dense mobility, and
  three independent seeds. Three-FE cases crossed 4/9/10/19 nuisance columns.
- The 48-case sensitivity overlay used matched topology seeds across 0/4/16
  controls, 1/8 threads, 25k/50k/100k observations, and forced
  `memory(low) poolsize(4)` cells. The original unmatched sensitivity rows
  were replaced rather than pooled with the corrected data.

The source is `scripts/solver_routing_calibration.py`. Run-level evidence is
in `solver_routing_calibration_runs.csv`; three-repetition medians are in
`solver_routing_calibration_summary.csv`; the per-case route/winner table is
`routing_decision_table.csv`; and solver phase diagnostics are in
`balanced_phase_table.csv`.

## Reliability and scientific agreement

All 4,914 successful runs certified. The 54 recorded `r(459)` results remain
in the raw file and are excluded from wins. Across 1,230 available successful
arm/case comparisons, reported coefficients and RSS were exactly equal to
auto at the captured precision.

| requested route | tested cells | direct eligible | certified fallback | `r(459)` |
|:---|---:|---:|---:|---:|
| auto | 408 | 398 | 10 | 0 |
| CMG | 408 | 408 | 0 | 0 |
| Schwarz-LSMR `1e-8` | 408 | 384 | 11 | 13 |
| Schwarz-LSMR `1e-10` | 408 | 396 | 11 | 1 |
| MAP representative subset | 12 | 10 | 0 | 2 |
| CG representative subset | 12 | 10 | 0 | 2 |

No CMG run failed, fell back, or disagreed scientifically. This makes CMG the
strongest route on reliability. It does not make CMG uniformly fast.

The noisy reruns classify the explicit failures. The selected LSMR and CG
failures reached OLS but were rejected by the independent finite score
certificate. The two MAP failures exhausted 16,000 iterations without a
passing final certificate. MAP was never the fastest arm: among its ten
successful representative cells its median was 7.911 seconds and 3,590
iterations. Successful CG had a 0.520-second median and 75.5 iterations. The
corresponding LSMR and CMG medians were 0.071 and 0.045 seconds.

## Remaining auto fallback

All 408 auto cells recorded zero MAP pilot iterations. The old pilot handoff
is therefore absent from this matrix and cannot explain the ten new fallback
cells. All ten fallbacks were deterministic over three repetitions.

The focused traces show a narrower certification issue:

- auto's internal LSMR used `1e-10` and returned `Uncertified` after 14--22
  reported iterations;
- LSMR's relative-infinity norm was at most `1.52e-10` and relative-two norm
  at most `2.46e-12`, but its normal-equation diagnostic was
  `4.80e-10`--`9.84e-10`;
- CG certified the same candidate: all ten before/after observation-space
  certificate norms were identical, while the reported normal-equation
  residual became zero;
- the LSMR attempt took 0.026--0.044 seconds and the CG continuation added a
  median 0.0025 seconds of solve time.

Thus these events are certification/normal-equation accounting problems, not
a recurrence restart, discarded candidate, or current performance cliff. In
nine of the ten cells, fresh Schwarz-LSMR at the requested public `1e-8`
certified directly. Fresh `1e-10` was ineligible in all ten (nine MAP
fallbacks and one `r(459)`). Globally, among the 374 cells where both explicit
LSMR tolerances certified without fallback, `1e-10` was 4.1 percent slower at
the median and used three additional iterations.

Fallback remains a safety mechanism and should not be normalized as an
ordinary route. The next solver change should make LSMR and CG apply the same
normal-equation certificate to the same retained candidate and should refine
that candidate only if the public certificate genuinely requires it. No
public tolerance should be weakened.

## CMG performance and its new cliff

CMG was the fastest certified explicit arm in 268 of 408 cells, versus 93 for
LSMR `1e-8` and 47 for LSMR `1e-10`. That comparison overstates the case for
wider CMG routing because current auto LSMR reuses prepared topology and is
usually faster than a fresh explicit LSMR command. Against actual auto command
time, explicit CMG was faster in 147 cells: 100/216 two-FE core cells, 27/144
three-FE cells, and 20/48 sensitivity cells.

The critical finding is a CMG size/graph-realization cliff. The table holds
mobility at 0.02, uses zero controls and eight threads, and reports median
command seconds.

| topology | 25k auto/CMG | 50k auto/CMG | 100k auto/CMG |
|:---|---:|---:|---:|
| 5 periods, 5:1 | .024/.013 | .040/.154 | .076/.363 |
| 8 periods, 2:1 | .033/.020 | .053/.038 | .099/.792 |
| 8 periods, 5:1 | .021/.023 | .034/.021 | .064/.095 |

CMG iteration counts change little across these cells, so the discontinuity
is work per iteration, not ordinary PCG convergence. Recorded mean degrees,
duplicate-edge fractions, and total component counts do not separate the
traps. The likely missing state is connected-component size distribution and
CMG hierarchy/terminal-factor structure. This should be instrumented before
another eligibility expansion. Routing every supported case to CMG would
make 132 newly routed cells faster but 201 slower, double aggregate time, and
produce a worst observed slowdown above 24 times.

## RHS, threads, and low memory

At 50k, one thread was generally faster than eight for LSMR with zero or four
controls. Eight threads became useful for wide batches: with 16 controls,
auto's 1-thread/8-thread speedup was 1.34x for the five-period 5:1 case, 1.12x
for the eight-period 2:1 case, and 2.60x for the eight-period 5:1 case. CMG
also benefits from across-RHS concurrency, but only when the underlying graph
route is not in the cliff region. A thread rule calibrated on one RHS would
therefore be wrong for a 17-RHS absorption.

Forced low memory was 2.44 times slower than fast/auto memory for auto at the
median of the 12 matched cells. For explicit LSMR the low/fast command ratio
was 1.87, while solve time was unchanged and recorded setup time rose 4.5
times. This cleanly identifies repeated Schwarz/topology setup across RHS
blocks as an implementation opportunity. CMG setup was already retained
(setup ratio 1.00), but its low-memory solve time rose 1.25 times because
blocking sacrifices some across-RHS execution.

## Candidate rules measured in sample

These are counterfactuals using actual auto time when the current rule is
unchanged and explicit CMG time only for a newly selected CMG cell. They are
ideas for a fresh holdout, not a production recommendation.

| candidate addition to current CMG rule | newly selected | faster/slower | geometric time change | worst slowdown |
|:---|---:|---:|---:|---:|
| two FE, at least 4 observations per largest level, ratio at least 7.5:1 | 30 | 27/3 | -1.91% | +8.1% |
| 10 nuisance columns at ratio at least 10:1 | 18 | 6/12 | +0.42% | +41.0% |
| deep sparse 5:1 profile using current degree summaries | 40 | 32/7 (one tie) | -2.68% | +68.2% |

The first rule is the only simple low-downside candidate in this matrix. It
still needs independent seeds and sizes because the current study contains no
matched sensitivity cells at 7.5:1. The nuisance-10 rule should be rejected:
explicit CMG can beat fresh explicit LSMR there, yet current retained auto
LSMR is faster in 12 of 18 cells. The apparent deep-sparse gain should not be
routed until the 100k CMG cliff is explained.

## Diagnosis and next sequence

1. **Solver reliability:** CMG is uniformly reliable here. Explicit LSMR and
   CG have bounded finite-score certificate failures; MAP has an unacceptable
   iteration-budget tail. MAP and CG should not gain normal routing scope.
2. **Tolerance/certification:** the remaining auto fallbacks are triggered by
   the LSMR normal-equation diagnostic at the internal `1e-10` target. CG then
   certifies an observation-space-identical candidate almost immediately.
   Make the certificate definition consistent and use adaptive continuation
   from `1e-8` rather than globally demanding `1e-10`.
3. **Pilot handoff:** no current categorical auto cell ran a MAP pilot. The
   prior handoff cliff is fixed and is not an active optimization target.
4. **Routing thresholds:** the current CMG rule is conservative but protects
   against real discontinuities. Validate the 7.5:1 two-FE expansion only
   after the implementation work below; do not broaden nuisance or 5:1 rules
   yet.
5. **CMG implementation:** add development-only internal metrics for largest
   connected component, component edge counts, hierarchy levels, terminal
   factor dimensions/fill, and serial/across-RHS/planned execution. Run a
   matched 40k--120k sweep around the observed cliffs and optimize the CMG
   hierarchy/terminal or batch policy before asking routing to hide it.
6. **Low-memory implementation:** retain one Schwarz/topology setup across RHS
   blocks. The measured opportunity is mostly setup duplication, so it can be
   attacked without changing numerical policy.
7. **Thread routing:** test a small deterministic rule using N, RHS count, and
   selected solver. At these sizes, one thread is preferable for narrow LSMR;
   eight becomes valuable for wide RHS batches.

The best performance path is therefore implementation first, routing second:
repair the LSMR certificate handoff, expose and remove the CMG
component/hierarchy cliff, and reuse low-memory setup. Then run an independent
holdout for the conservative 7.5:1 two-FE expansion and any topology metrics
that emerge from the CMG diagnosis. Updating auto before those changes would
risk replacing the old LSMR cliff with a new CMG cliff.
