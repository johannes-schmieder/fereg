# CMG hierarchy-guard fresh holdout

This is development evidence, not qualification.

## Design and execution

- Diagnostic plugin SHA-256: `ff8592ad6d0b3ad9b00027e5de898ab08bceb6d87a1a4d73e8344d341afbc680`.
- 32 datasets, 288 recorded runs, and exactly three repetitions per solver cell.
- Every dataset was generated once and shared by auto, explicit CMG, and explicit Schwarz-LSMR. Solver order rotated; warm-ups and trace calls were unrecorded.
- The matrix used fresh seeds and new material-component wiring at 3, 4, and 5 observations per worker, then crossed 0/4/16 controls with 1/8 threads on three representative graphs.
- Successful runs: 288/288; certified runs: 288/288; recorded fallbacks: 3.
- Auto used CMG in 90 recorded runs and LSMR in 6; auto fallbacks=0, warnings=0, MAP-pilot runs=0. Any difference between the all-arm fallback count and the auto count belongs to an explicit comparator arm.

## Holdout result

- Comparable certified CMG/LSMR cells: 31.
- Iterative-terminal cells: 2.
- No iterative-terminal cell favored CMG by more than five percent.
- Smallest work proxy among iterative-terminal CMG losses: 5.68.
- The terminal-aware W<=4 rule rejected 2 cells, falsely rejected 0 measured CMG wins, and retained 7 greater-than-twofold CMG losses.
- Its geometric-mean regret was 1.417; p90 3.011; maximum 8.190.

Thresholds 4, 6, 8, 12, and 16 are reported in `holdout_thresholds.csv`. The fresh holdout moves the first iterative-terminal CMG loss down to W=5.68, so the conservative implementation threshold is 4. This still lies above every measured iterative-terminal CMG win (W<=2.30) in the earlier calibration.
