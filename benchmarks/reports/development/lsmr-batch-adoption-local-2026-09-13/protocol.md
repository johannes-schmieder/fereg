# PPML timing follow-up: identical binaries and ordinary commands

Prospective September13 follow-up to cd298d5. The full candidate remains unadopted. Keep all previous timing regressions and decompositions. No source or plugin changes in this test.

The problematic K17 runs enabled Stata/phase profiling, and later native solver traces. Compare ordinary public commands without diagnostic tracing. Interleave an identical-baseline-binary A/A control with each baseline/candidate A/B pair, using exactly the same input and options: PPML weak graph N30720/K17/four native workers, PW/offset/one-way cluster. Seeds15485863/67867967/49979687, two repeats with opposite order. Alternate A/A-versus-A/B order within each seed. Each command has a fresh process and the existing untimed warm-up. Twelve pairs/24 measured commands, not a broad campaign.

The private measure wrapper overrides only the A/A candidate's source to baseline and corrects its recorded artifact fingerprint. A/A selected-model bytes must be exactly equal; every pair keeps all scientific and non-time solver gates. A/B omits only changed model equality, as before. All command/phase/native/RSS records remain. No build, test, profiler or other agent alongside timings. No SCC or qualification.

Keep max(10ms,5%) as the review envelope. The purpose is to discriminate a persistent production-command cost from variation or diagnostic instrumentation; do not call earlier losses noise solely because a later result improves. If the A/B loss remains systematic or exceeds controls, localize the changed stage before adoption. If ordinary A/B fits the existing envelope, assess it jointly with the preserved traced evidence and the source-identical A/A controls.
