# P3 categorical cluster certificate: evaluated and rejected

Reject the standalone representative-tuple certificate prototype against accepted
`257f4cee74ee0b3e80c55e1f1867b486e26410b5`. None of the three prospective primary
screens meets the joint 5% and 10 ms complete-command threshold. Accepted native
source is restored; no new automatic route, scientific rule or public interface
is adopted. The patch remains available for research in [prototype.patch](prototype.patch).

## Boundary and identities

The prototype replaces a second categorical cluster interaction factorization
with independent within-code tuple equality and strictly increasing representative
tuples. Those checks reject merged, split, missing, reordered and out-of-range
groups. Construction, final-sample codes, singleton/error precedence, weight kinds,
solvers and numerical inference certificates are unchanged. Simple clusters still
borrow final categorical sources. Workers receive no SPI session.

Its workspace is one `usize` per observed group. On the tested 64-bit platform,
8G bytes fit the replaced dense certificate's 4N code plus 4P map bytes because
G≤N and G≤P; they also fit the sorting fallback's 12N bytes. Existing component
views retain their lifetime, and no separate seen bitmap overlaps this workspace.
This payload comparison is a source-derived bound, not a measured action peak.

[Identities](identities.json) bind 179 inputs. Only
`crates/fereg-core/src/ols/prepared_categorical_clusters.rs` differs:

- Baseline plugin: `c64ead0763d662a8f7bd1c74f1058426ea03f90c871ff387e76a6fe935b2e3da`.
- Rejected candidate: `f8e1ccca1ffaf7b21dbce5abc9a777d9e7b90fafd85aa535f447177ef71271e5`.

Both artifacts are isolated from installed dist. The candidate compiled in its
own source directory for both architectures; universal static and actual arm64
ABI/self-tests pass. x86_64 execution was not performed.

## Complete command and phase evidence

The [prospective protocol](protocol.md) precedes timing. Three single-seed screens
rotate arm order in fresh local M2 Ultra Stata/MP 19 processes after small untimed
warm-ups. Dataset import/hash checking is outside estimation time. Native worker
requests differ from Stata's processor license limit; affinity is uncontrolled.

| Primary | N / K / workers | Baseline → candidate, seconds | Time reduction |
|---|---|---:|---:|
| FW OLS, dense interacted cluster | 245760 / 50 / 16 | 0.318 → 0.308 | 3.1% |
| PW OLS, sparse interacted cluster | 245760 / 10 / 4 | 0.172 → 0.170 | 1.2% |
| FW PPML, sparse interacted cluster, offset | 122880 / 10 / 4 | 0.363 → 0.358 | 1.4% |

All primaries include sparse missingness and `g1#c2` absorption. Sparse clusters
use shuffled distinct c1 labels, preserving missingness; their Cartesian product
exceeds dense factorizer admission. This recipe was fixed before timing. No extra
primary seeds were run after the failed screen.

The rejection branch adds four controls and two separate public phase profiles:
**nine pairs / eighteen successful measured commands** in total. Small AW slope
OLS is 23→22 ms, low AW interacted/saved OLS 74→77, weak two-FE PW OLS 62→60,
and explicit low-mobility three-FE PW PPML 90→83. The largest protected increase
is 3 ms/4.1%, below the prospective review envelope. Control gains are not repeated
performance claims.

The separate sparse OLS profile lowers preparation 0.054144→0.047738 seconds,
while fit stays 0.056651→0.056260; command is 0.171→0.166. Its score cache remains
usable (`reabsorbed=0`). PPML's existing preparation timer is
0.036818→0.036128, plugin time 0.344132→0.343761 and command 0.361→0.362.
That PPML timer has a narrower existing scope; no isolated cluster-certificate
cost is inferred from it. Profiles do not qualify the failed primaries.
[Phase review](phase-review.json) retains complete arrays, including warm-ups.

Primary native peaks are unchanged at 242.763/89.737/193.043 MB and fit their
unchanged selected models. Peaks are exact in seven of nine pairs. Weak OLS
changes 27.494→27.915 MB; three-FE PPML changes 19.328→18.949 MB. The causes
of those allocation differences were not traced. All reported models are exact.
PPML comparisons use action peak deltas; raw allocator peaks remain separately
available. Sampled RSS moves both ways, including increases in every primary;
there is no memory-saving claim.

Two controls have [observed/model gaps](model-gaps.json) on both artifacts:
small slope 1,388,568 versus 1,382,472 bytes, and weak OLS 27,494,384/27,914,968
versus 13,640,648. These are open P0 reporting/admission gaps, not successful
memory qualification. The rejected patch does not change reporting or solver
workspace policy. No PPML fallback, strict-profile rejection or step halving
occurs; OLS discrete solver fields match. Complete runtime tables retain setup,
solve, certification and scratch costs; no fallback-heavy benefit is claimed.

## Scientific checks and disposition

All nine pairs pass exact full b/V, sample/status/DoF, cluster/FE/separation and
certificate exports plus non-time solver decisions. Requested low-memory saved
FE/residual values agree exactly. Three focused unit tests pass on the final
prototype: row-order equivalence, forged-partition rejection, and allocation
failure/workspace bounds. Both artifacts pass the existing independent interaction
oracle: OLS FE-dummy weighted/full-covariance checks and full-dummy Poisson Hessian/
sandwich checks, with explicit zero return codes and terminal sentinels. The PPML
oracle uses the existing reported correction scalar; it is not an independent
DoF derivation. [Validation](validation.json) binds these results.

The initial missing Rust type annotation and an initial collector assertion are
preserved by log hashes. The latter incorrectly expected every control peak to
fit its model; final collection records both baseline/candidate gaps instead.
No numerical fit or timing pair was rerun. The collector explicitly separates
public profiles and action versus raw allocator peaks; final summaries use those
correct scopes.

[OLS comparisons](ols/comparisons.json), [PPML comparisons](ppml/comparisons.json)
and their [OLS](ols/archive.json)/[PPML](ppml/archive.json) checksummed numeric
archives retain every observation. The initial [probe](probe.md) and
[screen snapshot](screen.json) remain historical stages, not the final disposition.
Replay drivers are under [replay](replay/compare.py); use the shared
`benchmarks/local_fixest_gap` modules, an isolated `FEREG_LOCAL_BENCH_ROOT` and
licensed `STATA_MP_PATH`. Raw licensed logs, binaries and datasets stay outside Git.

The [consumer audit](consumer-audit.md) completes the bounded P3 cluster-state
reuse evaluation: existing consumers already share final codes and subset work,
and the identified remaining re-factorization alternative is rejected. Low-memory
QR score reconstruction remains separate P3/P5 work. No agents, SCC, qualification,
push or installed-dist replacement occurred; unrelated Veneto work is preserved.
