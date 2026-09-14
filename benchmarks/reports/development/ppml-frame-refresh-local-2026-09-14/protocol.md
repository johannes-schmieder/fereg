# P4 combined PPML frame refresh: prospective local evaluation

Baseline is accepted 7739e8e, isolated predictor plugin 2689faf0. Both ordinary
and final-Hessian calls will use a single complete-frame refresh. Validate actual
new response/weights once, preserve prior response storage for the categorical
warm projector, and invalidate response products once. Preserve full weight-bit
and all-scale-field identity as the sole permission to advance current within
products; any changed weighted input invalidates them. Sample/topology identity
requirements remain. No observation-sized state or new worker pool is introduced.

Focused tests compare paired refresh with the established separate updates on
changed weights, test every weight-scale field and a one-bit weight change, and
require invalid complete-frame inputs to leave previous output usable. Preserve
existing fresh/frozen/strict, stale-norm and rollback oracles. Independently check
public one-FE unit/FW/PW and generated-factor Poisson/Hessian/sandwich cases.

First screen the original three large one-FE predictor targets on unchanged
full/warm data. Record exact scientific results plus command and every phase,
modeled/native/RSS/scratch, fallback/retry and iteration costs. Required command
gain is both 5% and 10 ms; if no target qualifies, reject the standalone pass
change unless material admitted memory saving is demonstrated. If a target does
qualify, run the original other seeds and protected low/narrow/weak/slope/generated/
wide cases. Numerical route work may differ only when identical weights validly
avoid reprojecting the controls; never disguise changed scientific outputs as work.
No compilation/test/oracle math/data generation/compression overlaps timing.
No agents or cluster until P0–P7 finishes; R1–R5 are excluded by the user.
