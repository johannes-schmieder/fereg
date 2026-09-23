# ADR 0131: one accepted-weight frame for PPML slope finalization

Status: accepted for local development, 2026-09-23.

## Decision

After two independently accepted objective/score convergence passes, eligible
fast multipartition slope PPML may use a strict joint Newton witness followed by
one canonical final construction. Use ADR 0130's existing joint finite/FE owner;
do not introduce another solver, tolerance, workspace bank or public option.
Pending profile rebuild or checkpoint refinement retains the canonical path.
Low memory, categorical, one-FE and other solver families do not use this shortcut.

Refresh the working response and weights from the accepted predictor. Solve the
joint normal equations at the existing strict checkpoint tolerance, including
fresh FE projection, original-input finite score and reconstruction certificates.
Require its coefficients, predictor and evaluated objective to agree with the
accepted state under the unchanged strict publication comparison. This witness
does not supply a canonical rank decision or an inverse.

Keep the accepted predictor and coefficients fixed. Set the final response to
`eta - offset` and require exactly the same weight frame. Perform one canonical
strict transaction for this response, independently rebuilding the within design,
canonical rank, finite solve, bread and associated certificates. Require the
original basis/rank, strict objective/predictor/coefficient agreement and existing
final-Hessian synchronization bounds before committing. Publication still checks
the exact `q * mu` Hessian frame and constructs inference from canonical products.

This supersedes ADR 0130's two full canonical constructions at strict/final
acceptance on this route. ADR 0016's strict current-weight Newton check and final
rank/Hessian duties remain; they are discharged by distinct suitable solves.
The strict joint result cannot enter the inference interface. Unlike ADR 0128's
rejected response-history probes, this change removes one full canonical finite
construction and adds no response-history cache or threshold tuning.

## Independent acceptance and fallback

The outer Poisson score and objective already certify the accepted state. The
shortcut changes neither that state nor its sample, weights, prior weights or
offset, so it does not invalidate those checks. Both speculative predictors get
fresh objective evaluations. The canonical final solve independently establishes
the model-space, rank, projection, finite solve, inverse and reconstruction
conditions; a previously certified ordinary direction cannot substitute for it.

A numerical refusal of the strict joint direction, a moved predictor/coefficient/
objective, or a declined final comparison returns to the existing canonical
checkpoint/resume policy. A rejected canonical transaction rolls back. Fatal
domain, allocation and other errors retain their existing behavior. The shortcut
never publishes a speculative state. Existing canonical rank-loss rules, bounded
rebuild/refinement and failure atomicity remain authoritative.

The session may relabel cached FE geometry only after comparing every owned raw
weight bit and every bread-scale field. Sample and topology generations must
also match, and the geometry must still hold authority for the prior generation.
An error-revoked cache cannot be revived. Changed weights still rebuild numerical
geometry; immutable topology alone never authorizes weight-dependent factors.
This exact-identity transfer also permits other same-weight slope refreshes to
retain valid geometry, without extending the strict shortcut's routing.

## Memory and execution

Reuse the admitted joint owner, candidate predictor/coefficient buffers and
canonical transaction. No additional heap payload or retained numeric state is
introduced. Median tracked and modeled peaks are unchanged in the measured
development, fresh and excluded cells. The actual-plugin warmed allocation
audit remains zero-event/zero-growth at T1/T4. These are local development checks,
not physical-cap qualification or a repair of the static inventory drift.

All computation stays in safe Rust on Rust-owned inputs. Main-thread SPI and
interruption boundaries, the single guarded plugin call, and transactional Stata
publication are unchanged. Inner diagnostics include the strict joint Krylov
steps and canonical per-column counts; raw iteration totals are not comparable
units of work across the old and new finalization procedures.

## Evidence and remaining work

The [source-bound report](../../benchmarks/reports/development/ppml-single-frame-finalization-local-2026-09-23/README.md)
records the exact candidate, 53 distinct Rust checks, five licensed fixtures,
independent Poisson/Hessian pilots, public rotations and fresh confirmation.
Boundary tests cover analytic truth, exact final weights, disabled shortcut,
forced predictor movement, forced basis change, changed generation labels,
sample mismatch and revoked geometry authority.

Fresh native gains are 3.5–7.2%; excluded timing losses receive fixed-input
follow-up. One fresh stock fitted-mean comparison exceeds its unchanged envelope
and remains unranked. Retain this bounded improvement without claiming the
0.90-times-stock objective is complete. Initial/ordinary work and the remaining
canonical final construction still dominate important losses. Historical
comparator discrepancies, constrained-fit/CG failures and platform qualification
remain unresolved.
