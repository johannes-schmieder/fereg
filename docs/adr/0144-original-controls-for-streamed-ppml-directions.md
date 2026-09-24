# ADR 0144: original controls for streamed joint PPML directions

Status: accepted for local development, 2026-09-23.

## Decision

The admitted low-memory joint finite/FE engine borrows original prepared finite
columns during ordinary directions and the strict joint Newton witness. Its
stored-column identity maps those columns to the existing frozen basis. Numeric
columns use the existing bounded binding; generated columns use the prepared
recipe. There is no new N-by-K matrix, heap bank, topology, solver or route guard.
Remove the superseded scratch-row adapter for ordinary joint products.

This refines the representative choice in ADRs 0130, 0134 and 0136. An original
control and its retained residualized representative differ by an FE term and
define the same joint model. The response keeps its existing FE-equivalent
representative and original-response anchor. No current within-design authority
is inferred from either representative. Canonical construction and inference
still use the retained within design, including its scratch storage.

Streamed Hessian tiles normalize each raw finite row once and reuse it across
their projected columns. The existing beta bank is free after norm construction
and before the solve installs coefficients; reuse it as the row scratch. Each
normalization and Hessian accumulation retains its prior arithmetic/order.
The session's independent original-input finite-score/mean scan uses the same
numeric binding as outer PPML scores, including narrow numeric designs. Fast
slope directions also benefit from that binding. Fast categorical, one-FE and
one-shot OLS routes do not change.

## Scientific authority

Current-weight group QR, coupling, finite scales, products and preconditioner
are still reconstructed or reused under their existing explicit generation
rules. No new numeric state crosses weight frames. Fresh FE and original-input
finite scores, reconstruction, objective acceptance, two outer convergence
passes, canonical rank/inverse and exact final-Hessian acceptance remain.
The ordinary joint result still carries no bread or inference authority.
Samples, weight semantics, DoF, separation, tolerances, iteration caps, Stata/Rust
and main-thread SPI boundaries, and transactional publication are unchanged.

Using originals can increase Krylov work: accepted cases add two to eight
reported inner iterations while reducing data movement. These are not equivalent
recurrence bits. Existing numerical refusal and canonical retry remain required;
no threshold is fitted to a benchmark. Independent weighted WLS and explicit
Poisson/Hessian/sandwich oracles, rather than old-recurrence identity, establish
accuracy. Original-input controls are independently rescanned for acceptance.

## Memory and evidence

The new view borrows immutable prepared inputs, stored-column indices and a
fixed stack binding. Row/Hessian scratch reuses already admitted banks. All
sampled PPML modeled peaks and retained scratch sizes are unchanged. Tracked
peaks are unchanged except for a 32-byte decrease in one small final pilot.
Categorical and slope actual-plugin audits at pool/T=1/1 and 2/4 retain zero
allocator events, zero current growth, stable capacities and fresh/warm agreement.
This is not physical constrained-memory or cross-platform qualification.

The [source-bound report](../../benchmarks/reports/development/ppml-joint-original-controls-local-2026-09-23/README.md)
records both candidate organizations, diagnostics, 155 final-source Rust tests,
nine passing final licensed runs, independent oracles, all public distributions,
control losses and failures. All 28 fresh/weak native pairs improve; five fresh
medians improve 2.7–4.4%. Narrow two-FE confirmation improves 130 to 118 ms,
still versus stock 58 ms. This closes the bounded data-access experiment, not
the broad whole-command 0.90-times-stock objective.

A new generated-term boundary with a 1e6 FE component fails to converge on both
the unchanged baseline and candidate. The 0/1000 controls pass. The failure is
preserved and remains unresolved. A stock one-FE fitted-mean discrepancy is
independently localized to its group intercepts and remains unranked. Neither
that discrepancy nor older allocation-inventory/constrained-fit failures are
repaired by this decision.
