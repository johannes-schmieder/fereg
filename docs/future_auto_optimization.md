# Future automatic FE-solver optimization

This document is the resume point for future development of `fereg`'s
automatic fixed-effect solver routing. It records development evidence and
candidate work, not a qualification decision or a frozen production contract.

## Current development route

ADRs 0021 and 0022 currently route ordinary categorical, intercept-only
designs as follows:

- Select CMG before any MAP pilot when observations are at least three times
  the largest FE level count, the largest and second-largest FE counts have a
  ratio of at least 10:1, and additional effects contribute no more than nine
  non-base nuisance columns.
- After that cheap screen admits CMG, build its graph and preconditioner. Keep
  direct terminals. Keep iterative terminals only when repeat-weighted
  hierarchy work is at most four finest-level passes; otherwise release the
  hierarchy and use direct Schwarz-LSMR. This planned veto is not a fallback.
- Route other categorical multiway designs directly to Schwarz-preconditioned
  LSMR. A finite uncertified LSMR candidate continues into certified CG without
  discarding its state.
- Retain the four-sweep MAP pilot for slope-bearing and generic designs.

This rule removed the original 15k--30k sparse-three-FE cliff caused by a MAP
pilot, an attempted LSMR solve, and a CG restart. It also makes worker--firm
and worker--firm--year designs with a small third FE eligible for early CMG.
The rule remains a development policy and has not received frozen,
cross-platform qualification.

## Evidence so far

### Direct LSMR and certification continuation

The routing calibration showed that direct Schwarz-LSMR is the stable default
for many ordinary categorical designs. The old post-pilot LSMR path differed
from a fresh solve because the attempted result was certified on an
inconsistent scale and then discarded before CG. The implementation now:

- certifies against the original RHS scale;
- continues CG from a finite LSMR candidate;
- shares the iteration budget across attempted solvers; and
- reports all attempted setup, solve, iteration, preconditioner, fallback, and
  warning information.

Ten later balanced-AKM cells still had deterministic LSMR-to-CG continuation.
The retained observation-space result was unchanged before and after CG, while
CG added a median of only about 0.0025 seconds. This is a remaining
normal-equation certification/accounting question rather than the old
pilot-handoff failure.

### CMG wins and ordinary topology rules

Explicit CMG certifies reliably and can be the fastest solver for worker--firm
and worker--firm--year designs. It correctly selects the two largest FE
dimensions as its graph core and represents a small third FE through nuisance
columns independently of absorb-variable order. The original requirements of
eight observations per largest level, a 10:1 cardinality ratio, and zero
nuisance columns excluded useful cases; the current development thresholds
were chosen from the first calibration rather than treated as mathematical
constants.

The broader balanced diagnostic also found CMG runtime cliffs at fixed
cardinality ratios and similar iteration counts. Those cliffs indicated that
graph and hierarchy realization, not only N, level counts, or PCG convergence,
must enter any durable CMG route.

### Disconnected components

A controlled diagnostic held N, FE level counts, unique edges, cardinality
ratio, and both degree distributions fixed while changing only the connected
component partition. It used exactly three recorded repetitions per solver
cell, one unrecorded warm-up, rotated order, tolerance `1e-8`, and 16,000
maximum iterations.

The conclusion is precise:

- Component count alone does not hurt CMG.
- Equal-sized disconnected components, including thousands of tiny
  components, can be excellent CMG cases.
- CMG becomes pathological when hundreds or thousands of tiny components
  coexist with a still-material large component.

At the current auto-CMG boundary of three observations per largest level and a
10:1 worker/firm cardinality ratio, the four-control results were:

| Structure | Components | Largest component share | Auto | Schwarz-LSMR | CMG |
|---|---:|---:|---:|---:|---:|
| Connected | 1 | 100% | 0.094 s | 0.091 s | 0.095 s |
| One large plus tiny | 1,000 | 9.18% | 1.329 s | 0.116 s | 1.320 s |
| All equally tiny | 1,100 | 0.09% | 0.045 s | 0.098 s | 0.045 s |

