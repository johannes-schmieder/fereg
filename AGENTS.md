# fereg 1.0 Rust-First Implementation Plan for Codex

## 0. How to use this specification

Use this document as the repository-level `AGENTS.md` or implementation authority for **fereg**.

fereg is a new Stata package whose first stable release will provide:

- `fereg`: high-dimensional fixed-effect OLS/WLS.
- `feppml`: high-dimensional fixed-effect Poisson pseudo-maximum-likelihood.
- `fereg`: version, installation diagnostics, and native self-tests.

The project has four nonnegotiable goals:

1. Be significantly faster than `reghdfe` and `ppmlhdfe` on very large datasets.
2. Remain memory-efficient, including under constrained RAM.
3. Preserve the relevant scientific and Stata-facing behavior of the reference commands.
4. Require no Rust, Python, Julia, compiler, or separately installed numerical runtime on the user’s computer.

Do not implement this project as one monolithic task. Work phase by phase. Every phase has an exit gate. Do not advance to a later phase while an earlier scientific, installation, safety, or performance gate remains unresolved.

---

# 1. Governing architecture

## 1.1 Fundamental division of responsibility

The governing rule is:

> **Stata is the control plane. Rust is the data and computation plane.**

Stata owns:

- Command syntax.
- Option parsing.
- Stata factor-variable and time-series syntax resolution.
- Evaluation of arbitrary `if` and `in` expressions.
- Creation of output variables and matrices.
- `ereturn` posting.
- Coefficient names and display.
- `predict`, `margins`, and `estat` command interfaces.
- Platform-specific plugin selection.
- User-facing errors and help files.

Rust owns:

- Reading the candidate estimation sample into native memory.
- Authoritative missing-value and weight validation.
- Construction of the final estimation sample.
- Encoding fixed effects and clusters.
- Construction of categorical interactions.
- Construction of ordinary factor-variable model-matrix columns.
- Iterative singleton deletion.
- PPML separation detection.
- Fixed-effect solver setup.
- MAP, LSMR, preconditioning, and solver selection.
- OLS/WLS estimation.
- The entire PPML IRLS loop.
- Collinearity detection.
- Robust and multiway-clustered variance estimation.
- Degrees-of-freedom calculations.
- Fit statistics.
- Fixed-effect recovery.
- Observation-scale prediction calculations.
- Numerical convergence and score certificates.
- Memory planning and scratch-file management.

The amount of code in each language is not important. The amount of **observation-scale work** in each language is.

## 1.2 Native hot-path invariant

Except for explicitly documented exceptions, every operation whose cost grows with any of the following must occur in Rust:

- Number of observations \(N\).
- Number of finite-dimensional regressors \(K\).
- Number of fixed-effect dimensions \(G\).
- Number of fixed-effect levels.
- Number of clusters.
- Number of fixed-effect solver iterations.
- Number of PPML IRLS iterations.
- Number of separation iterations.

The Stata layer must not perform:

- Fixed-effect encoding.
- FE interaction materialization.
- Cluster-interaction materialization.
- Singleton deletion.
- Separation detection.
- Residualization.
- Cross-products.
- Score aggregation.
- VCE construction.
- DoF graph calculations.
- PPML iteration.
- Large fitted-value or residual calculations.

## 1.3 One-call estimation invariant

Each estimation command must make exactly **one substantive plugin call**:

```text
Stata parsing and output allocation
    ↓
one plugin call
    ↓
load candidate data once
    ↓
complete estimation in Rust
    ↓
write final successful outputs
    ↓
Stata ereturn posting and display
```

In particular:

- Do not call the plugin once per fixed-effect sweep.
- Do not call the plugin once per right-hand side.
- Do not call the plugin once per PPML iteration.
- Do not return residualized \(N \times K\) data to Mata for regression or VCE.
- Do not alternate repeatedly between Stata and Rust during estimation.

A later `predict` command may make its own plugin call. The one-call invariant applies to each estimation invocation.

## 1.4 Single-plugin invariant

For each supported platform, all native code must be shipped in one self-contained `.plugin` file containing:

- Stata’s plugin bridge.
- The Rust FFI layer.
- The fereg numerical core.
- The fixed-effect solvers.
- The PPML engine.
- Separation algorithms.
- Dense linear algebra.
- Threading support.
- All ordinary Rust crate dependencies.

There must be no native runtime sidecars such as:

```text
fereg_core.dll
libfereg.so
libwithin.so
libopenblas.so
libgomp.so
libomp.dylib
Python
Julia
Cargo
```

A package release may contain different plugin files for Windows, Linux, and macOS because native binaries are platform-specific. On any one platform, however, the user loads one self-contained plugin.

## 1.5 Fail-closed invariant

Performance must never be obtained by silently:

- Relaxing convergence tolerance.
- Changing the estimation sample.
- Skipping iterative singleton deletion.
- Skipping PPML separation checks.
- Using a cheaper VCE.
- Using `dof(none)` instead of the requested/default DoF policy.
- Dropping difficult observations.
- Returning an uncertified approximate fit.
- Suppressing nonconvergence.
- Reusing a mathematically invalid weight-dependent preconditioner.
- Posting incomplete `e(b)` or `e(V)` results.

If a certified result cannot be produced, return a clear error.

---

# 2. Product definition

## 2.1 Package and command names

Package:

```stata
fereg
```

Estimation commands:

```stata
fereg
feppml
```

Utilities:

```stata
fereg, version
fereg, doctor
fereg, selftest
```

Do not shadow `reghdfe` or `ppmlhdfe` in version 1.0. Compatibility wrappers may be considered later.

## 2.2 Canonical syntax

### Linear estimator

```stata
fereg depvar [indepvars] [if] [in] [weight], ///
    [absorb(absvars)] ///
    [vce(vcetype) options]
```

Examples:

```stata
fereg ln_wage education experience, ///
    absorb(worker firm year) ///
    vce(cluster firm)
```

```stata
fereg outcome treatment controls [aw=w], ///
    absorb(county##c.year industry#year) ///
    vce(cluster county year)
```

### Poisson estimator

```stata
feppml depvar [indepvars] [if] [in] [weight], ///
    [absorb(absvars)] ///
    [offset(varname) | exposure(varname)] ///
    [vce(vcetype) separation(methods) options]
```

Example:

```stata
feppml trade tariff distance, ///
    absorb(exporter#year importer#year pair) ///
    exposure(gdp) ///
    vce(cluster pair)
```

## 2.3 Required `fereg` 1.0 scope

Version 1.0 must support:

- No fixed effects.
- One fixed effect.
- Arbitrary multiple fixed effects.
- Numeric and string absorbed categorical variables.
- Categorical interactions such as `firm#year`.
- Intercept-plus-slope absorbed terms such as `firm##c.time`.
- Slope-only absorbed terms such as `firm#c.time`.
- Multiple slopes within one absorbed term.
- Ordinary continuous regressors.
- Ordinary factor variables.
- Ordinary factor-variable interactions.
- Time-series operators, initially through a documented Stata-materialization exception.
- `if` and `in`.
- All weight types accepted by the pinned reference command, unless a type is explicitly deferred in the Phase 0 compatibility matrix.
- Iterative singleton deletion.
- `keepsingletons`.
- Unadjusted covariance.
- Robust covariance.
- One-way clustered covariance.
- Multiway-clustered covariance.
- Cluster interactions.
- Reference-compatible default DoF adjustments.
- Relevant `dof()` controls.
- Collinearity detection after absorption.
- Disconnected fixed-effect components.
- No-regressor models.
- Residual saving.
- Saving the sum of absorbed effects.
- Saving individual normalized fixed-effect terms where requested.
- `e(sample)`.
- Core reference-compatible fit statistics.
- Supported `predict`.
- `test`.
- `estat summarize`.
- Verbose and stage-timing modes.

## 2.4 Required `feppml` 1.0 scope

Version 1.0 must support:

- Poisson pseudo-maximum-likelihood estimation.
- No fixed effects.
- One or multiple fixed effects.
- Categorical interactions.
- Heterogeneous absorbed slopes.
- Ordinary factor-variable regressors.
- `if` and `in`.
- All weight types accepted by the pinned `ppmlhdfe` reference, unless explicitly deferred before implementation.
- `offset()`.
- `exposure()`.
- Robust covariance as the default.
- One-way and multiway clustering.
- Iterative singleton deletion.
- Separation methods:
  - `fe`
  - `simplex`
  - `ir`
  - `mu`
- The reference default separation sequence.
- Exact observation-level separation/drop accounting.
- Relative-deviance convergence.
- Warm starts.
- Safe IRLS acceleration.
- Step-halving.
- Strict final fixed-effect and score certificates.
- Collinearity handling.
- No-regressor models.
- All-zero and zero-heavy outcomes.
- Large outcomes and offsets.
- Saving the absorbed linear-predictor contribution.
- Requested fixed-effect recovery.
- `e(sample)`.
- Core likelihood and fit statistics.
- Supported `predict`.
- `test`.
- `margins`.
- `estat ic`.
- `estat summarize`.
- `estat vce`.

