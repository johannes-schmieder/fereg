# Published PPML covariance inspection: local repair protocol

Baseline fd6b970a24f79a78e05fda585760beed59eb74dc, plugin
7b7c6664132041e60425382edb99728a20bc258c628a68369de32852d0bb65fe.
The wider moment prototype is absent. Change only the final PSD-report validator:
inspect the published matrix's own eigensystem, preserving the existing bound on
further repair, rank/minimum/reconstruction/orthogonality checks and every
reported-policy/tolerance field. Production CGM clipping is unchanged.

The exact original N7680/K257/PW/two-way/offset/missing/16-worker warm-up rejects
on baseline after native computation. Replay that request directly for functional
and independent covariance proof; do not count failure as timing. Also replay
the original N30720 full request directly in both arms and preserve full exports.
These cold invocations are separate from the original wider optimization's warm
protocol, which resumes only after this repair. No smaller substitute target.

Use the closed-form Householder covariance oracle and adversarial report/matrix
mutations, focused result/PSD/VCE and PPML plugin tests, ordinary independent
Poisson/sandwich and long-request fixtures. Independently solve the original
warm-up's profiled one-FE Poisson likelihood with NumPy, build current-weight
centered Hessian and complete two-way cluster covariance, then clip the identified
slope covariance with an independent eigensolver. Check sample, b, full V, rank,
cluster counts and publication certificates at existing registered envelopes.

Nine paired cost controls use seed31415927 and rotated order: complete PPML K50;
FW K129 with exposure/missing; PW K50 with offset/two-way clusters; weak three FE;
low FW; small/narrow; saved/separation; OLS AW K129 and narrow/no FE. Require exact
full b/V and discrete science/solver fields on successful pairs; record all phase,
native/model/scratch/RSS and retry/fallback costs. Investigate increases above
max(10ms,5%). This correctness repair does not depend on a speedup. If a target
saves at least 5% and 20ms in the changed inference/publication work, two further
seeds may assess repeatability; do not generalize from a single cold wide run.

No numerical tolerances, PSD policy or fixtures are weakened. No builds/tests
alongside timings. Dist stays unchanged; no agents, cluster, push or qualification.
