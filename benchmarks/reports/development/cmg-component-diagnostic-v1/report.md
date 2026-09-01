# CMG disconnected-component diagnostic

Development diagnostic only; not qualification and not a production routing
change. Run on 2026-09-01 with Stata/MP 18 on macOS 26.6.2, source
`8bec14e72c59472269361d0a83b14f43c45a2e23`, and development plugin SHA-256
`04cde2f5854e513567dd9551286090d42b01268e5748711c6dc7ab8a351e6be5`.

## Design

The generator fixes observations, worker and firm levels, unique edges, the
worker/firm cardinality ratio, both degree distributions, and all random data.
Only the connected-component partition changes. Every cell has one unrecorded
warm-up, exactly three recorded repetitions, and rotated solver order. All
timings exclude data generation and use tolerance `1e-8` and 16,000 maximum
iterations.

Two profiles were run:

- `p2_ratio5`: 100,000 observations, 50,000 workers, 10,000 firms, two unique
  firm neighbors per worker, and firm degree 10. Auto selects Schwarz-LSMR, so
  this profile isolates explicit CMG behavior without exposing current auto.
- `p3_ratio10_auto`: 99,000 observations, 33,000 workers, 3,300 firms, three
  unique firm neighbors per worker, and firm degree 30. This is exactly on the
  current auto-CMG boundaries: three observations per largest level and a
  10:1 cardinality ratio.

Balanced designs split every level evenly across components. Mixed designs
have one large component plus identical tiny components (12 vertices under
`p2_ratio5`; 33 under `p3_ratio10_auto`). The balanced and mixed endpoints in
which all components are tiny are scientifically the same size distribution.

## Result

The suspicion is directionally correct but component count alone is not the
cause. CMG is hurt by a mixed component-size distribution in which many tiny
components coexist with a still-material large component. Equal-sized
components, including thousands of tiny components, are cheap.

Primary four-control, eight-thread medians:

| Profile | Structure | Components | Largest share | Auto route | Auto s | LSMR s | CMG s |
|---|---|---:|---:|---|---:|---:|---:|
| p2 ratio 5 | balanced | 1 | 100.0% | LSMR | 0.057 | 0.083 | 0.095 |
| p2 ratio 5 | balanced | 1,000 | 0.1% | LSMR | 0.076 | 0.121 | 0.084 |
| p2 ratio 5 | one large + tiny | 1,000 | 80.0% | LSMR | 0.076 | 0.112 | 1.060 |
| p3 ratio 10 | balanced | 1 | 100.0% | CMG | 0.094 | 0.091 | 0.095 |
| p3 ratio 10 | balanced | 1,100 | 0.09% | CMG | 0.045 | 0.098 | 0.045 |
| p3 ratio 10 | one large + tiny | 1,000 | 9.18% | CMG | 1.329 | 0.116 | 1.320 |

Thus the currently auto-eligible mixed 1,000-component cell incurs about
1,046% regret relative to explicit Schwarz-LSMR with four controls. The
single-RHS median is 1.162 seconds for auto-CMG versus 0.090 for LSMR, about
1,191% regret. By contrast, CMG is more than twice as fast as LSMR when all
1,100 components are equally tiny. A hard component-count cutoff would reject
both the pathological case and a strong CMG win.

The mixed single-RHS sweep localizes a hierarchy cliff:

| Profile | Components | Largest share | CMG s | LSMR s | CMG/LSMR |
|---|---:|---:|---:|---:|---:|
| p3 ratio 10 | 600 | 45.6% | 0.099 | 0.091 | 1.1x |
| p3 ratio 10 | 800 | 27.4% | 0.834 | 0.094 | 8.9x |
| p3 ratio 10 | 1,000 | 9.18% | 1.159 | 0.090 | 12.9x |
| p3 ratio 10 | 1,100 | 0.09% | 0.036 | 0.083 | 0.4x |
| p2 ratio 5 | 600 | 88.0% | 0.112 | 0.083 | 1.3x |
| p2 ratio 5 | 800 | 84.0% | 0.628 | 0.088 | 7.1x |
| p2 ratio 5 | 4,000 | 20.0% | 5.466 | 0.169 | 32.3x |
| p2 ratio 5 | 5,000 | 0.02% | 0.038 | 0.189 | 0.2x |