Current auto therefore incurs about 1,046% regret in the mixed 1,000-component
cell, but a hard component-count cutoff would also reject the strong
1,100-component CMG win.

All 513 completed component-diagnostic runs certified without fallback.
Actual component counts matched construction, and exported coefficients and
RSS agreed across auto, explicit CMG, and explicit Schwarz-LSMR. This is a
performance problem, not a reliability or scientific-equivalence failure.

### CMG hierarchy mechanism

The bad mixed designs have ordinary CMG setup times and PCG iteration counts.
The cost per PCG iteration increases sharply because the global hierarchy
carries contracted tiny-component vertices through deeper levels while the
large component continues coarsening.

For the auto-eligible profile:

- 600 mixed components end in a compact direct terminal. A deterministic
  repeat-weighted hierarchy-work proxy is about 6.4 finest-level passes.
- 800 mixed components terminate through stagnated vertex reduction and the
  work proxy jumps to about 60 finest-level passes.
- 1,000 mixed components raise the proxy to about 82 passes.
- 1,100 equal tiny components reduce the proxy to about 2.3 passes.

The proxy sums `vertices + matrix_nonzeros` for every retained hierarchy level,
weighted by the product of all upstream recursive repeat counts. It tracks the
observed cliff much better than N, the number of components, the largest
component share, or PCG iteration count in isolation. ADR-0022 uses it only as
a terminal-aware automatic safety veto; it is not a general solver-winner
model.

### Terminal-aware hierarchy-guard calibration

A subsequent development calibration added a private, environment-gated Rust
trace at the exact post-hierarchy/pre-nuisance boundary. It records component
size summaries, terminal reason, hierarchy dimensions, effective repeat
counts, terminal-factor size, and the repeat-weighted work proxy without
changing the public Stata result or automatic route.

The focused holdout used 37 worker--firm designs at the current 3-observation,
10:1 auto-CMG boundary, including one or two material components plus many
tiny components, all-tiny controls, and 0/4/9 nuisance columns. It recorded
333 runs: three repetitions of auto, explicit Schwarz-LSMR, and explicit CMG
per dataset. All 330 successful runs certified without fallback and matched
on exported coefficients and RSS; explicit LSMR returned deterministic
`r(459)` in the remaining three repetitions of one cell. The diagnostic also
profiled 72 cases from the prior balanced AKM matrix and joined them to their
existing three-repetition medians. Together with seven controlled component
cells, the guard analysis contains 116 cells, 109 of which have directly
eligible CMG and LSMR medians.

The evidence rejects a single unconditional work ceiling. Several realistic
sparse graphs end in a direct terminal, have work proxies between 6 and 20,
and are strong CMG wins. The clean separator is terminal-aware:

- every measured iterative-terminal CMG win had a work proxy at or below 2.30;
- the first iterative-terminal loss occurred at 22.44;
- all 17 comparable iterative-terminal cells above that gap were CMG losses,
  with the worst CMG/LSMR ratio above 28; and
- graph-plus-hierarchy construction cost only 0.0037 seconds at the median and
  0.0046 seconds at the maximum in the focused holdout.

The initial development candidate was therefore a safety veto, not a full
winner model: after the existing topology screen accepts CMG, construct the
graph and hierarchy; retain direct terminals; reject high-work iterative
terminals to the existing direct auto-LSMR route with certified CG
continuation. Thresholds 4 through 20 made the same decision in that first
sample, so the provisional holdout value was 8.

This terminal-aware candidate rejected 17 of 109 comparable cells and did not
reject any measured CMG win. One remaining greater-than-twofold CMG loss had a
direct terminal, work 6.40, and nine nuisance columns. It is a separate
RHS/nuisance graph-solve-load problem. An additional nuisance rule can catch
it, but current balanced nuisance cases show meaningful false rejections, so
that rule should be calibrated separately from the hierarchy safety guard.

