# Indexed singleton pruning: local development protocol

Baseline main 681634d9968b81fc7ece5bd3a0f76b791658da4f; diagnosed singleton source
SHA256 3d32d2af0c03f45a2d46445b7bb7a458d40269f0548cb929a0f40c1ff47fba40 matches.
Installed plugin is preserved; isolated baseline/candidate builds only.

Acceptance before empirical rerun:
- Exact complete SingletonResult against the old queue scanner, independent
  simultaneous-pass masks, cascading paths, inactive masks, 1-5 effects and
  unit/integer-frequency multiplicities (including large integers).
- Every reservation/capacity failure remains fail-closed. No-singleton branch
  remains three allocations; indexed pruning's total payload does not exceed
  the old queue/count/mask/drop payload on matched shapes.
- Focused shared singleton/preparation tests and plugin transaction/interruption
  tests, plus independent licensed OLS and PPML sample/weight checks.
- Release scale experiment: same deterministically generated chain and 50%
  singleton panels at 8192/16384/32768/65536 rows, 3 repeats, alternating arms.
  Candidate-only larger 262144/524288/1048576 sizes. Exact masks/drops mandatory.
  Record full pruning time and live allocation peak, including all index setup.
  Target approximately linear candidate growth, demonstrate old quadratic trend.
  Balanced no-drop cases included as a protected control. No flaky wall-clock
  assertion in unit tests. No build/test/oracle work during timing.
- Empirical smoke then full 91,045,290-row case, original sample/controls/options,
  using isolated candidate and distinct logs/estimates. Verify 65,503 smoke and
  88,876,264 full retained rows. No external pruning or keepsingletons.
  Preserve original files and installed dist. Record all failures as failures.

Membership proof: validated positive integer mass 1 has exactly one physical
row. XOR of active row indices therefore returns that row, including row zero.
Each deletion XOR-removes it from every effect. The existing count arithmetic,
queue insertion/order and singleton predicate remain. L=sum(levels), D=effects:
O(D*N+L) work, existing output/mask/count banks plus L words for XOR paid for by
reducing queue elements from two words to one. Index created after no-drop exit.

Existing caller-thread interruption/publication boundaries are unchanged. Core
pruning previously had no SPI/callback poll; no worker is given SPI access.
