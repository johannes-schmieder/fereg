# Canonical PPML product design selection

Local macOS arm64 development experiment, 2026-09-24. **Select fused direct
products for production integration; no production adoption or public speedup
is claimed here.** [ADR 0167](../../../../docs/adr/0167-fused-canonical-ppml-product-design.md)
records the decision and required implementation boundary. The broader goal of
whole-public PPML at most 0.90 times comparable stock fixest remains unfinished.

## Source and receipts

The baseline is HEAD `2b6560974ed3680afec9ac8803225ae58ac1bc28`, whose accepted
computation remains `2c34503d` / `jointpanels2`. Production Rust, Stata and bridge
sources are unchanged. The committed development probe is injected only into
an isolated test module under `ols`; it has no production call site.

The [archive](receipts-content-addressed.zip) contains **580 logical receipts**,
221 payloads and 1,762,375 bytes, SHA-256
`cff116d4187f9b677bea8b57b4c748211c00c6062bc8d9d1efcf2d8c30983ab6`.
All logical receipts were verified and restored using the [restore tool](restore_receipts.py).
It includes complete captured sources, source hashes, declarations, controllers,
full build/test/measurement logs, all distributions, both independent oracle
outputs and terminal receipts. Synthetic data/array exports and executable
binaries are omitted with hashes. Paths are sanitized; no licensed logs appear.

The first snapshot failed compilation because its capture omitted a required
compile-time CSV fixture. That failure and all 171 captured files are retained.
The corrected captures include the fixture inputs, with **182 source inputs each**
verified during archiving. Each source has its own Cargo target. Both successful
builds and all tests/probes/oracles are terminal; compilation, profiling,
archiving and competing fits did not accompany measured calls.

- First successful algebraic probe binary SHA-256:
  `780375f935491f2f5309b84daaa2dc5084f85fc2875c5fd769f36886fd08adfb`.
- Final two-candidate probe binary SHA-256:
  `6c8c6a549941ff337f964b4f75868527649d555b5f7deffd46c84db0102c14a8`.

No public fit, stock reference, licensed-Stata fixture, OLS benchmark, physical
memory cap or platform qualification was run in this milestone. The latest
public ratios and unresolved disconnected comparison remain those in the
[preceding integration closeout](../ppml-frame-operator-integration-local-2026-09-24/README.md).

## Computational comparison

Use the four synthetic frozen final-weight frames from the earlier
[lifecycle experiment](../ppml-frame-operator-local-2026-09-24/README.md):

| Code | Shape | N | Controls | Left/right levels |
|---|---|---:|---:|---:|
| c0 | Wide | 16,320 | 32 | 2720 / 68 |
| c1 | Unbalanced | 16,320 | 12 | 204 / 68 |
| c2 | Narrow | 12,000 | 3 | 2000 / 50 |
| c3 | Disconnected | 20,160 | 12 | 3360 / 336 |

Frame f0 retains those exact original columns, working response and fitted-mean
weights. Frame f1 multiplies the weights by `exp(0.7*sin(row/17))`; it leaves
the response and controls unchanged. This positive perturbation is a declared
weighted-linear experiment, not another Poisson fit. No fresh validation seed
was consumed. All three variants see identical bytes at tolerance **1e-12**.

**B, accepted panels:** the actual production reusable categorical PPML
projector absorbs the response separately, then ten-control blocks. Its
certified outputs are patched into the actual row-major scratch owner. The
actual retained-scratch moment kernel constructs the normalized Gram and RHS.
Raw reference norms are included. The test uses this low-memory organization
for every shape; c2/c3 originally came from fast public fits.

**A, algebraic products:** eliminate the larger partition by weighted group
centering. Build the remaining weighted Schur Laplacian from positive pair
couplings, ground one coordinate per connected component, diagonally scale and
Cholesky-factor it. All RHSs use that factor. Form the centered Gram minus
forward-solution cross-products (`H-U'U`). Reconstruct from original columns and
both sets of FE coefficients for a separate complete group-score witness.

