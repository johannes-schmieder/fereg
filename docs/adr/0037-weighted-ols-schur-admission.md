# ADR 0037: weighted-operator admission for wide OLS Schur absorption

**Status:** Accepted for local development
**Date:** 2026-09-12
**Decision owner:** Johannes Schmieder

## Context

Roadmap #27 requests broader weighted OLS absorption while preserving difficult
graphs. ADR 0035 admits a wide unit-weight route around an existing weighted
Schur engine. Its three-RHS probe and the OLS wrapper, rather than that engine,
exclude supplied weights. Simple one-FE OLS already reaches direct weighted
projection and carries its certified residual norms into finite solving.

## Decision

Extend ADR 0035's OLS admission to validated supplied weights. The probe uses
the same maximum-normalized positive weights as the full engine for group
degrees, weighted RHS statistics, elimination and every Schur application:

`S = R' W R - R' W L (L' W L)^(-1) L' W R`.

The unweighted topology alone cannot admit a weighted problem. An eight-step
probe on the outcome, first and middle RHS must still contract by the existing
0.01 threshold. A graph with tiny-weight long-range edges may fail that probe
despite strong unweighted connectivity. Rejection selects the existing legacy
route and charges the probe to setup and the shared iteration budget.

All other restrictions remain: automatic fast OLS, two intercept-only effects,
at least 16,384 rows, 16–64 actual RHS columns, no saved-FE coefficient retention,
certified continuation enabled, more than 72 allowed iterations, and joint
memory admission for both Schur and continuation. An explicit constant counts
toward RHS width. Explicit solver choices, low memory, slopes and PPML retain
their existing behavior.

The full solve reuses the existing weighted engine. The 64-step cap, release of
Schur workspace before symmetric-CG continuation, original raw-RHS certificate
scales, internal tightening and independent final FE/OLS/inference certificates
are unchanged. No new observation-sized routing allocation is introduced.
The existing workspace bound also covers supplied weights because the weight
slice is borrowed; group weights were already allocated for the unit route.

Weights remain AW/FW/PW throughout preparation and inference. Common-scale
normalization never changes raw weight metadata, effective N, singleton
multiplicity, reference scales, DoF or covariance rules. Constant supplied
weights are eligible without being relabeled as unweighted observations.
Further constant-weight kernel specialization is not part of this decision.

## Evidence and limits

[Local source-bound evidence](../../benchmarks/reports/development/weighted-schur-local-2026-09-12/README.md)
compares baseline `39d1a2a` with isolated universal candidate artifacts. Weighted
dummy, literal frequency-expansion, common-rescaling, rank, forced-continuation,
memory-budget, thread and transactional checks pass. Unit-weight difficult
graphs retain exact results and solver decisions. Tiny weighted cuts reject
Schur and retain exact legacy scientific results.

The three primary paired command medians improve 53.0–80.8%. The large AW
target spends 12.4% more native peak memory within its admitted budget; FW/PW
targets use less. Weighted-cut rechecks cost a median 3.8% in paired ratios;
the separate trace attributes 2.7 ms to the probe, with additional variation
in the legacy absorption work. This bounded development sample does not prove
universal routing regret, cover every weighted graph, or qualify a release.
No weighted Schur region is added to PPML.
