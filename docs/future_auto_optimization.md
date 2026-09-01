# Future automatic FE-solver optimization

This document is the resume point for future development of `fereg`'s
automatic fixed-effect solver routing. It records development evidence and
candidate work, not a qualification decision or a frozen production contract.

## Current development route

ADR-0021 currently routes ordinary categorical, intercept-only designs as
follows:

- Select CMG before any MAP pilot when observations are at least three times
  the largest FE level count, the largest and second-largest FE counts have a
  ratio of at least 10:1, and additional effects contribute no more than nine
  non-base nuisance columns.
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
component share, or PCG iteration count in isolation. It is still a diagnostic
metric, not a calibrated routing threshold.

### RHS, thread, and memory opportunities

CMG's bad hierarchy cost repeats with each right-hand side. Existing parallel
execution across RHSs can mask this for small batches, but it does not remove
the excessive work. A separate auto-eligible 16-control mixed cell remained
CPU-bound for several minutes during an unrecorded warm-up even though its
zero- and four-control counterparts took about 0.09--0.10 seconds. That event
was stopped and excluded from the complete component matrix. It should be
localized separately with bounded per-stage timing.

The balanced diagnostic also found that low-memory Schwarz-LSMR repeatedly
pays setup across RHS blocks. Relative to the matched ordinary-memory cell,
median setup rose roughly 4.5 times while solve time was unchanged. Reusing
Schwarz topology and setup across low-memory blocks is therefore an independent
performance opportunity.

## Recommended development sequence

### 1. Expose internal component and hierarchy diagnostics

Add private Rust diagnostics sufficient for calibration:

- connected component count;
- largest component size and share;
- selected component-size quantiles or bins;
- hierarchy terminal reason;
- vertex and matrix-nonzero counts by level;
- effective repeat counts; and
- the repeat-weighted hierarchy-work proxy.

Do not add a public Stata option or public estimation result solely for this
work. A focused Rust test or development-only benchmark output is sufficient.

### 2. Prototype a post-setup CMG safety gate

For topology shapes that pass the cheap pre-routing rule, build the graph and
hierarchy, inspect the predicted recursive work, and choose between CMG and
Schwarz-LSMR before the first PCG iteration. Setup was only about 0.004 seconds
in the worst controlled cells, so this can remove the cliff at low regret.

Do not freeze a numerical work threshold from the controlled component matrix.
First calibrate it on realistic worker mobility, worker--firm--year designs,
third-FE nuisance columns, multiple N values, and several component-size
mixtures. Measure false rejections of genuine CMG wins as well as avoided
cliffs.

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

### 5. Localize the remaining certification and wide-RHS events

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

### 6. Recalibrate routing only after the prototypes

Compare these candidates on the same generated datasets:

1. current topology-only rule;
2. topology rule plus hierarchy-work guard;
3. component-aware CMG plus hierarchy-work guard; and
4. direct Schwarz-LSMR as the conservative reference.

Use exactly three recorded repetitions, unchanged tolerance and iteration
limits, rotated order, and no data-generation timing. Include worker--firm,
worker--firm--year, worker--firm--county, realistic mobility, one versus eight
threads, and 0/4/16 controls. Failed or uncertified runs remain ineligible
winners. Do not run frozen matrices or qualification until a simple candidate
rule has survived this development matrix.

## Candidate routing shape to test, not implement yet

A promising deterministic design is:

1. Apply the cheap FE-count/nuisance topology screen.
2. If it rejects CMG, use direct Schwarz-LSMR.
3. If it accepts CMG, construct the CMG graph/hierarchy once.
4. If predicted hierarchy work is moderate, run CMG.
5. If predicted hierarchy work is excessive, retain any reusable graph
   metadata but solve with Schwarz-LSMR.
6. When the component-aware CMG prototype is available, replace the rejection
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
- Mutable project state: `STATUS.md`

The next session should begin with the internal hierarchy-work diagnostic and a
bounded reproduction of the 16-control anomaly. No current evidence justifies
a component-count cutoff or a frozen numeric hierarchy-work threshold.