## 2.5 Common native options

Expose:

```stata
solver(auto | map | lsmr)
preconditioner(additive | diagonal | off)
threads(#)
memory(auto | fast | low)
poolsize(#)
tolerance(#)
iterate(#)
deterministic
timeit
verbose(#)
```

Where practical, accept familiar aliases such as `technique()`.

## 2.6 Explicit version 1.0 non-goals

The following do not block version 1.0:

- Group-level outcomes with individual fixed effects.
- `group()`, `individual()`, and `aggregation()`.
- IV, 2SLS, LIML, or GMM.
- Driscoll–Kraay and general HAC estimators.
- Logit, probit, negative binomial, or general GLM families.
- Quantile regression.
- Difference-in-differences estimators.
- GPU execution.
- Cross-command persistent native caches.
- Wild-bootstrap implementation.
- Full reproduction of PyFixest tables or plots.
- Compatibility with historical `reghdfe` versions.
- Every undocumented option of the reference commands.

Unsupported features must error explicitly and appear in the compatibility matrix.

---

# 3. Required layering

## 3.1 Runtime architecture

```text
┌──────────────────────────────────────────────────────────┐
│ Stata control plane                                      │
│                                                          │
│ syntax, fv/ts resolution, if/in candidate marker,        │
│ output allocation, coefficient names, ereturn, display,  │
│ predict/margins/estat interfaces                         │
└────────────────────────────┬─────────────────────────────┘
                             │ SPI 3.0
┌────────────────────────────▼─────────────────────────────┐
│ Minimal plugin bridge                                    │
│                                                          │
│ stata_call(), dispatch, SPI wrappers, panic boundary,    │
│ protocol validation, return-code mapping                 │
└────────────────────────────┬─────────────────────────────┘
                             │ safe internal Rust API
┌────────────────────────────▼─────────────────────────────┐
│ fereg Rust estimation engine                          │
│                                                          │
│ loading, sample validation, factorization, singleton,    │
│ FE solvers, OLS/WLS, PPML, separation, VCE, DoF,         │
│ FE recovery, predictions, memory planning                │
└────────────────────────────┬─────────────────────────────┘
                             │ pinned internal adapters
┌────────────────────────────▼─────────────────────────────┐
│ Native reusable components                               │
│                                                          │
│ within, faer or equivalent, Rayon, selected adapted      │
│ PyFixest kernels                                         │
└──────────────────────────────────────────────────────────┘
```

## 3.2 Stata control-plane responsibilities

The ado layer must:

1. Parse the command and options.
2. Validate mutually exclusive options.
3. Evaluate arbitrary `if` and `in`.
4. Create a candidate-sample byte variable without marking out all model variables.
5. Resolve ordinary Stata factor-variable syntax into canonical term metadata.
6. Resolve base levels and coefficient names.
7. Resolve absorbed-term syntax into:
   - source categorical variables,
   - interaction structure,
   - intercept flags,
   - slope source variables.
8. Resolve cluster syntax.
9. Materialize only explicitly permitted transformations.
10. Create requested output variables before entering the plugin.
11. Create result matrices before entering the plugin.
12. Select the correct platform plugin.
13. Pass a versioned model specification to Rust.
14. Post `e()` results.
15. Display the result table.
16. Provide Stata-facing postestimation commands.

The ado layer must not call `reghdfe`, `ppmlhdfe`, or `ftools` at runtime.

Parsing logic may be adapted from MIT-licensed reference code with attribution, but fereg must be self-contained.

## 3.3 Rust data-plane responsibilities

The Rust plugin must:

1. Read the candidate sample.
2. Read original source columns directly.
3. Validate outcome, regressors, weights, offsets, exposures, clusters, and FE sources.
4. Construct the authoritative initial usable sample.
5. Encode FEs and clusters into compact native codes.
6. Construct interactions without Stata temporary variables.
7. Generate ordinary factor-variable columns natively.
8. Delete singletons iteratively.
9. Run PPML separation.
10. Compact the final sample in native memory.
11. Complete estimation.
12. Certify convergence and first-order conditions.
13. Compute VCE and DoF.
14. Recover requested FEs.
15. Compute requested observation-level outputs.
16. Write final outputs only after full success.

## 3.4 Permitted initial exceptions

### Arbitrary `if` expressions

Stata evaluates the user’s arbitrary `if` expression. Rust receives the resulting candidate marker.

Rust remains responsible for all later sample restrictions.

### Complex time-series operators

For version 1.0, Stata may materialize variables involving complex time-series operators, because exact Stata panel/gap/calendar behavior is part of Stata semantics.

Examples:

```stata
L.x
F2.x
D.x
L(1/4).x
```

These temporary variables are passed to Rust as ordinary numeric sources.

This exception must be:

- Documented.
- Timed separately.
- Excluded from absorbed FE interaction construction.
- Revisited only if profiling shows it materially limits large-data performance.

### Requested output variables

Stata creates output variables because the plugin cannot create them. Rust fills them.

## 3.5 Forbidden architecture patterns

Do not:

- Materialize absorbed FE interactions in Stata.
- Use `egen group()` as a required path.
- Expand all ordinary factor-variable columns into Stata temporary variables.
- Mark out missing values across every variable in ado.
- Return residualized regressors to Mata.
- Compute VCE or DoF in Mata.
- Run one plugin call per PPML iteration.
- Launch separate Stata processes for parallelism.
- Embed Python or Julia.
- Require users to compile the plugin.
- Create a dynamically linked Rust side library.
- Store scientific state in undocumented process-global mutable variables.

---

# 4. Distribution and build architecture

## 4.1 Supported initial targets

Build:

```text
Windows x86-64
Linux x86-64
macOS universal binary: x86-64 + arm64
```

The installed package may therefore contain:

```text
fereg_windows.plugin
fereg_linux.plugin
fereg_macos.plugin
```

Each file must contain the entire native runtime for its platform.

## 4.2 Linking model

Preferred build:

```text
fereg-core       Rust library
fereg-plugin     Rust static library / FFI
stplugin.c          official Stata plugin source
stata_bridge.c      minimal entry point

static link
    ↓
one fereg_<platform>.plugin
```

The C entry point should remain minimal:

```c
STDLL stata_call(int argc, char *argv[])
{
    return fereg_dispatch(argc, argv);
}
```

All estimator logic belongs in Rust.

## 4.3 Runtime dependency policy

Permitted runtime dependencies:

- Stata itself.
- Standard operating-system libraries normally available on the supported OS.

Forbidden separately installed runtime dependencies:

- Rust.
- Cargo.
- Python.
- NumPy.
- SciPy.
- Julia.
- OpenBLAS.
- MKL.
- OpenMP runtimes.
- TBB.
- Conda.
- External solver executables.

## 4.4 Build profile