**C, fused direct products:** use the same coefficient solve, but omit the
initial centered Gram and its subtraction. A group-disjoint pass reconstructs
each normalized original row, forms the full direct Gram/RHS and accumulates
original-design FE scores. Each worker owns complete eliminated groups; only
the smaller partition needs private group-score banks. Reduction merges these
banks with compensation. There is no retained N-by-K result panel.

The joint production owner's existing Hessian is only partially absorbed and
is a preconditioner product; neither prototype treats it as final bread. The
new factor solves the remaining FE coordinates explicitly.

Seven rotations alternate B/A/C and C/A/B. Reference scaling, current-weight
moments, coefficient solving, original-input residual reconstruction and the
projection witness are timed. Exports and independent oracles run outside the
clock. **Full rank/QR policy, inverse certification inside the estimator, final
inference, requested outputs and public transactions are not timed here.**
The baseline retains rows; the candidates deliver products plus coefficients.
Those are equivalent inputs for a redesigned product consumer, not identical
storage outputs for the unchanged public pipeline. Future required row
materialization or streaming costs must be charged at integration.

## Independent science and boundaries

The first algebraic block passes all **48** projection/Gram configurations
(756 individual column checks). The final block passes all **72** configurations
(**1,134 column checks**) against independently assembled sparse explicit FE
designs and SciPy LSMR at 2e-15 internal tolerances. Acceptance requires
normalized projection error <=1e-10, normalized original-design score <=1e-12,
and normalized Gram error <=1e-10 against both the independent projection and
the exported rows. The whole Gram includes the response diagonal and RHS.

Both final candidates have maximum normalized projection error 1.169e-14 and
score 3.322e-16. Maximum independent Gram error is 1.876e-14 for A and
1.554e-14 for C. The baseline also passes all checks.

A separate explicit-FE projection plus Householder-QR oracle checks finite
coefficients and inverse products in all **72** configurations. All pass the
registered coefficient envelope and 5e-10 inverse check. Maximum coefficient
tolerance ratio is 3.290e-7 for A and 3.034e-7 for C; maximum left/right inverse
error is 1.968e-14 and 1.621e-14 respectively. These are frozen WLS/inverse
checks, not a complete nonlinear Poisson, inference or rank-deficiency oracle.

All **three final-source Rust boundary tests pass**. They cover independently
constructed exact weighted contrasts, changed weights, T1/T4/T8, disconnected
component masses differing by 160 orders, column scales spanning 140 orders,
FE-collinear and nearly FE-collinear columns, nonfinite values and weights whose
normalized representation underflows. A declines the small-variance product
subtraction. C computes a small nonnegative norm directly for later canonical
rank classification. Neither prototype owns that classification.

## Distributions, setup and losses

The table reports final-block medians in milliseconds. The last column is
`(C median + C setup)/(B median + B setup)`. **This is a preparation-charged
estimate**, using one separately observed setup per configuration; it is not a
distribution of independently timed cold calls. Full seven-value warm
distributions and paired differences are in `summary.json` and `run.log`.