The complete source, 333 run-level rows, medians, hierarchy profiles, joined
cell table, threshold counterfactuals, and report are under
`benchmarks/reports/development/cmg-hierarchy-guard-calibration-v1/`, with the
runner in `scripts/cmg_hierarchy_guard_calibration.py`.

### Fresh holdout and implemented guard

The fresh holdout used 32 newly generated datasets with new material-component
wiring at three, four, and five observations per worker. On three graphs it
also crossed 0/4/16 controls with 1/8 threads. Auto, explicit CMG, and explicit
Schwarz-LSMR each had exactly three recorded repetitions, with common saved
data, rotated order, and unrecorded warm-ups. All 288 runs succeeded and
certified. Three recorded fallback events belonged to one explicit LSMR
comparator cell; auto had none.

Only two fresh hierarchies had iterative terminals. Neither favored CMG by
more than five percent. One had work 56.21 and CMG/LSMR 2.66; the other had
work 5.68 and CMG/LSMR 1.18. The earlier calibration still puts every measured
iterative-terminal CMG win at work no greater than 2.30. The combined measured
gap is therefore 2.30 to 5.68, and the conservative round development cutoff
is 4 rather than the provisional 8.

ADR-0022 implements that cutoff only for `solver(auto)`. A rejected graph and
preconditioner are released before nuisance-factor and PCG work. Their setup
time is charged to the eventual LSMR report, but the planned route emits no
warning, sets no fallback flag, and runs no MAP pilot. Explicit `solver(cmg)`
is unchanged.

The post-change rerun recorded another 288 runs: 288/288 succeeded and
certified; auto used CMG in 90 runs and LSMR in the six guarded runs, with zero
auto fallbacks, warnings, or MAP pilots. A separate 18-run check retained CMG
for the low-work all-tiny iterative graph and selected LSMR for the high-work
mixed graph in all three auto repetitions. The focused licensed Stata route
fixture passed both sides plus explicit CMG. This remains development evidence,
not qualification.

### RHS, thread, and memory opportunities

CMG's graph-solve cost repeats with each right-hand side. The fresh bounded
0/4/16-control cross completed, so it did not reproduce the earlier
multi-minute warm-up. It did confirm a major direct-terminal cost interaction:
on the W=12.12 mixed graph, CMG/LSMR rose from 3.01 with zero controls to 6.13
with four and 8.18 with sixteen at one thread; at eight threads the ratios were
2.65, 2.07, and 3.71. The current hierarchy guard deliberately retains that
direct terminal. A second-stage cost model must therefore include RHS width
and effective parallelism, not only graph hierarchy work or nuisance columns.

The balanced diagnostic also found that low-memory Schwarz-LSMR repeatedly
pays setup across RHS blocks. Relative to the matched ordinary-memory cell,
median setup rose roughly 4.5 times while solve time was unchanged. Reusing
Schwarz topology and setup across low-memory blocks is therefore an independent
performance opportunity.

### Direct-terminal cost calibration and rejected route

A dedicated second-stage calibration then crossed direct-terminal hierarchy
shape, 0/4/16 controls, 1/8 threads, and 0/4/9 nuisance columns. Its calibration
population contained 84 datasets and 756 recorded runs; every run succeeded
and certified without fallback. A locked candidate charged four quantities
already available after hierarchy construction:

- hierarchy depth times repeat-weighted work per observation;
- repeat-weighted work times effective nuisance batches;
- effective RHS batches; and
- repeat-weighted RHS work when the direct terminal factor is nonempty.

Across 172 prior-plus-new calibration cells, that candidate rejected 65 cases,
false-rejected no measured CMG win above five percent, retained no CMG loss
above twofold, and had maximum regret 1.311 after charging discarded CMG setup.
The timing model itself therefore generalized substantially better than a raw
RHS, component-count, or nuisance-column cutoff.

