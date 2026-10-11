# Current architecture

There is one computational runtime for OLS and PPML. Stata owns syntax, metadata
and the transaction; Rust owns loading, preparation, absorption, estimation and
inference. [SPEC](SPEC.md) defines the scientific contract.

| Owner | Responsibility |
|---|---|
| `stata/` | Public ado, parsing, result validation, publication and rollback |
| `plugin/` | Small C bridge to Stata SPI |
| `crates/fereg-plugin` | Main-thread SPI loading, FFI panic boundary and publication |
| `crates/fereg-core` | Safe numerical core, memory admission and worker policies |
| `crates/fereg-oracle` | Independent dense scientific oracles; development only |
| `crates/fereg-testkit` | Native fixtures and independent regression support |

## Preparation and storage

Canonical preparation owns the immutable sample/topology and optional cluster
metadata. Consumers borrow this owner; original factor slots and base flags
survive final-sample changes. No duplicate legacy preparation pipeline is installed.
`memory::Capacity` and `fe::workspace_capacity` share checked shapes; actual
capacities remain authoritative. `action_memory::Composition` charges simultaneous
lifetimes once while keeping admission, expected heap and coverage distinct.

Dense, scratch and implicit storage retain mathematically appropriate kernels.
The Low path keeps one row-major scratch file through fit and inference. Scratch
read failures revoke cached pages; mutation failures poison the affected owner.
Public memory choices, fallible allocation and transactional failure are preserved.

The unrestricted numeric loader can complete categorical sources first and
factorize their Rust-owned columns while the caller reads remaining columns.
The optional `PreparedOlsLoadHint` contains encodings only. Authoritative sample
classification still precedes adoption, which requires the full original row
axis, identical source identities and matching canonical decoded values.
Singleton pruning, topology, DoF, clusters and original factor/base metadata
remain products of ordinary preparation. Missingness or an unproved identity
discards the hint. Packed-RHS source conflicts, strings and insufficient
admission use the existing loader. Both scopes join on errors or unwind; the
caller polls Break while waiting, though a running factorization can finish its
current whole-column call before observing cancellation.

PPML carries the hint outside `LoadedPpmlInput` in a per-call adapter. It charges
the surviving cache through adjustment/bias reads, then excludes its exact owned
capacities from the permanent embedding snapshot. A failed subtraction drops
the hint and repeats the original snapshot. Before the first memory decision,
full-row classification and a checked bound admit cache-plus-adoption descriptors
only within the existing, not-yet-live generation topology row bank. Generation
zero takes it once, including failed attempts; adoption moves encodings before
topology allocation. Later separation generations rebuild normally. No cache
becomes sample, Hessian, basis or certificate authority, and memory policy does
not change to accommodate the optimization.

## Finite operations and accepted results

`ols/finite_operations.rs` owns guarded factor/solve/inverse and explicit rank
policies. `ols/finite_wls_kernel` owns dense, scratch and implicit moments over
immutable projected rows. `ols/prepared_fit/one_shot.rs` owns the Fast/Low fit
lifecycle. OLS has a temporary finite workspace; PPML lends retained banks.
OLS does not create a PPML session.

`ols/prepared_products.rs` owns accepted fit statistics, canonical absorbed DoF
and distinct covariance/certificate payloads. Robust/clustered inference consumes
fit-stage projected score products without rebuilding an FE solver or reabsorbing
the design. Independent score-Gram, bread, covariance, PSD and Wald witnesses
remain separate from production accumulation.

`ols/linear_frame.rs` owns retained coefficients, finite bread, residuals and
optional decomposition. Canonical and frozen proofs remain distinct. Strict PPML
transactions retain accepted/speculative frames and restore accepted authority
without allocation. PPML outputs have one requested-product constructor, one xb
representation and one immutable final-row ledger shared with the sample.
Topology, weight, response and basis identities remain explicit dependencies.

## Host and execution

