# Fixed-effect kernel

Status: implemented in the public 0.9.0 OLS path; exact-SHA platform and
performance qualification remains pending.

The safe core provides:

- queue-based singleton deletion to a fixed point;
- multipartite connected-component labeling;
- one-/multiway weighted projections;
- intercept, slope-only, intercept-plus-slope, and multiple-slope local blocks;
- deterministic batched symmetric MAP and routed `within`/LSMR support;
- normalized absorbed-sum and saved-FE recovery; and
- fresh basis-invariant weighted FE-span projection certificates.

Each effect stores compressed level-to-observation membership. MAP allocates
and reuses bounded projection workspaces. Rank-deficient local slope blocks use
pivoted elimination; unidentified parameter directions may be normalized, but
fitted projections must remain identified.

A MAP/LSMR recurrence is never sufficient evidence. For each effect-level
block `B`, the final gate computes the projection of `sqrt(W) r` onto
`col(sqrt(W) B)` with scaled, reorthogonalized rank-aware QR and measures it
relative to the outcome/residual scale. Columns are normalized before this
check, making the certificate invariant to absorbed-channel units.

Hitting an iteration limit or failing the fresh certificate is fatal. Solver
threads operate only on Rust-owned storage; no Stata SPI call occurs off the
calling thread. Independent explicit-dummy fits validate small designs,
identified combinations, residuals, absorbed sums, and normalization rules.
