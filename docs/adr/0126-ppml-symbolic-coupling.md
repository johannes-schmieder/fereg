# ADR 0126: symbolic topology and compact PPML slope coupling

Status: accepted for local development, 2026-09-23.

## Decision

Extend the coefficient Schur solver from ADRs 0123–0124 with an independently
optional prepared operator. Separate group-pair adjacency and observation-to-block
maps, which depend on the immutable sample/design, from the numerical products
of current-weight local QR bases. Construct adjacency and sorted neighbor lists
once. Refill only numerical blocks when the owning sample/topology/weight
generation changes. Equal generations reuse both QR and operator products;
unframed calls, errors and unwinds revoke authority before fallible work.

For the eliminated partition Q0 and remaining bases B, apply the Schur operator
`D - C'C`, where `D = B'B` and `C = Q0'B`. Store diagonal and cross-partition
products as small group-pair blocks. Compute C and C' for one eliminated group
while its blocks are resident in cache. The compact path allocates no dense D
or Schur construction bank. Retain the measured local QR Gram blocks, including
rank-zero coordinates, rather than assuming exact identity or full local rank.

A sufficiently small remaining system can instead form and apply the explicit
Schur matrix. Select using declared channel dimensions and actual symbolic edge
counts: compact work is diagonal entries plus twice the pair-block entries;
explicit work is its square dimension plus C'C construction amortized across
maximum RHS width. Require the smaller estimate to save at least 20% against
`N * (2 * total_channels + 1)` observation work. Dense storage is considered only
if its application alone can meet that margin. This is a conservative structural
heuristic, not a prediction of public time. The margin was declared before this
successor and was not tuned to seeds, case names, widths or favorable clocks.
This adoption supersedes ADR 0125's rejection of the earlier implementation,
not its requirement to include preparation and memory in the decision.

The four CG vectors now contain only remaining-partition coordinates. Elimination
uses one group's scratch per worker, shared with compact coupling application.
Fresh transpose computes the all-partition projection maximum directly; eliminated
moments need not occupy every CG vector. Nonfinite eliminated moments explicitly
fail this diagnostic. Each RHS still converges independently with the same
recurrence/reconstruction schedule and iteration cap.

## Scientific authority

Prepared products change floating-point arithmetic and need not reproduce the
old recurrence bitwise. Cancellation-small or nonfinite prepared curvature falls
back to the observation operator and its squared-norm curvature. No speculative
product or recurrence residual certifies a result. Reconstruction uses the original
RHS and current local bases, freshly checks all partitions, and then receives the
full independent current-weight residual certificate at the unchanged tolerance.

No residual, score, fitted state, rank decision or acceptance result is cached.
The finite solve, model-space, rank, objective, two-consecutive-convergence,
reconstruction, final-Hessian, inference and transactional publication conditions
remain at their existing boundaries. Weight/sample/DoF/separation policy and
public solver ownership do not change. Prior response/control residuals remain
initial guesses under the current weights, with fresh acceptance.

## Memory and scope

Admit the numerical solver first, its optional ordered certificate bank second,
and the prepared operator last. Budget/allocation refusal cannot displace those
already admitted mechanisms. Operator admission covers descriptors, worst-case
pair adjacency, observation maps, local-row maps, numeric blocks, construction
marks/touched lists, and eligible dense Schur/sparse-row scratch. Capacities are
fallible and fixed; inner work does not grow them. The reservation is conservative
when actual edge counts or the selected compact representation need less storage.
Existing telemetry retains that conservative reservation through strict solving.

The representation is internal to publication PPML's multipartition slope MAP
owner. Categorical and one-effect PPML, mu separation, explicit non-MAP families,
and public OLS routing remain excluded. The compact base vectors also benefit
admitted slope solves whose optional operator is refused. A public OLS adapter
requires its own evidence; this change does not silently route OLS here.

Fresh local public gains range 2.4–32.3% against ADR 0124, at 0.19–4.30 MB extra
tracked peak on the captured cases. Serial two-FE and four-FE cells clear the
0.90-times-stock target; ordinary threaded and low-memory losses remain. The
[source-bound report](../../benchmarks/reports/development/ppml-symbolic-coupling-local-2026-09-23/README.md)
records distributions, independent oracles, source/artifact identities, controls,
reference accuracy failures and diagnostic attribution. This is not qualification
or closure of the broader PPML performance objective, static allocation drift,
constrained-fit/CG failures, or deferred platform checks.
