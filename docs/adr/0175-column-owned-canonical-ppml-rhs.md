# ADR 0175: column-owned canonical PPML right-hand sides

Status: accepted for local development, 2026-09-24. Not qualification.

## Decision

Replace the worker-private group-by-column RHS reductions in the low two-
categorical-FE canonical PPML product engine with one column-owned bank. Each
response/control RHS owns its complete retained-level vector. Workers iterate
independently over columns, visiting eliminated groups and their original rows
in fixed order. The subsequent triangular solve reads that same column-major
vector. There is no merge pass or alternate retained RHS engine.

Current-weight normalized geometry, component factors, work admission, grounding,
pivot guards, all-column solves and independent original-input residual/Gram
certification remain unchanged. The RHS is the same centered mathematical
quantity, accumulated in a different order. Original means use a compensated
sum over all groups in column order, followed by original scaling. RHS arithmetic
no longer depends on how groups were divided among workers. The existing
certificate still partitions groups and uses its compensated worker merge.
No tolerance or rank policy changes; bitwise equality with the former worker
recurrence is not a scientific requirement.

The owner is current-call only. No factor, weight, working response or product
acquires authority across a PPML generation. Samples, weight semantics, separation,
finite rank, model space, reconstruction, score, objective, final Hessian, VCE,
DoF and transactional publication keep their existing requirements. The admitted
canonical engine remains limited to the existing low two-FE PPML path. Fast mode,
ordinary joint directions, slopes, other FE counts and one-shot OLS retain their
implementations.

## Complete phase accounting

Let N be rows, P eliminated groups, Q retained groups, W response/control width,
T=min(workers,P), F=sum_c(Q_c-1)^2, C=PW for retained centers or zero for streaming,
and J=sizeof(usize). The former solve bound was:

    8[N+P+2Q+F+C+QW+2W+T(QW+3W)] + J[6Q].

It now becomes:

    8[N+P+2Q+F+C+2QW+2W] + J[6Q].

During RHS construction only one QW bank exists. During solving, it overlaps
at most QW solved coordinates. The factors/coordinate descriptors and other
existing live banks remain charged. Setup, full independent certification and
output reconstruction retain their previous bounds. Maximum-of-phases admission,
32W binding bytes, fixed descriptor allowance, fallible actual-capacity accounting,
and the unchanged 16*N*pool allowance still govern the complete call.

Before components are known, charge only the allocations actually made in that
phase: one Q-index parent table, Q ComponentFactor descriptors, column bindings
and the fixed allowance. The old projection buffer remains live and separately
subtracted from available space. The minimum future numeric plan must still fit
the full allowance. Once topology fixes component work and complete phase sizes,
all those checks must pass before the old buffer can be reclaimed. If the full
selected layout and old buffer can coexist, retain it unchanged; otherwise drop
it before numeric allocation. Refusal before that boundary preserves its capacity
and contents. This clarifies ADR 0171's intended staged ownership; it does not
remove the complete numerical memory check or increase a budget.

The topology-only implementation did not improve the measured public cases and
is not adopted as a separate performance result. It becomes useful alongside a
smaller complete RHS plan. No topology cache, new persistent bank or additional
solver layer is introduced. Certificate worker banks can still be the limiting
phase; this decision does not claim to eliminate every group-reduction cost.

## Validation and tradeoffs

The [source-bound report](../../benchmarks/reports/development/ppml-column-owned-rhs-local-2026-09-24/README.md)
records exact artifacts, rejected attempts, fresh distributions, memory and
excluded-path controls. Independent known WLS truth includes a new four-component,
100-control case whose old RHS phase exceeds the existing budget and whose new
complete plan fits. Paired observations have equal explicit coordinates and
reciprocal-weight errors, establishing nonzero residual truth without another
solver. Changed weights, direct residual Gram checks, retained/streamed/tiled
identity and the one-byte minimum boundary remain. Live-buffer tests separately
verify early refusal, complete-plan refusal, permitted retention and reclamation.

Existing extreme scales, FE-collinear channels, disconnected components, canonical
rank and strict rollback tests remain. Independent explicit Poisson/Hessian/
sandwich/means checks and licensed FW/PW/generated/cluster/saved-output fixtures
protect the public estimator. T1/T4 real allocation audits and OLS controls remain
separate from the static allocation inventory's known failure.

Column ownership limits RHS concurrency to W columns and changes memory access
from group-by-column to column-by-group. Small-width and serial performance must
therefore be measured, not assumed. Newly admitted direct work can increase
tracked peak inside the same whole-action model. Neither smaller RHS storage nor
a clear local timing monitor establishes a universal RSS, latency or platform
advantage. Unresolved constrained-fit, reporting and qualification gaps remain.
