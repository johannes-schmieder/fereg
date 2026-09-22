# ADR 0106: single-RHS PPML Schur checkpoints

**Status:** Accepted for local development
**Date:** 2026-09-22

Extend ADR 0067 to one actual RHS on an already admitted two-FE categorical
checkpoint-LSMR session with a retained Schur workspace and at least 16,384
observations. This targets fresh residual projection. The existing 16–64 RHS
route remains; 2–15 RHSs remain excluded. One-/three-FE, slope and owners without
Schur storage cannot enter the new route. Ordinary PPML engine selection and
public OLS routing are unchanged.

Generalize the reusable probe to the number of available columns, capped at
three. Wider callers retain the same selected columns and arithmetic. A
single-RHS call probes that RHS under freshly validated current weights for
the existing eight iterations. Contraction at most 0.01 permits the existing
bounded Schur attempt; this probe never supplies a scientific certificate.
Group weights and numerical state are rebuilt, not reused across generations.

The existing fresh initial FE certificate is still attempted first. An accepted
Schur candidate must pass the unchanged canonical residual certificate before
all downstream rank, score, reconstruction and final-Hessian checks. The
authoritative input remains untouched. Probe rejection or a finite uncertified
attempt retains the original LSMR retry and MAP fallback. Allocation,
interruption, invalid-data and internal errors remain fatal. Attempt costs and
retry evidence remain visible.

One N-element transient candidate substitutes for the admitted LSMR banks.
The existing explicit copy allowance is checked before allocation; no persistent
buffer, cross-frame geometry cache or memory-policy change is added. Existing
exact/one-byte-short copy checks remain; modeled allowance checks now include
one RHS.

The [source-bound report](../../benchmarks/reports/development/ppml-single-residual-local-2026-09-22/README.md)
records about 10% and 17% paired gains on screen/fresh degree-six two-FE PPML,
with a smaller weighted-cluster gain. Dense, weak, serial and excluded controls
are mostly flat; the sparse screen has a small measured loss. Keep those losses
visible and close this bounded experiment without tuning its guard to seeds.

All 93 headline attempts and 39 native scientific comparisons pass; 36 pairs
have identical scientific exports. The other three differ only at roughly
1e−16 in coefficients and 8e−21 in covariance. Final validation includes 378
distinct Rust tests, ABI containment and five licensed fixtures. A new explicit
LSMR fixture records six single-RHS Schur acceptances under unit/FW/PW and
T1/T4, checked against dummy Poisson and directly formed Hessian sandwiches.
Tracked peak differences range from −5.14 to +0.21 MB; neither exact peak
neutrality nor constrained-memory qualification is claimed. Existing allocation
inventory drift and other deferred scientific/platform issues remain open.
