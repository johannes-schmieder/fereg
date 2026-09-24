# Canonical PPML phase ownership: local development evidence

This milestone adopts **canonicalownership2**, preserving ADR 0168's direct
current-weight Schur products while changing temporary ownership and admission
according to [ADR 0169](../../../../docs/adr/0169-canonical-products-phase-ownership.md).
Wide low two-categorical-FE PPML reaches the existing direct engine within the
unchanged memory allowance. Whole-public speed improves 32–35% in three fresh
parallel cases and about 5% in the serial wide case. All scientific checks pass.
The broader goal of <=0.90× comparable fixest with at least two FE remains open.

## Exact identities and scope

| Artifact | Source | Plugin SHA-256 |
|---|---|---|
| Baseline `canonicalproducts1` | `432d5a6d1651c954843891954325d5b2b14050f7` + five identity-listed overlays; captured computation matches accepted `341477b5` | `04dc0eb35f02afd0bcc874e7b46368569411ec05d31f1e49e71ec7ee9d7f8418` |
| Intermediate `canonicalownership1` | `341477b58d802f039b0a48d0109615e3f11f03a0` + canonical_products.rs | `be1b17793eb1b8fb6b6bb392cee74516a5c41447d84dad2b769d54135befd3e6` |
| Adopted `canonicalownership2` | same base + canonical_products.rs | `3fcc35a1759e78d451cda4dc01a9d22d167cc46a79af848ba3a29c9ccea5bc0b` |

All 548 candidate captured inputs match final working sources. Build logs,
source hashes, full captured inputs, declarations, controllers and test receipts
are archived. Each build passes its ABI self-test. The intermediate candidate
ended setup/solve lifetimes but still retained an unnecessarily wide RHS bank;
source/lifetime review split the bank before public timing. It has no headline
performance claim. There were no source changes after candidate freeze.

Stock fixest 0.14.2 uses the pinned unmodified library with `glm.tol=1e-10` and
`fixef.tol=1e-11`. Source archive SHA-256 is
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
All PPML calls request robust inference and fitted means, matched rows,
regressor/FE designs and thread settings. BLAS threads are one. Native low mode
is compared against stock's supported execution; this is not an equal-memory
reference experiment. No instrumented reference supplies headline timings.

Times are the harness's whole public command, including loading, preparation,
estimation, inference and requested output. Fresh processes execute rotated
baseline/candidate/stock calls. Setup of external data and post-run audits is
outside timing. Native phase clocks provide attribution but are not additive
with one another or with the public timer. No compilation, profiling, archive,
or independent fit ran alongside headline calls. All application, overlap and
monitor checks pass; no timeout or application failure was discarded.

## Fresh validation

Seed **202609380 is consumed/frozen**. Ten cases were declared before generation,
with the candidate plugin frozen. Five rotations per case give 150 public calls.
All 150 public-state certificates, 50 native pairs, 50 baseline/stock pairs and
50 direct candidate/stock pairs pass. Values below are median [minimum–maximum]
in milliseconds; paired ratios use the corresponding rotation, not ratios of
marginal extrema.

| Case | Baseline ms | Candidate ms | Stock ms | C/stock medians | Paired C/B median [range] |
|---|---:|---:|---:|---:|---:|
| 0: N30720/K74/T8 | 459 [457–472] | 306 [302–307] | 449 [440–461] | 0.682 | 0.665 [0.640–0.669] |
| 1: N57600/K84/T8 | 904 [902–912] | 586 [582–593] | 830 [815–832] | 0.706 | 0.650 [0.640–0.655] |
| 2: N48000/K92/T1 | 1782 [1777–1797] | 1700 [1690–1712] | 3968 [3964–4059] | 0.428 | 0.952 [0.944–0.963] |
| 3: N19200/K72/T8 | 314 [308–321] | 208 [207–210] | 267 [259–270] | 0.779 | 0.666 [0.654–0.672] |
| 4: N20160/K98/T8 | 472 [470–478] | 477 [474–477] | 402 [395–407] | 1.187 | 1.011 [0.996–1.015] |
| 5: N20160/K32/T4 | 101 [100–102] | 101 [100–108] | 135 [131–138] | 0.748 | 1.000 [0.990–1.080] |
| 6: N23040/K16/T4 unbalanced | 80 [80–83] | 80 [79–81] | 70 [65–74] | 1.143 | 0.988 [0.976–1.012] |
| 7: N21120/K14/T4 disconnected | 88 [84–89] | 88 [86–89] | 68 [67–73] | 1.294 | 1.000 [0.966–1.060] |
| 8: N19200/K12/T4 slopes | 100 [98–102] | 101 [99–103] | 122 [118–124] | 0.828 | 1.010 [0.990–1.051] |
| 9: N20160/K10/T4 3FE | 85 [83–86] | 85 [83–86] | 254 [246–256] | 0.335 | 0.988 [0.976–1.024] |