Every one of the 342 `p2_ratio5` recorded runs and 171
`p3_ratio10_auto` runs returned successfully, certified, and used no fallback.
Actual component counts always equal their targets. Coefficients and RSS are
identical at the precision exported by Stata across auto, explicit CMG, and
explicit Schwarz-LSMR. This is a performance problem, not a reliability or
scientific-equivalence failure.

## Mechanism

CMG setup time does not cause the cliff: it falls to about 0.004 seconds in
the bad cells. PCG iteration counts also remain ordinary (about 21). The cost
per iteration rises from roughly 3 ms to 39--61 ms in the auto-eligible bad
cells.

The retained hierarchy explains that change. At 600 mixed components the
auto-eligible graph has four levels and ends in a direct terminal with repeat
counts `28;6;5;0`. At 800 components it has seven levels, terminates because
vertex reduction stagnates, and uses `28;10;3;3;3;1;0`. At 1,000 components,
the repeat schedule is `29;33;3;3;1;0`. Tiny components contract early but
their aggregate vertices remain in the global hierarchy while the large
component continues coarsening. Recursive centering, restriction,
prolongation, and smoothing repeatedly scan those mostly edgeless coarse
vectors.

A deterministic work proxy that sums `(vertices + matrix nonzeros)` at every
level multiplied by the product of upstream repeat counts rises from 6.4
finest-level passes at 600 components to 60.0 at 800 and 82.3 at 1,000. It
returns to 2.3 when all 1,100 components are tiny. The same proxy tracks the
more extreme ratio-5 sweep, reaching 310 finest-level passes at the 4,000
component peak. The hierarchy details are in `hierarchy_table.csv`.

## Implications and next work

1. Do not add a component-count cutoff to auto. Count is not monotone in CMG
   performance and would discard common favorable disconnected designs.
2. Add internal graph/hierarchy diagnostics: component count, largest and
   selected component-size quantiles, terminal reason, level dimensions,
   repeat counts, and the recursive-work proxy. These need not be public Stata
   results.
3. Prototype a post-setup safety gate. CMG graph/hierarchy setup is cheap in
   the bad cells, so auto can build it, reject a high-work hierarchy before the
   first PCG iteration, and use Schwarz-LSMR. Calibrate the work threshold on
   realistic AKM mobility graphs before changing production routing.
4. Pursue the structural fix in the CMG layer: solve disconnected blocks
   independently or build the multigrid hierarchy only for material
   components while handling tiny components directly. This attacks the cause
   and should preserve the strong all-tiny-component CMG wins. Parallel work
   can then be scheduled across components and/or right-hand sides.
5. Run the guard and component-aware prototype on worker-firm-year designs,
   realistic mobility distributions, and several component-size mixtures.
   Component structure should be varied independently of FE level counts.

One unrecorded 16-control auto-CMG warm-up in the auto-eligible 100-component
mixed case remained CPU-bound for several minutes even though its 0- and
4-control counterparts took about 0.09--0.10 seconds. That run was stopped and
excluded from the complete matrix. It should be localized separately with
per-stage timing and a bounded timeout; it is not needed for the component
conclusion above.

## Files

- Benchmark source: `scripts/cmg_component_diagnostic.py`
- Auto-eligible profile: `scripts/cmg_component_diagnostic_autoeligible.py`
- Ratio-5 raw runs: `cmg_component_runs.csv`
- Ratio-5 medians: `cmg_component_summary.csv`
- Ratio-5 compact table: `cmg_component_table.csv`
- Auto-eligible raw runs: `../cmg-component-autoeligible-v1/cmg_component_runs.csv`
- Auto-eligible medians: `../cmg-component-autoeligible-v1/cmg_component_summary.csv`
- Auto-eligible compact table: `../cmg-component-autoeligible-v1/cmg_component_table.csv`
- Exact hierarchy shapes and work proxy: `hierarchy_table.csv`
