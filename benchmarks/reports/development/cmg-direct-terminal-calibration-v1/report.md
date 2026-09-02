# Direct-terminal CMG routing calibration

This is development evidence, not qualification. Production automatic routing
was not changed.

## Result

The hierarchy/RHS/thread cost model predicts expensive direct-terminal CMG
cases well, but the proposed destination—requested-tolerance direct
Schwarz-LSMR—is not reliable enough to use as a planned route. The fresh
holdout would have sent 12 common cells into deterministic LSMR-to-CG fallback.
Fallback remains a correctness mechanism, not a normal route, so the candidate
was rejected before implementation.

## Calibration and holdout

- Calibration: 84 datasets, 756 recorded runs, exactly three repetitions per
  solver cell. All runs succeeded and certified without fallback.
- Fresh holdout: 48 datasets, 432 recorded runs, exactly three repetitions per
  solver cell. Auto and explicit CMG had no fallback. Requested-tolerance
  Schwarz-LSMR fell back deterministically in all three repetitions of 14
  multi-RHS cells (42 fallback runs); the final CG continuation certified.
- Every dataset was generated once and shared by auto, explicit CMG, and
  explicit Schwarz-LSMR. Data generation and one warm-up per arm were not
  timed, and solver order rotated.
- Tolerance was `1e-8`, the iteration budget was 16,000, and the matrix crossed
  0/4/16 controls with 1/8 threads. Calibration and holdout graph shapes were
  separate.

Before seeing the holdout, the locked candidate rejected direct-terminal CMG
when any of four charges was excessive:

1. hierarchy levels times repeat-weighted work per observation above 100;
2. repeat-weighted work times effective nuisance batches above 300,000;
3. at least five effective RHS batches; or
4. at least two RHS batches with a nonempty terminal factor and more than
   750,000 units of repeat-weighted batch work.

Across 172 prior-plus-new calibration cells, this candidate rejected 65 cells,
false-rejected no measured CMG win above 5%, retained no CMG loss above 2x,
and had maximum counterfactual regret 1.311 after charging discarded graph and
hierarchy setup. On the 34 holdout cells with certified no-fallback LSMR, it
again false-rejected no CMG win above 5%, retained no CMG loss above 2x, and
had maximum regret 1.315. However, it rejected 32 of all 48 holdout cells, and
12 of those 32 had no certified requested-tolerance LSMR route. That safety
failure dominates the timing fit.

## Certification diagnosis

In a representative failure, requested-tolerance Schwarz-LSMR stopped after
three iterations, but its FE normal-equation diagnostic was about 0.0076,
orders of magnitude above the `1e-8` contract. Fereg correctly rejected that
candidate and continued into CG. The final result certified, but the attempted
LSMR plus CG route was 7--69 times slower than a direct eligible comparator in
the focused failure subset.

A separate 14-cell diagnostic recorded another 84 runs:

- explicit CG certified without fallback in 14/14 cells;
- Schwarz-LSMR at `1e-10` certified without fallback in 10/14 cells and still
  fell back in four 17-RHS cells;
- the fastest eligible arm was tighter LSMR in eight cells, CMG in four, and
  CG in two; and
- an unrecorded `1e-12` probe was stopped when one p3/17-RHS/one-thread solve
  exceeded several minutes. Unbounded over-solving is therefore not a safe
  replacement policy.

## Decision and next work

Do not implement the locked CMG-to-LSMR rule. Preserve ADR-0022 unchanged.
The cost features are useful, but routing needs a reliable three-way
destination model:

1. Localize why LSMR's stopping diagnostic can be satisfied while the FE
   normal-equation certificate fails, including the RHS/column scaling that
   makes the event width-dependent.
2. Prototype a bounded certification-aware LSMR continuation. It must either
   certify within a small measured extra budget or decline the route before a
   long attempt; routine fallback is not acceptable.
3. Add direct CG as a first-class calibration arm for all cost-guard rejects.
   The focused holdout suggests CG is attractive for serial multi-RHS work but
   can lose to CMG with eight threads.
4. Refit the locked cost score only after the LSMR reliability boundary and
   CG destination costs are available. Test the resulting CMG/LSMR/CG rule on
   fresh graph seeds before changing auto.

The run-level data, medians, hierarchy profiles, locked candidate, and focused
fallback comparison are in this directory. The source runners are
`scripts/cmg_direct_terminal_calibration.py` and
`scripts/cmg_direct_terminal_cg_diagnostic.py`.
