# Direct-terminal CMG calibration

This is development evidence, not qualification.

- Diagnostic plugin SHA-256: `ff8592ad6d0b3ad9b00027e5de898ab08bceb6d87a1a4d73e8344d341afbc680`.
- 84 datasets and 756 recorded runs; every solver cell has exactly three repetitions.
- Each generated dataset was shared by auto, explicit CMG, and explicit Schwarz-LSMR; warm-ups and trace calls were unrecorded and solver order rotated.
- Successful runs: 756/756; certified runs: 756/756; fallbacks: 0.
- Certified comparable direct-terminal cells: 84/84.
- Locked candidate: rejected 61, including 0 cells without a certified requested-tolerance LSMR comparator; among comparable cells it false-rejected 0 CMG wins above 5%, retained 0 greater-than-twofold CMG losses, and had maximum regret 1.311.
- CMG/LSMR median ratio: min=0.617, median=1.572, max=8.156.
