# One-FE OLS: inference and saved-output breadth on the Mac

The recent IID win does not extend uniformly to other requests. With fifty
regressors, fereg wins every unsaved robust/clustered case measured here and
both clustered saved-FE cases. With ten regressors, robust and one-way
clustering still lose; two-way clustering is close or modestly faster. Saving
the FE under IID inference loses at both widths, and requesting the complete
output bundle increases that gap.

This is a focused local development comparison of the existing committed
implementation, not a new optimization or release qualification.

## Complete-request timings

The data have 983,040 observations, 196,608 ordinary intercept FE levels with
five observations each, numeric continuous controls, unit weights, no missing
values and no singleton removals. Both engines request sixteen native threads;
BLAS uses one thread. Each cell has three fresh-process, small-warm-up pairs
in alternating fereg/fixest order. Values below are medians in milliseconds.
Paired ratios are medians of the three same-repetition ratios, so they need
not equal the ratio of the displayed medians.

| Request | K=10 fereg | fixest | Paired ratio | K=50 fereg | fixest | Paired ratio |
|---|---:|---:|---:|---:|---:|---:|
| IID | 132 | 149 | 0.893 | 395 | 487 | 0.807 |
| Robust HC1 | 194 | 165 | 1.176 | 709 | 906 | 0.778 |
| One cluster: FE group | 208 | 189 | 1.096 | 662 | 975 | 0.675 |
| One cluster: crossing c1 | 222 | 194 | 1.150 | 674 | 824 | 0.823 |
| Two clusters: crossing c1, c2 | 254 | 268 | 0.940 | 748 | 1,312 | 0.570 |
| Two clusters: FE group, c2 | 238 | 249 | 0.956 | 753 | 1,249 | 0.602 |
| IID + saved FE | 291 | 220 | 1.327 | 757 | 528 | 1.434 |
| FE-group cluster + saved FE | 323 | 284 | 1.137 | 945 | 1,016 | 0.924 |
| Two crossing clusters + saved FE | 367 | 352 | 1.040 | 1,026 | 1,381 | 0.744 |
| IID + saved FE, residuals, xb, xbd | 368 | 226 | 1.615 | 928 | 538 | 1.719 |

The crossing dimensions have 1,024 and 257 levels. For zero-based observation
index i, c1 is `i % 1024 + 1` and c2 is `(i // 7) % 257 + 1`.
They are distinct from the FE grouping. The FE-group cluster cases exercise
the nested absorbed-DoF adjustment; the crossing cases retain the absorbed
DoF. These are balanced synthetic observations, not a claim about all cluster
counts, group orders, real data, weights or slopes.

Timing includes full estimation, Stata-to-native loading, requested inference,
transactional publication, and requested postestimation calls. It excludes
file import and audit exports on both sides. For saved effects, R performs
`fixef(fit, sorted=FALSE)`, expands the levels through `fit$fixef_id$g1`, and
normalizes the observation-level FE to mean zero. It does not pay for character
matching or a redundant regressor-matrix reconstruction. The output bundle
also retrieves residuals and fitted values and constructs the normalized
finite prediction. Stata saves the FE and residual in the estimation command,
then executes `predict double ..., xb` and `predict double ..., xbd` inside
the total timer. No claim equates the engines' internal work, only the requested
numerical results.

The original frozen y/X/FE values are retained, with the same added cluster
columns present in every case. Consequently this IID control is a new local
comparison, not a replacement for the earlier two-block IID acceptance test.
Three pairs are diagnostic; small differences merit confirmation before
selecting an implementation. Per-run values, ranges, post-fit output times
and sampled RSS are in [timings.json](timings.json) and
[receipts.json](receipts.json).

## Which optimizations actually apply

