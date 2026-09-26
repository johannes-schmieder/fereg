# One-pass canonical incidence experiment: rejected

Local development evidence, 2026-09-26; not qualification. **Do not adopt
incidencepass1.** The bounded experiment misses its declared control gate.
[ADR 0217](../../../../docs/adr/0217-one-pass-incidence-experiment-closeout.md)
restores the accepted ADR 0216 computational source and closes this probe.

## Identities and implementation

Baseline **residualrows2**, accepted at **ceb8f53d**, plugin SHA-256
`ef3ee205cab739352ccc1024500e26e9c7c874c609001c58cf0bc7038c32f519`.
Candidate **incidencepass1**, base ceb8f53d plus the single overlay
`crates/fereg-core/src/ols/canonical_products.rs`, plugin SHA-256
`9414e2ed3970e2f56dc99a49f00d53d7b743d153ce19e894c27da0f30d4690b8`.
Its new compaction test is in that file. The candidate matched all 567 captured
computational inputs before measurement. After rejection, all **567 production
inputs match residualrows2 exactly**; no candidate machinery or test remains in
production. Complete candidate source and patch are archived.

Implicit canonical setup previously traversed each left group, accumulated and
sorted right-neighbor weights for its diagonal, then repeated the traversal,
accumulation and sorting to pack the operator. The candidate retained ordered
raw pair masses during the first pass in the existing admitted incidence banks.
After roots and anchors were known, it compacted and scaled in place. At most N
distinct pairs fit the existing N values/indices; writes move only left. The
second observation scan and sort disappear, at the cost of writing and rereading
raw pair masses. Dense factor allocation remains after grounding.

No tolerance, sample, weight semantics, rank, DoF, VCE, separation or public
behavior changed. Current-weight geometry remained call-local. Independent
original-input projection/products, residual/score/model-space/objective,
reconstruction, final-Hessian and failure-atomic publication checks remained.

