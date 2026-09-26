# Corrected-source SCC smoke acceptance

Job 7745656 completed all 28 groups, with scheduler failed=0/exit_status=0,
879 seconds, 28 slots and 2.147G maxvmem. SYSTEMATIC_BUNDLE_COMPLETE is present.
The frozen controller accepted the exact runtime/plan/receipt hashes in
SMOKE.ACCEPTED.json. All 28 native fits pass independent sample, coefficient,
full-covariance, objective and applicable convergence checks. This includes the
previously failing low-memory OLS slope warm-up and the corrected full-cone
zero-count oracle cases. There is no gate waiver.

All 148 attempted arms remain visible: 140 pass and eight comparator worker
failures. Pair checks: 81 pass, 31 accuracy failures, eight unavailable. These
failures and sample mismatches are not ranked as timing losses. Smoke runs use
one repetition on bounded inputs; no full-size or distributional performance
claim follows. Default and strict stock PPML remain separate.

The accepted gate authorized pilot 7745818, task 1, using the attached pilot.json:
12 registered warm-1 groups, one 28-slot E5-2680v4 task, 3G/core, 12h maximum. Initial
historical cost forecast is 1.36 node-hours; pilot measurements will refine the
production packing. The pilot still requires terminal accounting and comparable
scientific pairs before production submission. Current state belongs in STATUS.md.
The old rejected c31bcb62 smoke and its evidence remain unchanged.
