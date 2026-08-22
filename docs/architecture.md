# Architecture

## Control and computation planes

Stata owns syntax parsing, factor/time-series semantics, candidate-sample
construction, syntactic weight-option parsing, output allocation,
estimation-result posting, and postestimation dispatch. Rust owns authoritative
weight/data validation, data ingestion, factorization,
interaction coding, memory preflight, threading, future fixed-effect transforms,
and all future numerical estimation.

The development build includes diagnostics, native preparation, certified
prepared OLS inference, a hidden shadow-result transport, a developer-only
licensed-Stata eclass transaction, and the bounded
`_stfixest_ols_hidden_numeric_v1` manifest-projection adapter. These are not
public activation. No public command may post `e()` results until broader
sealed parity and an independently reviewed activation gate accept the complete
supported surface.

## Call graph

```text
stfixest.ado
  -> _stfixest_load.ado
  -> plugin call __stfixest_plugin, <action> key=value ...
  -> plugin/stata_bridge.c::stata_call
  -> stfixest-plugin::stfixest_dispatch
       catch_unwind
       UTF-8/argument validation
       safe action dispatcher
       diagnostics, or hidden estimate-ols-test
         -> one native source load
         -> one certified prepared iid/HC1/clustered composition
         -> private shadow outputs with receipt COMPLETE last
  -> C display/error wrapper on Stata's calling thread

_stfixest_estimate_ols_test.ado (developer-only)
  -> private result/output shadows plus held prior estimates
  -> one canonical length-framed bundle
  -> one estimate-ols-test plugin call through the graph above
  -> complete receipt/table/sample validation
  -> whole-Mata b/V copies, eclass posting, final nobreak commit

_stfixest_ols_hidden_numeric_v1.ado (developer-only candidate adapter)
  -> public-shaped bounded numeric syntax
  -> exactly one _stfixest_estimate_ols_test invocation
  -> unmistakably private e(cmd)/cmdline identity
  -> sanitized sealed comparison against the frozen reference receipt
```

The hidden transport already proves one substantive plugin call for its bounded
numeric surface. Future public estimation retains the same outer graph.
Stata-side parsing, private output allocation, and final `ereturn` posting do not
count as substantive numerical calls.

## Atomicity boundary

Rust completes and certifies the selected estimator in owned memory before any
output store, then fallibly adapts and validates the full result. All N-, K-,
and K²-scale checks, including native omission markers, occur in Rust before
the first store. All 11 result-matrix names and layouts are converted to stable
borrowed C handles before commit. The final receipt cell is written only after
sample/residual/reason shadows and all result metadata; no work follows a
successful `COMPLETE=1` store. Store errors and their C reports are prepared
before commit.

This is logical atomicity, not physical SPI rollback. SPI can leave private
shadow cells changed if a later store fails, but `COMPLETE` remains zero and the
entire shadow must be discarded. The hidden eclass helper consumes only a
complete revalidated receipt under `_estimates hold ..., restore nullok
varname()`. It uses protected `_ms_findomitted` markers and whole-matrix Mata
copies so Stata neither warns about nor alters the pinned multiway CGM restored
covariance, including the allowed final negative determinant. Seven injected
failure stages, real scientific failure, and an actual Control-C after the sole
plugin call restore prior named/null estimates and `e(sample)` exactly. Only a
final `nobreak` residual rename plus unhold commits success. Public `stfeols`
remains `r(498)` pending broader sealed parity and activation review.

The active candidate receipt binds all Stata capture, rollback, parity,
store-failure, and actual-Break gates to one isolated staged universal plugin.
That plugin has SHA-256
`eda2629198429d419a476151886936e0761eca26f2a8cc34fe83c52146703980`
and embeds commit `d731b3276eab`. An external identity manifest separately
binds the committed receipt path, payload and file hashes, git blob, and mode;
committed content cannot be substituted by merely resealing a payload.
Candidate-bound failure injection covers 222 Fake-SPI and 219 live C-loader
store ordinals, each leaving `COMPLETE=0`. Stata's post-call work is limited to
constant-size receipt/table identity checks, transaction control, and final
`ereturn` posting.

## Safety boundary

`stfixest-core` and `stfixest-testkit` forbid unsafe Rust. Unsafe operations are
confined to `crates/stfixest-plugin/src/ffi.rs`: reading `argc/argv`, calling
C display/error wrappers, and exporting a stable C ABI. The exported function
is wrapped in `catch_unwind`; a panic becomes deterministic Stata return code
3499 rather than unwinding through C.

The C bridge contains no statistical or numerical logic. Stata SPI calls occur
only on Stata's calling thread. Worker threads operate solely on Rust-owned
memory.

## Artifact boundary

Each supported platform receives one plugin file containing the C SPI shim,
Rust standard library, core, and FFI code. There is no runtime sidecar, daemon,
Python environment, Julia runtime, BLAS installation, or process launch.