All five native pairs improve for cases 0–3. All paired stock ratios for these
four cases are below 0.90. Seven of ten fresh medians reach the target, including
previously fast paths. This is a selected development screen, not a coverage-wide
win rate. Cases 4, 6 and 7 remain slower than stock. Cases 4 and 8 add 5 and 1 ms
to their baseline medians; unchanged-path variability is retained, not attributed
to an algorithmic improvement. The previously admitted K32 development case also
adds 2 ms. Do not infer zero regression from these small samples.

The candidate and baseline scientific CSV exports are byte-identical on fresh
cases 4–9. Newly admitted cases 0–3 differ in their recurrence and pass the
registered independent and numerical envelopes; bitwise equality is not required.
The passing disconnected comparison here does not adjudicate the prior seed-378
stock discrepancy, and historical default-reference accuracy failures remain.

## Development and independent science

The fixed development seed is 202609347, three rotations (45 calls):

| Case | Baseline ms | Candidate ms | Stock ms | C/stock medians | Paired C/B median [range] |
|---|---:|---:|---:|---:|---:|
| 0: N24720/K72/T8 | 370 [370–372] | 248 [248–249] | 333 [328–335] | 0.745 | 0.670 [0.667–0.673] |
| 1: N49200/K80/T8 | 741 [741–742] | 489 [481–493] | 649 [643–649] | 0.753 | 0.660 [0.649–0.664] |
| 2: N49200/K96/T1 | 1937 [1918–1937] | 1857 [1854–1859] | 4365 [4349–4382] | 0.425 | 0.960 [0.959–0.967] |
| 3: N16320/K32/T8 | 80 [80–82] | 82 [81–83] | 89 [86–93] | 0.921 | 1.012 [1.012–1.025] |
| 4: N16320/K18/T4 unbalanced | 66 [64–67] | 66 [65–66] | 57 [54–58] | 1.158 | 0.985 [0.985–1.031] |

All 45 public states, 15 native pairs, 15 baseline/stock and 15 direct
candidate/stock comparisons pass. The 72/80-control cases showed the opportunity
before the fresh declaration. No thresholds were tuned to fresh outcomes.

A separate N6240/K72/T4 explicit-design Poisson oracle independently constructs
FE columns, fits the objective, projects the final weighted design, and forms
Hessian and robust sandwich covariance. All three estimator states pass; FE rank
is 1065, residual DoF 5103 and the oracle takes five Newton iterations. Candidate
maximum errors are 7.74e-13 for coefficients, 4.53e-17 for covariance, 1.47e-10 for
means and 7.28e-12 for log likelihood. Its one rotation is scientific evidence,
not a headline timing distribution. Across oracle/development/fresh there are
198 passing public states and 66 passing native pairs.

Thirty-five distinct focused core tests pass on the final source, including two
new ownership/admission tests: exact ledger release/reuse and a wide weighted
known-solution oracle that fits the new phase bound but fails the old sum.
Both unit and changing weights reach the direct session route; one byte below
the private plan refuses it. Existing tests cover extreme weights/scales,
rank/strict rollback, scratch poisoning/cleanup, joint categorical and slope
sessions, low-memory refusals, rebuilt designs, domain errors and interruption.
One plugin test independently proves a one-byte whole-action budget rejects
before canonical numeric allocation and publishes nothing.

Five licensed fixtures pass with terminal sentinels and Stata rc0: canonical
Poisson/Hessian/robust oracle, categorical allocation audit, multipartition
slope oracle, generated-factor oracle, and three-FE OLS oracle. These include
excluded engines and generated sources. This milestone does not repeat every
prior weighted/cluster/output fixture or the full suite.

The first public oracle declaration requested N6144, incompatible with the
shared generator's 240-row graph dimensions. It failed before any estimator
ran. That failure and original declaration are retained. Corrected `oracle2`
uses N6240; the not-run initial development declaration was similarly replaced
by `development2` with compatible rows. These are generator-shape corrections,
not sample/tolerance changes within an executed comparison. The custom Rust
N6144 construction has its own valid dimensions and is unaffected.

## Attribution and memory

This is a route-availability improvement. Explicit drops end setup matrix/graph
scratch, RHS reductions, factor/cross banks and solved coordinates at their last
use. Certificate reductions are allocated only when needed. Weight storage
becomes response storage only after all weight-dependent acceptance checks.
Actual capacities are charged; release consumes the owner. The maximum of
setup, solve and check payloads replaces an all-live sum, without changing the
structural work bound or the `16*N*pool` allowance. No certificate was removed.

