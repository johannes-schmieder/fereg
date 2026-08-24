# Plugin action and result protocol

The C ABI exports only Stata's required `pginit` and `stata_call`; the bridge
forwards to Rust's `stfixest_dispatch`. Scientific logic remains in Rust.

The action handshake, model wire, and OLS result wire are independently
versioned. Current hidden OLS transport requires action protocol 1.1, model wire
2.1, and result wire 1.1. No handshake performs a lossy conversion between
wire versions.

## Public actions

- `version`, `doctor`, and `selftest` expose bounded diagnostics and native
  smoke tests.
- `panic-test` is guarded and exists only to prove panic containment.
- Public `estimate-ols`, `estimate-ppml`, `stfeols`, and `stfepois` remain
  unsupported and return `r(498)`.

## Developer preparation action

`prepare-ols-test` is a guarded integration boundary, not an estimator. It
validates the complete request and output layout before observation reads,
loads each bound numeric source once, applies candidate/sample/weight rules,
performs supported singleton pruning and final compaction, rebuilds categorical
recipes/topology, and writes only private diagnostics.

The accepted bounded surface includes continuous regressors, numeric factor
main effects, all-categorical numeric interactions, unit/a/f/p objectives, and
unnamed intercept-only numeric FEs. Importance weights, strings, time-series
materialization, general absorbed slopes, saved FEs, and public estimation are
not enabled.

The optional hidden MAP check constructs one checked/fallibly reserved Y/X
batch, consumes it through strict borrowed MAP, discards the numerical result,
and writes diagnostics only after a successful final certificate.

## Hidden OLS result transport

`estimate-ols-test` is developer-only. Before reads it requires:

- exact action/model/result wire identities and loaded-plugin identity;
- `confirm=developer-estimation-test`;
- valid numeric bindings, candidate marker, cluster metadata, and nonaliasing
  private output layouts;
- an initially incomplete receipt.

For accepted surfaces it prepares one authoritative sample and selects exactly
one certified iid, HC1, or clustered composition. Categorical final-sample
rebasing may reduce the logical coefficient graph below caller allocation
capacity; the receipt reports the logical dimensions.

The guarded intercept-plus-one-slope preparation exists, but ADR 0006 requires
positive residual DoF. Until the option-A fixture is recaptured and exact-green,
absorbed-slope inference remains a blocker. Signed residual DoF and negative iid
covariance are not eligible result-wire states.

The caller precreates these private matrices:

| Key | Logical contents |
|---|---|
| `output_b_matrix` | canonical coefficients |
| `output_v_matrix` | full covariance |
| `output_status_matrix` | coefficient status and generated-column mapping |
| `output_fit_matrix` | fit statistics and defined flags |
| `output_dof_matrix` | distinct DoF values and defined flags |
| `output_solver_matrix` | solver/certificate summary |
| `output_fe_matrix` | absorbed-effect metadata |
| `output_cluster_matrix` | cluster-dimension metadata |
| `output_subset_matrix` | cluster-subset metadata |
| `output_certificate_matrix` | independent certificates |
| `output_receipt_matrix` | wire identity, dimensions, flags, completion |

Rust validates every N/K/K² result, omission marker, matrix dimension, output
position, and C handle before the first store. SPI has no physical rollback, so
the contract is logical atomicity: all shadow cells are invalid unless the
receipt's final `COMPLETE` cell is one. Completion is the final operation; no
allocation, poll, display, computation, or store follows it.

The hidden request is carried as one canonical count/byte-length framed bundle
when Stata's option limit would be exceeded. The decoder rejects malformed,
duplicate, unknown, truncated, or mixed bundle/argv requests before SPI access.

## Hidden licensed eclass transaction

`_stfixest_estimate_ols_test` is the sole ado consumer of a complete hidden
receipt. It:

1. creates private shadows;
2. holds prior named or null estimates;
3. makes one substantive plugin call;
4. validates receipt/table/sample identities and bounded metadata;
5. copies complete b/V matrices without numerical alteration;
6. posts selected private results;
7. commits the requested residual and estimate only in the final protected
   block.

Any native error, incomplete receipt, injected store failure, validation
failure, or Control-C must restore prior `e()`/`e(sample)` state and expose no
requested residual.

## Return codes

| Class | Stata code |
|---|---:|
| Syntax | 198 |
| Unsupported/invalid option | 498 |
| Protocol mismatch | 499 |
| Negative analytic weight | 402 |
| Invalid frequency-weight domain | 401 |
| Insufficient observations or residual DoF | 2001 |
| Invalid data | 459 |
| Allocation | 909 |
| User interruption | 1 |
| Native self-test failure | 3498 |
| Internal error or caught panic | 3499 |

## Current candidate boundary

The active accepted hidden candidate is the 31-executable/9-blocked projection
at `f61fee923db2bd3544d457dfe50de8117d27adda`. The later signed-slope
32/8 receipt is historical only under ADR 0006. See `STATUS.md` for the active
checkpoint and exact evidence paths.

## Platform loader

The ado loader selects:

| Stata report | Plugin |
|---|---|
| Windows | `stfixest_windows.plugin` |
| macOS / Unix with `Mac*` machine type | `stfixest_macos.plugin` |
| other Unix | `stfixest_linux.plugin` |
| unknown | fail closed |

Licensed fixtures are successful only when their final assertion emits the
exact terminal `END_FIXTURE|<name>|rc=0` sentinel; shell status alone is not
authoritative.
