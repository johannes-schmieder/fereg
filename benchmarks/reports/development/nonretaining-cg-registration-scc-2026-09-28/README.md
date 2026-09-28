# Nonretaining CG: direct group-contribution candidate

This is a source-bound **candidate registration**, not an accepted repair or a
performance result. Production package `c09eeab3…` (259 inputs, accepted estimator
commit `161b9491`) remains unchanged at controller checkpoint `f69f7c57`.
The full current-source OLS/PPML sweep continues on its frozen artifact.

## Diagnosis and candidate

The [OLS closeout](../two-fe-finishing-experiments-local-2026-09-21/README.md)
preserves a near-within CG failure: FE moment certification did not detect an
incorrect component outside the FE model space. Retained coefficient products
repaired the admitted automatic two-FE correction, while explicit and
insufficient-budget nonretaining cases remained open. The existing CMG unit test
deliberately skips those unadmitted branches. Its historical failures are not
current-source reproductions; this registration supplies that missing check.

Current `borrowed_map.rs::absorb_cg_with_reservation` forms its ordinary `I-S`
action through observation-vector subtraction. Reconstructed retained products
avoid that subtraction for the admitted correction. Separately, the reusable
categorical PPML CG accelerator already has
`borrowed_symmetric_projection_from_group_means`: it sums current weighted FE
contributions directly into the action, without coefficient history. This is
an existing, tested numerical mechanism, not a new solver or an extrapolation
experiment. Its existing test independently verifies identical-tuple and
additive-model-space identities.

The [candidate patch](candidate.patch) routes **nonretaining** generic CG's
initial and iterative actions through that routine. Retaining CG and its saved
products keep their prior implementation. Fixed 64-column tiles extend the
routine's serial fallback to wider matrices. For widths already used by PPML,
the group, row and summation orders are unchanged. Hestenes convergence, negative
energy audits, fresh FE certificates, stopping tolerances, reference scales,
memory admission and publication are untouched.

The hypothesis is that this removes the cancellation source in nonretaining
corrections without buying stability through four extra coefficient banks.
It is **not yet numerically established** on current OLS. No additional heap
or observation bank is introduced in the patch; the action uses its existing
512-byte mean scratch and admitted workspace. Changed access patterns and wide
serial tiles could cost time. Public timings and exact memory receipts are
required before adoption. This does not purport to repair every weighted
constrained-fit timeout or the separate explicit weighted-CG energy failure.

## Registered validation

[Registration](registration.json), [deployment](deployment.json), and
[submission](submission.json) bind **7767016**, eight slots, 3G/core, one-hour
cap. It is held behind **7766045 and 7766146**. Both test builds and all numerical
work occur after these dependencies. Run root:
`/projectnb/welfgr/fereg/dev/20260928-nonretaining-cg-01`.
The accepted source is read-only. Baseline and candidate have separate copied
source trees, source manifests, Cargo targets and test executables. Tests are
appended identically to both copies; the candidate alone receives the patch.

Each artifact runs **72 oracle cases**:

- 40 direct CG cases: 1/4/64/65/129 RHSs, T1/T4, unit weights and positive
  weights scaled by 1e-100/1/1e100.
- 32 exact historical-layout correction cases: T1/T8, the same four weight
  frames, and automatic/explicit, saved-product and insufficient-budget arms.
  The previously skipped branches now execute. Retained-product controls stay.

The known solution is independently constructed: four repetitions of every FE
tuple have the same weight and balanced signs. The signed noise is therefore
weighted-orthogonal to the explicit FE design. Inputs add a known tiny additive
FE signal. The final residual must equal that noise to the existing 1e-11
envelope, as well as passing the native 1e-12 certificate. Generic RHS columns
have distinct noise and FE multipliers. This checks model space independently
of the recurrence and its own FE normal equations. No benchmark seed is used.

Ten existing controls per artifact cover compensated energy and rejected steps,
weighted/batched determinism, breakdown/exhaustion, exact workspace accounting,
the constrained LSMR-to-CG boundary and one-byte refusal, current-weight PPML
reuse, the algebraic PPML action, admitted retained corrections, and the terminal
correction budget. Test filters are exact and must execute one test each.
Every probe case, including application errors and oracle failures, is retained.
Missing, duplicate or unregistered records fail closed. A five-minute per-test
timeout preserves its log and is not a successful numerical result.

The [local preflight](preflight.json) passes patch application, two Python syntax
checks, shell syntax, Rust parsing of the candidate and two probes, and
[four evidence-parser tests](harness-tests.txt). All 259 production inputs match
the accepted package. **No compilation or numerical test has run for this
candidate at registration.** Rust parsing is not type checking.

## Acceptance boundary

Collection requires accounting, exact source/executable identities, complete
probe coverage and all control results. The diagnostic distinguishes complete
execution from a green candidate. A reproduced baseline failure, green candidate
oracles and green controls only authorize the next public validation step; they
do not establish estimator correctness, timing acceptance or qualification.

Before adoption, exercise independent public weighted OLS/inference/saved-output
oracles, relevant categorical PPML controls and bounded constrained fits; then
measure alternating baseline/candidate command pairs under otherwise matched,
uncontended conditions. Preserve any remaining failure or slowdown and reject
or narrow the candidate on that evidence. Static allocation inventory drift is
not refreshed by this experiment. No frozen definition, accepted solver policy,
installed artifact or historical receipt changes.
