# Source overlap audit for the saved intercept route

This audit binds to the five-file candidate manifest and accepted 834703b. N is
the final row count, L the sum of final FE levels, D the effect count, and q the
requested output-channel count. New automatic Schur eligibility is restricted
to two intercept effects; the added final-recovery admission covers intercept
chains and leaves absorbed slopes/explicit-Hessian PPML unchanged.

| Phase / ownership | Charged boundary and lifetime |
|---|---|
| Initial Schur selection | `build_fit_fe_absorber` checks execution plus max(existing Schur model, nonretaining CG increment) before probing. A rejected probe restores the original retention requirement and enters existing legacy selection. |
| Initial/rejected legacy CG | Accepted ADR0060 checks execution plus CG and five coefficient banks when direct categorical CG retains them. Automatic LSMR/CG already checks those banks; CMG has its own construction/retained checks. |
| Schur exhaustion | `absorb_ols_schur_certified` releases graph/absorber and checks CG increment before continuation. The original input and shared iteration/certificate policy remain. |
| Final fit / inference | `combine_recovered_fe_coefficients` consumes the initial RHS bank and returns only one combined RHS. Ordinary robust/clustered inference consumes/drops the score design before returning. Initial solver objects are local to fit/score construction. |
| Caller during recovery | Prepared input/topology, certified fit/residual/coefficient state and xb/target products remain covered by the execution envelope; no full initial coefficient bank remains in the final fit. The additional finishing reserve also includes target-copy/publication products. |
| Retained product | One normalized L-vector, component union/find/labels and means, q requested N-vectors, residual and optional combined N-vector. Checked finishing reserve `8N(q+4)+128L+256D+4096`. |
| One-RHS retained CG | Existing three observation-matrix/group/scalar estimate plus five L-coefficient banks, plus the finishing reserve. The recovered residual is discarded before normalization/scatter. |
| SD / remainder LSMR | SD keeps two N-vectors, group workspace and two coefficient products while the existing one-RHS borrowed-LSMR call runs. Charge `16N+32L` plus max(off,diagonal,Schwarz) LSMR model plus finishing. One FE has no LSMR call. |
| Generic LSMR | Existing conservative one-RHS max-preconditioner model plus finishing covers transformed RHS, coefficient/result overlap, temporary reference copy and normalization. A failed preconditioner drops before unpreconditioned retry. |
| MAP fallback | One residual N-vector, L coefficients and small one-channel slope workspace; finishing creates q requested channels and the combined output. The finishing reserve bounds both. |
| Route failures | Each recovery helper returns and drops local workspace before the next admitted helper starts. Invalid retained products can use an admitted cheaper MAP route; no unused Schwarz reserve is demanded. No fit/public output is committed until successful validation. |

`finish_intercept_only_recovery` uses fixed row partials and per-level certificate
partials; `normalize_intercept_components` uses FE-level connected-component state.
The 128L and scalar/D terms conservatively cover those simultaneous metadata and
coefficient buffers for this intercept scope. Requested channels alone allocate
observation output vectors; the full combined vector remains required on CG/SD/
LSMR/MAP finishing and is charged even when subsequently discarded.

The solver model inherits the pinned Schwarz opaque-factor allowance and existing
execution-envelope policy. This audit does not establish unmodeled generic/slope
or unrelated OLS/PPML routes, nor change the public selected-memory scalar into a
complete model. The original unit/PW/low allocator traces distinguish real joint
checks from smaller public bases. Exact-budget tests exercise initial selection,
retained recovery, CG/MAP fallback and transactional rejection. All observed/model
gaps and failed attempts remain visible rather than being declared qualified.
