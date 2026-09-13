# Initial recovery admission findings (not a bound or acceptance)

Current source after reapplying the two-file prototype:
- estimate.rs::build_derived_observation_outputs_in_team keeps the prepared
  data and CertifiedPreparedOlsFit borrowed while allocating target, mask,
  recovered channels and optional xb/xbd/absorbed output.
- CertifiedPreparedOlsFit retains fast_score_cache and optional recovered
  coefficients alongside residuals and bread. Account for those actual
  lifetimes before adding recovery workspace; do not assume fitting freed them.
- recover_absorbed_effects_fast_with_weights_inner tries retained coefficients,
  then capped borrowed CG for multiple intercepts, SD-LSMR, LSMR, then MAP.
  Ordinary OLS propagates neither allocation failures nor a recovery budget;
  the PPML variant has separate allocation/fallback policy. Preserve that
  separate reconstruction work and do not import its policy silently.
- LsmrConfig carries tolerance, iterations, local_size, preconditioner and team,
  but no memory budget. Initial AbsorbConfig's solver_memory_budget therefore
  does not by itself bound this final recovery chain.
- Prototype permission is enabled in the OLS plugin; it currently has no final
  recovery admission proof. The existing Schur/CG initial guard is insufficient.

Next enumerate each retained/CG/SD-LSMR/LSMR/MAP recovery allocation and output
normalization overlap, reuse existing byte calculators where available, and
attach a conservative checked admission to this boundary. Test one byte below/
at required capacity and fail-closed rollback; keep full fallback costs in the
final report. Public model gaps shown by saved-unit/PW and original rejected AW
remain separate evidence, not repaired by enabling the prototype.
