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

The hidden OLS adapter and test transaction remain qualification-only and are
excluded from distribution archives. Public `feppml` is a one-call wrapper
over the guarded PPML transaction; it neither reparses nor posts results.

## Public PPML architecture

The public PPML path reuses the prepared model,
certified weighted-linear kernels, and sandwich reductions below the
estimator-policy boundary. OLS remains a one-shot caller; PPML owns separation
and a reusable changing-weight IRLS session. Public solver requests are the
four closed families `auto`, `map`, `lsmr`, and `cmg`; Rust selects the concrete
engine and owns every changing-weight solve. Under ADR 0029, auto selects LSMR
for two or three categorical intercept effects when fast execution and the
complete specialized workspace are admitted; other designs select MAP.
The canonical session owns checkpoint authority and active-basis freeze;
eligible explicit LSMR/CMG fits may compute initial, strict, and final-Hessian
projections with independently re-certified current-weight LSMR under
[ADR 0027](adr/0027-canonical-ppml-checkpoint-projection.md). The ownership,
invalidation, and zero-cost OLS rules are fixed by
[ADR 0015](adr/0015-shared-prepared-weighted-linear-boundary.md), and PPML
memory/routing rules by
[ADR 0018](adr/0018-ppml-memory-routing-performance.md). The concrete
generation-zero admission envelope, single retained projected design,
all-width guarded-Cholesky/retained-QR rank policy, strict RAII basis
transaction, and worker-team reuse
are fixed by
[ADR 0020](adr/0020-retained-ppml-session-execution-boundary.md).
Canonical PPML predictors and their cancellation-aware arithmetic check are
specified in [working-response reconstruction](ppml_reconstruction.md).

Fast PPML with one categorical intercept effect uses the existing reusable
categorical projector and fused changing-response/retained-design projection.
It preserves current-weight reference scales and independent certificates,
allocates no accelerator matrices, and leaves low-memory and slope routes
unchanged; see [ADR 0039](adr/0039-ppml-one-fe-retained-projection.md).
After a fused response/design projection, retained within-column norms must
also use the current weight frame. Strict rank and score checks consume those
norms even when the preceding frozen-basis solve did not need them; advancing
design generations cannot make old norms authoritative.
Large, wide one-FE retained reference refreshes assign columns to the existing
team while preserving each column's row order, stable norms and compensated
means. Frozen finite solves share an existing identity scratch bank across
disjoint QR/certificate lifetimes; see
[ADR 0040](adr/0040-ppml-stable-reference-pass-scheduling.md).

Numeric preparation first classifies borrowed sources without observation-sized
allocation. After generation-zero admission, the same visitor constructs the
row ledger and weight reports, and the owner consumes the offset/exposure buffer
through stable candidate-bounded compaction. It retains raw original-axis model
sources for separation rebuilds without making a throwaway compact source copy.
The existing categorical generation constructors, memory envelope and numerical
sessions remain authoritative; see [ADR 0038](adr/0038-ppml-numeric-selection-ownership.md).

The PPML ado wrappers preserve observation order without nested sort bookkeeping
and leave input completeness to native classification. They retain candidate and
weight-expression handling, private shadows and all final publication checks;
see [ADR 0041](adr/0041-ppml-ado-preparation.md).

The public call graph is:

```text
_fereg_ppml.ado public syntax adapter
  -> one _fereg_estimate_ppml.ado transaction
       -> one plugin call __fereg_plugin, estimate-ppml <framed request>
            -> exact PPML request 1.5/result 1.2 wire validation
            -> preparation, separation, topology rebase, IRLS, inference
            -> complete private shadows, including solver report; COMPLETE last
       -> validate shadows and receipt; protected post or full rollback

public feppml.ado
  -> one _fereg_ppml delegation
       -> the guarded transaction above
```

PPML transport is additive: its request/result wires and live-SPI action do
not alter the OLS action or result layout. Receipt-last publication and the
direct `fereg` baseline are governed by
[ADR 0019](adr/0019-additive-ppml-transaction-and-fereg-regression-boundary.md).
The prepared PPML transaction is the sole production estimator path. Obsolete
alternate production modules were removed or reduced to explicitly test-only
helpers; they are not a second engine. Public availability does not by itself
close the final scientific, platform, memory, performance, or release gates;
Windows licensed-Stata testing and formal cross-platform qualification remain
deferred.

The separation coordinator is the only production owner of sample mutation.
Its bounded simplex detector covers the complete prepared model span: finite
columns, finalized intercept-FE indicators, and level-by-raw-slope columns for
absorbed slope channels. This detector is the deliberate bounded exception to
the estimator's matrix-free/data-streaming rule. Checked preflight covers the
jointly live explicit detector design, FE construction scratch, null-space
work, phase-I tableau, and certificate buffers before the N-by-P design or
tableau is allocated. An over-cap simplex may be skipped only when
comprehensive IR certifies no unhandled separation on that same ledger
generation; a simplex-only over-cap request fails with `r(909)`. The explicit
detector design is released before IRLS and is never a fitting or inference
route.

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

Each estimation freezes an effective requested thread count and installs its
parallel phases through `ThreadTeam`. The team reuses one bounded process-local
Rayon pool when the count matches and replaces it when a later action requests
a different count; estimator and sample state remain action-local. Worker
threads see only Rust-owned storage. Default solver routes must agree within
registered scientific tolerances. Deterministic routes are tested for
same-architecture, same-thread-count bitwise reproducibility.

## Artifact boundary

Each platform receives one checksum-verified plugin containing the SPI shim,
Rust core, and FFI code. There is no runtime sidecar, daemon, process launch,
Python/Julia environment, BLAS/OpenMP installation, or numerical runtime
library. The macOS artifact is universal arm64/x86_64; Linux targets x86-64
glibc 2.28+; Windows targets x64 MSVC.
