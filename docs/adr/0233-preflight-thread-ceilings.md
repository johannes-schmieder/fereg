# ADR 0233: preflight thread ceilings

Status: candidate; focused science validation passed, performance acceptance
pending, 2026-09-28.

## Decision being tested

Implement the user-approved interpretation of `threads(N)` as an execution
ceiling. Preserve the original request in existing runtime fields. A shared
public-command planner chooses the effective team once, before construction of
any worker pool or solver memory admission. That team continues to govern the
entire action, including preparation, separation, IRLS, inference, saved-effect
recovery and publication. No generation may independently enlarge the team.

The initial candidate limits automatic, fast/auto-memory, three categorical
intercept FE PPML with at most 16 response/control coordinates to eight workers.
The width includes a response coordinate and conservatively counts declared
finite terms, including omitted coordinates, before data-dependent rank. The
choice uses no benchmark name, seed, outcome, convergence history or timing
probe. OLS, absorbed slopes, other FE counts, explicit solvers, low-memory
requests and wider frames retain their current choice. This bounded policy is
not a completed general-purpose concurrency model or a universal scaling claim.

The earlier source-bound development block measured narrow K6/K10 three-FE
PPML losses above eight workers, while wide K50 benefited substantially from
28. Fresh OLS nine-pair confirmation favored T8 on the smaller fast/low slope
cases; therefore this candidate does not impose an OLS T4 cap. Completed
[same-artifact PPML confirmation](../../benchmarks/reports/development/ppml-thread-scaling-scc-2026-09-28/final/README.md)
finds narrow T28/T8 paired medians of 1.113 on the reused input and 1.126 on a
fresh input, all twelve pairs slower. Fresh K32 favors T28 at 0.769. This supports
testing a width-sensitive choice; it does not establish the proposed boundary.

## Request, memory and scientific boundary

`ThreadConfig` stores a nonzero optional execution ceiling in its existing
descriptor padding. `requested` remains unchanged; `resolved()` respects both
the request and the ceiling. Lower requests never increase. A stricter ceiling
cannot subsequently be loosened. A focused size assertion protects the current
descriptor footprint. No numerical bank, predictor cache or persistent model
state is added.

OLS passes through the common planner unchanged. PPML selects the policy after
metadata/model validation but before input loading, team priming or fit-lifetime
memory planning. Existing native shadow validation compares the original request
and the selected effective count independently. Existing Stata result matrices
already expose both counts and permit them to differ. The one-team ownership
and SPI/transaction boundaries of ADR 0020 remain authoritative.

Numerical solvers, weights, sample, rank, DoF, tolerances, separation and all
current-weight, model-space, score, objective, reconstruction, final-Hessian and
publication checks are unchanged. Changing the team can alter deterministic
partitions, reductions, workspace admission and resulting recurrence. Acceptance
therefore needs independent explicit-design evidence; equality to old bits is
not a substitute. Reproducibility applies to the selected team and algorithm.

## Required development evidence

- Requested/selected boundary checks, lower/automatic counts, monotone ceiling,
  unchanged descriptor size and excluded families/widths/modes.
- Explicit Poisson, full Hessian and weighted sandwich comparisons at an actual
  capped request, robust/clustered inference, FW/PW semantics, saved FE/mean/eta/
  residual identities, and an uncapped low-memory control.
- Existing categorical/sloped PPML and OLS controls, main-thread SPI callbacks,
  one-team memory reporting and failure-atomic publication.
- Source-bound Linux candidate/baseline/comparable-stock rotation, followed by
  predeclared fresh width, sample, connectivity, weight and output controls.
  Report effective as well as requested teams; retain failed comparisons as NR.

## Evidence to date

Snapshot `1f290d1c…` passes five focused Rust tests and four licensed fixtures on
both Mac and Linux. Linux job 7762631 also passes the shared two-fixture smoke;
the Mac universal build passes its ABI/panic checks. The new independent fixture
checks all eight fast/low × FW/PW × robust/cluster combinations, saved outputs,
requested/effective switching and post-plugin publication rollback. The zero
outcome and broader performance boundary still need the registered rotation.
Receipts are in the [candidate report](../../benchmarks/reports/development/thread-ceiling-candidate-scc-2026-09-28/).

No acceptance or speed claim is made by this candidate ADR. The historical
allocation-inventory drift, constrained-fit/CG issues, comparator adjudication,
full performance sweep and wider concurrency policy remain separate open work.
