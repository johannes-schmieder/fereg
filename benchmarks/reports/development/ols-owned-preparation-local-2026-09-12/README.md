# P0/P1: owned OLS preparation across weights and inference

Local development evidence for [roadmap #27](https://github.com/johannes-schmieder/fereg/issues/27).
Adopt the OLS ownership extension described below. This changes preparation,
not solver admission or scientific policy. PPML materialization remains open.

## Identity and evaluation

The baseline is `e93fe437f1d8855e1c3aa47ad03026eb23a9942d`, which follows
`79f1ba6` and includes the separate PPML reconstruction repair. Both arms were
built from isolated source trees with Rust 1.85.1, locked dependencies and the
normal universal macOS build, binary audit and actual arm64 ABI/self-test.
The working-tree `dist` plugin was not used or replaced.

- Baseline plugin: `b5e1f1a6471e1b38cc20057499504c03442ef687499ef2fed09167dcb2d72331`.
- Candidate plugin: `5a478f839cc3b21d2fb66ceb28ecfcc2c8555a711d83c52cc44489d49178e0b6`.
- [Identities and replayable patch](identities.json) bind the measured source.
- [Protocol](protocol.json) fixes the target and review envelope before timing:
  at least 10% complete-command gain on a wide preparation target; investigate
  any slowdown exceeding both 5% and 10 ms.

There are **41 completed pairs / 82 successful measured calls**: 18 primary
pairs, 12 stratified controls, five separate traced pairs and six rechecks.
All run sequentially in fresh Stata/MP 19 processes on the local M2 Ultra,
after an untimed small warm-up. Primary order rotates over three seeds;
controls initially run baseline first, with reversed/rotated rechecks where
needed. Native thread requests are separate from Stata's eight-processor
license cap. macOS affinity is uncontrolled. These are development findings,
not SCC, cross-platform, release or equal-accuracy fixest performance claims.

## P0 coverage and cost audit

`S` denotes distinct loaded model/FE/cluster sources, `N` the original axis,
`Nc` the candidate count and `Na` the initially retained count. Supplied
weights remain a separate raw column; unit weights remain structural.

| Actual preparation branch | Baseline | Candidate | Remaining work/cost |
|---|---|---|---|
| Simple absorbed terms, unweighted IID | Complete owned shortcut; incomplete mask and column compaction | Same mechanism and worker threshold | SPI input, factorization, singleton and solve passes |
| Simple terms, weighted IID | Owned entry immediately falls back to borrowed preparation | Classify and compact owned AW/FW columns | Raw-weight validation/normalization and weighted solve remain |
| Simple terms, robust, any legal weight | Borrowed complete copy or serial row classification/materialization | Same owned classifier/compactor as IID; PW retained as PW | HC1 score/cache/meat and independent certificate work |
| Simple terms, one/multiway clusters, any legal weight | Borrowed preparation with cluster source roles | Owned preparation with the same source roles | Cluster-only sources still factorize on the final singleton sample |
| Interacted, slope or saved absorbed terms, any legal VCE/weight | Borrowed synthetic availability request with pruning disabled; true topology/singletons afterward | Owned availability request; unchanged true topology/singletons/rebasing | Complex topology and saved-effect recovery passes remain |
| Ordinary factor/interacted controls; mixed continuous/categorical/cluster roles | Native source registry and generated-column recipes | Same registry/recipes, owned numeric buffers | Ordinary factor-domain validation and generated RHS evaluation remain |
| Fixed-width strings or restricted candidates | Candidate-gated main-thread scalar SPI, native string coding | Loading unchanged; resulting native columns use owned preparation | Batched candidate runs and mixed-source loading remain open |
| PPML | Allocation-free classification before generation-zero admission; borrowed materialization afterward | Unchanged | Post-admission ownership, adjustments and ado audit remain open |

The four OLS dispatch exclusions (robust, clustered, weighted and categorical)
were ownership/dispatch exclusions, not numerical requirements. The retained
restrictions on worker use, buffer capacity and PPML preflight are deliberate.
All six public OLS preparation branches now use `prepare_loaded_ols`, which
captures no SPI session and installs the existing requested team only at
`S >= 16` and `N >= 16,384`. No new pool or observation-sized workspace is
added. Smaller inputs stay serial.

The authoritative row classifier and original-order domain checks are reused.
Weights are compacted without arithmetic. Candidate exclusion, source-use drop
precedence, raw AW/FW/PW values, frequency singleton multiplicity, final row
order and cluster missingness before singleton deletion remain unchanged.
Categorical availability still keeps synthetic-source singletons; true
interaction/slope pruning runs exactly once. Cluster encoding still follows
the true final sample. Borrowed constructors remain for PPML and independent
comparison; there is no second classifier or numerical implementation.

For complete unweighted robust/clustered input, ownership eliminates a separate
`8*N*(S+1)`-byte prepared value allocation and its read/write copy. At the
one-FE/50-control target this is 408,944,640 bytes of value storage and
817,889,280 bytes of nominal copy traffic. These are source-derived payload
counts, not hardware memory-counter measurements. For weighted/incomplete
inputs, the classifier still reads source values; retained payload is moved
columnwise rather than pushed into many destination streams in a row loop.
Complete weighted input skips identity copies after classification. Sparse
missing input still reads and writes retained values; its win is locality,
parallel classification/compaction and shorter allocation overlap, not the
claim that all data passes disappear.

Reuse is bounded by each input vector's capacity being no greater than `Nc`;
otherwise compaction fallibly allocates only `Na` values. Existing singleton
compaction and solver memory admission are unchanged. The unit tests check
retained value bits and candidate-capacity bounds, including extreme supplied
weights and restricted inputs.

After preparation, both arms reach the same implementations: direct weighted-
capable one-FE projection; existing guarded unit-weight two-FE Schur/CG/CMG
alternatives; categorical slope/interacted topology; guarded finite solving
with QR fallback; packed/strided inference caches and cluster reductions;
bounded low-memory scratch; and existing saved-effect recovery/publication.
The weak, dense, bottleneck, disconnected and unbalanced controls report the
same CG decisions; the cycle reports the same CMG decision. Every tested
initial/final engine, iteration, pilot, fallback and preconditioner field is
exactly unchanged. No fallback flags occur in these selected captures.
Operator-construction counts, hardware traffic and separately accumulated
discarded fallback costs were not instrumented; that broader P0 item remains
open. Recorded setup/solve times remain in the numeric archive.

## Measured complete-command gains

Medians in seconds; the last column is the median paired change, which need
not equal the ratio of the displayed medians. No row excludes the outlier.

| OLS target | N / controls / native threads | Baseline | Candidate | Paired change |
|---|---|---:|---:|---:|
| Unweighted robust, sparse missing | 983,040 / 50 / 16 | 1.870 | 1.294 | -30.8% |
| AW IID, sparse missing | 983,040 / 50 / 16 | 1.197 | 0.641 | -46.0% |
| FW one-way cluster, sparse missing | 245,760 / 50 / 4 | 0.652 | 0.527 | -17.6% |
| PW robust, complete | 245,760 / 50 / 16 | 0.490 | 0.352 | -28.0% |
| Interacted FE, two-way cluster, sparse missing | 245,760 / 50 / 16 | 0.638 | 0.510 | -20.1% |
| Unweighted robust, complete | 983,040 / 50 / 16 | 1.338 | 1.262 | -5.1% |

Separate traces locate the change in preparation:

| Trace | Baseline preparation | Candidate preparation | Baseline fit/inference | Candidate fit/inference |
|---|---:|---:|---:|---:|
| Robust sparse missing | 0.6418 | 0.0435 | 0.8560 | 0.8753 |
| AW IID sparse missing | 0.5984 | 0.0417 | 0.2034 | 0.2052 |
| PW robust complete | 0.1532 | 0.0121 | 0.2343 | 0.2380 |

The clustered baseline lacks a distinct preparation trace; its input/native
result-wire timings and candidate preparation trace are retained without
inventing a baseline decomposition. Across primary medians, SPI input stays
about 0.30 seconds for the million-row cases and 0.077 seconds for the smaller
ones. Robust inference now dominates the robust target's native work.

Native allocation peaks are **unchanged in 35/41 pairs**, including every
primary pair; solver/inference peaks still dominate the action peak. The
restricted retained-pair control falls from 129,074,497 to 118,331,542 bytes,
the saved-output control from 11,554,802 to 10,999,826, and each small narrow
pair from 749,534 to 740,428. Modeled budgets are unchanged. Sampled process
RSS is lower on all six primary medians: roughly 1.780 to 1.448 GiB for AW
IID missing and 2.137 to 1.807 GiB for robust missing. RSS includes Stata and
allocator retention and is sampled at 10 ms; it is distinct from native live
allocation and is not a release memory claim.

The 12 controls preserve exact science. Complete IID changes from 0.565 to
0.570 seconds. Weak robust changes from 0.239 to 0.220. The worst difficult-
graph increase is the bottleneck control's 0.106 to 0.113 seconds (7 ms).
The small/narrow AW robust control increases from 5 to 8 ms; three additional
seeds give 7 to 9 ms medians. Most of that difference is outside native work;
the repeat native medians are about 2.08 versus 2.46 ms. This is a disclosed
small-model regression within the predeclared absolute review envelope, not
a claim of universal improvement. The restricted control improves 0.155 to
0.125 seconds; low-memory and saved-output controls also improve slightly.

The original AW candidate at seed 8675309 takes 1.973 seconds versus 1.210,
with input loading 1.494 seconds versus 0.307. The loading code is unchanged,
but its precise external cause is not established. Repeating that seed gives
0.623 versus 1.194; two fresh paired seeds give 0.614/1.172 and 0.617/1.177.
These repeats remain separate from the primary medians.

## Scientific and boundary validation

All 41 pairs have byte-identical full b/V, status, fit, DoF, FE, cluster/subset
and certificate exports; exact sample hashes; and exact solver fields except
times. Requested saved FE/residual values are also byte-identical in their
paired control. [Validation](validation.json) records:

- 160 focused core prepared-OLS tests and 40 plugin action tests (the latter
  filter also matches PPML transaction tests).
- 29 independent Rust oracle/preparation tests, covering HC1, cluster
  inclusion-exclusion, disconnected effects and frequency replication.
- Six distinct licensed Stata fixtures, plus the memory fixture in its
  constrained mode: the new 1/4/16-thread dummy oracle for weights, clustering,
  fixed-width string clusters, interacted effects and slopes; existing native
  input, transaction, memory, saved-output and PPML smoke checks.
- The constrained 543,870,912-byte limit leaves 7,000,000 usable bytes:
  fast preflight rejects its 7,323,528-byte model with `r(909)`; low and auto
  succeed against a 3,492,104-byte low estimate with matching science.
- The Python boundary screen passes 25/26 tests on both baseline and candidate.
  The same existing `test_ppml_session_uses_a_concrete_prepared_view_in_hot_loops`
  assertion finds `dyn PreparedOlsFitView` in unchanged `ols/prepared_fit.rs`.
  This work does not alter that source or weaken the checker.

Every licensed fixture has an explicit zero Stata return code and the required
unique terminal sentinel. Two initial harness attempts failed before target
timing (argument quoting and sandbox RSS access); the first restricted attempt
failed in Stata CLI parsing before the worker ran. [Harness failures](harness-failures.json)
preserve these separately from estimator outcomes. No campaign job was touched.

## Evidence and remaining work

[Summary CSV](summary.csv), [complete paired comparisons](comparisons.json),
[numeric evidence archive](numeric-evidence.zip) and [archive checksum](archive.json)
retain the measurements. The local worker and collector are
`benchmarks/local_fixest_gap/preparation_compare.py` and
`benchmarks/local_fixest_gap/collect_preparation.py`. Set
`FEREG_LOCAL_BENCH_ROOT` to an isolated directory containing the bound
`baseline` and `candidate` trees and `STATA_MP_PATH` to licensed Stata.
Use `--part primary`, `controls` and `profile`, then the collector. Rechecks
use `pair` with the recorded seeds and order. Synthetic generated inputs,
licensed logs and binaries stay outside Git; the archive holds numeric results
and hashes only. Restricted recipes use a generated candidate marker.

P1's OLS weight, covariance, cluster-ordering, complex-term and worker-reuse
items are adopted for development. PPML ownership/preflight/adjustment/ado
work and restricted/mixed SPI batching remain open. No weighted Schur or PPML
automatic region was added. P2 must assess the actual weighted operator,
especially tiny-weight bridges, around the existing engine. The one-FE PPML
work, inference ownership and stable centered tiles retain #27's priority
order. The current evidence does not justify changing weak-graph routes or
removing independent numerical certificates.
