# Plugin action and result protocol

The platform plugin exports Stata's required `pginit` and `stata_call`; the C
bridge forwards to Rust's `fereg_dispatch`. Scientific logic and result
validation remain in Rust.

## 0.5.0 versions

The action handshake, model metadata, OLS result layout, and additive PPML
request/result layouts are independently versioned:

| Contract | 0.5.0 version |
|---|---:|
| Action protocol | 1.5 |
| Model wire | 2.1 |
| OLS result wire | 1.4 |
| PPML request wire | 1.5 |
| PPML result wire | 1.2 |

Estimation requires the exact action and estimator-specific request/result
versions. Diagnostic `doctor` and `selftest` callers retain the action
protocol's same-major, not-newer-minor compatibility rule. ADO identity must be
exactly 0.5.0. Model wire 2.0 remains readable only for the bounded legacy
no-absorbed-metadata case; model wire v1, unknown majors, and newer minors are
rejected. The additive PPML wires do not alter or alias the OLS result wire.

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
- `estimate-ppml` is the live-SPI public estimation action used by `feppml`.
  The public wrapper delegates once through `_fereg_ppml` and the guarded
  `_fereg_estimate_ppml` transaction. Non-SPI dispatcher attempts to invoke
  `estimate-ppml` are rejected.

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

## Public PPML transaction

The PPML adapter implements the same control-plane/data-plane boundary used by
the public command. Stata parses syntax and model
metadata, constructs the candidate marker, allocates private shadows, and
makes exactly one guarded `estimate-ppml` call. Rust validates the complete
request, PPML request/result wire identities, source and output bindings,
dimensions, nonaliasing destinations, memory/thread controls, and initially
incomplete receipt before reading observations.

Request wire 1.5 has one closed `solver` field with values `auto`, `map`,
`lsmr`, and `cmg`. The retired private solver, preconditioner, mode, policy,
trace, and experiment keys are not compatibility aliases. An older exact
request version is a protocol mismatch; an unknown key on 1.5 is syntax error.
Recognized solver/topology combinations outside the staged public envelope
return `r(498)`. A supported route that fails its checked workspace preflight
returns `r(909)`; neither condition is converted to an implicit solver choice.

The live action owns sample construction, singleton and separation processing,
topology rebasing, IRLS, inference, certificates, saved-output preparation,
and complete result validation. All result values are certified in Rust-owned
memory before the first SPI store. Result wire 1.2 includes fixed 16-row
inference/PSD evidence, a 14x2 drop-reason count table, and a 5x13
separation-sequence table. It adds a fixed 4x22 solver table whose rows are
MAP, fused LSMR, joint LSMR, and prepared CMG. The table binds the requested,
selected, concrete, effective, and checkpoint routes and per-engine frame,
retry, fallback, timing, and certificate evidence. Its independent 33-column receipt binds the other
tables, execution/generation counts, and uint32 limbs for the initial/final
generation hashes plus deterministic digests of the complete ordered
separation trace and generation-hash sequence. Timing and process-local values
are excluded from those digests. The receipt does not alias the OLS receipt,
and its column 33 `COMPLETE` cell is the final native operation.

Checkpoint family code 3 identifies the current-weight LSMR accelerator
accepted by ADR 0027; individual no-work or fallback checkpoint frames remain
attributed to the MAP row when MAP produced their returned projection. Solver
fallback codes are 0 for none, 1 for CMG ordinary-frame numerical
nonconvergence, 2 for checkpoint LSMR numerical nonconvergence, and 3 when
both occurred. The fixed result-wire dimensions are unchanged.

The ado transaction validates every private shadow and the completed
receipt before posting or renaming requested outputs. Syntax/domain error,
protocol mismatch, nonconvergence, allocation/store failure, panic, or
Control-C restores the prior estimation state and exposes no partial requested
outputs. Public `feppml.ado` performs no second parse, native call, or posting
outside this protected transaction.

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
| Numerical nonconvergence | 430 |
| Allocation | 909 |
| User interruption | 1 |
| Native self-test failure | 3498 |
| Internal error or caught panic | 3499 |

Licensed fixtures pass only when process status, Stata return code, status
files, and the unique terminal `END_FIXTURE|<name>|rc=0` sentinel agree.