| Optimization | Current reach and important exclusions |
|---|---|
| Guarded numeric SPI batch accessor | Shared by numeric OLS and PPML loading on macOS, including clustered and saved requests. Other platforms retain their existing accessor checks. |
| Owned full-sample input transfer | Shared preparation can avoid copying complete unweighted inputs, including robust/clustered and the availability stage of categorical preparation. Later categorical compaction/topology work still remains. |
| Bounded loading/conversion/RHS pipeline and deferred initialization | Public OLS dispatch currently requires full-candidate, unweighted, ordinary one-FE IID with no saved name. Size, width, thread and memory guards apply inside. Robust, clustered and saved-FE calls never receive this input team. |
| Raw-moment/preparation overlap | The ordinary IID branch consumes the loaded RHS. Robust/clustered inference and saved categorical preparation use separate entry points. |
| One-FE membership reuse and early factorization | Ordinary preparation, with no cluster sources and additional size/shape guards. Cluster-only missingness and singleton semantics must precede any reuse. Saved categorical preparation does additional topology work. |
| Borrowed one-FE descriptor reuse | Eligible fast, unweighted, ordinary one-FE fits without retained FE coefficient products. Robust/clustered fits can benefit; saved-FE coefficient retention excludes this path. |
| Narrow completion overlap | At least eight threads, 8–16 generated columns, large unit-weight one-FE fits, no retained sandwich score cache and no retained FE coefficients. Robust/clustered and saved-FE paths fail these guards. |
| IID outcome/DoF overlap | IID-specific, with narrow/large/fast/unit-weight guards; it is not a clustered-inference implementation. |
| Unit means, unit intercept scores and single-column unit norm | Shared native arithmetic where unit weights and the required shape hold. The unit norm is not tied to the VCE request, but it does not replace weighted or multiple-column arithmetic. |

The decisive public dispatch guard is in
[estimate.rs](../../../../crates/fereg-plugin/src/estimate.rs), around line 235.
The other relevant boundaries are ordinary preparation in
[prepared.rs](../../../../crates/fereg-core/src/ols/prepared.rs), saved categorical
preparation in
[prepared_categorical_data.rs](../../../../crates/fereg-core/src/ols/prepared_categorical_data.rs),
and descriptor/completion guards in
[prepared_fit.rs](../../../../crates/fereg-core/src/ols/prepared_fit.rs).
The macOS accessor is in
[stata_bridge.c](../../../../plugin/stata_bridge.c).

Thus shared loading, ownership and redundant-pass reductions should be extended
where the data dependencies match. A single blanket fast path would be wrong:
weighted means, missing cluster values, singleton changes, low-memory budgets,
multi-FE projection and saved-output precision have different requirements.
The current guards describe the implementation and tested adoption envelope,
not a mathematical reason that clustering must prevent loading overlap.

## Where the remaining cost sits

The uninstrumented production runtime receipt already separates input loading
from native work up to precommit. Median milliseconds:

| K | Request | Input | Native to precommit | Wrapper/store/cleanup | Post-fit predictions |
|---|---|---:|---:|---:|---:|
| 10 | IID | 69.9 | 49.6 | 12.2 | 0 |
| 10 | Robust | 81.3 | 100.5 | 12.2 | 0 |
| 10 | FE-group cluster | 82.6 | 112.9 | 12.7 | 0 |
| 10 | IID + saved FE | 80.7 | 182.6 | 24.5 | 0 |
| 10 | IID + all outputs | 79.1 | 182.7 | 56.0 | 50 |
| 50 | IID | 215.8 | 143.9 | 37.3 | 0 |
| 50 | Robust | 288.7 | 381.2 | 36.7 | 0 |
| 50 | FE-group cluster | 275.3 | 349.5 | 36.8 | 0 |
| 50 | IID + saved FE | 276.9 | 419.6 | 59.8 | 0 |
| 50 | IID + all outputs | 276.0 | 425.5 | 70.2 | 155 |

Preflight is below 0.5 ms; medians of separate phases need not sum exactly to
the median command. Wrapper/store/cleanup is the full estimation timer minus
the native precommit timer; it is not a pure SPI-store measurement.

Twelve additional native traces cover IID, robust, FE-group cluster, crossing
two-way cluster, saved IID and the full output bundle at both widths. Their
scientific exports are exactly equal to the corresponding primary native fit.
Selected traced initial-absorption and RHS-construction costs are:

| K | Request | RHS construction | Initial absorption |
|---|---|---:|---:|
| 10 | IID | 6.0 | 15.4 |
| 10 | Robust | 15.7 | 15.1 |
| 10 | FE-group cluster | 15.7 | 15.3 |
| 10 | IID + saved FE | 18.1 | 60.6 |
| 50 | IID | 13.8 | 36.8 |
| 50 | Robust | 34.4 | 36.5 |
| 50 | FE-group cluster | 35.1 | 37.4 |
| 50 | IID + saved FE | 37.5 | 207.0 |

