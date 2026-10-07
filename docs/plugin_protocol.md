# Plugin action and result protocol

The platform plugin exports Stata's required `pginit` and `stata_call`; the C
bridge forwards to Rust's `fereg_dispatch`. Scientific logic and result
validation remain in Rust.

## 0.5.0 versions

The action handshake, model metadata, OLS result layout, and additive PPML
request/result layouts are independently versioned:

| Contract | 0.5.0 version |
|---|---:|
| Action protocol | 1.6 |
| Model wire | 2.1 |
| OLS result wire | 1.5 |
| PPML request wire | 1.8 |
| PPML result wire | 1.5 |

Estimation requires the exact action and estimator-specific request/result
versions. Diagnostic `doctor` and `selftest` callers retain the action
protocol's same-major, not-newer-minor compatibility rule. ADO identity must be
exactly 0.5.0. Model wire 2.0 remains readable only for the bounded older
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

## Estimation request transport

OLS and PPML retain their canonical count/byte-length framed request. The ado
adapter sends `bundle=<payload>` through 4,000 payload bytes. Above that size,
it stores the identical payload in the caller-local `fereg_request_bundle` and
sends only `bundle_local=<byte-count>`. The SPI name is `_fereg_request_bundle`;
the leading underscore selects a local, not a global. The descriptor must be the
only action argument. No caller-supplied macro name is accepted.

Rust admits at most 16 MiB of metadata before the fixed main-thread macro read.
Its buffer includes one content byte beyond the declared size to detect SPI
truncation, plus a separate terminator byte. Exact NUL position, UTF-8 and the
unchanged canonical request decoder are checked before observations are read.
The buffer is dropped after parsing. Malformed/over-cap transport cannot complete
a receipt; scientific request/result versions and the one-call transaction are
unchanged. See [SPEC](SPEC.md).

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
accepted by the scientific contract; individual no-work or fallback checkpoint frames remain
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

## Shared memory result

The estimation action accepts four scalar baseline fields: `stata_used_entry`,
`stata_alloc_entry`, `stata_used_before`, `stata_alloc_before`. Each is a
nonnegative exact integer byte count or `.` for unavailable. The separate
`output_memory_matrix` is mandatory, distinct from every input and output
matrix, and has 11 rows and two columns (presence, value). Absent cells encode
`0,0`. Publication precedes COMPLETE and participates in the same rollback.

Rows are `schema_version`, `estimate_status`, `stata_used_before`,
`stata_alloc_before`, `native_live_before`, `native_peak_est`,
`stata_growth_est`, `combined_peak_est`, `additional_reserve`,
`safety_headroom`, `model_gap_bits`. Schema 2 retains status 0 unavailable,
1 partial, 2 estimated-complete-practical. Status 2 requires a known Stata-19
baseline, an expected native forecast and every action stage. It describes
the empirical engineering model in the scientific contract, not a strict allocation or RSS
bound. Other Stata versions, unknown baselines (including unmeasured frames)
and unknown attempt storage remain partial, without a combined peak or reserve.

The combined estimate is the maximum of entry allocation and pre-plugin Stata
allocation plus the larger of native execution and subsequent posting growth,
with the preexisting native owner added once. The additional reserve excludes
resident baselines. Solver admission and safety headroom remain separate.
The estimate's calibrated scope and extrapolation limits are described in
[the memory model](ARCHITECTURE.md#preparation-and-storage).

The low 16 gap bits retain strict proof limitations even at practical status 2.
Bits 17/18 mark unknown expected use/Stata growth, bit 16 an unavailable
baseline, and bits 20–29 missing action stages. Schema 2 status 2 permits only
low-16 proof gaps; its validator checks phase composition. Schema 1 remains
readable under its original stricter status-2 rule (zero gaps). Package metadata
binds the independently versioned memory schema. Existing OLS/PPML runtime
matrices retain their layout and observation boundaries.

## Optional PPML coefficient mean correction

PPML request 1.8/result 1.5 adds an optional correction inside the same
transaction. `bias_method`, `bias_unit`, `bias_probes`, `bias_seed`, and
`bias_stream` identify its calculation. `output_bias_count`,
`output_bias_channels`, `output_bias_matrix`, `output_bias_diagnostics`, and
`output_bias_support`
identify private destinations. The saved-channel count is zero or eighteen;
coefficient and diagnostic matrices have dimensions 9 by ordinary coefficient
count and 21 by 1; the support matrix is 2 by 11 in absorb order.
All three matrix destinations are mandatory for a correction request.
Option-off requests allocate none of these outputs.

All destinations, values, dimensions, sample identities and request bindings
are validated before publication. Correction columns and matrices precede the
ordinary COMPLETE receipt; no second substantive call or partial public result
is permitted. Missing unit IDs do not select rows. The complete statistical,
normalization, RNG and channel contracts are in
[ppml_bias_correction.md](ppml_bias_correction.md).
