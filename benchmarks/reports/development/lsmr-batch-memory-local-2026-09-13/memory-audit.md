# Categorical batch lifetime audit

This extends the [accepted transport audit](../lsmr-transport-local-2026-09-12/memory-audit.md).
The dependency remains pinned to within
`1ac3522a251c2622dbeab451415924f9ebe117b9` and its locked approx-chol dependency.
The latest candidate is source-bound by [identities](variants/wide-guard/identities.json),
but remains unadopted for the timing reason in the main report.

Let N be retained rows, L total categorical coefficients, R RHS columns,
T installed workers and h=min(requested history,N,L). All products and sums in
the implementation are checked u128 arithmetic before conversion/allocation.

| Live state | Revised accounting / evidence |
|---|---|
| Modified recurrence and local history | 8 × active_R × (2N + (6+2h)L), based on the previously isolated recurrence payloads. |
| Off recurrence | 8 × active_R × (2N + (5+h)L). |
| Outer RHS concurrency | Charge all R through 64 RHS or through 4T; otherwise the adapter explicitly admits at most T RHS per upstream call. Nested Rayon alone is insufficient: a pinned upstream replay observed five live modified recurrences on four workers. |
| Transformed inputs and scatter | Two N-vectors per active RHS; largest-FE atomic scratch and the pinned small-block fold leaves/identities. Locality permutation and sorted codes/weights are charged separately. |
| Returned outputs | Individual and combined active-window coefficient/residual banks can overlap. A windowed call additionally retains the complete final L×R and N×R banks. Small batches avoid early full-bank reservation. |
| Dense cross tabs | Charge each primary table when LiLj≤5,000,000 or 8LiLj≤12N, plus the separate weighted compensation table. All pairs can be constructed concurrently. The old OLS model omitted the primary dense table. |
| Schwarz local solve scratch | Each of the two vectors includes both FE blocks and reduced-factor augmentation: a conservative 6×max_level_count+4 f64 total. |
| Schwarz buffer stacks | WorkerBufferStack retains reentrant frames. Components with an FE block above 10000 can yield; charge T×(1+floor(pair_level_incidence/10002)) worker frames per apply. This is deliberately conservative. |
| Retained Schwarz pools | The pinned maximum of four applies to complete pooled objects, each of which can retain its full worker stack; it does not mean four numeric vectors. |
| PPML overlap | Checkpoint model shares the categorical batch estimate and adds its caller-owned input matrix and independent certificate workspace above the retained session. Ordinary joint solve arithmetic is unchanged. |

The standalone [final replay](allocation/final-results.json) excludes the caller's
original codes, weights and RHS from its measurement start. It includes copied
weights/RHS, construction, retained state and solve/output overlap. Nine final
shapes span unit/weighted, identity/diagonal/Schwarz, sorted/permuted, one/four/
seven workers, narrow/wide RHS, dense setup and the exact weighted chain.
All peaks fit; three bounded solves are unconverged and are not scientific
oracles. The replay duplicates the explicit window transport around pinned
within; real plugin comparisons independently measure production action peaks.

For the exact 320-row weighted chain with 160/161 levels, construction alone peaks
at 425777 bytes. A 327680-byte solver budget is admitted by the baseline model but
rejected before LSMR work by the revised model, which selects certified CG with
the same residuals. The revised solver estimate is 575016 bytes. An earlier
65536-byte test did not distinguish the implementations and is explicitly
excluded as proof; see [routing proof](allocation/routing-proof.json).

On the wide replay, complete peak is 222131347 bytes with the four-RHS window,
against a 571522576-byte revised incremental model. The earlier full-RHS model
was 3956084544 bytes. These are distinct operator examples from the full Stata
command; do not compare their incremental peaks directly with public action
models or infer a universal tightness ratio.

## Limits that remain open

* The inherited opaque edge/factor constants (80 bytes per retained edge/level
  incidence; 64 bytes per transient edge/level incidence) are estimates rather
  than a complete proof for every upstream allocation/layout.
* The reentry and four-pool accounting is conservative. Tighter bounds need a
  source proof plus observed lifetime checks; scheduler intuition is insufficient.
* The public OLS selected-model scalar still describes the execution model and
  omits topology solver increments. A repair must account for every actually
  attempted construction, discarded fallback and retained solver, not infer the
  peak solely from the solver that finally returned a certificate.
* Full constructor/fallback counters, broader graph/width coverage and formal
  memory qualification remain separate work. Do not report these diagnostic
  examples as a universal allocation bound or qualified release.
