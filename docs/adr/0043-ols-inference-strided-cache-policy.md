# ADR 0043: earlier retained score ownership for OLS inference

Status: accepted for local development, 2026-09-12.

## Decision

Ordinary fast OLS robust and clustered inference uses the existing packed score
cache only up to 32 MiB of candidate score values. Above that limit, eligible
guarded-Cholesky fits retain their standardized solve allocation and expose the
existing strided score view. The shared 512 MiB bound for one-shot weighted-linear
callers and explicit cache-limit overrides are unchanged. PPML's session policy,
IID fitting and low-memory execution are unchanged.

The selected strided route already computes stable raw reference moments and
uses them for residual construction. The decision adopts that complete route,
not only its avoided copy. It introduces no second score representation or
numerical implementation. The 32 MiB cutoff is a measured development crossover,
not a rank threshold or a universal optimum.

## Contracts and fallback lifetime

The existing current-frame weights, raw reference norms, stable scaled moments,
generated-column identities and independent score/covariance certificates remain
authoritative. AW/PW squared scores, FW multiplicity, multiway inclusion-exclusion,
nesting, DoF corrections and PSD policy are unchanged. No raw large-cross-product
subtraction or stale weighted statistic is introduced.

Householder QR still owns uncertain or deficient rank decisions. If its mutated
solve matrix cannot be retained, the existing score builder reconstructs and
reabsorbs the needed columns. A packed cache with mismatched kept-column identities
also remains unusable; this decision does not eliminate those fallback costs.
Small packed and scratch-backed paths remain available under their existing rules.

The memory planner is unchanged and remains conservative for the selected route.
No new memory admission or solver-family region is opened. Native allocation,
interruption, receipt-last publication and main-thread SPI boundaries remain.

## Evidence

The [report](../../benchmarks/reports/development/ols-inference-ownership-local-2026-09-12/README.md)
binds baseline `f4cebe7`, the native patch, source hashes and isolated artifacts.
The comment-reviewed source rebuild is byte-identical to the measured prototype.
Twenty-nine paired comparisons cover complete commands, phase costs, native/model
memory, RSS, graph controls and QR reconstruction.

Three-seed paired command gains are 38.8% for wide robust sparse-missing OLS,
31.4% for PW robust and 24.1% for FW clustering; native peaks fall about 25–26%.
The gains include fused reference/residual passes. Strided score certification is
slower than its packed counterpart, while covariance arithmetic costs are similar.
QR profiles improve 4.0% with score reconstruction retained in both arms.
Newly eligible weak, no-FE, 65-column and interacted two-way-cluster cases pass.
No protected control exceeds the declared max(10 ms, 5%) review envelope.

Samples/status/DoF and solver decisions agree exactly; full b/V differ only at
roundoff scale and pass unchanged certificates and registered tolerances.
Independent explicit-dummy covariance checks cover unit/AW/FW/PW, robust/clustered
inference and full/deficient rank. Focused Rust allocation/science tests and
native arm64 ABI audits pass. This is local development adoption, not release or
cross-platform qualification. Remaining cluster-state, tiling, skew and low-memory
work under roadmap #27 stays separate.