These are one diagnostic invocation per cell, not another repeated acceptance
matrix. They localize costs but do not isolate a single causal change. Saved
requests retain initial FE coefficient products for every response/regressor
RHS and combine them after solving. They also use categorical preparation and
lose several descriptor and overlap shortcuts. The 37-to-207 ms absorption
difference therefore describes the complete saved route, not just allocation
or writing the final FE variable. The main timing comparison shows that the
saved-output gap exists independently of tracing.

## Suggested development order

1. **Specialize ordinary one-FE saved recovery.** Investigate constructing its
   one final coefficient vector from the certified final absorbed target,
   rather than retaining response/regressor FE products through initial
   absorption. Reuse the existing certified final-target recovery concepts,
   preserve output precision, normalization, independent FE moments and memory
   admission, and avoid generic categorical re-preparation where identity can
   be proved. This is the largest measured opportunity here.
2. **Extend the owned loading/RHS pipeline to these requests.** Begin with
   complete numeric unit-weight one-FE robust, clustered and saved calls.
   Carry the owned RHS into their actual fit entry points; merely relaxing
   the input-team guard would construct work that those paths do not consume.
   Keep final sample validation ahead of reuse and retain cluster-source
   missingness/error precedence.
3. **Reuse finalized one-FE/cluster topology and reduce narrow inference
   passes.** Clustering at the FE level is an especially clear reuse case.
   Extend eligible raw-moment scheduling while preserving the existing score
   and covariance certificates. The wide clustered route already wins here;
   prioritize the narrow gap and avoid regressions in the wide cases.
4. **Reduce output publication and prediction passes.** The complete bundle's
   extra post-fit cost is about 50/155 ms for K=10/50. Profile the public `xb`
   and `xbd` paths separately before deciding which certified native vectors
   can be reused. Preserve transactional publication and sample semantics.

Do not extend unit-weight algebra to weighted models or ordinary group-mean
shortcuts to slopes/multiple FEs by dropping guards. Those require separate
bounded implementations and evidence. No production optimization was changed
in this investigation.

## Science, identity and limits

All 120 primary calls pass command return code, process status, terminal
sentinel, exact imported-value and estimation-sample checks. All sixty pairs
pass full finite-coefficient covariance and RSS comparisons; rank, correction
denominator and cluster count/DoF checks are exact. The largest observed
coefficient difference is 1.63e-14, covariance difference 4.76e-20, and requested
observation-output difference 7.78e-14. There are 28 small oracle calls across
the initial full smoke and the efficient saved-output rerun. The independent
oracle solves explicit FE dummies, constructs HC1/cluster inclusion-exclusion
covariance, applies the registered correction/PSD policy, and checks saved FE,
residuals and both predictions. All twelve traced calls reproduce the native
coefficient, covariance, status, DoF, certificate and fit exports exactly;
requested output binaries are byte-identical. Total: 160 accepted calls.

The [science archive](science.json), [input hashes](inputs.json),
[source identity](identity.json) and worker snapshots bind these checks. The
measured plugin is unchanged from the completed optimization:

- Source: `82e23357af64d65dd9e7162e2317b2ace62e1495`; all 509 artifact source
  manifest entries match its Git archive.
- arm64 plugin SHA-256:
  `0ac6a0449cff224f02d6d7b7b7696b79fe6c6d182d89e3b406b4ea26a761e9fa`.
- fixest 0.14.2 source archive SHA-256:
  `17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
- Apple M2 Ultra, macOS 26.6.2, R 4.6.1, Stata/MP 19 with an eight-processor
  Stata license; fereg and fixest request sixteen native threads, BLAS one.

The inherited R worker's descriptive `covariance_normalization` metadata says
`native IID` for all OLS cases. That label is not used in acceptance: the actual
VCE formulas and explicit `ssc(K.adj=TRUE, K.fixef="nonnested", G.adj=TRUE,
G.df="min", t.df="min")` are preserved in the worker/cell receipts, and full
covariances plus their correction denominators are checked. Historical raw
receipts are retained as emitted. The current harness corrects this descriptive
label when staging a new controller; the archived harness/worker snapshots
remain the versions that actually generated these measurements.

The report does not establish weighted, slope, multiple-FE, constrained-memory,
Linux or Windows performance. RSS sampling is diagnostic. No installed package
replacement, broad qualification, push or remote workload occurred.
