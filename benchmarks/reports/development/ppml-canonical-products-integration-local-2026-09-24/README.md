# Low two-FE canonical PPML products: production integration

Decision: **adopt**, under [ADR 0168](../../../../docs/adr/0168-fused-canonical-ppml-products.md).
This milestone integrates ADR 0167's selected fused direct products into the
public estimator. The goal of <=0.90 times comparable stock fixest on two-plus-FE
PPML remains active; three fresh families still lose.

## Source and receipts

Baseline is accepted computation `2c34503d`, artifact `jointpanels2`, plugin
SHA-256 `4d4c669d6716583b14be437e2732133d61f733840b610414429d8df5614168b1`.
Candidate `canonicalproducts1` is base `432d5a6d1651c954843891954325d5b2b14050f7`
plus the five captured overlays in its identity JSON. Plugin SHA-256:
`04dc0eb35f02afd0bcc874e7b46368569411ec05d31f1e49e71ec7ee9d7f8418`.
Its **548 captured inputs match the final working sources**. Documentation and
receipts are additional files, not an assertion that an unbuilt commit was tested.
The isolated release build passed the real plugin ABI/panic-containment harness.

[Receipts](receipts-content-addressed.zip) contain **8,775 logical files**, 2,334
content-addressed payloads and 18,325,459 bytes. Archive SHA-256:
`e002bf02e6b8a197986f42d88e883cb7f395a79c0f754f97be372a85c2c204f3`.
The [restore tool](restore_receipts.py) verifies the container and every payload;
all logical receipts were restored and verified. Complete baseline/candidate
sources, controllers, declarations, failed attempts, numerical exports, science
checks, phase/memory distributions and fixture protocols are preserved.
Raw licensed logs, datasets and binaries are omitted with hashes; machine paths
are sanitized. All builds, tests, fits, audits and archive/restore processes are
terminal. No compilation, profiling, archiving or competing fits ran alongside
headline measurements; every screen's overlap and monitor-error lists are empty.

Restore to a new directory with `python restore_receipts.py DESTINATION`.
The `canonical-products-integration/summary.json` receipt contains all marginal
and paired distributions, memory/iteration summaries and replayed structural
admission. `fresh-audit.json`, `fresh-direct-stock-audit.json`, corresponding
development files and `ols-audit.json` retain individual scientific checks.

## What changed

Each admitted canonical call borrows original response/controls and FE topology,
constructs one current-weight grounded/scaled Schur factor, and solves all RHSs.
One group-partitioned residual pass forms the direct fully absorbed Gram/RHS,
within norms and fresh original-FE score certificate. Canonical finite solving
consumes these same-call products directly. Scratch rows are still materialized
for unchanged QR/span, inverse, residual and inference consumers, but the new
writer replaces all cells once without reading prior tiles.

This removes the old separate response solve, repeated control-panel projection
and scratch patching, and redundant finite-moment scan. It does not replace
ordinary joint IRLS directions or lower any tolerance. Factors/products do not
survive a weight/design/response call. It is neither the rejected operator-only
experiment nor the prototype's algebraic Gram subtraction.

Work and full allocation admission remain structural. Only low categorical
two-FE canonical MAP boundaries enter; fast mode, slopes, one/three-plus FE and
ordinary OLS retain their prior algorithms. Budget/work/numerical refusal uses
the existing solver. Any scratch write error poisons its private file and prevents
publication. The ADR documents the reduction-error argument and retained checks.

## Whole public command

The development block reuses seed 202609347 for eight shapes and three rotated
baseline/candidate/stock repetitions. Its wide low N16320/K32/T8 median improves
118 to 80 ms versus stock 90; unbalanced K12 improves 65 to 54 versus 47; serial
K16 improves 95 to 83 versus 153. These are selection evidence, not fresh claims.

