# Canonical PPML products experiment

This development-only probe compares the accepted low-memory canonical
projection/product pipeline with two complete fixed-frame alternatives. It is
not linked into the production library and does not grant rank, final-Hessian,
inference, memory-admission or publication authority.

Both alternatives eliminate the larger categorical partition, assemble the
remaining weighted Schur Laplacian, ground one coordinate per connected
component, scale its diagonal and factor it once. All RHSs share that factor.

1. Algebraic products form the centered Gram minus forward-solution products.
   Their cancellation diagnostic is deliberately not represented as a full
   factorization error bound. Independent reconstruction supplies FE scores.
2. Fused direct products use one reconstruction pass over disjoint complete
   groups to form the Gram and fresh FE scores. Private right-group score banks
   are reduced after the pass. No observation-by-control result panel is stored
   and no nearly equal Gram terms are subtracted.

The baseline uses the production `ReusableCategoricalMapAbsorber`, a separate
response solve, ten-control row-major scratch patches, and the production
retained scratch moment kernel. All frozen frames use this low-memory
organization, including frames originally obtained from fast public fits. It
does not model complete public fits or the entire canonical rank boundary.

Use a Python environment with NumPy and SciPy, and the pinned offline Rust
dependencies. Each preparation needs a new output directory:

```sh
python benchmarks/ppml_canonical_products/run.py prepare target/products-probe --frames PATH_TO_FROZEN_FRAMES
python benchmarks/ppml_canonical_products/run.py build target/products-probe
python benchmarks/ppml_canonical_products/run.py run target/products-probe
OPENBLAS_NUM_THREADS=1 python benchmarks/ppml_canonical_products/audit.py target/products-probe
OPENBLAS_NUM_THREADS=1 python benchmarks/ppml_canonical_products/finite_oracle.py target/products-probe
```

Input files `c0.bin` through `c3.bin` follow the explicit little-endian layout
read by `audit.read_frame`. The captured design-selection report identifies
their exact provenance and hashes. Preparation also declares a second weight
frame per case using a deterministic positive perturbation. These are weighted
linear frames, not additional Poisson fits or fresh validation seeds.

Preparation snapshots HEAD's core, pinned dependencies and compile-time fixture
inputs, then injects this probe into a test-only module. Each snapshot uses its
own Cargo target. Source hashes and the executed binary hash bind the receipts.
Build first, finish it, and run the paired probe without competing work. Oracle,
export and archive work stays outside measured calls. Cold comparisons charge
one complete factor preparation; no amortization is assumed.

The prototype uses ordinary development allocations. Its payload estimates
exclude allocator overhead, source storage and oracle/export storage; they are
not tracked public peaks or a production memory plan. Production integration
must introduce fallible budgeted storage, structural setup-work admission,
source-layout access, complete generation ownership and transactional refusal.

The exact-contrast boundary checks include changing weights, disconnected
component mass ratios of 160 orders of magnitude and column scales spanning
140 orders. Near-FE-collinear inputs must decline algebraic products; direct
products retain a small nonnegative norm for canonical rank classification.
Independent sparse explicit-design WLS checks every exported projection and
Gram. A separate explicit-FE/Householder-QR oracle checks finite coefficients
and both inverse sides. No public tolerance is changed.
