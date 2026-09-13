# P6 saved two-FE Schur/final-target recovery experiment

Accepted numerical baseline 257f4ce; evidence HEAD 75ad177; baseline plugin
c64ead07. Candidate changes only the ordinary OLS publisher's permission to
defer its initial coefficient product and the existing Schur admission. Default
weighted-linear policy remains false. Existing automatic fast, two intercept FE,
N>=16384, actual RHS 16..64, weighted probe, shared iteration budget and solver
workspace guards remain. Only an accepted probe omits the initial batch; rejected
probes keep coefficient retention. No PPML session or reconstruction change.

This is an unadopted prototype. The source audit identifies incomplete OLS
publication/recovery memory accounting, including legacy LSMR fallbacks. A
bounded local screen can establish whether the mechanism is worth pursuing;
it cannot admit the new route. Adoption additionally requires an explicit bound
for every overlapping recovery/fallback allocation and constrained-budget tests.
Do not interpret current public modeled peaks as a complete solver/recovery proof.

First run the core Schur guard test (accepted and rejected probes with saved
retention; default, solver, memory and width exclusions) and the independent
saved-output dummy oracle on both isolated plugins. Oracle: N18432/K16, unit,
AW/FW/PW, one/two FE, fast/low, missing/restricted samples, full/partial/combined
outputs, mean-zero normalized channels, original order and transactional rollback.
Candidate fast two-FE oracle fits must demonstrate actual accepted probe dispatch.

First-screen seed104729, fresh local Stata processes, N7680 warm-up, rotated arm
order. Three targets: unit strong two FE N245760/K50/T16, saved both channels;
AW strong two FE N983040/K50/T4, saved both; PW strong two FE N245760/K50/T16,
two-way clusters, missing and partial saved output. Include requested residuals.
Require full b/V, residual/saved tolerance parity, exact sample/status/DoF and
independent native certificates. A promising screen requires >=5% AND >=10 ms
command saving, or >=5% AND >=2 MiB native peak saving with <=5% command slowdown.

If no screen qualifies, run one separate original-target phase profile and
reject; no seed search. If a screen qualifies, next resolve memory admission
and recovery/fallback behavior before primary repetitions at seeds8675309 and
20260819. Protected controls then cover weak weighted cuts and unit weak graphs,
disconnected/unbalanced, narrow/small, slopes/three FE, explicit solver, low mode,
rank deficiency and constrained memory. Review every increase above max(10ms,5%).
Scientific failure stops timing; diagnose and preserve all failed attempts.

Record total/input/native/outer time, available phase traces, selected solver,
probe/retry/fallback costs, native modeled/observed peak, scratch and sampled RSS.
Separate profile seed67280421; profiles are not pooled. No hardware-traffic
claim or external-comparator claim. Build only in the isolated candidate copy;
do not replace main dist, launch agents/SCC/qualification, or push.