| Fresh case | Old required bytes | Phase peak bytes | Existing allowance | Work admitted | Memory admitted |
|---|---:|---:|---:|---|---|
| 0 | 5,153,840 | 4,486,192 | 4,915,200 | True | True |
| 1 | 10,810,848 | 9,090,528 | 9,216,000 | True | True |
| 2 | 8,179,760 | 7,028,912 | 7,680,000 | True | True |
| 3 | 3,235,584 | 2,882,304 | 3,072,000 | True | True |
| 4 | 4,604,912 | 4,193,648 | 3,225,600 | True | False |
| 5 | 1,562,048 | 1,248,176 | 3,225,600 | True | True |
| 6 | 665,136 | 363,488 | 3,686,400 | True | True |
| 7 | 3,100,624 | 2,201,824 | 3,379,200 | False | True |

These are replays of the initial structural plan on the captured data; all rows
are retained. Numerical acceptance and live projection capacity are separately
checked at runtime. Case 4 still needs more than its allowance. Case 7 has enough
memory but fails the unchanged dense-factor work bound. The summary includes
all phase peaks and structural work counts, not just admissions.

| Fresh case | Projection ms B→C | Finite ms B→C | Strict final ms B→C | Inner diagnostic count B→C |
|---|---:|---:|---:|---:|
| 0 | 108.70→60.32 | 53.74→25.79 | 137.10→55.50 | 63→47 |
| 1 | 226.49→131.26 | 98.76→48.38 | 288.94→112.78 | 57→42 |
| 2 | 676.02→699.77 | 206.01→157.56 | 532.53→466.81 | 54→41 |
| 3 | 69.68→35.96 | 32.73→17.22 | 93.22→36.30 | 54→41 |

Each of these has four outer iterations in both versions. Inner counts mix
direct solves and iterative steps and are not operation-equivalent units.
The measured phase reductions localize the parallel gain to projection,
canonical finite products and strict final work. The serial direct product
phase has less leverage: projection itself increases while finite and strict
work fall. No fixest internal attribution is inferred from its total time.

Newly admitted cases increase whole-action tracked peaks by 1,235,952,
3,030,848, 1,952,152 and 832,736 bytes, respectively, over the old fallback.
Already-admitted cases 5 and 6 reduce peaks by 313,608 and 99,703 bytes.
Modeled budgets and scratch sizes are identical across baseline/candidate.
The maximum candidate tracked/model ratio is 0.8753 in this fresh screen.
The complete memory and phase distributions are in `summary.json`; these are
local tracked-allocation diagnostics, not constrained-memory qualification.

## OLS controls and remaining work

Three baseline/candidate rotations of four public OLS controls (24 calls) all
pass science, requested outputs and overlap checks. Medians B→C: one-FE robust
with all outputs 17→17 ms, categorical two FE 29→29, low two-partition slopes
105→104, low three FE 62→62. Individual native ratios span 0.967–1.063;
all distributions are retained. The production OLS route is not modified.

The next bounded target is componentwise organization for disconnected Schur
systems. The current global Q³ work bound prevents admission even when separate
components may be cheap. A successor must change actual factor work and its
bound together, retain original-design certificates and validate fresh graphs;
merely loosening this guard is not justified.

Unbalanced case 6 remains 80 versus stock 70 ms, with projection about 17 ms,
strict final about 16 ms and preparation about 8 ms. Inference is only about
1.2 ms. The outstanding work is canonical/joint/preparation overhead, not an
inference micro-optimization. K98 remains constrained by a 4,193,648-byte phase
bank versus 3,225,600 available; narrower certificate/RHS organization may help,
but raising the budget would not answer the low-memory problem.

Preserve the 77/16/111 frozen definitions, consumed seeds, static allocation-
inventory drift, weighted constrained-fit and nonretaining-CG failures, prior
comparator discrepancies and platform deferrals. This evidence authorizes no
release, push, SCC retry or restart of stopped campaigns.

## Receipt restoration

`receipts-content-addressed.zip` contains immutable sanitized receipts with
original and archived hashes, full source snapshots, declarations, failures,
raw per-call numeric exports, controls, checks and reproducible controllers.
Data and binary payloads are omitted with hashes; licensed logs are replaced
by status files and protocol sentinels. Paths are sanitized. Restore and verify:

```sh
python3 restore_receipts.py /tmp/fereg-canonical-ownership-receipts
```

The sibling SHA file authenticates the container; the helper verifies every
logical payload. Archive counts and the independently verified restoration are
recorded alongside this report. Historical reports are unchanged.
