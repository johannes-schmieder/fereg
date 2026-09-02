# Direct-terminal LSMR fallback diagnostic

This is development evidence, not qualification.

- 14 cells selected because requested-tolerance Schwarz-LSMR fell back deterministically in the fresh holdout.
- 84 recorded runs: CG and Schwarz-LSMR at 1e-10, exactly three repetitions per cell and arm.
- Explicit CG certified without fallback in 14/14 cells.
- Schwarz-LSMR at 1e-10 certified without fallback in 10/14 cells; it still fell back in 4.
- Fastest eligible arm counts within the subset: CMG=4, CG=2, tighter LSMR=8.
- A separate unrecorded 1e-12 probe was stopped after one p3/17-RHS/one-thread call exceeded several minutes; over-tightening is not a bounded routing solution.

Requested-tolerance LSMR stopped after only a few iterations in the probed failure, but its FE normal-equation diagnostic was about 0.0076. The certified CG continuation was scientifically valid but 7--69 times slower than a direct eligible comparator in this subset. These are deterministic certification/stop-criterion mismatches, so fallback cannot be treated as a normal direct-terminal route.
