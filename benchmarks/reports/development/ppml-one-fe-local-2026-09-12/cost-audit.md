# P4 one-FE coverage and pass audit

Source: `0889721` plus the adjacent frozen implementation patch. This audit is
specific to PPML fast execution with one categorical intercept effect. Zero-FE and low-memory designs, slopes, two/three-FE LSMR auto regions and
one-shot OLS retain their prior implementations. Intercept-only categorical interactions can enter through the
existing borrowed intercept design; no new ado expansion is involved.

| Consumer | Previous implementation / retained cost | Candidate behavior |
|---|---|---|
| Numeric admission, separation and generation construction | Allocation-free preflight, admitted row ledger, original-axis raw sources and generation-specific sample/weight normalization. | Unchanged. Separation still runs its own certified methods; a new sample builds a new session. |
| Initial fresh solve | Raw response-plus-X matrix construction; stable weighted raw norms and compensated means; generic MAP projection; retained within norms, finite rank solve and independent certificate. | Existing reusable categorical projector admits one FE. It evaluates the initial certificate, then projects group means using the requested team when the existing work threshold is met. It certifies after the first group sweep. No accelerator or two-FE PCG state is allocated. |
| Ordinary response/weight refresh | Changing Hessian weights invalidate weighted products. Generic MAP separately projects response and retained X. | Existing fused retained path reuses the same matrix capacity for response plus X. Its response candidate is old projected response plus the new-minus-old raw response. Current-weight projection removes the old FE component. Current raw reference norms/means and weighted within norms are still refreshed. |
| Already-within candidate | Independent current-weight certificate can authorize zero sweeps. | Preserved, including the initial certificate and its unchanged reference scaling. No assumption that a warm residual is valid after weights change. |
| Strict rank/checkpoint solve | Prior active basis revalidation, strict complete-basis solve and RAII transaction; rejected candidate restores prior state. | Same authority and rollback. The new one-FE projector and existing response fusion serve the canonical MAP projection; strict tolerances, finite guarded Cholesky/QR fallback and rank rules remain unchanged. |
| Final-Hessian synchronization | Refresh to final q×mu weights, strict basis/score checks and final bread. | Same synchronization, with the categorical projector where eligible. No stale means, moments, factorizations or certificates are reused. |
| Inference and saved recovery | Current-weight projected design, existing score caches/reductions, independent sandwich certificate; final-state FE recovery and transactional publication. | Unchanged consumers and scientific rules. Saved outputs remain bounded comparisons. |

The two-line admission extension reuses existing mechanics rather than introducing
another PPML solver. Public family/engine selection remains MAP for one FE. The
two/three-FE LSMR automatic region and its checkpoint specialization are unchanged.
Workers use only Rust-owned arrays; this work adds no SPI access or worker pool.

The group workspace can add `max_levels × simultaneous_RHS` means plus group
weights and the already modeled narrow deterministic-reduction bank. The PPML
memory envelope already charges these for fast execution. Single-FE admission
explicitly excludes the three observation-sized accelerator matrices. Retained
response fusion expands/compacts one admitted allocation; it does not retain a
second N-by-K projected matrix.

Remaining passes include raw matrix construction, serial stable reference-norm
and compensated-mean reads, warm stride expansion/compaction, within norm refresh,
moment/predictor arithmetic and independent certificates. Current-frame norm
reuse, skipping a known-fresh initial certificate and centered-tile estimation
remain hypotheses. No large raw cross-product subtraction is introduced.

Existing timers overlap: preparation includes initial session construction;
strict/final includes subordinate projection/finite work; solver setup/solve/
certification diagnostics are not additive to command-phase times. The report
uses each scope separately and does not claim measured hardware bytes or total
construction counts from source-derived pass observations.
