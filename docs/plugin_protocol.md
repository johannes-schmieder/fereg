# Plugin action protocol

The C ABI exports only Stata's required `pginit` and `stata_call` symbols.
`stata_call` forwards `argc/argv` to Rust's `stfixest_dispatch`.

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
