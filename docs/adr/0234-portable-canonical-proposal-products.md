# ADR 0234: portable canonical proposal products

Status: accepted for bounded development, 2026-09-28. Not qualification.

## Decision

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
complexity or exclusions regress. Acceptance requires source-bound evidence; qualification remains separate.

## Science checkpoint

Snapshot `4aa44429…` passes 18 canonical Rust tests on both Mac and Linux.
Linux job 7763043 also passes PPML smoke and four licensed fixtures: iterative
canonical and final categorical Poisson/Hessian/sandwich oracles, slope PPML and
multipartition OLS controls. Mac universal build, ABI/panic containment and the
same four licensed fixtures pass. Linux exercises the changed portable branch;
ARM retains fused arithmetic. Receipts are in the
[candidate report](../../benchmarks/reports/development/canonical-portable-scc-2026-09-28/).
The [initial screen](../../benchmarks/reports/development/canonical-portable-scc-2026-09-28/initial02/README.md)
and [fresh confirmation](../../benchmarks/reports/development/canonical-portable-scc-2026-09-28/fresh03/README.md)
complete 174 headline applications, 58 native science passes, 55 stock passes and
three initial default-stock slope NR comparisons. All jobs have clean accounting
and terminal sentinels. Strict slope comparisons in confirmation pass; they do
not overwrite the earlier default discrepancy.

Fresh low/wide/sparse C/B paired medians are 0.791/0.757/0.839. Small fast/serial
are 0.943/0.912 and larger 1.010. Stock ratios remain2.015/2.130/1.607 for small
fast/serial/low; the 1.20 ceiling and 0.90 aspiration are not generally achieved.
The unchanged fresh OLS control is 1.021; fresh slopes 0.970. Nine pairs on the
initially losing slope input span 0.804–1.255 with median 1.018, five losses and
marginal medians494→541ms (9.5% loss). This does not prove regression freedom.
The initial uniform8% slope loss is not consistently reproduced, and the
candidate changes no slope algorithm/arithmetic. Accept the small implementation
for repeatable broader gains with these controls visible; avoid another
control-specific tuning cycle. No numerical storage or admission change occurs.

The separate [DWARF profile](../../benchmarks/reports/development/canonical-portable-scc-2026-09-28/cpu04/README.md)
resolves baseline canonical build → normalized product → libm FMA callchains.
Displayed FMA self share falls 9.39%→2.15%, consistent with the mechanism but not
a wall-time estimate. Ordinary categorical joint LSMR and Schwarz reconstruction
remain measured callers for the next bounded investigation.