Release builds must use:

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "unwind"
strip = true
```

Do not use `panic = "abort"`.

Keep separate symbol-rich profiling builds for maintainers.

Do not use `target-cpu=native` for distributed binaries. Build against a conservative CPU baseline and use runtime CPU-feature dispatch for optimized kernels.

## 4.5 Plugin diagnostics

`fereg, doctor` must report:

- Ado version.
- Plugin version.
- Model-protocol version.
- Git commit.
- Build date.
- Operating system.
- Architecture.
- Stata version.
- Plugin ABI version.
- CPU dispatch path.
- Available thread count.
- Configured thread count.
- Plugin filename.
- Ado/plugin compatibility.
- Native self-test status.
- Embedded dependency audit status.

---

# 5. Proposed repository structure

```text
fereg/
├── AGENTS.md
├── README.md
├── LICENSE
├── THIRD_PARTY_NOTICES.md
├── references.lock.toml
├── Cargo.toml
├── Cargo.lock
│
├── crates/
│   ├── fereg-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs
│   │       ├── options.rs
│   │       ├── diagnostics.rs
│   │       ├── memory.rs
│   │       ├── protocol.rs
│   │       ├── data/
│   │       │   ├── mod.rs
│   │       │   ├── load_plan.rs
│   │       │   ├── sample.rs
│   │       │   ├── factorize.rs
│   │       │   ├── interactions.rs
│   │       │   ├── model_matrix.rs
│   │       │   ├── weights.rs
│   │       │   └── clusters.rs
│   │       ├── fe/
│   │       │   ├── mod.rs
│   │       │   ├── design.rs
│   │       │   ├── singleton.rs
│   │       │   ├── components.rs
│   │       │   ├── one_way.rs
│   │       │   ├── map.rs
│   │       │   ├── lsmr.rs
│   │       │   ├── auto.rs
│   │       │   ├── certificate.rs
│   │       │   └── recover.rs
│   │       ├── linalg/
│   │       │   ├── mod.rs
│   │       │   ├── crossprod.rs
│   │       │   ├── solve.rs
│   │       │   ├── collinearity.rs
│   │       │   └── standardize.rs
│   │       ├── ols/
│   │       │   ├── mod.rs
│   │       │   ├── fit.rs
│   │       │   └── statistics.rs
│   │       ├── ppml/
│   │       │   ├── mod.rs
│   │       │   ├── family.rs
│   │       │   ├── initialization.rs
│   │       │   ├── irls.rs
│   │       │   ├── step_halving.rs
│   │       │   └── certificate.rs
│   │       ├── separation/
│   │       │   ├── mod.rs
│   │       │   ├── fe.rs
│   │       │   ├── simplex.rs
│   │       │   ├── ir.rs
│   │       │   └── mu.rs
│   │       ├── vce/
│   │       │   ├── mod.rs
│   │       │   ├── iid.rs
│   │       │   ├── robust.rs
│   │       │   ├── cluster.rs
│   │       │   └── corrections.rs
│   │       ├── dof/
│   │       │   ├── mod.rs
│   │       │   ├── components.rs
│   │       │   ├── nesting.rs
│   │       │   ├── slopes.rs
│   │       │   └── policy.rs
│   │       └── prediction/
│   │           ├── mod.rs
│   │           ├── linear.rs
│   │           └── poisson.rs
│   │
│   ├── fereg-plugin/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── dispatch.rs
│   │       ├── spi.rs
│   │       ├── load.rs
│   │       ├── store.rs
│   │       ├── protocol.rs
│   │       └── error_map.rs
│   │
│   └── fereg-testkit/
│       └── src/
│           ├── explicit_dummy.rs
│           ├── generators.rs
│           ├── references.rs
│           └── assertions.rs
│
├── plugin/
│   ├── stplugin.c
│   ├── stplugin.h
│   ├── stata_bridge.c
│   └── Makefile
│
├── stata/
│   ├── fereg.ado
│   ├── fereg.ado
│   ├── feppml.ado
│   ├── fereg_p.ado
│   ├── fereg_estat.ado
│   ├── fereg_parse.ado
│   ├── fereg.sthlp
│   ├── fereg.sthlp
│   ├── feppml.sthlp
│   ├── fereg.pkg
│   └── stata.toc
│
├── tests/
│   ├── rust/
│   ├── mock_spi/
│   ├── stata/
│   │   ├── parser/
│   │   ├── parity_reghdfe/
│   │   ├── parity_ppmlhdfe/
│   │   ├── postestimation/
│   │   └── installation/
│   ├── fixtures/
│   ├── separation/
│   └── fuzz/
│
├── benchmarks/
│   ├── generators/
│   ├── manifests/
│   ├── stata/
│   ├── rust/
│   ├── expected/
│   └── reports/
│
├── docs/
│   ├── architecture.md
│   ├── compatibility.md
│   ├── scientific_contracts.md
│   ├── factor_protocol.md
│   ├── plugin_protocol.md
│   ├── memory_model.md
│   ├── performance_contract.md
│   ├── release_process.md
│   ├── upstream_map.md
│   └── adr/
│
└── scripts/
    ├── build_linux.sh
    ├── build_macos.sh
    ├── build_windows.ps1
    ├── package_release.py
    ├── audit_binary.py
    ├── run_stata_tests.py
    └── run_benchmarks.py
```

Keep Stata-specific code out of `fereg-core`. Keep estimator mathematics out of the ado and FFI layers.

---

# 6. Frozen references and source hierarchy

## 6.1 Pin all references

At project start, create `references.lock.toml` containing exact commits and file hashes for:

- `py-econometrics/pyfixest`
- `py-econometrics/within`
- `sergiocorreia/reghdfe`
- `sergiocorreia/ppmlhdfe`
- Stata’s SPI source files used to build the plugin

Record:

- Commit hash.
- Version.
- License.
- Relevant source paths.
- Relevant test paths.
- Date pinned.

Do not develop parity against floating branches.

## 6.2 Source-of-truth hierarchy

When sources disagree:

1. Estimator mathematics and published scientific conditions.
2. Pinned Stata reference behavior for Stata-facing contracts.
3. Independent explicit-dummy or direct-likelihood ground truth.
4. Pinned Rust/PyFixest implementation details.
5. Performance heuristics.

A speed optimization cannot override levels 1–4.

## 6.3 Upstream reuse

Prefer to:

- Use or adapt the pure Rust `within` core through an internal adapter.
- Extract reusable PyFixest Rust kernels away from PyO3 and NumPy types.
- Port PPML and separation logic from pinned reference implementations.
- Retain all required MIT notices and attribution.

Do not make fereg’s public API depend directly on unstable upstream internal types.

---

# 7. Model specification protocol

## 7.1 Why a protocol is needed

The plugin must receive a structured model description without requiring Stata to materialize the model matrix.

Use a versioned numeric protocol. Do not depend on long JSON strings or undocumented positional conventions.

## 7.2 Candidate sample

The ado layer should use the equivalent of:

```stata
marksample touse, novarlist
```

This evaluates `if` and `in` but deliberately avoids marking out all model variables.

Rust then constructs the authoritative usable sample based on:

- Missing outcomes.
- Missing regressors.
- Invalid factor sources.
- Missing FE values.
- Missing clusters.
- Invalid weights.
- Missing offset or exposure.
- Singleton deletion.
- PPML separation.

## 7.3 Factor-variable metadata

Use Stata’s factor-variable parser to produce canonical term metadata without materializing full dummy columns.

For each ordinary model-matrix term, pass enough information for Rust to generate it:

- Term order.
- Source variable positions.
- Operator type:
  - continuous,
  - categorical,
  - categorical-by-categorical,
  - categorical-by-continuous,
  - continuous-by-continuous.
- Level values.
- Base status.
- Omitted status.
- Coefficient-column order.
- Corresponding canonical Stata coefficient name.
- Whether a source was pre-materialized because it used time-series syntax.

The coefficient names remain in Stata. Rust receives numeric column identities and returns kept/dropped status in the same order.

## 7.4 Absorbed-term metadata

For each absorbed term, pass:

- Term identifier.
- Source categorical variable positions.
- Interaction arity.
- Whether the term includes an intercept.
- Slope source variable positions.
- Requested saved-FE output variable positions.
- Canonical descriptive label.

Rust builds the actual encoded interaction.

## 7.5 Cluster metadata

For each cluster dimension, pass:

- Source variable positions.
- Interaction structure.
- Canonical cluster label.

Rust builds compact cluster codes and all required multiway intersections.

## 7.6 Variable role table

Every variable passed in the plugin varlist must have a role code such as:

```text
candidate sample
dependent variable
continuous source
categorical source
pre-materialized time-series source
weight
offset
exposure
cluster source
FE source
FE slope source
output sample marker
output residual
output FE sum
output individual FE
diagnostic drop-reason output
```

## 7.7 Result containers

Before the plugin call, Stata creates:

- Coefficient matrix.
- Covariance matrix.
- Coefficient-status matrix.
- Fit-statistics matrix.
- Solver-diagnostics matrix.
- FE metadata matrix.
- Cluster metadata matrix.
- Requested output variables.
- Final sample marker.

Rust fills these only after the estimation succeeds.

## 7.8 Protocol versioning

Every plugin call must include:

- Ado version.
- Plugin version.
- Model-protocol major version.
- Model-protocol minor version.

Reject incompatible versions before reading the data.

---

# 8. Native data loading and model construction

## 8.1 One native loading phase

Read all required original source columns during one native loading phase.

A small preliminary scan to count candidate observations is permitted only if measured memory benefits justify it. Do not perform repeated SPI reads during estimation.

## 8.2 Preserve Stata row order

Create:

```rust
obs_map: Vec<u64>
```

mapping native row positions to Stata observation numbers.

Never sort or reorder the Stata dataset.

Native internal arrays may use their own permutations, but final outputs must be mapped back to original observation order.

## 8.3 Role-specific loading

Do not blindly copy every source variable as `f64`.

Examples:

- Continuous regressors: store as `f64`.
- Outcome: store as `f64`.
- Weights: store as `f64`.
- Numeric categorical FE: factorize directly into `u32`.
- String categorical FE: factorize directly into `u32` while reading.
- Cluster variables: factorize directly into `u32`.
- Interaction sources: retain encoded source codes, not concatenated strings.
- Model-matrix factor sources: retain compact codes and level metadata.

## 8.4 Native factorization

Factorization must:

- Be collision-safe.
- Be invariant to row order.
- Produce contiguous zero-based `u32` codes when possible.
- Support numeric and fixed-width string FEs.
- Avoid retaining full string copies after encoding.
- Preserve level metadata needed for saved fixed effects.
- Reject unsupported `strL` behavior explicitly unless fully implemented.
- Never truncate or hash without collision verification.

## 8.5 Categorical interactions

Build interaction codes from tuples of already encoded sources.

Do not:

- Build concatenated string keys.
- Require `egen group()`.
- Store all source tuples after final interaction coding unless required for output metadata.

## 8.6 Ordinary factor-variable columns

Rust must generate ordinary factor-variable model columns.

For a term such as:

```stata
i.industry##c.age
```

Stata passes canonical metadata. Rust constructs:

- Retained industry indicator columns.
- `age`.
- Retained industry-by-age columns.

In `memory(low)`, generate factor columns in blocks, process them, and discard them as soon as possible.

## 8.7 Time-series exception

For version 1.0, Stata may create temporary variables for complex time-series expressions. These variables are passed as continuous source columns.

Do not use this exception for:

- Plain factor variables.
- FE interactions.
- Cluster interactions.
- Absorbed slopes.
- Ordinary categorical interactions.

## 8.8 Missing values and validation

Rust is authoritative for:

- Missing outcomes.
- Missing ordinary regressors.
- Missing FE values.
- Missing cluster values.
- Missing weights.
- Invalid weight sign or scale.
- Missing offsets.
- Invalid exposure values.
- PPML negative outcomes.
- Nonfinite numeric values.

Return clear drop or error reasons consistent with the reference command.

## 8.9 Drop-reason mask

Maintain an observation-level reason code:

```text
0 retained
1 excluded by if/in
2 missing outcome
3 missing regressor
4 invalid or missing weight
5 missing FE source
6 missing cluster
7 missing offset/exposure
8 singleton
9 FE separation
10 simplex separation
11 IR separation
12 mu separation
13 other model-specific exclusion
```

Exact reason precedence must be documented.

Optionally write this mask to a user-requested diagnostic variable.

---

# 9. Native data model

## 9.1 Core prepared-data object

```rust
struct PreparedData {
    n_original: usize,
    n_candidate: usize,
    n_active: usize,

