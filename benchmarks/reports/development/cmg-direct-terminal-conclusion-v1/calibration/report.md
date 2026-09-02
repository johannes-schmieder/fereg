# Direct-terminal conclusion: calibration

This is focused development evidence, not qualification.

Schwarz-LSMR at 1e-11 was removed after a normal five-RHS, one-thread calibration cell exceeded the bounded diagnostic runtime. The final comparison therefore contains only the two reliable explicit destinations, CMG and direct CG.

- Datasets: 61; recorded runs: 366; exactly three repetitions per solver cell.
- Winner counts: {'cg': 32, 'cmg': 29}.
- Fallback cells by arm: {}.
- Provisional candidate route counts: {'cg': 20, 'cmg': 41}.
- Candidate ineligible destinations: 0; regrets above 1.25: 8; regrets above 2.0: 0.
- Candidate median regret: 1.000; maximum regret: 1.681.
- Secondary extended route counts: {'cg': 25, 'cmg': 36}; ineligible destinations: 0; regrets above 1.25: 5; regrets above 2.0: 0; maximum regret: 1.627.
- Diagnostic plugin SHA-256: `66a460108f85c41c1e60114af062ff40d3547ec9e6c1177236bc5a3f84d73b39`.
