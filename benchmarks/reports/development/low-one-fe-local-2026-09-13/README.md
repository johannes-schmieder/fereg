# P2/P6: fresh low-memory OLS blocks, adopted locally

Adopt [ADR 0051](../../../../docs/adr/0051-low-memory-fresh-block-projection.md)
against accepted native `4e18c223a6b741b04bbcee35d10790729f5c8171`, at repository
checkpoint `01f95bdf9918bbebacbad353ef37a7be6bf9badd`. This extends existing
one-FE projection and norm reuse to bounded low-memory blocks and consumes
the projected outcome allocation. It does not add a projection algorithm.

[Identities](identities.json) bind 152 native inputs, the
[one-file implementation/test patch](implementation-reviewed.patch), fixture,
runner and both isolated universal plugins. Candidate plugin SHA-256 is
`b8333c4ac9e28137c1ee457f95d7cf637bed824ab8d80cad67b67d6f84527b35`;
baseline is `ea63bae26377af9ae561c03df8acdb95039461c6124258fe8e05a54955960415`.
The installed plugin was neither used as the baseline nor replaced.

## Coverage and pass audit

`N` is final observations, `K` generated controls and `B` bounded pool width.

| Low-memory consumer | Accepted change | Remaining cost / protected boundary |
|---|---|---|
| Newly built outcome and regressor blocks, automatic one intercept FE | Existing fresh entry skips the redundant raw certificate and uses existing norm scheduling | Raw reference norms, projection and independent final certificate remain; no setup cache is introduced |
| Intercept-only categorical interaction | Same finalized-partition projector as simple one FE | Topology validation, membership and weight semantics remain |
| One-column projected outcome | Consume its matrix allocation as the outcome vector | Removes one retained `8*N` allocation and nominal `16*N` copy traffic; no claim of hardware traffic |
| Cached within outcome/regressor norms | Move and reuse the existing certified norm vector | A scalar max-weight pass checks frame normalization; differently normalized explicit frames and missing caches retain the original norm scan |
| Explicit solver, retained saved-FE, slopes, multiple FE, retained CMG/LSMR | Existing dispatcher keeps its appropriate projector/session; outcome ownership and available norm reuse still apply | Warm residuals, retained products, solver continuation and independent certification remain |
| Finite solve and inference | Same bounded column buffer, QR, scratch layout and score consumers | Raw column/reference construction, scratch writes/reads, rank and covariance work remain |
| Fast execution and ordinary retained PPML | No production change | Existing estimator-specific paths and policies remain |

Eligible within-norm reuse removes one observation pass per outcome/control;
fresh one-FE blocks omit the initial raw certificate for the outcome and each
of `ceil(K/B)` regressor blocks. These are source-derived traversal counts,
not timing attribution or measured memory-controller bytes. The old outcome
matrix remained owned after its separate vector copy; consuming it shortens
that overlap. Existing solver setup, finite QR and scratch work remain.
Explicit frame divisors are checked because absorption normalizes by the
maximum raw weight, whereas a caller may choose another valid solve scale.
No raw reference scale is replaced by a within scale.

## Complete command, phase and memory results

The [prospective protocol](protocol.md) admits a target with >=5% AND 10 ms
command gain, or >=8% native peak reduction with command cost within
max(10 ms,5%). Three first-seed screen pairs were followed by six predeclared
repeat pairs because the narrow AW target qualified. Seven protected pairs
and one separately traced pair give **17 pairs / 34 measured commands**.
Every pair is retained. Fresh local Stata/MP 19 processes on the M2 Ultra use
a small untimed warm-up; primary seeds and arm order rotate. Native workers
are separate from the Stata license processor cap. Affinity is uncontrolled.
These are local development findings, not comparator or release claims.

Seconds below are independent medians; paired percentages need not equal
their ratios. No overlapping phase medians are added.

| Primary | N / K / workers / pool | Command | Median paired gain | Native phase | Native peak MB |
|---|---|---|---:|---|---|
| AW IID | 983040 / 1 / 1 / 1 | 0.369 → 0.336 | 8.9% | 0.30995 → 0.27496 | 95.62 → 87.75 |
| Unit IID | 245760 / 10 / 4 / 10 | 0.332 → 0.314 | 4.6% | 0.30081 → 0.28342 | 60.52 → 58.55 |
| PW two-way cluster, sparse missing | 245760 / 16 / 16 / 5 | 0.959 → 0.930 | 2.6% | 0.91284 → 0.88302 | 68.31 → 66.35 |