The fresh 48-dataset holdout invalidated the proposed destination route. All
432 recorded runs ended with certified results, and auto and explicit CMG had
no fallback. Requested-tolerance Schwarz-LSMR, however, fell back
deterministically in all three repetitions of 14 multi-RHS cells. The locked
candidate would have rejected CMG in 32 holdout cells, including 12 cells
without a certified no-fallback LSMR comparator. Among the 34 directly
comparable cells it still false-rejected no CMG win above five percent and had
maximum regret 1.315, but routing common cells into fallback is a safety
failure regardless of that fit. No production rule changed.

A representative trace showed the mechanism: LSMR stopped after three
iterations while its FE normal-equation diagnostic remained about 0.0076 at a
requested tolerance of `1e-8`. Fereg correctly rejected the candidate and
continued CG, but attempted LSMR plus CG was 7--69 times slower than a direct
eligible comparator in the focused failure subset. A further 84-run diagnostic
found:

- direct CG certified without fallback in all 14 affected cells;
- Schwarz-LSMR at `1e-10` certified without fallback in 10 of 14 and still
  fell back in four 17-RHS cells;
- the fastest eligible solver was tighter LSMR in eight cells, CMG in four,
  and CG in two; and
- an unrecorded `1e-12` probe exceeded several minutes in one
  p3/17-RHS/one-thread call and was stopped.

The direct-terminal opportunity is therefore a joint cost-and-reliability
problem. Simply tightening LSMR can help, but an unbounded tolerance multiplier
creates another performance cliff. A durable route needs a bounded
certification-aware LSMR continuation and explicit CG destination costs before
it can use the calibrated CMG cost score.

## Recommended development sequence

### 1. Completed: expose internal component and hierarchy diagnostics

The private environment-gated trace now records:

- connected component count;
- largest component size and share;
- selected component-size quantiles or bins;
- hierarchy terminal reason;
- vertex and matrix-nonzero counts by level;
- effective repeat counts; and
- the repeat-weighted hierarchy-work proxy.

No public Stata option or estimation result was added.

### 2. Completed: implement the post-setup CMG safety gate

The implementation retains direct terminals and iterative terminals with work
at most 4. It rejects costly iterative terminals after cheap hierarchy setup
and before nuisance-factor/PCG work. The fresh-seed, wiring, thread, and RHS
holdout selected 4 from the measured 2.30--5.68 gap. The separate
direct-terminal RHS/thread model was deliberately not folded into this guard.

### 3. Implement the component-aware CMG prototype

The structural solution is to respect the block diagonal graph:

- solve material connected components with their own CMG hierarchy;
- handle tiny components directly or with a specialized shallow path;
- avoid carrying edgeless tiny-component aggregates through the hierarchy of a
  large component; and
- schedule work across components and/or RHSs without changing scientific
  normalization.

Thousands of independent hierarchies could introduce their own setup and
allocation costs. The first prototype should therefore bin components by size
and use a hybrid policy rather than naively building one full CMG object per
component. Preserve component-wise null-space projection, coefficient
normalization, deterministic ordering, memory accounting, and certification.

### 4. Reuse Schwarz-LSMR setup across RHS blocks

Separate immutable topology/preconditioner construction from per-block solve
state so low-memory execution can retain and reuse the same Schwarz setup.
Measure setup, solve, total native time, peak memory, and saved-FE recovery.
This work benefits the default fallback route independently of CMG changes.

### 5. Continue localizing certification and wide-RHS events

Use bounded, development-only traces to distinguish:

- LSMR normal-equation diagnostic scaling from the observation-space FE
  certificate;
- a valid continuation into CG from an unnecessary fallback;
- multi-RHS CMG solve time from final residual correction;
- retained coefficient recovery; and
- OLS score certification after absorption.

