# Plugin action and result protocol

The platform plugin exports Stata's required `pginit` and `stata_call`; the C
bridge forwards to Rust's `fereg_dispatch`. Scientific logic and result
validation remain in Rust.

## 0.5.0 versions

The action handshake, model metadata, and OLS result layout are independently
versioned:

| Contract | Public 0.5.0 version |
|---|---:|
| Action protocol | 1.3 |
| Model wire | 2.1 |
| OLS result wire | 1.3 |

Public estimation requires the exact action/result versions. Diagnostic
`doctor` and `selftest` callers retain the action protocol's same-major,
not-newer-minor compatibility rule. ADO identity must be exactly 0.5.0. Model
wire 2.0 remains readable only for the bounded legacy no-absorbed-metadata
case; model wire v1, unknown majors, and newer minors are rejected.

## Actions

- `version` reports plugin/ADO versions, all wire versions, ABI, source
  commit, and build date.
- `doctor` validates the handshake and runs the native diagnostic suite.
- `selftest` runs bounded native tests; compatible diagnostic handshakes are
  accepted.
- `panic-test` is confirmation-guarded and exists only for ABI containment.
- `estimate-ols` is the one live-SPI public estimation action used by
  `fereg`.
- `prepare-ols-test` and the hidden OLS adapter remain developer qualification
  actions and are excluded from distributions.
- `estimate-ppml` is unsupported in 0.5.0; `feppml` fails in ado before a
  substantive plugin call.

## Public OLS transaction

Before any observation read, Rust validates the complete request, exact
action/model/result versions, source and output bindings, dimensions,
nonaliasing, memory/thread options, and initially incomplete receipt. Rust then
owns sample construction, singleton pruning, computation, inference,
certification, saved-output preparation, and result validation.

The caller precreates private shadows for coefficients, covariance, statuses,
fit/DoF/solver/FE/cluster/subset/certificate tables, resource/timing
instrumentation, `e(sample)`, optional residual/absorbed/saved-FE outputs, and
the receipt. Every N/K/K² value and output position is checked before stores.
SPI calls stay on the Stata thread.

SPI has no physical rollback, so logical completion is receipt based. The
`COMPLETE` cell is the final native operation; no allocation, polling,
display, computation, or store follows it. The ado transaction validates the
complete private receipt, posts `e()`, and commits requested output variables
and estimates only in its final protected block. Native error, store failure,
panic, validation failure, or Control-C restores prior `e()`/named estimates
and exposes no requested outputs.

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

Licensed fixtures pass only when process status, Stata return code, status
files, and the unique terminal `END_FIXTURE|<name>|rc=0` sentinel agree.
