# Existing PPML weighted Schur integration

Source identity: evidence HEAD 09f0f2f; accepted native 7739e8e; 178 inputs in
baseline-identities.json; isolated plugin 2689faf0. Main/dist are not built.

ReusableCategoricalMapAbsorber in fe/borrowed_map.rs already owns
TwoFePcgWorkspace for two FE, N>=128 and maximum RHS<=64. After the fixed MAP
pilot, it calls absorb_two_fe_pcg_reusing. OLS absorb_ols_schur_at_cap calls
that same recurrence. No second mathematical Schur implementation is needed.

The current categorical projector validates current weights, retains raw input
reference norms and performs an initial certificate. Eligible checkpoints enter
LSMR before the MAP pilot. Successful initial/warm certificates return directly.
The PPML MAP accelerator has a reserved MAP continuation budget and retains its
workspace. OLS uses a separate eight-step current-weight three-column probe,
a capped 64-step Schur attempt and separately admitted direct-CG continuation.
These different entry/fallback policies cannot be interchanged by renaming them.

In ppml/prepared_irls.rs, publication iteration one establishes the canonical
basis. Automatic two/three-FE fast ordinary iterations use the retained joint
LSMR solver; explicit MAP uses canonical frozen solves. Explicit CMG uses
ReusableCategoricalFwlCmg and current-weight graph projection, with unchanged
canonical initial/strict/final checkpoints. Shared MAP-only acceleration does
not automatically speed the ordinary auto path. Measure all complete routes.

The existing PPML plan already charges the two-FE level/RHS workspace, but a new
three-column probe alongside it or a new fallback overlap must be charged before
construction. Prefer reusing existing workspace if an early-Schur prototype is
warranted. Preserve current-weight validation and independent observation-space
certificates, reference scales, complete retained design, strict basis rollback,
specialized-state invalidation and transactional publication.
