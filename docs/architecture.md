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

Both estimation actions carry the same canonical framed metadata either inline
or, above 4,000 bytes, through one fixed caller-local macro read within the same
plugin call. Rust checks a bounded descriptor, exact bytes and framing before
observation loading; no worker reads Stata macros. See
[ADR 0055](adr/0055-bounded-local-request-transport.md).

The hidden OLS adapter and test transaction remain qualification-only and are
excluded from distribution archives. Public `feppml` is a one-call wrapper
over the guarded PPML transaction; it neither reparses nor posts results.

## Public PPML architecture

After the canonical initial rank solve, fast one-categorical-FE publication
fits may use analytic group-intercept elimination and concentrated finite
Poisson steps. Full outer, strict and final-Hessian acceptance remains independent;
the optional owner and exact-state reuse boundary are specified in
[ADR 0127](adr/0127-profiled-one-fe-poisson.md).

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
[ADR 0027](adr/0027-canonical-ppml-checkpoint-projection.md). Eligible wide two-FE
checkpoints may instead use the existing current-weight Schur accelerator under
[ADR 0067](adr/0067-guarded-ppml-schur-checkpoints.md): the weighted probe and full
candidate reuse the retained graph, a transient copy preserves original-input LSMR
fallback, and the canonical certificate retains its exact tolerance. Ordinary joint
LSMR and the narrow final checkpoint are unchanged. The ownership,
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
Ordinary PPML tolerances preserve tighter requirements established by rejected
strict checkpoints. A stagnated same-basis profile at the inner floor permits
one rebuild from canonical generated inputs into existing storage, followed by
bounded strict attempts; persistent failure returns no result. All certificates
and reference scales remain authoritative. See ADR 0072 for the recovery budget.

Fresh/strict numeric predictors use bounded row tiles under the existing worker
team while preserving selected-column FMA order and the independent reconstruction
check. Eligible generated predictors borrow ordinary/topology factor codes into
fixed row tiles, including wider designs; small/narrow models retain their prior
path. Ordinary OLS subtraction keeps its separate absent-indicator arithmetic.
See [ADR 0066](adr/0066-retained-numeric-predictor-tiles.md) and
[ADR 0068](adr/0068-retained-generated-predictor-tiles.md).

PPML with one through three categorical intercept effects can reuse the
categorical projector in both memory modes. Fast storage fuses changing-response
and retained-design projection; low storage projects bounded scratch blocks and
the response separately. One FE needs no accelerator matrices. Current-weight
reference scales and independent certificates remain authoritative. Publication
PPML multipartition slopes in the MAP family may instead use the independently
certified coefficient Schur owner, with generation-bound weighted QR and optional
symbolic group-pair coupling under [ADR 0123](adr/0123-ppml-coefficient-schur-slopes.md),
[ADR 0124](adr/0124-ppml-frame-geometry-and-parallel-certificates.md), and
[ADR 0126](adr/0126-ppml-symbolic-coupling.md). Admitted ordinary directions
couple finite controls directly to that FE operator under
[ADR 0130](adr/0130-joint-ppml-slope-directions.md), borrowing retained
FE-equivalent representatives. Fast mode reuses the normalized column bank;
low mode streams bounded tiles through existing worker buffers under
[ADR 0134](adr/0134-bounded-low-memory-joint-ppml-slopes.md). Their
separate certified result carries no within-design inverse or final-Hessian
authority. Canonical initial construction remains independent. Eligible slope
finalization uses a strict joint Newton witness and one independent canonical
rank/bread/Hessian construction on the unchanged accepted weight frame under
[ADR 0131](adr/0131-single-frame-ppml-slope-finalization.md), in either admitted
memory mode; a declined witness
retains canonical checkpoint/retry. Fresh final acceptance remains mandatory.
Other slope calls retain their existing projector. See [ADR 0039](adr/0039-ppml-one-fe-retained-projection.md)
and [ADR 0069](adr/0069-low-memory-ppml-categorical-projection.md).
Low-storage categorical MAP with multiple effects and finite controls also uses
the admitted joint owner under [ADR 0136](adr/0136-low-memory-categorical-joint-ppml.md).
Two-/three-FE sessions retain the borrowed categorical projector for canonical
rank/bread construction and use joint directions between those boundaries.
The owned FE design exists once. Four-plus FE use the generic coefficient
projector at canonical boundaries. Optional-state refusal restores the prior
route; all current-weight and final acceptance requirements remain unchanged.
Final robust PPML inference borrows its certified design through fixed row-block
visitors. Scratch-backed blocks own immutable scan cursors under
[ADR 0135](adr/0135-immutable-ppml-inference-scans.md); numerical reductions and
independent certificate storage remain unchanged. Clustered inference and
one-shot OLS retain their existing access paths.
After a fused response/design projection, retained within-column norms must
also use the current weight frame. Strict rank and score checks consume those
norms even when the preceding frozen-basis solve did not need them; advancing
design generations cannot make old norms authoritative.
One-FE fused projections copy norms certified by that same call into retained
storage when the frame and projector weight divisors match. Multi-FE handoffs
and alternative frame divisors retain the stable current-frame scan; see
[ADR 0052](adr/0052-ppml-same-call-projection-norms.md).
Fresh retained construction selects these same-call norms using its stored
generated-column mapping, avoiding duplicate norm arithmetic during matrix
compaction while preserving raw reference scales and the initial certificate;
see [ADR 0053](adr/0053-ppml-fresh-design-norm-reuse.md).
Large, wide one-FE retained reference refreshes assign columns to the existing
team while preserving each column's row order, stable norms and compensated
means. Frozen finite solves share an existing identity scratch bank across
disjoint QR/certificate lifetimes; see
[ADR 0040](adr/0040-ppml-stable-reference-pass-scheduling.md).

