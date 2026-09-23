# ADR 0111: Guarded full-rank solve from scratch moments

Status: accepted for local development, 2026-09-22.

## Decision

Extend the existing guarded diagonal-pivoted Cholesky trial to scratch-backed
finite OLS fitting. This supersedes the low-mode-only-QR implementation scope
in ADR 0030, without changing rank or omission policy. Low mode already forms
standardized compensated Gram and RHS moments before QR. Pass those unchanged
moments through the shared trial, with the same square-root rank guard, finite
score draft and two-sided inverse certificates as the dense path.

Accept only a complete full-rank trial. Weak, deficient, nonfinite-pivot or
uncertified-inverse trials return to the unchanged scaled pivoted QR, which
owns omissions and its independent final scratch rank check. The trial does
not mutate scratch or the response. Its local owners drop before fallback.
Allocation/internal errors and injected certificate corruption remain fatal.
Do not turn failed final residual or inference certificates into successful
results or weaken their tolerances.

Make the shared helper's standardized dense design optional. Dense callers
retain their existing reconstructed within residual. Scratch callers do not
construct a dense design or extra residual. Their existing caller reconstructs
from original columns, independently projects the final residual, and certifies
fresh scores and reported bread. Robust/clustered score and covariance
certificates continue their separate fresh traversals.

This is same-call finite work. It introduces no topology, weight, working-response
or iteration cache, and does not generalize OLS cache validity to PPML. Shared
weighted-linear callers keep their own fresh-frame and final-Hessian contracts.
Samples, weight normalization, rank thresholds, omitted slots, DoF, solver
selection for FE absorption, publication atomicity and retained-session policy
are unchanged.

## Memory

The trial's maximum simultaneous square owners are the caller Gram, trial
Gram, Cholesky factor, inverse factor and canonical inverse. The inverse-factor
owner drops before finite bread allocation; that later stage also has at most
five square banks. Linear coefficient/status/permutation storage remains
within the existing finite frame's linear allowance. There is no additional
row-sized buffer or private file payload.

The existing internal OLS finite envelope, `128*q*q + 1024*q + 4096` bytes for
maximum generated/reported width q, already charges QR/Cholesky trials in both
modes. It remains unchanged, as do published legacy base estimates. The focused
allocator check passes, but does not close the previously documented whole-action
or constrained-budget gaps.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/low-memory-cholesky-local-2026-09-22/README.md)
records 8.3–38.8% paired median gains across fifteen eligible low-memory rows.
The wide finite phase falls from 924 to 299 ms; the uninstrumented public command
changes 1661→1046 ms, with fresh K40 confirmation at 2126→1304 ms. Current native
status and DoF exports are identical; small floating-point and descriptive
condition-proxy differences are expected from changing the finite algorithm.
All 48 native science pairs pass unchanged envelopes, as do five independent
dummy-design pilots and saved-output comparisons.

Twenty distinct Rust tests, an optimized direct trial test, actual-plugin ABI
containment, three licensed OLS oracle/transaction fixtures and a nine-case
low-memory PPML fixture pass. Tests include wide weak/deficient fallback,
weighted frames, rank zero, every injected low-fit reservation failure and
coefficient/bread corruption. Stock still wins eleven of twelve comparable
rows. The disconnected three-FE stock covariance has different DoF and remains
unranked; diagnostic rescaling does not adjudicate rank. No qualification,
constrained-memory or cross-platform claim follows.
