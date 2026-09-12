# ADR 0039: One-FE PPML retained categorical projection

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-12

Issue #27 P4 identifies one categorical FE as an accidental exclusion from
PPML's reusable categorical MAP implementation. Fast one-FE sessions used the
generic projector, while the existing categorical implementation also enabled
fused projection of a changing response and retained regressors.

Admit one categorical intercept effect through that existing PPML-only
constructor. Certify immediately after its first group projection and retain the
initial current-weight certificate that can accept an already-within candidate
without a sweep. One FE allocates no two-FE PCG state and no observation-sized
accelerator matrices. Existing group/RHS scheduling runs inside the requested
native team. Workers never call SPI.

This extends the existing retained response fusion: the old projected response
plus the change in raw response differs from the new raw response only by an FE
component. Projection under the current Hessian weights removes that component.
The same admitted dense allocation expands/compacts in place; raw reference
norms/means, within norms, weighted products and independent certificates still
refresh under the existing generation rules. No fresh-input certificate or norm
pass is skipped by this decision.

ADR 0020's original reference scales, guarded finite Cholesky/QR and rank policy,
strict RAII transaction, final-current-weight synchronization and memory admission
remain authoritative. The fast memory model already charges the reusable group
workspace. Low memory and slope-bearing designs retain their generic paths.
Public MAP/LSMR/CMG selection and ADR 0029's two/three-FE LSMR region are unchanged.
The separate predictor reconstruction policy is unchanged.

The [source-bound local report](../../benchmarks/reports/development/ppml-one-fe-local-2026-09-12/README.md)
records independent weighted group-mean and full-dummy Poisson/sandwich oracles,
paired complete-command gains of 15.7–16.3%, protected controls, phase costs and
memory. Primary native peaks rise 3.4–3.5% within unchanged bounds. All measured
outer/reported-inner counts, step halvings, retries and fallback decisions remain
unchanged. Actual MAP iteration totals fall 5→3 and 6→4 on the primary cases as
warm candidates certify without projection. This is a local development decision,
not cross-platform qualification.

The private audit reports stable retained capacities and fresh/warm equivalence
but also 16 allocator events in 16 iterations in both arms, without live/peak
growth. Its whole-iteration allocation-free certificate remains false. This
existing gap is explicitly retained; neither the memory bounds nor scientific
certificates are substituted for that operational certificate.