    obs_map: Vec<u64>,
    active: BitVec,

    y: Vec<f64>,
    x_sources: SourceStore,
    user_weights: Option<Vec<f64>>,
    offset: Option<Vec<f64>>,

    fe_design: FeDesign,
    clusters: ClusterDesign,
    model_terms: ModelTermGraph,
    drop_reasons: Vec<u8>,
}
```

## 9.2 FE representation

```rust
struct EncodedFactor {
    codes: Vec<u32>,
    n_levels: usize,
    metadata: LevelMetadata,
}

struct EffectTerm {
    factor: Arc<EncodedFactor>,
    intercept: bool,
    slopes: Vec<SourceColumn>,
}

struct FeDesign {
    effects: Vec<EffectTerm>,
    connected_components: ComponentInfo,
    topology_fingerprint: Fingerprint,
}
```

## 9.3 Separate symbolic and weighted state

```rust
struct FeSymbolicState {
    design: FeDesign,
    permutations: Permutations,
    domains: DomainStructure,
    component_info: ComponentInfo,
}

struct FeNumericState {
    weights: Vec<f64>,
    group_denominators: GroupBuffers,
    preconditioner: Option<Preconditioner>,
    solver_workspace: SolverWorkspace,
}
```

PPML must reuse symbolic state across IRLS iterations.

Only weight-dependent numeric structures may be rebuilt.

---

# 10. Memory architecture

## 10.1 Memory is a release contract

Every major array must be included in a documented peak-memory model.

Track:

- Estimated peak native allocation.
- Observed peak native allocation.
- Process peak RSS.
- Scratch-file size.
- Per-stage allocation high-water marks.

## 10.2 Memory modes

### `memory(fast)`

Use:

- Fully resident continuous source data.
- Fully resident transformed regressor matrix where affordable.
- Batched RHS fixed-effect solves.
- Parallel column processing.
- Resident cluster-score workspaces.

Optimize for wall-clock speed.

### `memory(low)`

Use:

- Regressor blocks of size `poolsize`.
- Blockwise native factor-column generation.
- Blockwise FE residualization.
- Streaming cross-products.
- Temporary binary scratch storage for transformed regressors when needed.
- Streaming residual and cluster-score calculations.
- A bounded number of RHS workspaces.

Target memory complexity:

```text
O(N × poolsize + N × G + K² + level buffers)
```

instead of:

```text
O(N × K)
```

### `memory(auto)`

Before allocating:

1. Estimate peak fast-mode memory.
2. Estimate peak low-mode memory.
3. Query available system memory where reliable.
4. Reserve safety headroom.
5. Choose a mode.
6. Record the decision and estimates.
7. Fail before allocation if neither mode is safe.

## 10.3 `fereg` memory policy

In fast mode:

- Load `y` and generated `X`.
- Residualize `[y, X]` in place.
- Do not retain an unnecessary untransformed copy.
- Release original source columns once no longer needed.
- Compute regression, VCE, and predictions from transformed data.
- Recover FEs only if requested.

In low-memory mode:

1. Residualize `y`.
2. Generate and residualize X blocks.
3. Write transformed blocks to scratch or stream them.
4. Accumulate `X'X` and `X'y`.
5. Solve for `beta`.
6. Stream transformed blocks again to compute residuals and scores.
7. Remove scratch data.

## 10.4 `feppml` memory policy

PPML requires original finite regressors across IRLS iterations.

Fast mode may retain:

- Original `X`.
- One `X_tilde` workspace.
- IRLS vectors.
- FE workspaces.

It must not allocate a new `X_tilde` on each iteration.

Low-memory mode must:

1. Retain compact original source data.
2. Generate X blocks on demand.
3. Residualize each block under current IRLS weights.
4. Accumulate cross-products.
5. Stream blocks to construct the finite linear predictor.
6. Reuse one bounded block workspace.

## 10.5 Scratch files

Scratch files must:

- Use a path created or approved by the ado layer.
- Have a versioned binary format.
- Be removed on success.
- Be removed on ordinary error.
- Be protected by RAII cleanup in Rust.
- Include enough metadata to detect stale or incompatible files.
- Never be presented as a user-visible dependency.

## 10.6 Allocation discipline

- Use fallible reservation for large buffers.
- Preflight all large allocations.
- Avoid allocation inside MAP, LSMR, or IRLS inner loops.
- Reuse group buffers.
- Reuse cross-product buffers.
- Reuse score buffers.
- Avoid thread-private level buffers when their aggregate size exceeds a configured budget.
- Return a Stata error instead of allowing an allocation abort where possible.
- Add allocation-failure injection tests.

---

# 11. Fixed-effect preprocessing

## 11.1 Iterative singleton deletion

Implement queue-based fixed-point pruning:

1. Count active observations per relevant FE level.
2. Enqueue singleton levels.
3. Drop affected observations.
4. Decrement counts in every connected FE dimension.
5. Enqueue newly created singleton levels.
6. Continue until stable.

Match the pinned reference behavior for:

- Weight types.
- Frequency weights.
- Interactions.
- Slope-only terms.
- `keepsingletons`.
- PPML sequencing with separation.

Return:

- Initial candidate size.
- Number dropped.
- Cascade rounds.
- Final active size.
- Drop-reason mask.

## 11.2 Connected components

Compute connected components where required for:

- Identification.
- Fixed-effect normalization.
- Degrees of freedom.
- Solver diagnostics.
- Mobility-group output.

Do not mistake nonunique FE coefficients for nonunique fitted values.

## 11.3 Final sample compaction

After missingness, separation, and singleton deletion stabilize:

- Compact all native arrays once.
- Re-encode active FE levels if this materially improves memory and speed.
- Preserve a map to original Stata observations.
- Do not repeatedly compact during IRLS.

---

# 12. Fixed-effect solver layer

## 12.1 Required paths

### Closed-form one-way solver

For one intercept-only FE:

- Compute weighted group means directly.
- Residualize each RHS with one accumulation and one scatter pass.
- Use this rather than a general iterative solver.

### Native accelerated MAP

Implement:

- Weighted projections.
- Intercept FE blocks.
- Intercept-plus-slope blocks.
- Slope-only blocks.
- Symmetric Kaczmarz default sweeps.
- Batched RHS processing.
- Conjugate-gradient acceleration where valid.
- Reused work buffers.
- Periodic convergence checks.
- Strict final orthogonality certificate.

Do not merely copy an unaccelerated alternating-demeaning loop and assume native compilation is sufficient.

### Modified LSMR

Integrate a pinned `within` implementation through an internal adapter.

Require support for:

- Weighted solves.
- Batched RHS solves.
- Varying slopes.
- Diagonal preconditioning.
- Additive-Schwarz preconditioning.
- Persistent symbolic structure.
- Unidentified-direction reporting.
- Solver diagnostics.

## 12.2 Automatic solver selection

`solver(auto)` must be deterministic and recorded.

Suggested policy:

1. One intercept FE: closed form.
2. Compute cheap FE graph diagnostics.
3. Run a small MAP pilot.
4. Measure actual contraction.
5. Estimate remaining MAP work.
6. Prefer MAP for dense, well-connected systems.
7. Prefer LSMR for sparse or weakly connected systems.
8. Select preconditioner based on setup-cost and graph diagnostics.
9. Permit one certified fallback if the first method stalls.
10. Record:
   - initial choice,
   - reason,
   - pilot result,
   - fallback if any,
   - setup time,
   - iterations,
   - final certificate.

No fallback may be silent.

## 12.3 PPML repeated-weight solves

Investigate and implement safe separation of:

- Immutable FE topology.
- Symbolic domain decomposition.
- Weight-dependent numerical factors.

Preferred API:

```rust
solver.update_weights(new_weights)
```

Reuse only state that remains mathematically valid.

If safe weight updates are not available:

- Rebuild weight-dependent preconditioners.
- Measure rebuild time.
- Use MAP when repeated LSMR setup is uneconomical.
- Never reuse a stale preconditioner for speed.

## 12.4 Parallelization

Use a local Rayon pool per plugin call.

Select one dominant parallel axis at a time:

- Across RHS columns.
- Across observation chunks.
- Across independent cluster subsets.
- Across groups only when memory-safe.

Avoid nested parallelism.

For reductions:

- Use deterministic chunk boundaries.
- Use thread-private buffers only when they fit a configured memory/cache budget.
- Merge buffers in a fixed order where practical.
- Avoid uncontrolled floating-point atomics in the default path.

## 12.5 Convergence certificates

Every FE solve must certify:

```text
D' W r ≈ 0
```

including each intercept and slope channel.

Record:

- Absolute infinity norm.
- Relative infinity norm.
- Relative two-norm.
- Internal solver stopping status.
- Final certificate status.

A failed final certificate means nonconvergence.

---

# 13. `fereg` implementation

## 13.1 Pipeline

```text
parse in Stata
→ one plugin call
→ load candidate source variables
→ validate and encode
→ remove singletons
→ compact sample
→ generate finite model matrix in Rust
→ residualize [y, X]
→ certify FE orthogonality
→ standardize transformed X
→ detect collinearity
→ solve finite regression
→ compute residuals
→ compute VCE and DoF
→ compute fit statistics
→ recover requested FEs
→ final consistency checks
→ write outputs
→ post e() in Stata
```

## 13.2 Dense finite regression

Use a stable rank-aware solver:

- Column scaling.
- Pivoted Cholesky or LDLT for the normal well-conditioned path.
- Rank-revealing QR fallback.
- No explicit unpivoted matrix inverse.
- Explicit collinearity tolerance.
- Stable undoing of standardization.

An explicitly requested `fastregress`-style path may use normal equations more aggressively, but it must be labeled less stable.

## 13.3 Collinearity status

Return status for every canonical finite regressor:

```text
kept
base level
omitted by Stata syntax
collinear with fixed effects
collinear after residualization
numerically rank-deficient
```

Stata uses this to construct coefficient names and omitted markers.

## 13.4 Variance estimation

Implement natively:

- Unadjusted.
- Robust.
- One-way clustered.
- Multiway clustered through inclusion–exclusion.

Do not return transformed data to Stata for VCE.

## 13.5 Degrees of freedom

Implement a centralized DoF policy matching the pinned reference:

- One-FE exact loss.
- Two-FE connected-component adjustments where applicable.
- Multi-FE conservative approximation.
- Heterogeneous-slope adjustments.
- FE nesting within clusters.
- Cluster finite-sample corrections.
- Requested `dof()` options.

## 13.6 Fit statistics

Return the reference-compatible set including:

- `N`.
- Rank.
- Residual DoF.
- Absorbed DoF.
- RSS.
- TSS.
- Within TSS.
- \(R^2\).
- Adjusted \(R^2\).
- Within \(R^2\).
- Adjusted within \(R^2\).
- Root MSE.
- F or Wald statistic where defined.
- FE-level counts.
- Cluster counts.
- Singleton count.
- Mobility-component count where applicable.

## 13.7 Fixed-effect recovery

Only run when requested.

Requirements:

- Use the final fit relation.
- Apply an explicit normalization within each connected component.
- Report unidentified directions.
- Verify reconstructed absorbed contribution.
- Write only requested outputs.
- Preserve missing values outside the final sample.

---

# 14. `feppml` implementation

## 14.1 Separation is part of the estimator

The production command must implement:

- FE separation.
- Simplex separation.
- Iterated rectifier separation.
- Mu-based separation.

The default order must match the pinned reference.

Maintain exact observation-level drop reasons.

## 14.2 PPML pipeline

```text
parse in Stata
→ one plugin call
→ load and validate candidate data
→ encode FE and clusters
→ separation sequence
→ singleton fixed point
→ final sample compaction
→ initialize eta and mu
→ IRLS loop entirely in Rust
    → working weights
    → working outcome
    → repeated weighted FE absorption
    → finite regression
    → candidate eta and mu
    → deviance
    → step-halving when required
    → convergence check
