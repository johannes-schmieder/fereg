# ADR 0154: bounded full-batch PPML Schur checkpoints

Status: accepted for local development, 2026-09-24.

## Decision

An admitted categorical two-FE checkpoint with its existing Schur graph uses
one component-scaled coefficient solve before original-input LSMR fallback.
Retain the first eight full-batch iterations as useful solve work. Remove the
separate three-column pilot and its discarded current-weight preparation.
At step eight, the maximum across unfinished columns of the square root of
current/initial preconditioned residual energy must be at most 0.01 to continue. Use the existing 64-step cap within
the configured budget. This diagnostic selects work; it never grants acceptance.
Every column keeps its own readiness state and original canonical reference norm.

Existing topology admission remains: at least 128 rows and owner capacity at most
64 RHSs. Remove the separate 16384-row checkpoint threshold after identical-frame
comparison supports the coherent solver below that size. Widths outside admission,
other FE counts, slopes, one FE and other engines retain their routes. Eligible
low-memory checkpoint ownership may also reach this path; memory mode alone is
not an exclusion. Ordinary joint categorical LSMR remains unchanged.

Supersede the checkpoint implementation in ADR 0067. A private work-limit enum
separates ordinary MAP, OLS and checkpoint purposes. A bounded PPML call no longer
disables the component scaling established in ADR 0147. Immutable component roots
remain PPML-owned; current group masses, RHS moments and scales are rebuilt for
each absorption. No weighted state receives authority across calls or frames.
Ordinary OLS constructors have no PPML component banks and retain their prior
probe, edge-cache, continuation and budget policies.

## Scientific acceptance and fallback

Reconstruct a converged candidate from original RHSs and FE coefficients, then
require the fresh complete original-weight canonical certificate at its unchanged
tolerance. Initial certification/early return, finite checks and per-column
original reference scales remain. A progress refusal, cap, finite breakdown or
certificate failure drops the candidate before selective prepared LSMR works on
untouched inputs under ADR 0153. Fatal allocation, invalid-state and interruption
errors remain fatal. The independent canonical rank, finite solve, model-space,
score/objective, strict profile, reconstruction, final-Hessian and inference
checks are unchanged, as are samples, weights, rank/DoF, separation and publication.

The change removes redundant speculative work, not an acceptance certificate.
Contraction and batch iteration counts cannot be compared directly with fixest's
accelerated per-column iterations or LSMR's per-column steps.

## Memory and reporting

One fallible candidate copy remains bounded by the existing checkpoint LSMR
transient allowance and is released before fallback allocations. The retained
Schur graph was already admitted. Add one fixed 64-double (512-byte) stack bank
for initial diagnostic energies; no new heap bank, session geometry, weight copy
or worker team. Existing modeled heap admission remains unchanged. Actual
allocation peaks are measured separately; no physical-cap qualification claim.

Successful attempts report zero discarded pilot iterations. Private traces expose
in-solve contraction and useful iterations; contraction is zero when convergence
precedes the step-eight diagnostic. Rejected work adds its complete
copy/setup/solve cost, actual iterations, retry and original-input warning to
fallback reports even though the old separate pilot no longer exists.

## Evidence and remaining work

The [source-bound report](../../benchmarks/reports/development/ppml-coefficient-checkpoint-local-2026-09-24/README.md)
records 56 distinct Rust tests, four licensed fixtures, independent weighted
projection and Poisson/Hessian/sandwich oracles, 192 public applications, fresh
five-rotation cases and OLS controls. All public scientific checks pass. The
isolated weak stock inner-frame projection discrepancy remains unranked.

Development wide/unbalanced public medians improve 24.8%/38.0%; fresh unbalanced
improves 43.1% and narrowly clears the 0.90 stock target (70 versus 78 ms). Fresh
narrow improves 26.4% but still loses to stock. Larger dense/disconnected gains
are only 1–2 ms; sparse has a +1-ms median and unchanged slopes a +3-ms median.
Keep those losses visible. Modeled memory stays unchanged; tracked peaks can
move either way, with substantial reductions in checkpoint-limited cases.

The broad public-speed objective remains unfinished. Ordinary categorical joint
directions now dominate more of the remaining native time (about 24 ms wide,
17 ms narrow). Compare those directions with existing coefficient-space joint
mechanisms while preserving canonical checkpoint independence. Do not tune the
new progress threshold on consumed fresh cases or treat kernel parity as whole
public parity.