Scaled retained dense finite moments reuse an observation's standardized values
within each existing Gram-row partition at up to 64 active columns. A fixed
local cache removes repeated division while preserving each entry's row/FMA
order. Wider scaled dense partitions use two fixed 64-value tiles, bounding
stack payload at 1024 bytes per partition without a heap bank. Scratch,
unscaled and single-column partitions retain their loops; no cross-frame
numerical cache or new rank policy is introduced. See
[ADR 0054](adr/0054-retained-dense-moment-row-standardization.md) and
[ADR 0057](adr/0057-wider-retained-dense-moment-tiles.md).

Changing-weight retained scratch projection extracts and patches pool-width
blocks through the existing bounded complete-row I/O bank. It retains the same
file, row/column ordering, projector and arithmetic, with no additional resident
storage. See the [local transport evidence](../benchmarks/reports/development/german-ppml-block-transport-local-2026-09-14/README.md).

Retained scratch finite moments batch complete rows inside each existing
Gram-column partition. A checked bounded tile bank replaces per-row positional
reads while preserving observation/FMA order and selected stored-column identity.
Preflight covers every possible active rank; no resident N-by-K copy is added.
See [ADR 0070](adr/0070-bounded-retained-scratch-moment-tiles.md).

Retained scratch QR, its direct dropped-span check and residual reconstruction
also use a bounded immutable positional-read cursor across rows. The cursor's
8 KiB encoded page belongs to one scan, cannot coexist with owner mutation and
changes no numerical operations or certificates. Dense paths and moment tiles
retain their existing access; see [ADR 0129](adr/0129-retained-scratch-read-cursors.md).

PPML covariance production and independent publication inspection use the same
maximum-pivot Jacobi arithmetic. Above 64 identified dimensions, upper-row maxima
reuse the eigenvalue vector within its existing modeled lifetime. Only affected
rows are rescanned after a rotation; pivot ties, stopping, clipping and independent
certificates remain exact. This cache belongs to one eigensystem, with no new heap
allocation or state shared across weight frames; see
[ADR 0058](adr/0058-exact-ppml-jacobi-pivot-cache.md).

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

PPML final covariance validation independently inspects the published matrix's
eigensystem and checks its production report. A further clipped comparison
retains the existing repair-distance bound but does not supply replacement
diagnostics or public values; see [ADR 0056](adr/0056-published-ppml-covariance-validation.md).

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