→ strict final weighted solve
→ FE and finite-score certificates
→ robust or clustered VCE
→ fit statistics
→ requested FE recovery
→ write outputs
→ post e() in Stata
```

## 14.3 Persistent PPML workspace

Allocate once:

```rust
struct PpmlWorkspace {
    eta: Vec<f64>,
    mu: Vec<f64>,
    working_weights: Vec<f64>,
    sqrt_weights: Vec<f64>,
    z: Vec<f64>,
    z_tilde: Vec<f64>,
    residual: Vec<f64>,
    xb: Vec<f64>,
    x_block: MatrixBlock,
    x_tilde_block: MatrixBlock,
}
```

Do not allocate these vectors on every iteration.

## 14.4 IRLS behavior

Implement:

- Reference-compatible initialization.
- Reference-compatible relative-deviance convergence.
- Inner FE tolerance schedule.
- Warm starts.
- Safe acceleration.
- Step-halving.
- Maximum-iteration handling.
- Strict final alignment of:
  - coefficients,
  - `eta`,
  - `mu`,
  - working weights,
  - transformed regressors,
  - residuals,
  - VCE.

Do not return transformed state from an earlier unaligned iteration.

## 14.5 Inner FE tolerance

A looser FE tolerance may be used early, but:

- It must have a documented upper bound.
- It must tighten near convergence.
- It must never silently exceed the requested scientific tolerance at the final solve.
- The last solve must use the strict requested tolerance.
- Final score and FE certificates must pass.
- High verbosity must report the per-iteration tolerance.

## 14.6 Numerical safeguards

- Validate nonnegative outcome.
- Validate exposure strictly positive where required.
- Use stable exponential evaluation.
- Reject NaN and infinity.
- Detect overflow before it corrupts state.
- Do not silently clip fitted means in a way that changes the estimator.
- Require objective improvement or acceptable converged deviance behavior.
- Fail if step-halving cannot find a valid update.
- Fail if final first-order conditions do not pass.

## 14.7 PPML covariance

Compute robust and clustered covariance in Rust from the final aligned state.

Do not rerun model preparation in Stata.

## 14.8 PPML fixed effects

Distinguish:

- Finite-regressor linear prediction.
- Offset or exposure contribution.
- Sum of absorbed fixed effects.
- Individual normalized FE terms.
- Full linear predictor.
- Fitted mean.

Prediction semantics must match the documented Stata command contract.

---

# 15. Separation implementation plan

## 15.1 Reference mapping

Create `docs/upstream_map.md` mapping reference functions and tests to Rust modules.

Inspect at minimum:

```text
ppmlhdfe:
    main estimator source
    helper functions
    iterated-rectifier source
    simplex source
    separation fixtures
    complete test suite

PyFixest:
    Poisson IRLS source
    separation source
    Poisson tests
