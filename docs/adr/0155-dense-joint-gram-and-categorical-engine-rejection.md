# ADR 0155: dense joint Gram products and rejected categorical engine replacement

Status: accepted for local development, 2026-09-24.

## Decision

Use eight fixed FMA accumulation chains for each upper-triangle inner product
of the already normalized, projected finite columns in the dense joint PPML
engine. Copy that triangle as before. This supersedes the retained single-chain
dense dot in ADR 0148; ADR 0151's streamed products remain unchanged. Scheduling
cannot change an entry's reduction tree. The Gram is a current-call preconditioner,
not rank, inverse, covariance or acceptance authority.

Keep the existing ordinary slope solver, recurrence, RHS products, geometry
lifetime, reconstruction and independent certificates. All original-input finite
scores, outer score/objective, strict rank/bread, final-Hessian synchronization,
sample, weights, DoF, separation and transactional publication requirements remain.
No tolerance or iteration budget changes. Cross-version bitwise equality is not
required; independent numerical envelopes and fixed-configuration reproducibility
remain authoritative.

There is no new heap allocation, cache, thread team, admission formula or route.
The helper has eight f64 accumulators on the stack. Every measured final PPML pair
has identical tracked peak, modeled peak, scratch and iteration metadata. This
is not a physical-cap or cross-platform qualification claim.

Extend the existing disconnected, extreme-weight Poisson truth test to fast
storage as well as low storage. The original 1e-10 predictor envelope, iteration
limit and strict-rejection assertion remain. The accepted route passes both.

## Rejected categorical replacement

Frozen identical-frame experiments establish that the existing coefficient
joint solver can outperform categorical joint LSMR. The dense Gram reduction
removes its wide serial preparation loss. That is insufficient for adoption:
whole-command integration incurs generic-owner setup, independent original-control
checks and different canonical/response-state work.

Reject automatic two-FE MAP routing and the attempted explicit fast categorical
joint extension. Public development cases show reliable medium-width serial
losses despite wide and narrow gains. More decisively, extending the independent
extreme-weight test exposes a fast-mode truth failure. The committed baseline
passes. Keeping a separate canonical projector, enabling component-balanced
checkpoints, and the original-control/norm-sharing successor do not repair it;
the last returns relative predictor error 1.2343253e-9 against 1e-10.

Do not retain the new routing, dual-owner admission extension, publication-envelope
change, generic dense input adapter or shared finite-norm authority. Preserve their
source-bound diagnostic evidence. The earlier route-report rejection also remains:
two licensed prototype fits failed atomically with r(3499), before the prototype
validator was corrected. No rejected source is an adopted performance claim.

Automatic/explicit categorical LSMR, existing MAP routes, low memory, one-FE
profiling and OLS retain their policies. In particular, this decision does not
reopen the all-coefficient canonical route rejected in ADR 0136.

## Evidence and next design

The [report](../../benchmarks/reports/development/ppml-joint-engine-screen-local-2026-09-24/README.md)
binds the adopted source, 66 Rust tests, four licensed fixtures, 132 public
applications, independent weighted Poisson/Hessian and WLS oracles, paired stock
measurements, controls and failed experiments. Fresh wide slope medians improve
12.8% and 10.9%; narrower gains are 1.4% and 3.3%. A narrow fresh case still loses
to stock. An unchanged OLS control has a +1-ms median and all three pairs slower;
no universal non-regression claim is made.

The broad <=0.90-times-stock objective remains unfinished. The next coherent
candidate is a coefficient-space Newton operator behind the existing categorical
incremental-direction boundary, retaining canonical response-history ownership.
First localize the extreme-component failure at initial, ordinary and strict/final
states; do not assume the recurrence itself is responsible. Frozen solve gains
justify that investigation, not another threshold change or blanket guard removal.
