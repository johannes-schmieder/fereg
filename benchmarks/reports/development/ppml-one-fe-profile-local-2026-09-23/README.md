# Concentrated one-FE Poisson development milestone

Local macOS arm64 development evidence, September 23, 2026. Accept `oneprof6`,
based on `386b2170`; all **534 computational inputs** match the final source.
Plugin SHA-256:
`c1aa681c6c160d60381d8483b783a1bbed057c63d4f0d81c89f9e02485ac67b3`.
Baseline is accepted `slsymbol2`, plugin
`f86fd3e0b9864874df5a5700233e7e5a1b9bce347181d4772e07982db6159740`.
This completes one bounded milestone; the public PPML 0.90-times-stock objective
remains active and no qualification was run.

## Algorithm and scope

[ADR 0127](../../../../docs/adr/0127-profiled-one-fe-poisson.md) introduces analytic
group-intercept optimization at every finite-beta trial, centered gradient and
Hessian accumulation and safeguarded finite-dimensional Newton steps. Retain the
canonical initial rank decision, independent outer scores, two consecutive
convergence passes, strict canonical WLS and final predictor-response/Hessian
reconstruction. When strict WLS confirms the accepted point, retaining that exact
point lets final synchronization reuse same-weight controls. Fresh acceptance
checks remain mandatory; no tolerance, sample, weight, VCE, DoF, separation or
failure-atomicity policy is relaxed.

The optional production owner serves actual one-intercept-FE, fast, MAP-owned
publication fits. Slopes, multi-FE, low memory, synthetic no-FE constants, other
explicit engines and mu separation remain excluded. Workspace and numerical
refusal preserve the independently certified canonical route. This replaces
ordinary repeated response/control absorption on the admitted path; it adds no
new Stata command or FFI computation.

## Fresh public timings

Predeclared seed **202609324**, three alternating baseline/candidate/stock
rotations. Times are milliseconds, median [minimum, maximum]. Public timers
include loading/preparation, estimation, robust inference, fitted means and
publication. Data generation/import, process startup, warmup and exports are
outside. All timed applications were uncontended by builds, profiles, archiving
or other fits; overlap and monitor checks are clear.

Stock is the unmodified pinned fixest 0.14.2 archive, SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched application threads,
single-thread BLAS and the existing comparable robust covariance normalization.
Diagnostic artifacts contribute no headline clocks.

| One-FE case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| N61440 K10 T4 | 147 [144,147] | 129 [129,130] | 53 [50,54] | 2.43 |
| N61440 K10 T1 | 194 [188,196] | 172 [166,177] | 98 [92,98] | 1.76 |
| N30720 K32 T4 | 186 [185,187] | 151 [148,155] | 116 [114,119] | 1.30 |
| Unbalanced N61440 K10 T8 | 140 [139,140] | 129 [125,129] | 49 [47,50] | 2.63 |
| Small N3840 K3 T1 | 25 [25,25] | 20 [18,23] | 11 [10,11] | 1.82 |
| Larger N245760 K6 T8 | 474 [474,496] | 406 [401,426] | 158 [150,162] | 2.57 |

Native median improvements are **12.2%, 11.3%, 18.8%, 7.9%, 20.0%, 14.3%**.
All public states and baseline/candidate/stock pairs pass. Both fresh small
explicit-design Poisson/Hessian pilots pass for both native artifacts and stock.
Every final headline cell remains slower than stock. For K10/T4, reaching 0.90
would require another 81.3 ms, approximately 63% of current public time. The
one-FE nonlinear step alone does not explain or close that remaining gap.

## Attribution and development experiments

All earlier development timings use consumed seed321 and remain source-bound.
`oneprof2` introduced the ordinary profiled step; its K10/T4 medians were
152/133/53 ms baseline/candidate/stock. `oneprof3` retained the independently
confirmed accepted point, giving 148/128/54 ms on the same development design.
These blocks do not measure final `oneprof6`. The receipt summary retains every
cell and repetition, including excluded low-memory losses in both blocks.

A separate `oneprofdiag2` build on the actual K10/T4 development FE model reports:

| Work | Diagnostic elapsed ms |
| --- | ---: |
| Initial retained design and full rank solve | 18.64 |
| Initial design construction within that phase | 10.14 |
| Initial full rank solve within that phase | 8.49 |
| Strict candidate solve | 17.08 |
| Strict response/control reprojection within that phase | 5.16 |
| Strict norm refresh within that phase | 2.53 |
| Strict full rank solve within that phase | 7.99 |
| Final predictor-response solve, identical weights | 11.23 |
| Final full rank solve within that phase | 8.42 |

Nested times must not be added as independent costs. Ordinary iterations 2 and 3
already pass the outer score with maxima 8.58e-16 and 5.18e-17. Final weights
are bitwise identical, and no control reprojection occurs in final synchronization;
the finite system is nevertheless rebuilt. These measured duplicate construction
costs support a shared final-state owner. They cannot by themselves explain the
entire remaining public gap. No instrumented stock attribution was collected
in this milestone; previous pinned-source investigations remain separate.

Final-artifact native phase records reinforce the broader cost budget. A fresh
K10/T4 application spends 35.83 ms in strict-final work, 15.75 in line search,
15.82 in preparation and 9.69 in publication. The larger N245760/K6 application
spends 131.34, 51.19, 53.63 and 39.22 ms in those respective phases. These are
individual native phase records, not decompositions of the public median.

