# Repeated joint PPML proposal products: initial screen

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 27/27 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| profiled-small | 0.872 / 0.875 / 0.898 | 1.642 / 1.693 / 1.902 |
| small-low | 0.832 / 0.855 / 0.964 | 1.346 / 1.642 / 1.725 |
| small-serial | 0.938 / 0.940 / 1.050 | 1.617 / 1.781 / 2.142 |
| larger-wide | 0.888 / 0.932 / 1.006 | 1.062 / 1.089 / 1.123 |
| sparse-two | 0.984 / 1.005 / 1.065 | 0.563 / 0.565 / 0.623 |
| wide-serial-adjoint | 0.819 / 0.884 / 0.992 | 0.430 / 0.572 / 0.674 |
| three-fe-core-pair | 0.836 / 0.954 / 1.005 | 0.486 / 0.500 / 0.534 |
| excluded-slope | 0.818 / 0.856 / 0.947 | 0.295 / 0.309 / 0.311 |
| excluded-ols | 0.989 / 1.005 / 1.082 | 1.428 / 1.472 / 1.701 |

profiled-small: selected native baseline / candidate teams = 4 / 4.

small-low: selected native baseline / candidate teams = 4 / 4.

small-serial: selected native baseline / candidate teams = 1 / 1.

larger-wide: selected native baseline / candidate teams = 8 / 8.

sparse-two: selected native baseline / candidate teams = 4 / 4.

wide-serial-adjoint: selected native baseline / candidate teams = 4 / 4.

three-fe-core-pair: selected native baseline / candidate teams = 8 / 8.

excluded-slope: selected native baseline / candidate teams = 4 / 4.

excluded-ols: selected native baseline / candidate teams = 8 / 8.

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

## Native thread scaling

| Lower → higher setting | Higher/lower min / median / max | Slower on every repetition |
|---|---:|---|

Independent-oracle failures: 0.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.

## Scope and interpretation

All **81 applications, 27 native scientific pairs and 27 stock scientific
pairs** pass on candidate snapshot `4c3d7c44…` / Linux plugin `a7e95e67…`, versus
accepted baseline `d1156e6d…`. Stock is pinned unmodified fixest 0.14.2: default
for 24 pairs, explicitly strict for the 3 slope pairs (`glm.tol=1e-10`,
`fixef.tol=1e-11`). Robust PPML and IID OLS inference are included; no optional
saved observation outputs. Separate independent explicit-design/Poisson/Hessian/
sandwich fixtures validate the scientific boundary; rotations add no new
independent large-design oracle. The frozen comparator checks sample, coefficients,
SE/full covariance, objective and native rank/DoF/team identity without relaxation.

The candidate keeps joint LSMR and all independent acceptance/reconstruction
arithmetic. Its small fast C/B paired median is **0.875**, all three faster;
wide K24 is **0.884**, all three faster. Serial is 0.940 with one loss;
K12 is 0.932 with one loss; three-FE 0.954 with one loss. Sparse losses up to6.5% appear in two pairs and its median 1.005 is neutral. Marginal B/C/stock medians
(ms) are 441/386/226 fast,390/335/218 low,514/482/225 serial,725/676/621 K12,
619/622/1100 sparse,1251/1055/1844 K24,2587/2348/4692 three-FE,
500/460/1479 slopes and183/184/125 OLS.

Low and slope engines are excluded from the changed ordinary operator yet their
paired medians also improve, to 0.855 and 0.856, with every pair faster. These
controls prevent attributing every public-clock gain to the new kernel. OLS
instead loses slightly, median 1.005, range 0.989–1.082, two losses. Fresh
confirmation on unchanged source is required; there is no performance adoption
at this checkpoint. Do not tune routes, thresholds or guards to these results.

## Recorded phases and memory

Native outer and reported inner iteration counts match across every paired
case. Small fast retains 4 outer/114 reported inner iterations while median
published absorption time falls 204→174ms; K24 retains 4/109 while absorption
falls 564→416ms. These aggregate native clocks are recorded in phases.json;
they may nest and must not be summed. Native iteration counts are not equivalent
to fixest's iteration definition. Low/slope phase clocks also improve, so these
observations alone are not a controlled causal decomposition.

No heap bank or modeled allocation is added. Tracked peaks match in every PPML
case except three-FE, whose candidate peak increases by 1,646,344 bytes within
its unchanged admission envelope. Sampled process-peak differences (MiB) are
+0.19,+4.48,+2.10,-1.40,+1.14,+1.17,-1.67,+0.46,-3.39 in table order. Actual
allocation and RSS can vary with execution; no physical-memory qualification
or zero-cost claim follows.

Job **7763162** has clean accounting: 298s,28 isolated slots,maxvmem2.323G,
plus rotation and wrapper terminal sentinels. Two additional Linux strict-final/
retry/nested-rank/zero-control Rust tests pass before any timing. Together with
build validation, both Mac and Linux pass 32 focused Rust tests and four licensed
fixtures; Linux also passes PPML smoke. No builds, profiling, archiving or
competing fits overlap headline commands. All source/artifact and per-fit file
bindings remain in the JSON evidence. The 1.20 ceiling and 0.90 aspiration remain
unmet in small dense PPML and OLS.