| Frame | T | B panels | A algebraic | C fused | C setup | C/B with setup |
|---|---:|---:|---:|---:|---:|---:|
| c0-f0 | 1 | 24.117 | 10.645 | 9.744 | 0.352 | 0.416 |
| c0-f0 | 4 | 19.540 | 3.227 | 3.299 | 0.349 | 0.186 |
| c0-f0 | 8 | 19.177 | 2.252 | 2.267 | 0.352 | 0.136 |
| c0-f1 | 1 | 24.269 | 10.726 | 9.695 | 0.339 | 0.412 |
| c0-f1 | 4 | 19.851 | 3.281 | 3.202 | 0.340 | 0.178 |
| c0-f1 | 8 | 19.329 | 2.124 | 2.072 | 0.361 | 0.125 |
| c1-f0 | 1 | 9.229 | 3.207 | 2.872 | 1.012 | 0.417 |
| c1-f0 | 4 | 7.270 | 1.082 | 1.204 | 0.997 | 0.299 |
| c1-f0 | 8 | 7.853 | 0.846 | 1.020 | 0.997 | 0.254 |
| c1-f1 | 1 | 9.367 | 3.215 | 2.849 | 0.990 | 0.406 |
| c1-f1 | 4 | 7.613 | 1.099 | 1.370 | 0.983 | 0.306 |
| c1-f1 | 8 | 7.772 | 0.918 | 0.902 | 1.002 | 0.243 |
| c2-f0 | 1 | 3.982 | 0.552 | 0.503 | 0.235 | 0.182 |
| c2-f0 | 4 | 3.339 | 0.172 | 0.196 | 0.244 | 0.129 |
| c2-f0 | 8 | 3.656 | 0.300 | 0.397 | 0.243 | 0.172 |
| c2-f1 | 1 | 3.954 | 0.545 | 0.498 | 0.237 | 0.183 |
| c2-f1 | 4 | 3.269 | 0.175 | 0.195 | 0.249 | 0.133 |
| c2-f1 | 8 | 3.844 | 0.362 | 0.353 | 0.246 | 0.154 |
| c3-f0 | 1 | 15.066 | 8.480 | 8.092 | 5.266 | 0.880 |
| c3-f0 | 4 | 8.257 | 2.821 | 2.786 | 5.255 | 0.961 |
| c3-f0 | 8 | 9.460 | 1.875 | 2.037 | 5.165 | 0.753 |
| c3-f1 | 1 | 14.919 | 8.481 | 8.035 | 5.383 | 0.893 |
| c3-f1 | 4 | 7.817 | 2.816 | 2.674 | 5.065 | 0.976 |
| c3-f1 | 8 | 9.320 | 1.898 | 2.011 | 5.224 | 0.768 |

All 168 warm pairs improve for each final candidate. C's preparation-charged
estimates range from 0.125 to 0.976 times B. The wide T8 f0 pairs save
16.56–17.75 ms before setup. Unbalanced T4 f0 saves 5.93–6.54 ms before setup.
These gains justify integration, not a transfer to public-command timing.

The disconnected reduced system has 336 levels. Its factor setup costs
5.07–5.38 ms and triangular solves remain material. C's T4 preparation-charged
gain is only 2.4–3.9%. The earlier A-only block has T4 cold estimates **1.043
and 1.019**, i.e. losses after setup, versus 0.965/0.994 for A in the final
block. Preserve that variation; it does not support broad dense-factor routing.
Setup-work admission must be structural and include assembly and factor costs.

C is not uniformly faster than A: final unbalanced T4 f1 is 1.370 versus
1.099 ms, and narrow T8 f0 is 0.397 versus 0.300. C removes the separate product-
subtraction numerical problem while retaining the large baseline gains. That
maintainability/scientific advantage determines the selected single candidate.

Representative C phase medians in wide T8 f0 are reference preparation 0.332,
cross moments 0.208, coefficient solve/reconstruction preparation 0.305 and
fused Gram/certificate pass 1.199 ms. The final pass is the remaining large
kernel. Phase medians do not sum exactly to the median total and omit small
validation/allocation overhead. The final phase includes Gram formation; it is
not pure checking overhead.

## Memory and implementation boundary

Illustrative conservative explicit payload estimates are 1,373,408 bytes for
wide T8, 259,520 for unbalanced T4, 220,016 for narrow T1 and 1,808,864 for
disconnected T4. They include coefficient/factor/reduction banks and exclude
original source, allocator overhead and export/oracle arrays. They are neither
tracked whole-fit peaks nor an admitted memory envelope. The prototype uses
ordinary development allocations; production must use checked dimensions,
fallible fixed capacities and full simultaneous-lifetime accounting.

Each geometry object owns one immutable normalized weight vector and borrows
immutable topology. A changed frame constructs a new object. This avoids stale
state in the probe but does not yet implement PPML generation-bound reuse,
budget refusal, interrupted construction or failure-atomic publication.

Proceed with C only: borrow source-oriented columns, make factor/setup admission
structural, hand fully absorbed products to canonical consumers, and keep
materialized QR/span fallback. Charge inference and output reconstruction in an
early public comparison. No further standalone factor, matvec or seed-specific
threshold experiment is selected. Existing 77/16/111 definitions, consumed
seeds, allocation-inventory drift, constrained-fit/nonretaining-CG failures,
unresolved disconnected science and deferred platforms remain unchanged.
