# ADR 0134: bounded joint PPML slope directions in low memory

Status: accepted for local development, 2026-09-23.

## Decision

Extend the admitted joint finite/FE Schur owner of ADR 0130 to retained scratch
storage. Use the same block-preconditioned conjugate-gradient recurrence,
coefficient coordinates, current-weight QR/coupling, numerical refusal and
independent acceptance. Supersede its fast-only storage restriction. No new
solver family, rank policy, tolerance, sample rule or public option is introduced.
One-effect, categorical, non-MAP, mu-separation and public OLS routing remain
outside this extension.

The low owner keeps its pool-bounded RHS capacity. Admit the joint finite block
by total generated width independently of that capacity. Borrow the retained
FE-equivalent finite representatives through a fallible row visitor; each scan
owns an immutable positional reader. Safe borrowing excludes mutation or scratch
replacement during a scan. Read failures propagate through the existing
authority-revocation and transactional boundaries.

For V = sqrt(W) X and P0 = I - Q0 Q0', prepare stable current-weight column norms
in one complete-row scan. Reuse the already admitted worker observation buffers
as a bounded tile of normalized columns. Project that tile through P0, form
the remaining FE cross-products, and stream complete rows once more for its
finite products. Compute H_ij = (P0 V_i)' V_j, with the existing column scaling;
this equals (P0 V_i)'(P0 V_j) by orthogonal projection. Symmetrize the computed H
before its Cholesky preconditioner. Do not subtract two large Gram matrices.
The factor bank supplies temporary row scratch, and the eventual beta bank
temporarily holds stable-norm accumulators. Neither value has solve authority
during preparation.

With K active controls and W admitted worker buffers, preparation requires
1 + 2*ceil(K/W) complete-row scans. This is a code-derived count, excluding
reconstruction and independent certificates. The rejected per-column adapter
required 3*K scans; its in-memory kernel gains did not survive public scratch
execution. There is no new N-by-K bank, projected-column cache or concurrent
mutable file cursor. Fast storage retains its existing projected-column bank.
Both preparations call one shared recurrence; there is no second low solver.

## Verification and finalization

Every direction reconstructs from current coefficients, current weighted FE
bases and the original response/control representatives. Fresh FE projection,
finite moments, original-input finite scores and combined predictor checks remain
mandatory. Recurrence diagnostics only schedule acceptance. Changed weights
rebuild numerical geometry under the existing sample/topology/weight authority;
no prior solution or representative supplies acceptance for a new frame.

The admitted low owner may also supply ADR 0131's strict Newton witness. After
the unchanged two-pass outer convergence, require strict current-weight joint
acceptance and agreement with the accepted predictor, coefficients and objective.
Then perform one independent canonical final construction for rank, within
design, finite solve, inverse/bread and final-Hessian synchronization. Keep the
accepted state fixed and check the exact weight frame. Refusal, movement or
rank disagreement retains canonical checkpoint/retry; a rejected transaction
rolls back. Joint results still carry no inference or inverse authority.

This supersedes ADR 0131's low-mode exclusion because the necessary joint owner
now has bounded scratch access. It does not remove the scientific duties of the
strict witness or the independent canonical final solve. Tests cover both memory
modes, changing weights, rollback, forced predictor movement, forced basis change,
rank-deficient FE channels, scaled/permuted controls, nonfinite inputs and revoked
geometry. Independent explicit-design WLS and Poisson/Hessian oracles complement
reference agreement.

## Memory and runtime audit boundary

Charge ADR 0130's existing optional payload against the complete linear-session
envelope before allocation. Refusal cannot evict an affordable existing solver.
The RHS pool and worker buffers retain their existing bounds; scan cursors add
one 8 KiB stack page with no heap allocation. Release joint storage before
inference. Measured tracked and modeled increments agree, including K greater
than the default pool width. This is not physical-cap qualification and does not
repair the static allocation inventory drift.

The production-plugin 16-iteration audit passes at pool widths 1/2 and T1/T4
when each configuration runs in a fresh process: exactly zero allocation events,
zero current growth, stable capacities and warm/cold equivalence. Preserve the
stricter mixed-thread-sequence failure separately. After retiring a T4 pool,
the final candidate releases 4,376 bytes during a later T1 audit, with zero
allocation events and stable numerical capacities. An isolated intrusive build
attributes the two frees (2,072 and 2,304 bytes) to Crossbeam epoch reclamation
of work queues. The baseline passed this sequence. This extension does not
repair that runtime-accounting limitation or claim the mixed sequence passes.
The audit's exact-zero rule is unchanged; the new focused fixture selects one
configuration per process. No arbitrary sleep, extra warmup or relaxed threshold
is introduced. The instrumented build is neither allocation nor timing evidence.

## Evidence and remaining costs

The [source-bound report](../../benchmarks/reports/development/ppml-low-joint-local-2026-09-23/README.md)
preserves frozen prototypes, rejected public adapters, final source identities,
all distributions, scientific checks, failures and excluded-path follow-up.
Fresh low-memory gains are 4.2–15.9%, with every native pair improving. Three
fresh cells clear 0.90 times comparable stock; two still lose. An excluded low
one-FE timing loss ties on fixed-input follow-up, while its original loss remains
part of the evidence. No universal performance non-regression claim is made.

The remaining public deficit is not a proof that certification is intrinsically
too expensive. Canonical projection, scratch inference and transport remain
substantial. Measure these separately; carry forward the plan for categorical
joint directions and whole-command one-FE work. Do not continue tuning this
adapter's tile width against named benchmark cases. Scientific discrepancies,
constrained-fit/CG failures and deferred platforms retain their prior scope.