`model_metadata.rs` owns shared model decoding and wire construction.
`input.rs` delegates numerical loading to the common admitted preparation loader.
Each estimator has one named matrix schema for layout, capacity, precommit and
publication. `publication.rs` supplies ordered scalar/matrix/output publication;
completion is written last. Workers only touch Rust-owned memory. FFI catches
panics; SPI stays on Stata's main thread.

`kernels.rs` holds hot `mul_add` loops and the borrowed-MAP group-mean passes.
Baseline x86-64 code calls the library
`fma` per element, so the plugin's `fused_kernels.rs` installs FMA-compiled
instantiations of the same bodies when it detects FMA and AVX at runtime;
otherwise callers run the portable bodies inline. FMA is exactly rounded, so
results match wherever the platform `fma` is exactly rounded.
Processor features may select different arithmetic under SPEC's independent
scientific checks; both the portable and accelerated paths remain tested.

The general three-FE LSMR gather, elimination, sparse products and Schwarz setup
live in the pinned `within`/`schwarz-precond` dependency. Changes there obey the
same scientific and performance evidence rules and enter through a pinned
revision bump.

Thread counts are worker budgets. Shared planned lanes retain deterministic
arithmetic leaves and ordered merges. One-FE, two-FE and general-FE algorithms
remain distinct where mathematics warrants them; benchmark-specific route fences
are prohibited. Explicit LSMR/CMG and both bias-correction modes remain supported.

### Fixed-leaf norms and outer scores

Reusable MAP norms use 512-row leaves and merge their scaled sum-of-squares
summaries in ascending leaf order, including with one worker or no available
scratch. RHS tiles are bounded at 64 and waves at 128 leaves; scratch capacity
changes scheduling only. Categorical MAP lends its idle dot/group-statistic bank,
and slope MAP lends its group-column bank, distinct from cached certificate
geometry. Every later consumer rebuilds these banks before reading them. The
fallback uses bounded stack storage; no additional capacity or memory route is
required. A nonzero leaf keeps the maximum computed weighted element as its
scale and a sum of at least one. Merging retains the larger scale and its
nonnegative contribution, so the final norm still dominates every computed
weighted element. The initial rejection screen's proof below remains applicable.

The independent PPML outer score uses the same fixed row leaves for its scale
and finite-column moments. Each leaf retains its compensated sum and correction;
both components enter the ordered compensated merge. Rounding a leaf to a single
total before merging is insufficient under cancellation. The two existing FE
score banks hold partials before their unconditional clearing and unchanged
FE-group accumulation. Their capacity controls tiles/waves, never the arithmetic
tree; serial work avoids submitting to the team. Scores still read original
outcome, mean, prior weights and original/generated controls. No projected design
or cached Newton score substitutes for the independent certificate. Single-leaf
inputs retain their former totals. Strict PPML solving and final-Hessian checks
remain separate and fresh.

### Initial FE rejection screen

The reusable categorical MAP owner may reject its zero-iteration shortcut from
one exact group, but that rejection grants no certificate. Every successful
shortcut still runs the complete independent certificate, as does every final
solver acceptance. A partial `FeCertificate` is never published.

Skipping the initial gather must also preserve its errors. Both domains require
immutable validated nonempty memberships, checked maximum-level and total-moment
dimensions, and norms freshly computed from the unchanged initial matrix.
Every nonzero norm must lie in `[1e-20, 1e20]`; these are conservative
finite-arithmetic bounds, not convergence tolerances. Genuine unit weights may
include zero norms. Supplied weights require every norm to be positive, at most
`2^32` observations, raw scale in `[2^-32, 2^32]`, and every computed normalized
weight in `[2^-32, 1]`. One raw-minimum scan and a division by the fixed positive
scale establish the last bound by monotonicity, without allocating a bank.
Every unproved case uses the original complete certificate.

The finite-error proof uses binary64 round-to-nearest, ties-to-even. For a
power-of-two bound `A`, rounded accumulation from zero of terms with magnitude
at most `A` cannot cross `2^54 A`: spacing at that boundary is `4 A`, and spacing
immediately below it is `2 A`. Thus the bound does not require a small `n epsilon`.
For Neumaier accumulation the primary sum is bounded by `2^54 A`; each correction
increment is bounded by four times that, and its accumulated correction by
`2^110 A`. These bounds include cancellation and subnormal rounding.

