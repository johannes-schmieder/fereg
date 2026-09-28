# Portable Linux slope CPU attribution

Job 7761864 completed after the uncontended nine-pair block, on shared scc-tb4
with eight slots. Clean accounting: 57 seconds, failed=0, exit_status=0, 1.315G
maximum virtual memory. All three application sentinels and perf reports pass;
zero lost samples. These are sampled user CPU cycles over the full warm-plus-
timed process, including loading and exports, not estimation-only wall shares.

The same source 852b03a9/plugin 2b57467f… processes the literal seed 202609280
N245760/K10 two-partition slope input. The prepared coupling apply routine has
41.50% of sampled self cycles at T1, 49.46% at T8 and 57.07% in the separately
labeled T8 passive-wait diagnostic. The libm addresses 0x39420/0x39424/0x39429
sum to 14.43%, 9.54% and 10.09%. Disassembly resolves these to endbr64,
vfmadd213sd and ret: **hardware FMA behind an out-of-line call**, not software
emulation. Sampled compact-product instructions include indirect calls and
stack reloads around them. This identifies a concrete portable-kernel target;
shares in the caller and callee are distinct, and cannot be summed as a wall-
time saving prediction.

Passive OpenMP waiting reduces visible Stata worker activity but leaves the
same dominant coupling routine. This single shared-node diagnostic is not an
accepted timing improvement or authority to change users' Stata settings.
The three-FE low-memory control regression is a separate unresolved problem;
its route does not use this new two-partition owner. No arithmetic, tolerance,
certificate or threading policy changed during these measurements.
