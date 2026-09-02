# Direct-terminal conclusion: holdout

This is focused development evidence, not qualification.

Schwarz-LSMR at 1e-11 was removed after a normal five-RHS, one-thread calibration cell exceeded the bounded diagnostic runtime. The final comparison therefore contains only the two reliable explicit destinations, CMG and direct CG.

- Datasets: 54; recorded runs: 324; exactly three repetitions per solver cell.
- Winner counts: {'cg': 36, 'cmg': 18}.
- Fallback cells by arm: {}.
- Provisional candidate route counts: {'cg': 15, 'cmg': 39}.
- Candidate ineligible destinations: 0; regrets above 1.25: 17; regrets above 2.0: 5.
- Candidate median regret: 1.000; maximum regret: 3.250.
- Secondary extended route counts: {'cg': 21, 'cmg': 33}; ineligible destinations: 0; regrets above 1.25: 12; regrets above 2.0: 3; maximum regret: 3.250.
- Diagnostic plugin SHA-256: `66a460108f85c41c1e60114af062ff40d3547ec9e6c1177236bc5a3f84d73b39`.
