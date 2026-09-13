# P3/P4 wide PPML covariance pivots: adopted local checkpoint

Adopt [ADR 0058](../../../../docs/adr/0058-exact-ppml-jacobi-pivot-cache.md).
Above 64 identified covariance dimensions, the existing maximum-pivot Jacobi
algorithm caches upper-row maxima in its already required eigenvalue vector.
Each rotation invalidates affected maxima, rescans those rows, and updates the
two changed upper entries of other rows. Strict greater-than comparisons retain
the exhaustive scan's first-row/first-column tie order, including signed-zero
stopping behavior. Final diagonal values replace the temporary maxima.

The rotation arithmetic, convergence threshold, iteration cap, PSD/clipping
policy, rank decisions, output reports and independent certificates are unchanged.
The cache belongs to one eigensystem, with no cross-matrix or changing-weight
reuse. There is no new heap allocation, retained workspace, worker team or SPI
call. The eigenvalue vector moves earlier within its existing modeled overlap
with the work matrix and eigenvectors. Up to 64 dimensions uses the original
exhaustive search. All validation still precedes transactional publication.

## Baseline and cost attribution

Accepted baseline is `d9aa8c01f8bc94395fe84edd1d6e401f2603aacb`. The
[identities](identities.json) bind 176 source/ado/fixture inputs, locked dependencies,
the [exact patch](implementation.patch) and isolated universal plugins:

- Baseline: `d1e8efc290851ace10f917ac3cb630415bfaa975a2da588dead4a85b87fa2abc`.
- Candidate: `c64ead0763d662a8f7bd1c74f1058426ea03f90c871ff387e76a6fe935b2e3da`.

The changed crates visibly compiled for both architectures. Every measured
candidate input matches the worktree. The installed plugin was not used as a
baseline or replaced. No separate Veneto work was changed.

The [timing-only baseline diagnostic](diagnostic-summary.json) uses actual
identified covariance matrices from the accepted wider-moment experiment.
At K129, 32,848 pivot searches scan 271,193,088 upper entries: search takes
0.333 seconds of the 0.352-second eigensystem. At K257, 78,432 searches scan
2,580,099,072 entries: search takes 2.971 of 3.075 seconds. Rotation counts are
one lower because the final search proves convergence. These are measured
logical scan counts and instrumented phase times, not hardware traffic counts
or complete-command timings.

A separate [uninstrumented standalone comparison](standalone-summary.json), run
after all command measurements, times the whole published-covariance inspection,
including any clipped comparison reconstruction. It falls 0.358→0.043 seconds
at K129 and 3.118→0.293 at K257. Full inspection reports are exactly equal.
These standalone scopes differ from the internal eigensystem diagnostic;
neither is pooled into the command evidence below. Matrix identities and the
standalone sources are in [independent evidence](independent-archive.json).

## Complete-command evidence

The [prospective protocol](protocol.md) fixes nine primary pairs over three seeds,
nine protected pairs and one separate private allocation audit. All **19 pairs /
38 measured commands** pass. Fresh local M2 Ultra Stata/MP 19 processes use a
small untimed warm-up, rotated arm order and 4 or 16 requested native workers.
Affinity is uncontrolled. Builds, tests and standalone profiling do not overlap
command timing. This is local development evidence, not qualification or an
external-estimator comparison.

| Public target | N / K / native workers | Command medians, seconds | Median paired change |
|---|---|---:|---:|
| Unit weights, one FE, robust | 122880 / 65 / 4 | 1.637 → 1.597 | −2.4% |
| FW, exposure, sparse missing, robust | 61440 / 129 / 4 | 3.148 → 2.264 | −28.0% |
| PW, offset, sparse missing, two-way cluster | 30720 / 257 / 16 | 14.784 → 4.072 | −72.3% |

Every primary pair improves. Adoption rests on the repeated K129/K257 gains;
K65 does not meet the prospective 5% and 20 ms complete-command threshold.
The changes are located in the modified inference and publication consumers:

| Target | Inference medians, seconds | Publication medians, seconds |
|---|---:|---:|
| Unit K65 | 0.11340 → 0.07934 | 0.04055 → 0.02526 |
| FW K129 | 0.81043 → 0.22988 | 0.34735 → 0.05458 |
| PW K257 | 8.60887 → 0.82690 | 3.23883 → 0.31985 |

Finite/strict solving and preparation remain close. Independently computed and
overlapping phase medians must not be summed; median paired percentages need
not equal ratios of displayed command medians. All observations, phase fields,
iterations, retries and memory models remain in [comparisons](comparisons.json),
[summary](summary.csv) and the [checksummed numeric archive](archive.json).

