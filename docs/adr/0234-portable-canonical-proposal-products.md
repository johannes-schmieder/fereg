# ADR 0234: portable canonical proposal products

Status: candidate, 2026-09-28.

## Decision being tested

Keep the two-categorical-FE canonical Schur builder and iterative solver of
ADR 0193. On portable targets without compile-time FMA, use inline multiply
and add in repeated dense matrix-vector applications. Preserve the existing
eight independent accumulation lanes and ordered merge. ARM and targets with
compile-time FMA retain their existing product. No routing threshold, bank,
iteration diagnostic, tolerance or solver family changes.

The [Linux public-command profile](../../benchmarks/reports/development/thread-ceiling-candidate-scc-2026-09-28/control04/README.md)
places 5.90% of sampled self cycles in `normalized_product` and 10.14% at the
out-of-line hardware FMA entry/return on the remaining small PPML loss. The
helper serves both Gram construction and canonical proposal products; incomplete
unwinding prevents attribution between them. This candidate changes only the
latter. It tests call overhead rather than assuming the profile predicts a
wall-time gain. The accepted portable coupling optimization in ADR 0232 supplies
related Linux evidence; it does not prove this candidate helps.

This does not reopen the incidence representation or thresholds rejected in
ADR 0200. Dense geometry, ownership, component/RHS stopping and the recurrence
remain in place. The materially new evidence is portable Linux instruction-call
cost; the earlier incidence experiment changed storage and operator work on Mac.

## Scientific and ownership boundary

The matrix application proposes coefficients only. Current-weight geometry,
Gram products, factorization, curvature diagnostics and original-observation
residual reconstruction retain their arithmetic. Fresh original-row FE moments,
direct finite Gram/RHS/norms, finite rank and status, score, objective, strict
checkpoint and exact final-Hessian acceptance remain mandatory. Refusal retains
the existing certified fallback. No authority crosses a weight frame.

Only the admitted dense iterative two-FE canonical PPML route changes. Affordable
Cholesky, implicit incidence, three-FE canonical products, ordinary OLS and
sloped engines remain controls. Unfused proposals need not reproduce old bits;
independent projection and Poisson/Hessian/sandwich oracles govern acceptance.
No new allocation, descriptor, persistent state, memory credit or public option
is introduced. Main-thread SPI and transactional publication are unchanged.

## Required bounded evidence

Run the canonical independent weighted-contrast tests, including changing and
extreme weights, disconnected designs, nonzero residuals, rank, memory refusal
and iteration caps, on the changed portable Linux target. Add null, negative and
nonfinite curvature refusal coverage. Run the licensed iterative canonical and
categorical final Poisson/Hessian oracles, plus slope and OLS controls. ARM tests
protect the unchanged path but do not validate the portable arithmetic change.

Use source-bound Linux candidate/baseline/pinned-stock rotations on the profiled
small case, low/serial and width controls, followed by predeclared fresh cases
if gains justify adoption. Keep failed comparisons NR; do not relax stopping or
certificate bounds. Reject if savings are small/inconsistent relative to
complexity or exclusions regress. This ADR makes no acceptance or speed claim.

## Science checkpoint

Snapshot `4aa44429…` passes 18 canonical Rust tests on both Mac and Linux.
Linux job 7763043 also passes PPML smoke and four licensed fixtures: iterative
canonical and final categorical Poisson/Hessian/sandwich oracles, slope PPML and
multipartition OLS controls. Mac universal build, ABI/panic containment and the
same four licensed fixtures pass. Linux exercises the changed portable branch;
ARM retains fused arithmetic. Receipts are in the
[candidate report](../../benchmarks/reports/development/canonical-portable-scc-2026-09-28/).
The 63-fit isolated SCC screen is registered and queued. Performance acceptance
remains pending; this checkpoint does not adopt or qualify the candidate.
