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

Thread counts are worker budgets. Shared planned lanes retain deterministic
arithmetic leaves and ordered merges. One-FE, two-FE and general-FE algorithms
remain distinct where mathematics warrants them; benchmark-specific route fences
are prohibited. Explicit LSMR/CMG and both bias-correction modes remain supported.

## Repository boundaries

One plugin per platform serves both public commands. `engine()` and the old
engine identity results are removed. Scientific solver-route diagnostics remain.
There is no legacy binary, selector, fallback, comparator packaging or required
historical timing campaign. The retained benchmark is descriptive and local.
Tests, pinned independent references and the complexity ratchet remain maintained.
Builds and captures go under ignored `target/`; only one current local plugin is
retained in `dist/`. Git history preserves the completed development programs.