The fresh unit-weight scaled norm is at least the column maximum, and zero norm
implies an all-zero column. Hence inputs are smaller than `2^67`, shifted inputs
than `2^68`, shifted primary sums than `2^122`, and corrections than `2^178`.
Including final addition and division, one-pass means are bounded by `2^180`.
The scaled fallback accumulates inputs of magnitude at most one, so its final
means are bounded by `2^178`. Unit group weights remain positive and finite;
their square roots are at most `2^32` for supported 64-bit observation counts.
Positive norm denominators exceed `2^-67`. Relative moments are therefore
bounded by `2^279`, their exact squares by `2^558`, and the entire ordered FMA
square accumulation by `2^612`, below binary64 overflow. Zero columns have zero
means, so the zero-denominator error is impossible. Validated structure and
checked moment counts discharge the remaining certificate errors.

For the weighted domain, let `u = 2^-53`, let `m <= 2^32` be a nonempty group's
size, and let `W` be the exact sum of its normalized weights. Since `m u <= 2^-21`,
the positive primary accumulation stays in `[W/2, 2 W]`. In each Neumaier update,
the larger operand and rounded sum satisfy Sterbenz's condition; the computed
correction increment has magnitude at most `8 u W`. The accumulated correction,
including its own rounding, is bounded by `2^-17 W`. These normal weights and
their intermediate sums lie on a grid no finer than `2^-84`, so underflow does
not invalidate this argument. The computed group weight therefore lies in
`[W/4, 4 W]`, hence in `[2^-34, 2^34]`.

Fresh weighted norms dominate every computed `sqrt(q) * x`. Since `sqrt(q)` is
at least `2^-16`, raw input magnitudes are bounded by `2^83`. The saturation
lemma then bounds shifted weighted terms by `2^84`, their primary sums by
`2^138`, and corrections by `2^194`. Including division by the group weight,
one-pass means are bounded by `2^230` and scaled-fallback means by `2^228`.
Relative certificate moments are bounded by `2^314`, their exact squares by
`2^628`, and the entire ordered FMA square accumulation by `2^682`. Raw absolute
moments are bounded by `2^296`. All power-of-two upper bounds are inclusive and
remain below overflow; positive norm denominators remove the zero-scale error.

The weighted zero-norm exclusion is necessary even in this weight window.
With singleton groups, weights `[1, 0.25]` and a column `[0, 2^-1074]`, the
weighted norm is zero, while the second group's scaled mean is `2^-1074`.
The original certificate must retain its nonzero-moment/zero-scale error.
Tiny values in a column with a positive moderate global norm remain supported,
including their whole-group scaled fallback. No tolerance or acceptance rule
changes in either domain. Solver tracing records successful screen rejections
and whether they used weighted, parallel or wide arithmetic; normal runs add
no screen output or clock.

The screen runs inside the existing pilot-scratch loan and follows the exact
route that the full certificate would use: one-pass means with whole-group
fallback for serial/narrow work, scaled tiles for wide parallel work. It computes
every RHS of the first group, because a tiny RHS can trigger fallback for all
RHSs, and folds it with the unchanged certificate arithmetic. Residual norms,
RHS scales and per-RHS maxima are initialized as before. Downstream solvers
initialize their required scratch; retries receive fresh complete certificates.

## Repository boundaries

One plugin per platform serves both public commands. Scientific solver-route
diagnostics remain.
There is no legacy binary, selector, fallback, comparator packaging or required
historical timing campaign. The retained benchmark is descriptive and local.
Tests, pinned independent references and the complexity ratchet remain maintained.
Builds go under ignored `target/` and task captures under `target/tmp/`, which is
deleted after use; only one current local plugin is retained in `dist/`. Git history preserves the completed development programs.