Native action peaks are exact in 18/19 pairs, including all primary pairs:
364.49/287.97/253.09 MB for K65/K129/K257. All modeled phase budgets are exact,
and all observed action peaks are admitted. The explicit low-mobility three-FE
control changes 38,484,962→38,509,677 bytes against a 58,376,842-byte model
(+24,715 bytes, 0.064%); its event counts also differ. The allocation source of
that small peak difference was not traced. No peak equality is claimed for it.

Primary sampled process RSS is 0.349→0.362 / 0.378→0.342 / 0.332→0.349 GiB.
RSS includes Stata and allocator retention and moves both ways; there is no
memory-saving claim. The source introduces no new heap payload.

The [protected review](control-review.json) covers unchanged complete K50 and FW
K50, low FW K10, small K2, saved/separated K3, weak degree-three **two-FE** PW
clustered K65, explicit low-mobility **three-FE** K65, one-FE K64 and no-FE K2.
Largest slowdown is small/narrow 27→29 ms (+7.4%, +2 ms); none crosses the fixed
max(10 ms, 5%) review envelope. Weak and three-FE commands fall 469→412 and
430→379 ms. Those are single protected pairs, not repeated graph-speed claims.
All reported fallback counts, step halvings and strict-profile rejections are
zero in all 19 pairs; no fallback-heavy speedup is claimed.

The separate private allocation audit falls 19.344→8.623 seconds, outside public
aggregates. Both arms retain zero allocation events and zero live/peak growth
across 16 audited iterations, stable PPML/linear/scratch capacities, and passing
fresh/warm and allocation-free certificates. [Full audit fields](allocation-audit.json)
remain available.

## Scientific and boundary checks

Every scientific CSV is byte-identical across the 19 pairs, including full b/V,
sample receipts, rank/status/DoF, FE/cluster/subset/separation tables, PSD reports
and certificates. The scientific comparator also checks discrete solver fields
and saved outputs. [Science review](science-review.json) distinguishes exact
numerical fields from the one nonexact allocation peak.

[Validation](validation.json) records 49 distinct core tests and 22 distinct PPML
plugin tests. The new pivot test compares every cached choice with an exhaustive
scan through 512 rotations on dimensions 1/2/7/63/64/65/129/257, dense/tied/sparse/
diagonal patterns, signed zero and scales 2^-400/2^400. Each updated row maximum
is independently rescanned and capacity checked. Existing closed-form spectral,
forged-report/indefiniteness, VCE, memory formula and transaction tests pass.
Plugin checks include phase allocation failure, interruption, receipt-last
publication and requested worker-team switching.

Both artifacts pass the independent full-dummy Poisson/Hessian/sandwich licensed
fixture with explicit r(0) and terminal sentinels. The candidate also passes the
unchanged original PW K257/N7680/seed104730 functional request and its independent
NumPy profiled-Poisson/full-CGM oracle: maximum b/V errors are 3.97e-15/2.87e-16.
It retains 7,651 rows, 1,536 FEs, 64/3/192 subset groups, 257 identified slopes,
the normalized constant and covariance rank 65. As in the accepted PSD repair,
near-zero eigenvalues produce different raw negative counts in NumPy and native
arithmetic (159 versus 185); the independent covariance agreement, not count
parity between eigensolvers, is the oracle. Native baseline/candidate PSD reports
are exact. The cold functional call is excluded from performance aggregates.

Universal static and actual native arm64 ABI/self-tests pass. The expected
injected ABI panic is contained and followed by the PASS sentinel. The x86_64
slice was built but not executed. No estimator failure or numerical test was
rerun. A read-only identity checker initially used the wrong JSON field name;
the corrected check verifies all 176 inputs. The initial collector missed one
plugin test whose successful marker was interleaved mid-line with child-process
output; its terminal summary already reported all 22 passing. Parsing markers
independently of line starts recovers the complete distinct list without reruns.
Both collection attempts and original logs are retained locally.

P3/P4's actual wide covariance cost is reduced, but cluster-state reuse,
low-memory duplicate residualization, remaining P4 construction/predictor/state,
P5 and the other open #27 packages remain separate work. This change does not
close an unrelated roadmap checkbox. Complete #27 before the authorized two
independent agents, upstream CMG review and later small SCC campaign. No agents,
SCC, qualification, push or installed-dist replacement occurred.

Replay copies the drivers beside the isolated source/run trees and uses the shared `benchmarks/local_fixest_gap` modules on `PYTHONPATH`,
`FEREG_LOCAL_BENCH_ROOT` for isolated source/run trees and `STATA_MP_PATH` for the
licensed executable. [compare.py](compare.py), [collect_results.py](collect_results.py)
and [profiled_oracle.py](profiled_oracle.py) preserve the prospective workload and
independent calculation. Numeric archives exclude licensed logs, binaries and
raw datasets.