Stock fixest was unmodified, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`, with
glm.tol=1e-10, fixef.tol=1e-11, matched threads, BLAS one and matched requested
inference/outputs. Public clocks include loading, estimation and publication;
harness setup/import is outside them. No compilation, profiling, audit, archive
or competing fit accompanied headline measurements. All completed monitors are
clean. Separate diagnostic clocks do not rank stock performance.

## Public results and retention decision

B=residualrows2, C=incidencepass1, S=comparable stock; medians in ms.
[Ordered distributions](distributions.md) retain all clocks, losses and repeats.
Pilot seed202609347, three rotations:

|Case; N/K/T|B / C / S|C/S paired range|
|---|---:|---:|
|Two-FE numeric low;983040/6/8|934 /926 /864|1.068–1.078|
|Matched fast|859 /858 /862|0.987–0.995|
|Two-FE two slopes low;30720/6/4|70 /70 /120|0.574–0.598|
|Three-FE two slopes fast;61440/10/8|151 /151 /231|0.649–0.654|
|Three-FE factors, cluster, fast;153600/19/8|570 /567 /697|0.799–0.821|
|Two-FE serial fast;245760/6/1|539 /538 /929|0.572–0.582|

Pilot B/C: **nine wins, three losses, six ties**. Four of six families meet 0.90
stock in every comparable pair. Large low gains 0.9%; fast gains 0.1%.

Fresh **seed202609447**, five predeclared rotations, is consumed. Source stayed
frozen through pilot, fresh validation and one bounded control repeat. The large
fast/low stock comparisons fail the mean envelope; their stock clocks are retained
in the distributions and receipts but are unranked here.

|Fresh case; N/K/T|B / C / S|C/S paired range|
|---|---:|---:|
|Two-FE numeric fast;737280/6/8|664 /655 /unranked|Mean discrepancy|
|Matched low|708 /704 /unranked|Mean discrepancy|
|Two-FE numeric fast;368640/6/4|415 /409 /661|0.610–0.624|
|Unbalanced three-FE factors, cluster, fast;184320/19/8|712 /715 /3134|0.226–0.230|
|Three-FE two slopes fast;76800/10/8|195 /190 /296|0.627–0.657|
|Three-FE four slopes low;38400/6/8|115 /115 /117|0.958–1.017|
|Two-FE serial fast;245760/6/1|541 /540 /935|0.573–0.582|
|Weak_d3 fast;61440/6/4|161 /160 /821|0.195–0.198|
|Disconnected two FE low;61440/6/4|92 /94 /99|0.920–0.960|

Fresh B/C: **27 wins, nine losses, nine ties**. Large fast/low gains are
**1.4/0.6%**, T4 fast 1.4%. Five of seven scientifically comparable families meet
0.90 in every pair; two more are unranked. Four-slope and disconnected controls
miss the target. Changes on untouched engines are controls, not attributed gains.

A predeclared five-pair repeat of the factor and disconnected controls preserved
the source, seed and definitions. Hashes verify identical full/warm numeric input
arrays. Factor B/C becomes **715/714 ms**; its original **712/715** loss did not
repeat. Disconnected remains **92/94**, repeating the original 2-ms loss.
Both blocks are retained separately; they are not pooled to hide the loss.

The experiment's declared retention gate required useful public savings without
a reproducible control loss. That gate is not met. The gain is small enough that
the repeated disconnected loss and added organization are not worth retaining.
Restore the accepted solver, preserve this evidence and move to a different
boundary. Do not infer that compaction itself caused the disconnected whole-fit
loss; no such attribution was established. No source/layout/threshold tuning was
performed against the fresh seed.

## Paired diagnostic attribution

Isolated **rankprofile2/incidenceprofile1**, two alternating pairs per case, use
the same phase meanings. Incidence-bank allocation moves from the packing phase
to the matrix phase in C; compare their sum and total setup, not packing alone.

|Boundary, total over two canonical calls|Million fast B/C ms|Million low B/C ms|
|---|---:|---:|
|Complete geometry setup|39.625 /33.582|40.893 /33.695|
|Matrix/diagonal and early raw incidence|11.954 /16.922|12.345 /17.152|
|Packing/compaction|14.433 /3.462|15.428 /3.426|
|Topology|5.126 /5.096|5.004 /5.043|
|Weight normalization and masses|7.676 /7.672|7.665 /7.654|
|Containing canonical preparation|220.770 /217.021|249.759 /240.226|

This establishes a **6–7 ms** setup saving. It does not establish a 12-ms whole-fit
saving from the packing span alone. Nested phases must not be added twice.
Whole diagnostic plugin times are **838.848/831.148 ms** fast and
**921.658/900.041** low; the factor control is **484.310/487.720**. Keep that
diagnostic control loss. No diagnostic clock is a stock headline result.

Other candidate phases stay substantial: canonical norm scans **15.9/15.8 ms**,
reported-intercept/bread finalization **20.6/20.0**, and finite score checks
**13.5/13.7**. Prior current-source diagnostics also put low retained-control
writing at about 23 ms; it already parallelizes encoding inside bounded tiles.

The next materially different hypothesis is a **single-use canonical response
statistic handoff**. `Products.means[0]` and `Products.norms[0]` are computed from
the current call's inputs, but `try_canonical_products` transfers only control
statistics and the finite Gram to the rank owner. `solve_full_retained_rank`
rescans response norms; `finalize_reported_bread_and_intercept` separately obtains
the outcome mean for its independent intercept identity.

Determine which repeated statistics can be carried with the already single-use
`CanonicalMoments`, while retaining the fresh raw-residual mean and independent
score/identity checks. Prove normalized weight units, actual reconstructed-row
rounding, zero/underflow behavior, rank sensitivity and generation/rollback
ownership. A norm derived from a scaled Gram diagonal is not automatically an
exact replacement for the stable residual scan. The entire 36-ms containing cost
is not assumed removable. Do not reopen incidence tuning or a 5-ms topology cache.

## Scientific checks, controls and memory

The **rejected candidate** passed **74 distinct Rust tests**, **nine licensed
Stata fixtures**, **233 public PPML state audits** and **12 explicit Poisson/
Hessian/sandwich comparisons**. The new independent operator test covers repeated
pairs, every anchor position, interleaved components, anchor-only rows, common
weight scales 1e-200/1/1e200, changing weights, nonfinite refusal and diagonal-bank
preservation. Full frame tests cover exact/one-byte-short admission, independent
weighted contrasts, dense/scratch equality, current weights and strict rollback.

All **83 native PPML pairs** have exact coefficients/covariance/means/requested
outputs. All **18 OLS pairs** have exact coefficient/covariance exports. Anchored
comparisons pass **139/150**; direct C/stock **56/67**, with no missing pairs.
One older small saved-output discrepancy and ten fresh large-case mean comparisons
remain unranked. All native and stock states pass the separate original-input
sample/score/model-space/objective audit; that does not override pairwise envelopes.

Fresh large fast/low means differ from stock by at most **5.073e-7 /5.034e-7**,
with **six** failing coordinates per comparison and maximum tolerance ratios
**1.213/1.204**. Coefficients, covariance and objectives pass their pairwise
checks. Native B/C are exact, so the new kernel does not explain the discrepancy.
No tolerance is relaxed and no stock speed ranking is made for those cases.

OLS B/C medians are **67/64, 75/76, 15/14, 207/207, 160/158, 17/17 ms** for
slopes, three-FE low, small numeric, factors, large numeric and one FE. Preserve
the 1-ms three-FE loss. This is a focused development control, not qualification.

Tracked whole-fit heap and scratch peak deltas are **zero**. Modeled bounds vary
**-48..+48 bytes** with unchanged admission. No additional explicit bank or stack
page is introduced; existing setup buffers are reserved earlier, within the same
charged coexistence. Physical-cap/platform qualification is not claimed.

No build/test failure, timed application failure, timeout, interruption or
contended measurement occurred. The failed retention gate and scientific
comparison failures remain visible. Known static inventory drift, wire-minor/
checkpoint-count failures, constrained-fit/nonretaining-CG failures, SCC deferral,
frozen 77/16/111 definitions and stopped campaigns are unchanged. No push/release.

## Receipts and restored state

The content-addressed archive preserves candidate/baseline/diagnostic sources,
builds, source-bound identities, declarations, monitors, all timing distributions,
independent science, both control blocks, tests, filtered licensed sentinels and
the rejected patch. Generated datasets and binary exports are hashed but omitted;
raw licensed logs and personal paths are not committed. Use the adjacent
`restore_receipts.py` into a new directory; `archive-verification.json` records
hash/restoration checks. Local work root: `target/ppml-incidence-pass-20260926`.

Production remains exactly the accepted residualrows2 computational snapshot.
This rejection does not transfer candidate timings to the restored source,
complete the 0.90-stock objective, or reopen previous rejected probes.
