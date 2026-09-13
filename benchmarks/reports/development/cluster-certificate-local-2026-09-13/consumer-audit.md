# P3 final-sample cluster and score consumers

This audit concerns accepted `257f4ce`; [source hashes](audit-identities.json)
identify the inspected implementations. The rejected patch changes only the
categorical interaction certificate. It does not change any consumer below.

| Boundary | Existing ownership/work | Disposition |
|---|---|---|
| Ordinary OLS cluster preparation | Simple dimensions borrow final categorical source codes; interactions are built once after final sample selection. Certification checks bounds/contiguity without sorting again. | Retain; there is no second construction to cache. |
| Categorical OLS/PPML cluster preparation | Simple dimensions borrow source codes; interactions are built and then independently re-factorized during certification. | Representative-tuple alternative tested and rejected by the prospective command screen. |
| Cluster-aware FE DoF | Reads final code slices to establish nesting against FE membership. | Different relationship from cluster subset ordering; retain the independent nesting calculation. |
| OLS covariance and certificate | Builds one score view. `build_raw_covariance` reduces production and independent certificate subsets together; eligible scratch batches share score construction across subsets. | Retain accepted shared mechanisms. A persistent order cache has no additional same-frame consumer here. |
| Dense subset reduction | `aggregate_ordered_dense` builds one stable radix order for the selected subset and uses it for both reduction consumers; bounded dense tables use their own admission. | Retain one order per actual subset, not an order per certificate. Distinct subset keys cannot share one arbitrary ordering. |
| Positive-rank PPML clustered VCE | `reduce_clustered_score_meats` returns both meats and observed group counts in the same subset pass. | No second inventory factorization. |
| Rank-zero PPML clustered VCE | Runs the count-only inventory because the score reducer rejects a zero-dimensional design. | This replaces the score pass; it is not an additional duplicate pass. |
| Sample/separation changes | New final codes and membership refer to the new ledger generation. | Do not reuse a previous sample's codes merely because syntax/topology names match. |

Relevant files are `ols/prepared_clusters.rs`,
`ols/prepared_categorical_clusters.rs`, `ols/prepared_clustered.rs`,
`sandwich/qualified_cluster.rs`, `sandwich.rs` and `ppml/vce.rs`.

The remaining low-memory score cost is a different lifetime problem. In
`ols/prepared_fit.rs::fit_with_controls_low_internal_with_frame`, initial
projected columns are written to column scratch in poolsize-bounded blocks.
`fit_scaled_pivoted_qr_low` then scales and overwrites that scratch with
Householder vectors and transformed trailing columns; it also transforms the
within response. The file is no longer the original within design.
`ols/prepared_scores.rs::build_scratch_score_storage` consequently rebuilds and
absorbs the selected original columns, writes column scratch and transposes it
into bounded row access for scores. Rank-zero scores need no such storage.

Fast inference already consumes a compatible packed/strided cache. When QR/rank
changes make the cache incompatible, it reabsorbs selected columns; the accepted
OLS inference ownership report measured that cost. A direct low-memory scratch
handoff is therefore rejected on representation correctness, not merely speed.
Reconstructing from QR or retaining a separate scratch file would add work/storage
and requires its own rank, certificate, memory and command comparison. Centered
tiles offer another representation for one FE. This audit does not claim that
low-memory duplicate residualization has been eliminated or close its broader
P3/P5 item.
