# Prepared PPML coefficient operators: first integration rejected

Local macOS arm64 development, September 23, 2026. Production remains the
computational source of `9955bd36` and accepted `slframe3`; the candidate below
is **not adopted**. The broader target of public PPML runtime at most 0.90 times
scientifically comparable pinned stock fixest remains active.

## Question and implementation

After ADR 0124 reduced certificate costs, compare the observation-based Schur
product `B'(I-Q0Q0')B` with prepared `D-C'C`, where `D=B'B` and `C=Q0'B`, and
with an explicit reduced Schur matrix. The same weighted local QR bases,
original RHSs, per-column CG stopping, fresh reconstruction and independent
acceptance remain authoritative. No tolerance, rank, sample, inference or
certificate schedule changes were made.

The isolated prototype uses three preserved development frames, not fresh
holdouts. Each mode ran three rotated repetitions at T1 and T4. All 54 native
calls passed. All 18 first-repetition outputs passed independent explicit-design
weighted least-squares, local projection and model-space audits at the existing
1e-12/1e-9/1e-10 respective bounds. Maximum relative oracle error is 6.08e-12;
maximum model-space error is 4.26e-11. Native test executable SHA-256:
`d686351e0c42178e22ad328af74205b391bf1b384199dface8a20e7a07337a37`.

Frozen absorption milliseconds, median [min,max], including numerical coupling
preparation but excluding owner construction, CSV loading and thread creation:

| Frame / threads | Observation product | Compact products | Explicit dense Schur |
| --- | ---: | ---: | ---: |
| Two FE / T1 | 29.662 [29.522,37.131] | 27.547 [27.335,28.635] | 15.219 [15.112,15.699] |
| Two FE / T4 | 12.462 [12.386,12.523] | 12.690 [12.684,12.703] | 8.775 [8.744,9.114] |
| Three FE / T1 | 36.193 [35.880,37.843] | 21.878 [20.907,22.112] | 25.501 [25.333,26.686] |
| Three FE / T4 | 14.188 [14.103,14.337] | 10.637 [10.568,10.948] | 14.096 [14.084,14.972] |
| Disconnected / T1 | 9.764 [9.708,10.205] | 10.441 [10.373,10.899] | 31.061 [31.003,31.402] |
| Disconnected / T4 | 5.283 [5.131,5.441] | 5.629 [5.629,5.721] | 16.279 [15.726,16.632] |

These are diagnostic inner-solver clocks, not public command or stock comparisons.
The earlier Python/SciPy comparison is preserved separately; its absolute timings
and relative dense-kernel advantage did not predict all native results.

The production-shaped candidate `slcouple1` adds complete conservative admission,
fallible vector reservations and fixed capacities, generation-scoped numeric
reuse, and an observation-product fallback for potentially cancelled curvature.
It uses a topology-sized preparation bound, then selects a representation by
estimated products, charging explicit Schur formation over maximum RHS width.
A predeclared 20% operator-work margin is an engineering choice, not a numerical
tolerance. It rebuilds sparse row patterns and constructs D in a dense bank on
each changed weight frame. That last organization proves costly.

All 532 candidate computational inputs are bound in the receipt. Plugin SHA-256:
`328a6638958936602c2cfe9177ebb394fab03ce56b49b3d834854337851c2121`.
The unchanged baseline plugin is
`c5e41cef298abced05af47874afc957eb4e6ffa0e5ad13174e58d3e7624c1b5a`.

## Public-command screen

Predeclared seed 202609318, three uncontended baseline/candidate/stock rotations.
Matched threads, robust covariance and timed fitted means; process startup,
data generation/import, warmup and exports are outside the command timer.
Native loading, preparation, estimation, inference and publication are inside.
No compilation, profiling, archiving or competing fit accompanied headline calls.

Stock is the unchanged pinned fixest 0.14.2 archive
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
using `glm.tol=1e-10`, `fixef.tol=1e-11` and single-thread BLAS.
Milliseconds, median [min,max]:

| Case | Baseline | Candidate | Stock | Candidate change |
| --- | ---: | ---: | ---: | ---: |
| Two slope partitions, N30720 K6 T4 | 186 [185,188] | 180 [179,180] | 114 [114,116] | -3.2% |
| Unbalanced three FE, N30720 K6 T4 | 181 [180,181] | 176 [173,178] | 115 [112,118] | -2.8% |
| Larger unbalanced three FE, N61440 K10 T8 | 387 [381,394] | 399 [393,400] | 230 [226,230] | +3.1% |
| Unbalanced slopes, native low memory | 361 [354,365] | 339 [338,359] | 118 [116,119] | -6.1% |