**Fresh seed 202609379 is consumed/frozen.** The candidate was fixed and all 12
cases declared before generation. Five rotated runs include robust inference
and fitted means, matched samples and requested threads. Stock is unmodified
pinned fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`, at
`glm.tol=1e-10`, `fixef.tol=1e-11`, with serial BLAS. Native low memory is not a
claim of equal peak memory or equal memory budgets between packages.

All times below are milliseconds. C/B ranges are ratios from matched rotations;
C/F is the ratio of marginal command-time medians.

| Fresh case | Baseline | Candidate median [min,max] | Stock | C/B paired median [min,max] | C/F |
|---|---:|---:|---:|---:|---:|
| Wide low 2FE N23040/K36/T8 | 160 | 112 [110,114] | 129 | 0.704 [0.683,0.713] | 0.868 |
| Unbalanced low 2FE N28800/K18/T4 | 115 | 104 [103,106] | 88 | 0.904 [0.896,0.938] | 1.182 |
| Larger low 2FE N51840/K24/T8 | 226 | 168 [167,169] | 200 | 0.739 [0.664,0.748] | 0.840 |
| Wider low 2FE N26880/K72/T8 | 399 | 406 [388,412] | 352 | 0.987 [0.926,1.038] | 1.153 |
| Serial low 2FE N16320/K22/T1 | 130 | 108 [107,110] | 213 | 0.837 [0.811,0.849] | 0.507 |
| Narrow fast 2FE N16800/K7/T1 | 67 | 68 [64,70] | 80 | 1.015 [0.941,1.030] | 0.850 |
| Two slope partitions low N21120/K12/T4 | 108 | 110 [108,111] | 133 | 1.019 [0.991,1.028] | 0.827 |
| Two slope partitions fast N21120/K10/T4 | 100 | 98 [97,100] | 109 | 0.980 [0.970,1.042] | 0.899 |
| Low 3FE N19200/K12/T4 | 88 | 89 [88,91] | 279 | 1.011 [0.967,1.023] | 0.319 |
| Weak low 2FE N11520/K8/T1 | 55 | 50 [45,52] | 1420 | 0.897 [0.828,0.945] | 0.035 |
| Disconnected low 2FE N21120/K14/T4 | 89 | 88 [87,89] | 70 | 1.000 [0.967,1.035] | 1.257 |
| Excluded one FE N19200/K6/T4 | 35 | 34 [34,35] | 22 | 1.000 [0.971,1.000] | 1.545 |

All five native pairs improve for the wide, unbalanced, larger, serial and weak
families. Eight of the eleven in-scope fresh medians reach C/F <=0.90, including
previously accepted paths; this milestone does not cause all eight advantages.
The wide case's paired C/F range is 0.840–0.918, so its 10% margin is not present
in every rotation. The larger range is 0.807–0.844. Fast two-slope median 0.899
has a 0.890–0.926 range and is a narrow margin on an unchanged path.

The K72 marginal median loses 1.8% to baseline, while its paired median is 0.987
(three better, two worse pairs). Its new owner is refused for memory, and native
iteration counts/peaks are identical. Disconnected likewise uses the fallback
because of factor work. Small losses on unchanged/excluded paths remain in the
table; they are not silently dropped from an aggregate.

The 24-call OLS control block has three matched rotations: categorical two-FE
29→29 ms, low-memory two-partition slopes 101→101, one-FE robust plus all saved
outputs 16→17, and low three-FE 60→61. All 12 scientific/output pairs pass. These
short controls do not establish zero regression; the 1 ms differences are retained.
No historical 77/16/111 definition or earlier artifact's win claim changed.

## Science and failures

All **252 PPML applications and independent public-state checks pass**, as do
84 baseline/candidate and 84 direct candidate/stock comparisons. The checks
verify sample identities, original-design score, model-space feasibility and
objective replay, plus coefficients, covariance and means in paired exports.
Large-screen checks are not a full explicit Poisson covariance oracle per cell.
The earlier seed-378 disconnected stock discrepancy remains unresolved; a
passing different seed here does not resolve it.

**33 distinct focused Rust core tests and one plugin test pass.** The new path
is explicitly reached in the session test and validates full/duplicated finite
rank, strict candidate rollback and changed weights. Independent analytic
weighted contrasts cover T1/4/8, disconnected component mass ratios of 160 orders,
column scales of 140 orders, near-FE-collinearity and zero columns. Direct Gram
and materialized rows are checked independently. Exact optional-memory refusal,
work refusal, nonfinite input and unachievable-certificate refusal are tested.
The scratch test verifies one write per tile, no reads, unchanged buffer capacity,
poisoning after partial callback/I/O failure and cleanup. Existing strict,
canonical fallback, slope, three-FE, zero-control and capacity tests pass.
The plugin's one-byte budget test rejects before numeric allocation and stores
nothing. This is not physical constrained-memory qualification.

**Ten licensed fixture identities pass** with Stata return codes and terminal
sentinels. They include explicit dummy Poisson and independently formed final
Hessian/sandwich oracles, unit/FW/PW, robust/cluster inference, generated factors,
all requested prediction channels, categorical/slope/one-FE controls, warmed
allocation audits and focused OLS scientific/memory checks.

One diagnostic factor-oracle draft failed: its alternating factor was exactly
in the first FE span, so an identified factor-coefficient comparison was invalid.
The corrected design varies independently, selects the factor coordinate by
name and passes. Both sources and original failure receipts are retained.
Two intermediate Rust test compilations failed because a test called `as_slice`
on an existing slice; corrected tests pass. No estimator error was hidden and
no scientific tolerance was changed to repair a fixture.

## Memory and remaining costs

Modeled budgets are unchanged on every fresh pair. Candidate whole-action
tracked peaks remain below the model (maximum observed ratio 0.892). Wide peak
falls 31,186,393→30,887,289 bytes; unbalanced 26,443,317→24,328,949; larger
58,052,867→56,564,899; serial 17,233,260→16,449,868. Scratch sizes are unchanged.
The optional bank substitutes for existing projection reservations; it is not
an extra uncharged cache. Native tracked allocation is distinct from process RSS.

The timing reductions occur where intended. On fresh wide, recorded FE projection
falls 43.93→24.77 ms and strict final solving 47.40→21.58. On larger, these are
64.93→37.75 and 65.85→36.02. Outer iterations are unchanged in every pair.
Reported inner counts drop on the direct path, but one factor solve is not an
operation-equivalent MAP iteration. No instrumented reference build contributes
to headline timings. Phase counters are reported as recorded; they are not used
to manufacture a summed public-command decomposition.

The next bounded implementation should target **temporary ownership in canonical
products**, then componentwise factor organization:

- K72 requires 4,421,888 bytes against 4,300,800 available in the existing
  reservation. The current planner deliberately sums all buffers without
  lifetime credits, and the implementation keeps setup matrices until return.
  Explicitly end setup/solve lifetimes before final products/output and derive a
  reviewed peak bound. This is a concrete way to admit wider designs without
  raising memory budgets or fitting a width threshold. Its unchanged fallback
  spends about 95 ms in FE projection and 121 ms in strict final solving.
- Disconnected Q352 fails the global dense-factor work bound: approximately
  14.59 million estimated operations versus 2.53 million Gram work. Its actual
  disconnected components suggest separate grounded factors; the present code
  still allocates/factors the global reduced matrix. Compare componentwise work
  and memory against the same old fallback on a new declared design.
- Unbalanced passes both gates and still loses all stock pairs. Its recorded
  projection is 23.96 ms, finite solving 8.98, strict final 22.31, preparation
  11.23 and inference 2.35. It needs further canonical/joint/preparation work;
  inference alone cannot close its 104 versus 88 ms gap.

No SCC retries, stopped campaigns, full qualification, push or release occurred.
Static allocation-inventory drift, weighted constrained-fit failures,
nonretaining-CG instability and platform deferrals remain open.
