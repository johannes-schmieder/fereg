# Plugin action protocol (separate from model wire)

The C ABI exports only Stata's required `pginit` and `stata_call` symbols.
`stata_call` forwards `argc/argv` to Rust's `stfixest_dispatch`.

The action/diagnostics handshake remains version `1.0`. The expanded-model
metadata wire is independently versioned at `2.0`; neither version number is
an alias for the other. An action handshake never performs a lossy model-wire
conversion.

## Actions

### `version`

No handshake is required. The plugin emits machine-readable lines containing
plugin/ado versions, protocol version, SPI ABI, source commit, and build date.

### `doctor`

Required fields:

```text
ado=0.1.0-phase01
protocol_major=1
protocol_minor=0
stata=18.0
threads=0
plugin_file=<resolved path>
```

The `protocol_major`/`protocol_minor` input keys are retained for compatibility
with the Phase 0 ado loader. They identify the **action handshake**, not the
model wire. Machine-readable `version` and `doctor` output uses the unambiguous
labels:

```text
stfixest.action_protocol=1.0
stfixest.model_wire=2.0
```

The action validates compatibility, resolves the local thread count, runs the
native self-test, and prints OS/architecture/CPU/thread/dependency metadata.

### `selftest`

Accepts the same handshake fields and runs protocol, factor, interaction,
metadata, and parallel-repeatability tests.

### `panic-test`

Requires `confirm=panic-boundary-test`. It injects a panic solely to verify that
`catch_unwind` returns code 3499. The action is never called by production ado.

### `estimate-ols`, `estimate-ppml`

Return `r(498)` in this phase.

## Developer-only preparation action

`prepare-ols-test` is an internal integration action, not an estimator action.
It requires `confirm=developer-preparation-test`, the standard action
handshake, model-wire v2 matrices and length-delimited registries, an explicit
numeric binding matrix, and three pre-created diagnostic output variables. The
binding roles are candidate (`1`), outcome (`2`), continuous source (`3`),
optional analytic weight (`4`), and optional frequency weight (`5`). At most
one weight binding is permitted. Inputs and outputs must cover the plugin
varlist exactly.

The action is restricted to continuous, unweighted, analytic-weighted, or
frequency-weighted no-fixed-effect models and never posts `e()` results.
Weights must be passed as one numeric source binding. Frequency weights must
be finite, strictly positive integral values representable as `u32`; this
developer action returns native invalid-data code `459` for a candidate-row
domain violation and does not claim the public command's eventual Stata-facing
error code. Weight expressions are not materialized by this developer helper.
All binding, matrix, type, and output validation precedes
observation reads, and diagnostic writes begin only after native preparation
succeeds. Both `estimate-ols` and `estimate-ppml` remain unconditionally
unsupported.

## Return-code map

| Class | Stata code |
|---|---:|
| Syntax | 198 |
| Unsupported/invalid option | 498 |
| Protocol mismatch | 499 |
| Invalid data | 459 |
| Allocation | 909 |
| User interruption | 1 |
| Native self-test failure | 3498 |
| Internal error or caught panic | 3499 |

All failures are fail-closed. The ado layer must not post partial results after
a nonzero return code.

## Platform loader contract

Stata's macOS console reports `c(os)==Unix`, so `c(os)` alone is not a complete
platform discriminator. The ado loader selects one artifact with this ordered
rule:

| `c(os)` | `c(machine_type)` | Artifact |
|---|---|---|
| `Windows` | any | `stfixest_windows.plugin` |
| `MacOSX` | any | `stfixest_macos.plugin` |
| `Unix` | starts with `Mac` | `stfixest_macos.plugin` |
| `Unix` | any other value | `stfixest_linux.plugin` |
| any other value | any | fail with `r(9)` |

The macOS rule covers both observed Stata 18 labels: `Mac (Apple Silicon)` and
`Macintosh (Intel 64-bit)`. Unknown operating systems never default to Linux.

Licensed-Stata fixtures emit `END_FIXTURE|<name>|rc=0` only after their final
assertion. `scripts/run_stata_tests.py` sets the repository ado-path in an
isolated wrapper and requires that exact terminal sentinel; a zero Stata
subprocess status without the sentinel is a failure.