```

Use the Stata reference as the primary Stata behavioral oracle.

## 15.2 Separation result object

```rust
struct SeparationResult {
    dropped_fe: BitVec,
    dropped_simplex: BitVec,
    dropped_ir: BitVec,
    dropped_mu: BitVec,
    iterations: SeparationIterations,
    converged: bool,
    diagnostics: SeparationDiagnostics,
}
```

## 15.3 Separation release gate

`feppml` must remain explicitly experimental until:

- Every pinned separation fixture passes.
- The retained sample matches the reference exactly.
- Nonseparated models do not suffer false drops.
- Separation nonconvergence returns an error.
- Default sequencing matches.
- Coefficients and deviance match on the final sample.
- All methods have independent small-case tests.

---

# 16. Variance and cluster architecture

## 16.1 Observation scores

Compute scores in Rust using the final transformed or PPML state.

Do not construct an `N × K × C` object.

## 16.2 Cluster aggregation

For each cluster subset:

1. Encode cluster tuples natively.
2. Accumulate score vectors by cluster.
3. Form the meat from cluster-score outer products.
4. Apply the required finite-sample correction.
5. Add or subtract according to inclusion–exclusion parity.

Reuse buffers across subsets.

## 16.3 Multiway clustering

Support the number of cluster dimensions accepted by the pinned reference, subject to the explicit 1.0 compatibility matrix.

Guard against exponential subset growth with a clear limit and error.

## 16.4 Nesting

Detect FE dimensions nested within cluster dimensions without Stata temporary interactions.

Feed nesting results into:

- Absorbed DoF.
- Residual DoF.
- Cluster finite-sample corrections.
- Stored metadata.

---

# 17. Prediction and postestimation

## 17.1 Interface split

Stata owns commands such as:

```stata
predict
margins
estat
test
```

Rust performs observation-scale calculations.

## 17.2 `fereg` prediction

Support the agreed reference-compatible statistics, potentially including:

- `xb`.
- Absorbed contribution.
- `xbd`.
- Residuals.
- Scores.
- Standard prediction error where supported.

For 10 million or more observations, avoid interpreted ado loops.

## 17.3 `feppml` prediction

Support the agreed reference-compatible statistics, including those required by `margins`:

- Finite `xb`.
- Full linear predictor.
- Absorbed contribution.
- Fitted mean.
- Deviance or response residuals where supported.
- Scores.

## 17.4 Native postestimation state

Do not require an unsafe persistent pointer to native estimation state.

Prefer to save the minimal required Stata-side outputs at estimation time:

- Coefficients.
- Covariance.
- Final sample.
- Absorbed sum if requested or required.
- Offset/exposure metadata.
- Required normalization metadata.

A native cache handle may be considered only after version 1.0 and only with explicit lifecycle management.

---

# 18. Safety architecture

## 18.1 Unsafe-code boundary

Use:

```rust
#![forbid(unsafe_code)]
```

throughout the scientific core.

Confine unavoidable unsafe code to the FFI/SPI layer.

The unsafe surface must be small enough for line-by-line review.

## 18.2 Panic boundary

At the exported dispatch function:

1. Validate protocol and pointers.
2. Enter `catch_unwind`.
3. Convert all normal errors to typed `Result`.
4. Catch unexpected panics.
5. Convert them into a nonzero Stata return code.
6. Never allow a Rust panic to cross the C ABI.

## 18.3 SPI thread rule

All SPI functions must run on the Stata calling thread.

The `SpiSession` wrapper must not be `Send` or `Sync`.

Worker threads operate only on native arrays.

## 18.4 Atomic output

Rust must compute into shadow buffers.

Only after all checks pass may it write:

- Final `e(sample)` marker.
- Requested residuals.
- Fitted values.
- FE outputs.
- Coefficients.
- Covariance.
- Diagnostics.

On error:

- No successful estimate is posted.
- No partial output variable is left looking valid.
- Temporary Stata objects are cleaned by the ado error path.
- Scratch files are removed.

## 18.5 Memory failure

Preflight large allocations and use fallible reserve paths where possible.

A model that cannot fit safely in memory must return a useful error containing:

- Estimated fast-mode peak.
- Estimated low-mode peak.
- Available memory estimate.
- Suggested `memory(low)` or smaller `poolsize()`.

---

# 19. Performance instrumentation

## 19.1 Required stage timings

`timeit` must report:

```text
Stata syntax and metadata preparation
Stata output allocation
SPI input read
native validation
factorization and interaction encoding
ordinary model-matrix generation
singleton deletion
separation
sample compaction
FE symbolic setup
preconditioner setup
FE solve
dense regression
each PPML iteration
VCE
DoF
fixed-effect recovery
SPI output write
Stata ereturn posting
total
```

Also expose machine-readable timing scalars for benchmark scripts.

## 19.2 Required diagnostic metrics

Record:

- Candidate and final `N`.
- Number of source variables read.
- Number of generated finite columns.
- FE dimensions and levels.
- Singleton drops.
- Separation drops by method.
- Solver selected.
- Solver iterations.
- Pilot contraction.
- Preconditioner type.
- Preconditioner setup time.
- Final FE certificate.
- PPML outer iterations.
- Step-halving count.
- Peak native allocation estimate.
- Scratch-file size.
- Thread count.
- CPU feature path.

---

# 20. Performance contracts

## 20.1 Benchmark validity

A performance comparison is valid only when both commands use:

- The same original data.
- The same candidate sample.
- The same final estimation sample.
- The same weight semantics.
- The same singleton policy.
- The same separation methods.
- The same FE specification.
- The same VCE.
- The same DoF policy.
- The same tolerance.
- The same requested outputs.
- The same permitted thread count.

Do not benchmark fereg with full inference against a reference run using `dof(none)` or a cheaper VCE.

## 20.2 Stata-side overhead gate

On approved benchmarks with at least 10 million observations:

- Stata preparation before the plugin call, excluding unavoidable `if` evaluation and explicitly documented time-series materialization, must be less than 10% of end-to-end runtime.
- Stata postprocessing must be less than 5% of end-to-end runtime.
- Stata must not materialize FE codes, FE interactions, cluster interactions, or residualized regressors.

## 20.3 Data-transfer gate

On large absorption-dominated benchmarks:

- SPI input and output transfer should account for less than 20% of total runtime.
- If the limit is not achieved, include a measured bandwidth analysis proving the SPI is the bottleneck and document the best achievable path.

## 20.4 `fereg` speed gate

For approved large benchmarks with \(N \ge 10\) million:

- Geometric-mean end-to-end time must be no more than 40% of `reghdfe`.
- This corresponds to at least a 2.5× geometric-mean speedup.
- At least 80% of approved large cases must be at least 2× faster.
- Dense absorption-dominated cases should target 4–8×.
- Sparse weakly connected cases should target at least 2×.
- Single-thread execution should show at least a 1.5× geometric-mean speedup.
- No approved case may be more than 25% slower without a documented cause and tracked optimization issue.

## 20.5 `feppml` speed gate

For approved large PPML benchmarks:

- Geometric-mean end-to-end time must be no more than 50% of `ppmlhdfe`.
- This corresponds to at least a 2× geometric-mean speedup.
- At least 75% of approved large cases must be at least 2× faster.
- Absorption-dominated cases should target at least 3×.
- Separation, final strict solve, VCE, and default DoF must be included.

## 20.6 Memory gate

For common cases with moderate \(K\):

- Peak RSS should be no greater than the reference in at least 80% of approved cases.
- No case may exceed 1.25× reference peak RSS without automatically selecting low-memory mode or documenting that the reference performed less work.
- Observed native peak allocation must be within 25% of the documented memory model.
- PPML memory must not grow with iteration count.
- Repeated estimator calls must not leak memory.
- Low-memory mode must complete at least one model that fast mode cannot complete under an imposed RAM limit.

---

# 21. Benchmark suite

## 21.1 Dataset sizes

Where feasible:

```text
100,000
1,000,000
10,000,000
50,000,000
100,000,000 observations
```

## 21.2 Required linear designs

- One dense FE.
- Two dense well-connected FEs.
- Three or four dense FEs.
- Sparse worker–firm with high mobility.
- Sparse worker–firm with low mobility.
- Multiple disconnected components.
- Singleton cascades.
- FE interactions.
- Intercept-plus-slope FE.
- Slope-only FE.
- Multiple absorbed slopes.
- Ordinary factor-variable regressors.
- Ordinary factor interactions.
- One-way cluster.
- Two-way cluster.
- Three-way cluster.
- FE nested in cluster.
- Collinear regressors.
- No finite regressors.
- All supported weights.
- Small \(K\), huge \(N\).
- Moderate \(K\), huge \(N\).
- Fast and low-memory modes.

## 21.3 Required PPML designs

- Trade-style exporter-year/importer-year/pair FEs.
- Sparse worker–firm-style design.
- Many zeros without separation.
- FE separation.
- Regressor separation.
- Combined separation.
- Simplex-specific cases.
- IR-specific cases.
- Mu-specific cases.
- Offset.
- Exposure.
- All supported weights.
- Multiway clustering.
- Heterogeneous slopes.
- No finite regressors.
- All-zero outcome.
- Very large valid outcome.
- Step-halving case.
- Ill-conditioned regressors.
- Fast and low-memory modes.

## 21.4 Reporting

Record:

- CPU.
- RAM.
- OS.
- Stata version and edition.
- Reference versions.
- Commit hashes.
- Plugin hash.
- Rust version.
- Compiler.
- Thread count.
- Cold timing.
- Warm timing.
- Median of repeated runs.
- Dispersion.
- Peak RSS.
- Final sample parity.
- Numerical parity.
- Solver diagnostics.

---

# 22. Numerical validation

## 22.1 Independent small-model oracle

For small datasets, explicitly construct dummy-variable designs and calculate:

- OLS/WLS coefficients.
- Residuals.
- Robust VCE.
- Clustered VCE.
- Fixed-effect fitted contributions.
- Poisson likelihood.
- Poisson score.
- Direct Poisson estimates.

These tests must not depend on the reference commands.

## 22.2 Differential references

Compare against:

- Pinned `reghdfe`.
- Pinned `ppmlhdfe`.
- Pinned PyFixest.
- Pinned `within`.
- Explicit-dummy calculations.

PyFixest and `within` are development references only, not runtime dependencies.

## 22.3 Exact parity requirements

Require exact agreement for:

- Final estimation sample.
- Singleton drops.
- Separation drops.
- Retained observation identities.
- Omitted finite regressors.
- Cluster counts.
- Weight interpretation.
- Success versus error status.
- Requested VCE type.
- Default separation sequence.
- DoF-policy selection.

## 22.4 Numerical tolerances

For well-conditioned models, initial targets:

```text
coefficients:       rtol 1e-7, atol 1e-10
standard errors:    rtol 1e-6, atol 1e-10
residuals/fitted:   rtol 1e-7, atol 1e-9
deviance/log-lik:   rtol 1e-8
```

For ill-conditioned models, compare:

- Fitted values.
- Residuals.
- Objective values.
- Scores.
- FE orthogonality.
- Identified linear combinations.

Do not require matching arbitrary FE normalizations.

## 22.5 Property tests

Test invariance to:

- Observation permutation.
- FE level relabeling.
- Cluster relabeling.
- Equivalent FE term ordering.
- Equivalent interaction construction.
- Frequency-weight row expansion.
- Analytic-weight scaling according to reference semantics.
- Fast versus low-memory mode.
- Thread count within documented tolerance.
- MAP versus LSMR on identified fitted values.
- Different valid FE normalizations.

## 22.6 Failure tests

Include:

- Empty candidate sample.
- One observation.
- All observations dropped.
- Missing values.
- Invalid weights.
- Negative PPML outcome.
- Zero or invalid exposure.
- More regressors than observations.
- Perfect collinearity.
- Near collinearity.
- Disconnected FE graph.
- Huge FE level count.
- Allocation failure.
- MAP nonconvergence.
- LSMR nonconvergence.
- PPML nonconvergence.
- Separation nonconvergence.
- Exponential overflow.
- NaN during IRLS.
- Protocol mismatch.
- Ado/plugin version mismatch.
- Wrong-platform binary.
- Output-write failure.
- Injected panic.

---

# 23. CI and release testing

## 23.1 Hosted CI

Every relevant pull request must run:

- `cargo fmt --check`.
- `cargo clippy` with warnings denied.
- Rust unit tests.
- Property tests.
- Mock-SPI tests.
- Fuzz smoke tests.
- Linux build.
- Windows build.
- macOS arm64 build.
- macOS x86-64 build.
- Binary dependency audit.
- License audit.
- Reproducibility metadata check.

## 23.2 Mock SPI

Implement a fake SPI capable of simulating:

- Numeric reads and writes.
- String reads.
- Candidate samples.
- Stata missing values.
- Matrices.
- Scalars.
- Macros.
- Output-write errors.
- Protocol errors.
- Interrupted reads.
- Partial failures.

## 23.3 Licensed Stata runners

Use self-hosted runners for:

- Parser tests.
- End-to-end plugin loading.
- `ereturn` parity.
- Reference-command parity.
- Prediction.
- `margins`.
- `estat`.
- Installation.
- Large benchmarks.

Run a fast subset on pull requests and the full suite nightly.

## 23.4 Clean-machine test

Test on clean machines without:

- Rust.
- Cargo.
- Python.
- Julia.
- Developer compilers.
- OpenBLAS.
- MKL.
- OpenMP runtimes.

Run:

```stata
fereg, doctor
fereg, selftest
fereg ...
feppml ...
```

## 23.5 Binary audits

Linux:

- `ldd`.
- glibc symbol-version inspection.

Windows:

- `dumpbin /DEPENDENTS`.

macOS:

- `otool -L`.
- Universal-binary architecture check.

Fail release if forbidden dynamic dependencies appear.

---

# 24. Implementation phases

## Phase 0 — Freeze contracts and prove metadata-only Stata parsing

### Tasks

- Create repository and this `AGENTS.md`.
- Pin all reference commits.
- Record licenses.
- Build compatibility matrix.
- Capture representative `ereturn list` outputs.
- Build baseline benchmark scripts.
- Measure reference speed and peak RSS.
- Prototype factor-variable metadata extraction without materializing dummy columns.
- Prototype absorbed-term metadata extraction.
- Verify candidate-sample handling using `if`/`in` without full Stata `markout`.
- Verify available Stata processor/license metadata.
- Document weight semantics.
- Write scientific, memory, and performance contracts.

### Exit gate

- References are frozen.
- The 1.0 scope is explicit.
- A metadata-only term graph can represent required ordinary factor-variable cases.
- FE and cluster interactions can be described without temporary variables.
- Baseline reference benchmarks run reproducibly.
- Unsupported features are documented.

---

## Phase 1 — Self-contained plugin and safety boundary

### Tasks

- Create Cargo workspace.
- Add SPI source.
- Implement minimal C bridge.
- Implement Rust dispatch.
- Add panic containment.
- Add typed error mapping.
- Add protocol handshake.
- Add local Rayon pool.
- Build all platform plugins.
- Implement:
  - `fereg, version`
  - `fereg, doctor`
  - `fereg, selftest`
- Implement mock SPI.
- Add binary dependency audits.

### Exit gate

- All platform plugins load.
- No Rust/Python/Julia runtime is required.
- Injected panic does not terminate Stata.
- Protocol mismatch is handled cleanly.
- Clean-machine diagnostics pass.
- Each platform uses one self-contained plugin.

---

## Phase 2 — Rust-first data ingestion and model construction

### Tasks

- Implement the model protocol.
- Implement candidate-sample loading.
- Implement row maps.
- Implement native missing-value validation.
- Implement numeric and string factorization.
- Implement categorical interactions.
- Implement cluster encoding.
- Implement ordinary factor-column generation.
- Implement time-series pre-materialized source handling.
- Implement drop-reason masks.
- Implement preflight memory estimates.
- Benchmark SPI read order.

### Exit gate

- Rust constructs the authoritative initial sample.
- Plain factor-variable models require no Stata dummy tempvars.
- FE and cluster interactions require no Stata tempvars.
- Numeric round trips are exact.
- String factorization is collision-safe.
- Data-loading speed is measured and acceptable.
- Memory estimates match observed preparation allocations.

---

## Phase 3 — FE preprocessing and solver core

### Tasks

- Implement FE term representation.
- Implement iterative singleton deletion.
- Implement connected components.
- Implement one-way weighted projection.
- Implement absorbed slopes.
- Implement accelerated MAP.
- Integrate `within` LSMR.
- Add diagonal and additive preconditioners.
- Implement auto solver pilot.
- Implement final FE certificates.

### Exit gate

- Explicit-dummy projection tests pass.
- Weighted intercept and slope certificates pass.
- Singleton sample matches reference.
- Dense and sparse difficult graphs converge.
- Auto solver chooses reasonable paths.
- No allocation occurs in solver inner loops.
- Solver diagnostics are complete.

---

## Phase 4 — Production `fereg`

### Tasks

- Implement full FWL pipeline.
- Implement native model-matrix block generation.
- Implement stable dense solve.
- Implement collinearity status.
- Implement all required weight semantics.
- Implement robust VCE.
- Implement multiway cluster VCE.
- Implement DoF policy.
- Implement fit statistics.
- Implement FE recovery.
- Implement result posting.
- Implement prediction.
- Implement help and compatibility documentation.

### Exit gate

- Required estimation-sample parity passes.
- Coefficients and VCE pass.
- DoF passes.
- Saved residual and FE outputs pass.
- Postestimation passes.
- Unsupported options error explicitly.
- No large numerical work remains in Stata.

---

## Phase 5 — OLS memory and performance hardening

### Tasks

- Profile all stages.
- Eliminate avoidable copies.
- Tune SPI loading.
- Tune block sizes.
- Tune thread-reduction strategies.
- Implement scratch-based low-memory mode.
- Implement `memory(auto)`.
- Add runtime SIMD only where measured.
- Tune solver auto-selection.
- Run full benchmark suite.

### Exit gate

- `fereg` speed gate passes.
- Memory gate passes.
- Stata-side overhead gate passes.
- Fast and low-memory results agree.
- Constrained-RAM benchmark succeeds.
- No scientific check was weakened.

---

## Phase 6 — PPML IRLS core

### Tasks

- Implement Poisson family.
- Implement initialization.
- Implement persistent IRLS workspace.
- Implement repeated weighted absorption.
- Implement weight-update path.
- Implement inner-tolerance schedule.
- Implement warm starts.
- Implement step-halving.
- Implement offset and exposure.
- Implement strict final solve.
- Implement score and FE certificates.
- Implement robust and clustered VCE.
- Port nonseparated PPML tests.

### Exit gate

- Known nonseparated models match direct Poisson and the reference.
- Offset, exposure, and weight tests pass.
- Final state alignment passes.
- Nonconvergence fails closed.
- No per-iteration memory growth occurs.
- `feppml` remains marked experimental.

---

## Phase 7 — Complete PPML separation

### Tasks

- Port FE separation.
- Port simplex separation.
- Port iterated rectifier.
- Port mu separation.
- Implement reference default sequence.
- Integrate separation with singleton deletion.
- Port all separation fixtures.
- Add exact drop-reason reporting.

### Exit gate

- Every separation fixture passes.
- Final sample parity is exact.
- No known false positive or false negative remains.
- Separation nonconvergence fails closed.
- Experimental status may be removed only after this gate.

---

## Phase 8 — PPML postestimation, memory, and performance

### Tasks

- Implement PPML fit statistics.
- Implement FE recovery.
- Implement prediction.
- Implement `margins` support.
- Implement required `estat`.
- Implement PPML low-memory mode.
- Optimize repeated weight updates.
- Tune MAP versus LSMR selection.
- Tune separation performance.
- Run full PPML benchmark suite.

### Exit gate

- Postestimation tests pass.
- `feppml` speed gate passes.
- Memory gate passes.
- Stata-side overhead gate passes.
- No runtime dependency or sidecar exists.

---

## Phase 9 — Release candidate

### Tasks

- Run complete cross-platform suite.
- Run clean-machine installation tests.
- Freeze compatibility matrix.
- Freeze help files.
- Produce numerical parity report.
- Produce speed report.
- Produce memory report.
- Produce license and SBOM report.
- Package platform binaries.
- Sign/notarize where needed.
- Create checksums and release metadata.

### Exit gate

- No release-blocking correctness issue.
- No known crash path.
- No memory leak.
- No installation dependency.
- All release gates pass.
- Documentation accurately states supported and unsupported behavior.

---

# 25. Recommended pull-request sequence

1. Repository, reference lock, compatibility matrix, and baseline benchmarks.
2. Metadata-only factor-variable and absorb parser prototype.
3. Rust workspace and cross-platform hello plugin.
4. Panic boundary and mock SPI.
5. Version, doctor, self-test, and binary audit.
6. Versioned model protocol.
7. Candidate-sample and numeric loading.
8. String factorization and native interactions.
9. Native ordinary factor-column generation.
10. Memory preflight and allocation instrumentation.
11. One-way weighted FE projection.
12. Singleton fixed-point deletion.
13. Connected components and FE certificates.
14. Accelerated MAP.
15. `within` adapter and LSMR.
16. Auto solver and certified fallback.
17. Native model-matrix block engine.
18. Dense regression and collinearity.
19. Robust covariance.
20. Multiway clustered covariance.
21. DoF policy.
22. First end-to-end `fereg`.
23. `e()` results, prediction, and FE recovery.
24. OLS low-memory mode.
25. OLS performance gate.
26. PPML family and IRLS.
27. PPML weight-update optimization.
28. PPML offset, exposure, and weights.
29. PPML final certificates and VCE.
30. FE separation.
31. Simplex separation.
32. Iterated-rectifier separation.
33. Mu separation.
34. PPML prediction and postestimation.
35. PPML low-memory mode.
36. PPML performance gate.
37. Release packaging and clean-install certification.

Each PR must contain:

- Tests.
- Documentation.
- Before/after timings where relevant.
- Before/after peak-memory measurements where relevant.
- An explanation of any numerical tolerance change.

---

# 26. Codex operating rules

Codex must:

1. Read `AGENTS.md`, `references.lock.toml`, and relevant ADRs before editing.
2. Work only on the current phase or named PR.
3. Keep Stata as control plane and Rust as data/computation plane.
4. Put all observation-scale estimation work in Rust.
5. Preserve the one-plugin-call estimation architecture.
6. Never add a runtime dependency.
7. Never introduce a native sidecar library.
8. Never make SPI calls from worker threads.
9. Keep unsafe code confined to the FFI layer.
10. Never let a panic cross the C ABI.
11. Never post partial estimation results.
12. Never weaken singleton, separation, convergence, VCE, or DoF behavior for speed.
13. Never widen a numerical tolerance merely to make a parity test pass.
14. Never change frozen reference outputs without documented evidence that the reference capture was wrong.
15. Pin all dependencies and commit `Cargo.lock`.
16. Do not depend on floating Git branches.
17. Add attribution when adapting upstream code.
18. Add a regression test for every bug.
19. Add a benchmark for every optimization.
20. Measure peak memory for every change affecting large arrays.
21. Avoid allocation in numerical inner loops.
22. Preserve deterministic diagnostics and a deterministic execution option.
23. Keep `main` loadable in Stata.
24. Update the compatibility matrix whenever behavior changes.
25. Error explicitly on unsupported options.
26. Treat reference tests as scientific evidence, not merely coverage.
27. Stop and document a contract conflict rather than silently guessing.
28. Do not optimize peripheral display code before estimation kernels meet their gates.
29. Do not use Stata tempvars for FE, cluster, or ordinary factor expansion unless an ADR explicitly permits a bounded exception.
30. Preserve easy one-command installation as a product requirement.

---

# 27. Definition of done for fereg 1.0

## Installation

- [ ] Users do not need Rust, Python, Julia, a compiler, BLAS, or OpenMP.
- [ ] Each supported platform loads one self-contained plugin.
- [ ] Windows x86-64 passes.
- [ ] Linux x86-64 passes.
- [ ] Intel macOS passes.
- [ ] Apple-Silicon macOS passes.
- [ ] `fereg, doctor` passes on clean machines.
- [ ] Ado/plugin mismatches are detected.
- [ ] Binary dependency audits pass.

## Rust-first architecture

- [ ] Each estimation makes one substantive plugin call.
- [ ] Rust constructs the authoritative final sample.
- [ ] Rust encodes FEs and clusters.
- [ ] Rust generates ordinary factor-variable columns.
- [ ] Rust builds FE and cluster interactions.
- [ ] Rust performs singleton deletion.
- [ ] Rust performs separation.
- [ ] Rust performs all estimation and inference.
- [ ] Rust performs observation-scale prediction calculations.
- [ ] No residualized \(N \times K\) dataset is returned to Mata.
- [ ] Stata-side overhead gates pass.

## `fereg`

- [ ] Required categorical FE cases pass.
- [ ] Required absorbed-slope cases pass.
- [ ] Ordinary factor-variable cases pass.
- [ ] Required weights pass.
- [ ] Singleton sample parity passes.
- [ ] Robust and multiway-cluster VCE pass.
- [ ] DoF parity passes.
- [ ] Collinearity behavior passes.
- [ ] Residual and FE output pass.
- [ ] Supported prediction and postestimation pass.
- [ ] Speed gate passes.
- [ ] Memory gate passes.

## `feppml`

- [ ] Required FE and slope cases pass.
- [ ] Ordinary factor-variable cases pass.
- [ ] Required weights pass.
- [ ] Offset and exposure pass.
- [ ] FE separation passes.
- [ ] Simplex separation passes.
- [ ] IR separation passes.
- [ ] Mu separation passes.
- [ ] Default separation sequencing passes.
- [ ] Exact retained-sample parity passes.
- [ ] IRLS convergence and step-halving pass.
- [ ] Final score and FE certificates pass.
- [ ] Robust and multiway-cluster VCE pass.
- [ ] Prediction, `margins`, and required `estat` pass.
- [ ] Speed gate passes.
- [ ] Memory gate passes.

## Safety and quality

- [ ] No panic crosses the C ABI.
- [ ] No known valid or invalid input crashes Stata.
- [ ] No SPI call occurs on a worker thread.
- [ ] No repeated-call memory leak exists.
- [ ] Allocation failures return cleanly.
- [ ] No partial `e(b)` or `e(V)` is posted.
- [ ] No performance result weakens the scientific contract.
- [ ] All reused code is attributed.
- [ ] All unsupported behavior is documented.
- [ ] Release reports contain correctness, speed, and memory evidence.

---

# 28. First assignment to Codex

Begin with **Phase 0 and Phase 1 only**.

Do not implement fixed-effect estimation, OLS, or PPML yet.

The first deliverable must include:

1. Repository skeleton.
2. This specification as `AGENTS.md`.
3. `references.lock.toml`.
4. Initial compatibility matrix.
5. Baseline `reghdfe` and `ppmlhdfe` benchmark scripts.
6. A metadata-only prototype for:
   - ordinary factor variables,
   - ordinary factor interactions,
   - absorbed categorical terms,
   - absorbed slopes,
   - cluster interactions.
7. Proof that plain factor-variable columns do not need to be materialized in Stata.
8. Rust workspace.
9. Minimal C bridge.
10. Self-contained plugin for each target.
11. `fereg, version`.
12. `fereg, doctor`.
13. `fereg, selftest`.
14. Panic containment.
15. Mock-SPI tests.
16. Linux, Windows, and macOS build scripts.
17. Dynamic dependency audits.
18. Clean-machine installation test plan.
19. Architecture documentation proving:
    - one-call estimation is feasible,
    - all Rust code can be linked into one plugin per platform,
    - no runtime sidecar is required,
    - SPI calls remain on the Stata thread,
    - Stata can pass a model term graph without constructing the full model matrix.

The initial pull request must not claim estimator functionality. Its purpose is to prove the Rust-first architecture, Stata metadata layer, safety boundary, cross-platform build, and dependency-free installation model before scientific estimation code is added.