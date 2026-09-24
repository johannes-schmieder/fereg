# ADR 0153: selective current-frame PPML checkpoint retries

Status: accepted for local development, 2026-09-24.

## Decision

Within one admitted categorical checkpoint absorption, retain the existing
prepared LSMR solver through its original-input attempts. Its owner borrows
immutable topology, solver weights and original RHSs. Release it before another
absorption, weight frame or MAP fallback. This changes the two-/three-FE
checkpoint adapter, including admitted low-memory checkpoints; ordinary joint
LSMR, one-FE, slopes, explicit other engines and one-shot OLS keep their routes.

After a failed fresh canonical certificate, use its per-RHS infinity norms to
select only failing columns for the existing tighter retry. Each selected solve
starts from the untouched original RHS, with the same tolerance tightening and
iteration cap as before. Passing columns retain their exact bits. Reassemble the
complete result and recompute the full original-weight canonical certificate.
Selection never supplies acceptance. An empty/all-column selection or a refused
overlap budget uses the established full retry, releasing the candidate first.
The number of permitted tighter attempts does not increase.

## One canonical certificate per attempt

The previous generic adapter created a temporary certificate workspace and
checked the residual before the owning canonical workspace checked it again.
The caller replaced that report and accepted only on its own original-weight
certificate. The inner certificate was not an additional acceptance gate; under
component scaling it also used the solver's differently normalized weights.

The specialized checkpoint adapter now returns an explicitly uncertified report.
Only the caller's fresh original-weight check can turn it into a certified
result. Preserve original-input reference norms, the complete residual check,
exact canonical tolerance and the initial certificate/early return. Remove the
temporary certificate and its repeated reference-norm construction. This
supersedes that duplicated implementation in ADR 0027, not its scientific purpose.
The generic OLS/direct adapters keep their independent checking behavior.

The solver still reconstructs from original RHSs and FE coefficients. Subsequent
canonical rank, finite solve, model-space, inverse/bread, score, objective,
reconstruction and final-Hessian checks remain independent and unchanged. No
sample, weight semantics, rank/DoF, separation, tolerance or public transaction
changes. Previously accepted columns are valid only inside this immutable call;
they cannot substitute for checks under changed PPML weights.

After both attempts fail, drop all scoped state before MAP receives the untouched
inputs. Allocation, invalid-state and interruption errors remain fatal and
transactional. The existing positive-definiteness safeguard may replace a
rejected preconditioner; the prepared owner retains that decision through retry.

## Memory and diagnostics

Use the already admitted full-batch LSMR workspace as a hard local allowance.
Before selecting a partial retry, charge the narrow batch estimate, retained
full candidate, full result-metadata allowance and actual selection-vector
capacity. This includes one retained current-weight solver and its bounded lazy
pools, not a second solver. If overlap does not fit, discard the rejected output
and perform the full retry. Runtime result conversion checks actual capacities
against the same allowance. No new session payload or whole-fit admission formula.

The prepared solver now survives certification and a selective retry retains
accepted output while solving other columns. This is a lifetime tradeoff, not
a claim that memory is free. All measured whole-fit selected/scratch estimates
remain unchanged; actual tracked peaks can move in either direction. Existing
static allocation drift and physical-cap qualification remain unresolved.

Report aggregate attempt time and retry counts as before. Private traces expose
actual solved RHSs, per-column recurrence steps, setup builds and canonical
acceptance. The removed duplicate check no longer emits its generic warning;
canonical rejection remains visible in retry tracing and fallback diagnostics.
Summed batch maximum iterations are not comparable work units. The measured
wide diagnostic instead counts 53 to 37 RHS solves and 555 to 334 summed column
recurrence steps, with unchanged acceptance thresholds.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-selective-checkpoint-local-2026-09-24/README.md)
contains 55 distinct Rust tests, four licensed fixtures, independent weighted
projection and Poisson/Hessian/sandwich oracles, current-frame/memory refusal and
transaction checks, public rotations, fresh cases and OLS controls. All scientific
checks pass. Development wide PPML improves 17.7%; fresh unbalanced two FE improves
17.8%. Other fresh median gains are 1–3 ms and losing pairs remain visible.

The whole-public <=0.90-times-stock objective is unfinished. Retain this bounded
organization change and move to the cost of the remaining full checkpoint solves;
do not tune selective thresholds to consumed seeds or treat this gain as parity.
