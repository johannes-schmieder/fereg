# Architecture

## Control and computation planes

Stata owns syntax parsing, factor/time-series semantics, candidate-sample
construction, syntactic weight-option parsing, output allocation,
estimation-result posting, and postestimation dispatch. Rust owns authoritative
weight/data validation, data ingestion, factorization,
interaction coding, memory preflight, threading, future fixed-effect transforms,
and all future numerical estimation.

The Phase 0–1 build implements only diagnostics and preprocessing primitives.
No Stata estimator is allowed to post `e()` results until a later native action
returns a complete, certified result.

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
       stfixest-core protocol/self-test/threading primitives
  -> C display/error wrapper on Stata's calling thread
```

Future estimation commands retain the same outer graph but make exactly one
substantive plugin call. Stata-side parsing and output posting do not count as
substantive numerical calls.

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