Treat fallback as a safety mechanism. A common deterministic route that always
requires fallback is a routing or certificate-accounting defect, even when the
added runtime is small.

The new direct-terminal holdout makes the immediate sequence more concrete:

1. expose the per-attempt LSMR stopping and FE normal-equation diagnostics by
   RHS/column before fallback;
2. test a bounded continuation that tightens only while a small explicit work
   budget remains;
3. record direct CG in every cost-guard rejection cell, because one-thread and
   eight-thread winners differ; and
4. recalibrate a three-way CMG/LSMR/CG destination rule on a fresh holdout.

Do not implement the locked CMG-to-requested-tolerance-LSMR candidate. Its
timing score survived holdout, but its destination did not.

### 6. Recalibrate routing only after the prototypes

Compare these candidates on the same generated datasets:

1. historical topology-only rule;
2. current topology rule plus hierarchy-work guard;
3. component-aware CMG plus the hierarchy-work guard; and
4. direct Schwarz-LSMR as the conservative reference.

Use exactly three recorded repetitions, unchanged tolerance and iteration
limits, rotated order, and no data-generation timing. Include worker--firm,
worker--firm--year, worker--firm--county, realistic mobility, one versus eight
threads, and 0/4/16 controls. Failed or uncertified runs remain ineligible
winners. Do not run frozen matrices or qualification until a simple candidate
rule has survived this development matrix.

## Current routing shape and next extension

The implemented deterministic design is:

1. Apply the cheap FE-count/nuisance topology screen.
2. If it rejects CMG, use direct Schwarz-LSMR.
3. If it accepts CMG, construct the CMG graph/hierarchy once.
4. Retain direct terminals; for iterative terminals run CMG only at `W <= 4`.
5. If iterative hierarchy work exceeds 4, release the graph/preconditioner,
   account for its setup, and solve with Schwarz-LSMR.
6. If a component-aware CMG prototype succeeds, replace the rejection
   in step 5 with the hybrid component solve when its own cost estimate wins.

This avoids a discontinuous PCG performance cliff while preserving the
all-tiny-component CMG wins that a component-count rule would lose.

## Reproducible evidence and resume points

- Main calibration source: `scripts/solver_routing_calibration.py`
- Initial report: `benchmarks/reports/development/solver-routing-calibration-v1/report.md`
- Current auto-rule diagnostic: `benchmarks/reports/development/solver-routing-auto-akm-v2/REPORT.md`
- Balanced sensitivity report: `benchmarks/reports/development/solver-routing-balanced-diagnostic-v1/report.md`
- Component benchmark source: `scripts/cmg_component_diagnostic.py`
- Auto-eligible component profile: `scripts/cmg_component_diagnostic_autoeligible.py`
- Component report: `benchmarks/reports/development/cmg-component-diagnostic-v1/report.md`
- Component hierarchy table: `benchmarks/reports/development/cmg-component-diagnostic-v1/hierarchy_table.csv`
- Auto-eligible run-level data: `benchmarks/reports/development/cmg-component-autoeligible-v1/cmg_component_runs.csv`
- Hierarchy calibration: `benchmarks/reports/development/cmg-hierarchy-guard-calibration-v1/report.md`
- Fresh holdout: `benchmarks/reports/development/cmg-hierarchy-guard-holdout-v1/report.md`
- Post-change rerun: `benchmarks/reports/development/cmg-hierarchy-guard-postchange-v1/report.md`
- Direct-terminal calibration and rejected route: `benchmarks/reports/development/cmg-direct-terminal-calibration-v1/report.md`
- Mutable project state: `STATUS.md`

The next session should prototype bounded LSMR certification continuation and
measure direct CG across the rejected direct-terminal population; separately,
it can prototype a component-aware CMG solve. No current evidence justifies a
component-count cutoff, unconditional direct-terminal work veto, or routine
LSMR-to-CG fallback. The iterative-terminal cutoff 4 is a development safety
threshold, not a frozen qualification constant.
