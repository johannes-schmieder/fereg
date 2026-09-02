# Direct-terminal CMG holdout

This is development evidence, not qualification.

- Diagnostic plugin SHA-256: `ff8592ad6d0b3ad9b00027e5de898ab08bceb6d87a1a4d73e8344d341afbc680`.
- 48 datasets and 432 recorded runs; every solver cell has exactly three repetitions.
- Each generated dataset was shared by auto, explicit CMG, and explicit Schwarz-LSMR; warm-ups and trace calls were unrecorded and solver order rotated.
- Successful runs: 432/432; certified runs: 432/432; fallbacks: 42.
- Certified comparable direct-terminal cells: 34/34.
- Locked candidate: rejected 32, including 12 cells without a certified requested-tolerance LSMR comparator; among comparable cells it false-rejected 0 CMG wins above 5%, retained 0 greater-than-twofold CMG losses, and had maximum regret 1.315.
- CMG/LSMR median ratio: min=0.909, median=1.304, max=3.545.
