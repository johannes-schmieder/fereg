# Architecture

## Control and computation planes

Stata owns syntax parsing, factor/time-series semantics, candidate-sample
construction, syntactic weight-option parsing, output allocation,
estimation-result posting, and postestimation dispatch. Rust owns authoritative
weight/data validation, data ingestion, factorization,
interaction coding, memory preflight, threading, future fixed-effect transforms,
and all future numerical estimation.

The development build includes diagnostics, native preparation, certified
prepared OLS inference, and a hidden shadow-result transport. It still contains
no Stata-facing estimator transaction. No public command may post `e()` results
until licensed-Stata transaction and sealed-parity gates accept the complete
native result.

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
```

The hidden transport already proves one substantive plugin call for its bounded
numeric surface. Future public estimation retains the same outer graph.
Stata-side parsing, private output allocation, and final `ereturn` posting do not
count as substantive numerical calls.

## Atomicity boundary

Rust completes and certifies the selected estimator in owned memory before any
output store, then fallibly adapts the full result. All 11 result-matrix names
and layouts are converted to stable borrowed C handles before commit. The final
receipt cell is written only after sample/residual/reason shadows and all result
metadata; no work follows a successful `COMPLETE=1` store. Store errors and
their C reports are prepared before commit.

This is logical atomicity, not physical SPI rollback. SPI can leave private
shadow cells changed if a later store fails, but `COMPLETE` remains zero and the
entire shadow must be discarded. No current ado consumes the receipt or mutates
`e()`. The next licensed-Stata gate must wrap private temporaries and the one
plugin call in `_estimates hold`, restore the held estimate on every failure,
and post only a revalidated complete result. Public `stfeols` remains `r(498)`
until that transaction and sealed parity pass.

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
