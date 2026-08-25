# Architecture

## Control and computation planes

Stata owns public syntax expansion, factor/time-series metadata, candidate
`if`/`in` construction, option parsing, private output allocation,
transactional `ereturn`, and postestimation dispatch. Rust owns authoritative
data/weight validation, observation ingestion, factorization, interactions,
singleton pruning, FE/cluster topology, memory planning, threading, numerical
estimation, inference, certificates, and saved-output values.

## Public call graph

```text
fereg.ado
  -> _fereg_estimate_public.ado
  -> _fereg_ols.ado
       parse/expand syntax and allocate private shadows
       -> one _fereg_estimate_ols.ado transaction
            -> one plugin call __fereg_plugin, estimate-ols <framed request>
                 -> plugin/stata_bridge.c::stata_call
                 -> fereg-plugin::fereg_dispatch
                      catch_unwind
                      complete request/output validation
                      authoritative sample and Rust-owned ingestion
                      fast/low certified OLS and inference
                      private shadows; receipt COMPLETE last
            validate complete receipt and exact identities
            post e(), sample, residual/absorbed/saved-FE outputs
            final protected commit or full rollback
  -> fereg_p.ado / fereg_estat.ado for public postestimation
```

The hidden adapter and test transaction remain qualification-only and are
excluded from distribution archives. `feppml` fails in ado with `r(498)`
without a substantive plugin call.

## Atomicity and interruption

Rust computes and certifies the result in owned memory before stores. Every
N/K/K² value, omission marker, dimension, and destination is validated before
the first store. SPI calls stay on Stata's thread. Completion is the final
receipt cell; no allocation, polling, display, computation, or store follows.

SPI cannot physically roll back private cells, so an incomplete receipt makes
all shadows invalid. The ado transaction holds prior named/null estimates and
defers requested variable renames until its final protected block. Native
error, injected store failure, validation failure, panic, or actual Control-C
restores prior `e()`, `e(sample)`, estimates, and output-variable state.

## Threading and safety

`fereg-core` and `fereg-testkit` forbid unsafe Rust. Unsafe operations are
confined to `crates/fereg-plugin/src/ffi.rs` for C argument access, platform
RSS APIs, allocator instrumentation, display/error wrappers, and the exported
ABI. `catch_unwind` converts panics to deterministic Stata errors.

Each estimation creates an isolated Rayon pool at the effective requested
thread count. Worker threads see only Rust-owned storage. Default solver routes
must agree within registered scientific tolerances. Deterministic routes are
tested for same-architecture, same-thread-count bitwise reproducibility.

## Artifact boundary

Each platform receives one checksum-verified plugin containing the SPI shim,
Rust core, and FFI code. There is no runtime sidecar, daemon, process launch,
Python/Julia environment, BLAS/OpenMP installation, or numerical runtime
library. The macOS artifact is universal arm64/x86_64; Linux targets x86-64
glibc 2.28+; Windows targets x64 MSVC.