The real allocation audit initially observed one allocate/free event despite
stable vector capacities. An isolated fixed-counter diagnostic locates it inside
profile direction preparation. Pinned Rayon/crossbeam source shows that external
job submission can allocate another queue block. This is a supported explanation,
not an allocation stack trace. Coarsening dependent work from seven to two external
jobs per ordinary iteration makes the unchanged T1/T4 real audits pass and avoids
repeated scheduling. It is not a guard fitted to the audit window or benchmark.

## Science, excluded controls and visible failures

Final-source checks pass **134 distinct Rust tests**: 132 prepared-PPML boundary,
transaction, separation, rank and solver tests, plus two concentrated-step tests.
The latter compare against a separately assembled full finite-plus-dummy WLS
system across T1/T4, unequal weights and common scales 1e-40/1/1e40; they also
check analytic group balance, rank refusal, zero-outcome groups and dimension
overflow. Actual-plugin ABI/panic checks pass.

Four licensed fixtures pass explicit Stata rc=0, process rc=0 and terminal
sentinels: weighted explicit-dummy one-FE Poisson inference, wider weighted
one-FE passes, separation, and the new allocator/output fixture. Coverage includes
unit/frequency/probability weights, offsets, robust/cluster inference, T1/T4
sixteen-iteration allocation windows, saved effects and a zero-control analytic
weighted-group oracle. The earlier fixture error used eta=xb+d+offset; ADR 0016
defines xb to include offset. Correcting the new fixture to eta=xb+d preserves
the existing public contract. Both erroneous attempts and corrected receipts remain.

Fresh excluded native medians, baseline/candidate ms, two pairs:

| Case | Baseline | Candidate |
| --- | ---: | ---: |
| One-FE low memory | 179.5 [179,180] | 183.5 [181,186] |
| Dense three categorical FE | 201 [200,202] | 201.5 [200,203] |
| Degree-six two categorical FE | 111.5 [111,112] | 115.5 [115,116] |
| Two slope partitions | 177.5 [177,178] | 176 [175,177] |
| One-FE OLS robust/full outputs | 20.5 [19,22] | 20 [20,20] |
| Three-FE OLS robust | 44.5 [43,46] | 45 [44,46] |

The two larger excluded losses receive five additional alternating pairs on
identical numerical input hashes: low memory **187 [172,197] / 188 [178,203]**,
degree-six **113 [113,117] / 113 [110,115]**. Preserve both blocks. The initial
degree-six loss does not reproduce; low memory retains a 1-ms median loss with
overlapping ranges. All **22 excluded native pairs** preserve exact scientific
exports. This bounded screen does not establish universal non-regression.

Across earlier and final public screens, all **206 applications** pass with no
timeouts, build/fit overlaps or monitor errors. Scientific checks are separately
captured. Initial overly broad no-FE routing failed three driver tests; narrowing
to actual ordinary absorbed FE corrected it before headline measurements. An
audit integration initially asked an invalidated linear session for its old fit;
retaining the already frozen active indices corrected that ownership error.
The obsolete `estimate_ppml_hidden_allocation` fixture fails the same expected
wire-minor assertion on both accepted baseline and prototype; this milestone
does not repair or count that fixture as passing. Its preceding rollback checks
and the new passing real audit are distinct evidence.

The first diagnostic instrumenter failed before compilation at a stale probe
marker. The next controller accidentally omitted the categorical absorb list;
its protocols describe no-FE fits and are excluded from the attribution above.
Corrected `profile-fe-*` protocols contain the intended model. All failed
recipes, captured sources and results remain in the archive.

## Memory and remaining work

| One-FE case | Baseline tracked bytes | Candidate tracked bytes | Increase MB |
| --- | ---: | ---: | ---: |
| N61440 K10 T4 | 32,882,510 | 39,688,894 | 6.81 |
| N61440 K10 T1 | 31,548,206 | 38,354,590 | 6.81 |
| N30720 K32 T4 | 34,549,396 | 44,136,068 | 9.59 |
| Unbalanced N61440 K10 T8 | 31,283,682 | 37,690,322 | 6.41 |
| N3840 K3 T1 | 1,353,725 | 1,553,437 | 0.20 |
| N245760 K6 T8 | 104,986,912 | 123,874,320 | 18.89 |

Modeled peaks exceed these measured peaks. The optional owner adds an N-by-K
copy and other retained trial buffers; it is explicitly excluded from low
memory. A successor should share prepared finite storage and eliminate duplicate
same-state construction while retaining independent final acceptance. The prior
low-memory finite/strict and slope RHS bottlenecks remain substantial. No claim
is transferred from this one-FE result to those unchanged paths.

Static allocation inventory drift, weighted constrained-fit failures, generic
nonretaining-CG instability, older reference discrepancies and Linux/SCC/physical
cap deferrals remain open. Frozen 77/16/111 definitions remain unchanged. No push,
release, SCC authentication retry or stopped campaign restart occurred.

## Receipts

The content-addressed archive preserves exact identities and source snapshots,
declarations, successful and failed controllers, every timing distribution and
science audit, focused test logs and separate diagnostic protocols. Raw licensed
logs, binaries and generated datasets are excluded; omitted-data hashes remain.
The restoration script verifies the container and every logical receipt before
extracting to a new directory. `verification.json` records the completed restore.

Sealing yields **11,494 logical files**, **2,069 distinct payloads**, SHA-256
`c9b6d302f392429be442c67b5eb115884d510469343f1e6020a9c2d62a85b691`.
The first privacy guard matched its own source-code string literal. Reviewing
the payloads and restricting the guard to actual banner-field lines resolved
that false positive; the failed sealer and its result remain captured. An early
restore attempt preceded completion of sealing and found no helper; the final
verification receipt records the subsequent complete restore.
