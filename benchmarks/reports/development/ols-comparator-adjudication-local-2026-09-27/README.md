# OLS comparator adjudication: accuracy and absorbed DoF

Three predeclared small designs were compared to an independent full-dummy SVD
oracle. Native fereg passes coefficient, full finite covariance, RSS, exact sample
and rank checks in all three. Fixest's stricter stopping tolerance is useful, but
`K.exact=TRUE` does not reliably supply the same inference for three FE. No old
benchmark cell is reclassified, and these untimed checks establish no speed ratio.

## Results

Counts below include the six finite controls. All commands keep identical samples.

| Design | N | Explicit rank / native | Stock default count | Stock strict count | Stock strict + K.exact count |
|---|---:|---:|---:|---:|---:|
| Strong degree two | 960 | 497 | 497 | 497 | 497 |
| Sparse two FE | 3840 | 820 | 823 | 823 | 820 |
| Sparse three FE | 3840 | 827 | 832 | 832 | 826 |

All coefficient and RSS comparisons pass the existing development tolerances
on these small inputs. Default and strict inference fail in both sparse cases.
Exact-count inference passes for two FE but still fails for three FE, this time
undercounting rank by one. Native covariance matches the independent oracle to
at most 8.1e-18. The largest native coefficient error is 1.5e-14.

The explicit design has a clear singular-value gap: smallest retained singular
values are 0.552, 0.120 and 0.0822, versus discarded values below 2.85e-14.
Independent normalized residual scores are below 1.30e-14. These rank differences
are not ambiguous numerical cutoffs. Dividing each covariance by its reported
RSS/residual-DoF scale also verifies the finite bread independently; the sparse
covariance discrepancies are explained by the count used for inference.

The pinned fixest source computes `K.exact` by recovering FE reference counts
(`R/VCOV.R`, `ssc_compute_K`). The general three-plus-FE recovery in
`R/methods.R`, `fixef.fixest`, includes an approximate reference-count route.
Its option name is therefore insufficient evidence of mathematical rank on an
arbitrary multipartition design. Moreover, `fit$nparams` stays at 823/832 even
when inference uses 820/826; this probe records `attr(vcov(fit, attr=TRUE), "df.K")`
as well as that original field.

## Commands and identities

Native: public `fereg y x1-x6, absorb(...) threads(1)`, IID inference, plugin
SHA-256 `571eccb27a6cafb3239b163a37344ffb3e0e917d56b2b7d53c5a73762f250a50`,
computational source committed at `f8803410` and bound by the candidate04 manifest
in the separation-coefficient-proof report. Native absorption and DoF policy are
unchanged.

Reference: unmodified fixest 0.14.2 rebuilt directly from the pinned archive
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9` into a fresh
isolated library. `feols` uses one thread and IID covariance in three arms:
defaults; `fixef.tol=1e-11`; and that tolerance plus `ssc(K.exact=TRUE)`.
The old source directory failed preflight because two files had changed, so it
was not reused. This says nothing about the identity of previously installed or
previously measured reference artifacts. `reference-build.json` records the new
installed-file identities and successful build.

The independent oracle constructs every dummy and finite column and computes a
full SVD. It knows neither native absorption nor fixest demeaning. Stata and R
each replay their loaded doubles byte-for-byte against the generated input before
comparison. All science tolerances come unchanged from manifest-light-v2.json.
`registration.json` predates these fits; `results.json` retains every passing and
failing check, numerical error and output identity. Raw licensed logs stay under
ignored target/. `replay.py` and `analyze.py` reproduce the experiment with explicit
local runtime/output paths.

## Consequence for the benchmark plan

For disconnected two FE, a separately declared stricter stock arm with
`K.exact=TRUE` is a viable candidate for restoring comparable IID inference;
larger inputs still need confirmation, and the extra work must be timed.
For three FE, retain NR when requested inference disagrees. A coefficient-only
or unscaled-bread agreement is useful diagnosis but cannot qualify a complete
command comparison with mismatched covariance. Neither changing native DoF nor
silently rescaling stock output is adopted here.

The original large degree-two residual discrepancy is not reproduced at N960.
Stricter large-input reference checks remain necessary. These small results also
do not prove that native's documented pairwise three-FE rank policy equals exact
rank on every larger design. The ongoing SCC rotation and frozen manifests stay
unchanged.