All nine primary pairs improve. Narrow AW saves 48/33/26 ms and exactly
7,864,320 native peak bytes per pair (8.2%), meeting both adoption criteria.
Unit saves 15/11/19 ms and PW 25/20/37 ms; these smaller gains are retained
without promoting either to the 5% primary criterion.

Initial solver setup/solve medians are AW 0.01041/0.08473 → 0.01045/0.05869,
unit 0.00511/0.01643 → 0.00161/0.00914, and PW 0.00754/0.02373 →
0.00293/0.01092. The separate AW trace shows preparation nearly unchanged
(0.03026→0.03006 s), the enclosing fit/inference scope 0.28129→0.24325,
and IID statistics/covariance/certification 0.02397→0.02398. It therefore
locates the gain in native fitting, not ado, input or the covariance kernel.
There is no new dedicated copy/norm timer. Full
[work diagnostics](work-diagnostics.json) preserve the solver and runtime
arrays; aggregate low-memory iteration reports retain their existing maximum
over blocks, rather than being misrepresented as a total construction count.
No fallback or retry decision changes or fallback flags occur in these pairs;
discarded-attempt full-command costs were not newly measured.

All candidate primary peaks fit unchanged models: 91.49, 94.44 and 82.64 MB.
The narrow AW baseline exceeds its 91.49 MB model; ownership reduction brings
this candidate inside it. Scratch peaks remain 15.73, 21.63 and 62.72 MB.
Primary sampled RSS medians are 0.1947→0.1638, 0.1206→0.1182 and
0.2084→0.1646 GiB. RSS is a separate 10-ms sampled process-tree measure,
not a portable consequence of the native allocator counts.

## Protected controls and scientific validation

Single-pair command changes are small AW 0 ms, low FW interaction −2 ms,
saved interaction −6 ms, slope 0 ms, weak two-FE −5 ms, no-FE −4 ms and fast
IID −2 ms. None crosses the review envelope; these are protected diagnostics,
not repeatable control-speed claims. Saved output values also agree exactly.
Some process RSS observations rise: saved 0.0676→0.0857 GiB and weak
0.1421→0.1535. No universal RSS improvement is claimed.

Existing native/model discrepancies remain: low FW interaction is unchanged
at 8.77 MB versus 6.70 MB; weak LSMR falls 23.19→22.87 MB versus 9.41 MB.
Those are unresolved P0 model gaps, not passing memory qualification cells.
Other control native peaks fit their unchanged models.

All seventeen full b/V comparisons are exact, with zero maximum absolute
difference. Sample/status/rank/DoF, FE/cluster/subset tables and solver
family/iteration/retry/fallback decisions are exact. Full fit diagnostics are
exact in 7/17 and certificate exports in 5/17; floating certificate metrics
retain their original tolerances and every independent runtime certificate
passes. No tolerance, omission policy, weight scale or transaction rule changes.

[Validation](validation.json) records 32 distinct Rust tests selected by the
`low_` filter, including some matching overflow tests. The new test also ran
separately: unit/AW/FW/PW, normal and eight-times-larger explicit solver
divisors, matched reference scaling, pools 1/3/5, workers 1/7, large group
means, collinearity and FE-collinear controls. Existing wide weighted rank,
low robust/clustered, retained LSMR, PPML low-route and memory/failure tests
pass. Both artifacts pass the same
[independent Stata oracle](../../../../tests/stata/development/ols_low_one_fe.do):
interaction dummies, every legal selected weight/VCE combination, frequency
replication, common rescaling, rank-deficient controls and residual outputs.
Universal static audit and native arm64 ABI/self-test pass. The initial Rust
launch failed because `rustc` was absent from PATH, before any test executed;
the pinned-toolchain retry passed without assertion changes.

The [summary](summary.csv), [pair records](comparisons.json) and
[checksummed numeric archive](archive.json) preserve all measurements and
full scientific exports. The portable runners reuse the existing harness;
set `FEREG_LOCAL_BENCH_ROOT` to isolated baseline/candidate trees and
`PYTHONPATH` to `benchmarks/local_fixest_gap`. No licensed logs, binaries,
machine paths or unrelated evidence are committed.

This closes the remaining P2 fresh-block evaluation. Broader PPML passes,
inference reuse, centered tiles, saved recovery and P0 accounting remain
separate work under #27. No agents, upstream dependency changes, cluster
jobs, push or qualification occurred.