The candidate/stock ratios are 1.58, 1.53, 1.73 and 2.87. Low memory versus
ordinary stock is not an equal-budget comparison. The 36 screen calls and six
small N1200 pilot calls all passed, with no timeout, overlap or monitor error.
Every public-state and pairwise audit passes. Both native artifacts and stock
pass the independent Poisson/Hessian checks on both small pilot designs.

Tracked peak increments are 3,984,872; 10,173,192; 20,843,840; and 10,173,192
bytes. Each exactly matches the modeled increment. This is conservative optional
storage accounting, not whole-action or physical-cap qualification. In particular,
the dense construction bank and worst-case capacities are retained through IRLS.
The memory increase and wide-case loss do not justify adopting these small gains.
No fresh acceptance block or excluded-path timing block was launched after rejection.
Seed 202609319 remains unused.

## Why the inner gains did not carry through

A separate instrumented candidate, plugin
`0b001c7a49390a97a7581ee5f9b66d3259a86e71fa4d6462273018fae1a39e19`,
runs the identical four screen inputs after timing finishes. All four calls pass
explicit Stata rc/sentinel checks. These diagnostic clocks are not headline clocks.

| Case | Representation / preparations | Coupling setup ms | C / D construction ms | Prepared applications | Observation fallback applications |
| --- | --- | ---: | ---: | ---: | ---: |
| Two FE | Schur / 9 | 24.83 | 8.76 / 4.88 | 413 | 0 |
| Three FE | Compact / 8 | 22.40 | 11.27 / 9.04 | 334 | 0 |
| Larger three FE | Compact / 8 | 48.62 | 27.22 / 17.02 | 540 | 0 |
| Low memory | Compact / 8 | 22.03 | 11.08 / 8.87 | 399 | 0 |

Total absorption is 77.31, 75.85, 167.26 and 95.16 ms, respectively. These
include setup, weighted QR and certification; do not add nested clocks together.
On the wide input, fresh QR is 20.10 ms and full certification 19.79 ms. Coupling
setup is a substantial additional cost. The similarity between this setup cost
and the earlier frozen product savings is evidence that preparation consumes
much of the opportunity; it is not an exact subtraction across different inputs.
No fallback product was required, so cancellation fallback does not explain this
candidate's slowdown. This does not prove fallback is unnecessary elsewhere.

The next materially different implementation should retain **symbolic sparsity
and observation-to-block mappings** for the immutable sample/topology and refill
only weighted numeric blocks. Avoid the dense D construction bank. Charge both
planning and construction peaks. Then measure setup plus applications again;
RHS tiling and compact remaining-coordinate vectors are additional concrete
opportunities if applications still dominate. Do not tune the 20% selector to
these seeds or simply enable the frozen-frame winner everywhere.

Dense PPML already warm-starts with prior within-controls and the previous
response residual plus working-response change. Zero initial correction
coefficients do not mean that the original full projection starts cold. A new
coefficient-history cache must improve on that existing mathematical warm start.

## Validation, failures and preservation

35 distinct focused Rust tests pass for the candidate: analytic weighted
projection, rank-deficient/duplicate/zero and slope-only channels, T1/T4,
weight scales 1e-60/1/1e60, direct prepared-versus-observation products,
capacity bounds, geometry-generation invalidation, strict transactions and
optional-memory admission. The actual-plugin ABI/panic harness passes.
Three licensed fixtures pass: multipartition unit/FW/PW explicit Poisson/Hessian
robust/cluster inference, saved slopes, and separation methods.

The first native prototype recipe omitted an existing compile-time fixture and
failed compilation; the corrected fixture-bearing build is separately identified.
The first production-shaped diagnostic recipe failed before compilation because
its injected function name collided with a local import; a new artifact name and
corrected recipe preserve that failure. Neither failure supplied timing evidence.

The candidate and its diagnostics are preserved as source snapshots and immutable
receipts. Production source is restored to the accepted checkpoint; none of this
rejected machinery remains enabled. Earlier allocation drift, constrained-fit/CG
failures, comparator discrepancies and Linux/SCC deferrals remain open. Frozen
77/16/111 definitions are unchanged. No push, release, qualification, SCC retry
or stopped campaign restart occurred.

The receipt container holds declarations, source identities and changed source
snapshots, build/test results, controller snapshots, numerical exports, audits,
all successful application receipts and diagnostic protocols. Raw licensed logs,
binaries and data matrices are excluded. Omitted data/output hashes remain in
`omitted-data-hashes.json`; their original local payloads are preserved. The
restoration script verifies the container and every logical receipt. Original
and sanitized hashes are retained; absolute local paths are redacted.

Verified container: **3,071 logical receipts**, 1,024 distinct payloads, SHA-256
`9f87180b9d85d2c954192e5cb489706adb4983160d5f48c95f16fee57dff43a9`.
