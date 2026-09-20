# Categorical DoF reuse and validation overlap

This local Mac development checkpoint removes a duplicate categorical topology
validation from absorbed-DoF construction and overlaps its allocation-free
numeric/topology validation prefix with eligible narrow IID/robust fits. It
preserves the mathematical computation, input checks, first-error order,
certificates, memory admission and transactional publication.

## Scope

The topology receipt is bound to the same immutable single-source,
single-effect owner. Prepared numeric and metadata validation still precede
DoF computation; slope, policy and cluster-nesting checks are unchanged.
This reuse applies to categorical callers, including weighted and clustered
saved-FE requests. Generic topologies retain full validation.

The additional overlap requires unit weights, fast execution, one intercept FE
without slopes, at least 16,384 active observations, at most 16 generated
columns and at least eight native workers. A genuine interaction does not enter
the prefix check. The prefix owns no heap buffer; effect-view and DoF-report
allocations remain at their original boundary after fitting. Its result or
error is consumed only after the fit succeeds. Other routes keep their existing
schedule and checks.

Public ordinary one-FE requests without saved effects use the legacy prepared
path. Their timings here are controls, not evidence that these categorical
changes improved ordinary IID or robust estimation. The added `robust_saved`
case is diagnostic; the default ten cases and frozen acceptance matrix remain
unchanged.

## Measurements

All main cells use the same audited N=983,040 data, K=10/50, 16 native/fixest
threads, Stata MP eight processors and BLAS one. Each comparison has three
rotated repetitions in fresh processes with a small warm-up. Imports and audit
exports are excluded. Full outputs include saved FE, residuals and public
`predict` calls for xb/xbd; fixest includes the matching recovery and expansion.
No builds, tests or data preparation ran during timing.

The direct overlap screen compares `dofreceipt` to `dofoverlap`:

| Case | K | Receipt only (ms) | With overlap (ms) | fixest (ms) |
|---|---:|---:|---:|---:|
| Saved FE, IID | 10 | 160 | 156 | 228 |
| Full outputs, IID | 10 | 209 | 201 | 215 |
| Saved FE, IID | 50 | 437 | 435 | 525 |
| Full outputs, IID | 50 | 493 | 494 | 526 |

The median paired ratios for narrow saved FE and full outputs are 0.981132 and
0.961722. Wide models are outside the overlap guard and show no clear gain.
The preceding receipt-only screen records full-output medians 216/213 ms
before/after at K=10 and 496/492 ms at K=50; other movements should not be
interpreted as improvements on routes that do not execute the changed code.

The saved-inference breadth screen compares the prior committed core to the
combined candidate:

| Case | K | Prior core (ms) | Candidate (ms) | fixest (ms) |
|---|---:|---:|---:|---:|
| Robust, saved FE | 10 | 178 | 172 | 250 |
| FE cluster, saved FE | 10 | 190 | 188 | 247 |
| Two crossing clusters, saved FE | 10 | 235 | 231 | 329 |
| Robust, saved FE | 50 | 561 | 560 | 933 |
| FE cluster, saved FE | 50 | 544 | 545 | 1,030 |
| Two crossing clusters, saved FE | 50 | 638 | 637 | 1,423 |

The final rebuilt `dofcore` artifact records full-output medians **204/222 ms**
at K=10 and **500/535 ms** at K=50 versus fixest, winning all six pairs.
Median paired ratios are **0.918919/0.934579**. Neither cell yet satisfies the
frozen 0.90 threshold. These are development screens, not either of the two
required final five-pair blocks.

## Regression and scientific evidence

All eleven original size/thread/group-layout controls pass for `dofcore`
against the frozen `normonly` fereg baseline. The maximum cell median paired
ratio is **1.021705** and their geometric mean is **0.995187**, within the
registered 1.05/1.02 limits. Their 33 nonself native scientific comparisons are
exact. These controls measure the cumulative current core, not only the DoF
increment. The sparse saved-output control retaining 49,150 of 983,040 rows
also passes: medians 149/147 ms before/after, median paired ratio 0.986577.

This archive contains **284 distinct timed calls**: 198 in the initial screens,
two in the explicit-dummy oracle, six sparse controls, 66 size/thread/group
controls and twelve final full-output calls. There are **102 nonself exact
native scientific comparisons**, including 39 byte-identical saved-output
comparisons. Exact comparisons cover sample, coefficients, VCE, statuses, DoF,
fit and acceptance-certificate exports; they do not claim identical wall times
or solver timing/diagnostic-norm tables. Pinned fixest comparisons use the
unchanged registered numerical tolerances. The added robust-saved diagnostic
also passes the independent full explicit-dummy covariance and FE oracle.

The final artifact passes six focused Rust tests, five licensed Stata fixtures
and the real-plugin ABI/panic-containment harness. The tests include 80 DoF
receipt comparisons across four policies, cluster nesting/crossing/malformed
inputs, and slope metadata/value availability, plus wrong-owner rejection.
Overlap checks retain full fit/inference equality, guard behavior and fit-error
precedence over speculative validation errors. Stata fixtures cover numeric,
string and interaction FE, unit/AW/FW/PW cases, missing/restricted samples,
saved effects, prediction edits and rollback boundaries.

No new observation-sized owner is introduced. Sampled process RSS and native
allocation observations are retained in the receipts; this is not new release
memory qualification. The full twenty-cell breadth regression matrix has not
been rerun for this final artifact, and both final acceptance blocks remain
outstanding.

## Reproducibility

The final plugin is
`8ef3246e53db3385580c6bff3072498155d436f0cf7783707cf084371171218a`,
built from `2e2f1a152adf2307e8c86692b8fcdb731165b15d` plus the five exact source
overlays in [identity-dofcore.json](identity-dofcore.json). Those files were
verified against the working source. Compared with measured `dofoverlap`, this
finalization adds only the explicit deferred-error unit test and equivalent
crate-visibility spelling in the root-level prepared module; all identities
are preserved separately. Build metadata retains the harness's September 19
stamp; these executions occurred September 20.

fixest is the pinned stock 0.14.2 archive with SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
See [primary timings](primary/timings.json),
[final output timings](final-outputs/timings.json),
[regression controls](controls/regression.json),
[exact scientific checks](exact-science.json) and
[validation](validation.json). Controls retain their additional exact checks
under [controls/science.json](controls/science.json). Every source/artifact,
input, sample, application status and terminal sentinel remains identifiable.
Raw licensed logs stay local. No installed package, remote state or
cross-platform qualification changed.
