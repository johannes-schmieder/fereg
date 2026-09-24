# ADR 0145: column-oriented streamed PPML finite products

Status: accepted for local development, 2026-09-23.

## Decision

Prepare low-memory joint PPML directions by independent projected finite columns
and Hessian rows on the existing worker team. Extend the immutable finite-design
view with fallible column access. Original numeric controls borrow their source
slices; generated controls evaluate the selected prepared recipe. The stored
column mapping and selected-coordinate ordering remain authoritative. The dense
adapter supports the same operation for the independently tested kernel.

Each worker loads one normalized current-weight column into its existing
observation buffer, eliminates the selected FE partition, forms its FE cross
products and response moment, then reads each original finite column to form
one Hessian entry. With a single admitted worker buffer, form that Hessian row
from complete input rows, keeping independent entry accumulators available to
the processor. Column loading remains direct. This structural scheduling choice
uses neither a sample-size/width threshold nor a benchmark identity. Its row
scratch borrows the factor bank before the preconditioner exists.

Each entry retains its original observation/FMA order and normalization
arithmetic. Symmetrization and the common block-preconditioned
Krylov solve are unchanged. Stable scales still come from the existing row
visitor; reconstruction and independent acceptance keep their existing scans.

This supersedes ADR 0144's serial normalized-row Hessian tiles. Retain original
representatives, but remove that obsolete product loop rather than adding a
second solver or benchmark-specific guard. The first parallel-row prototype
repeated complete-row materialization and had a smaller public gain; it is
preserved as evidence and not retained as another engine. A column-only
successor regressed the serial numeric control in fixed-input follow-up; the
single-worker organization corrects that computational mismatch. Validate the
final source with a new predeclared seed, not by relabeling that consumed seed.

## Scientific and ownership boundaries

No solver family, tolerance, rank policy, sample, weight semantics, DoF,
separation behavior, convergence scheduling or acceptance condition changes.
Current-weight QR/coupling, finite products, fresh FE and original-input finite
scores, model-space reconstruction, objective checks, two outer convergence
passes, canonical rank/inverse and final-Hessian synchronization remain required.
No numerical state gains authority across weight frames. Failed column access
propagates through the existing geometry-revocation and transactional boundaries.

Only low-storage joint preparation changes, for both categorical and slope
publication PPML. Fast PPML, one-FE concentration, canonical projection/inference
and one-shot OLS retain their existing algorithms. Worker code accesses only
immutable Rust-owned inputs and disjoint preallocated destinations; no SPI call,
unsafe core code or publication work moves onto workers.

The implementation does more column traversals and repeated normalization than
serial row tiles. Contiguous numeric access, keeping one accumulating Hessian
entry local, and independent scheduling outweigh that work in measured threaded
cases. Counts describe source accesses, not physical memory traffic. Generated
recipes and single-thread execution require separate controls; do not infer
speed from the numeric layout alone.

## Memory, evidence and limits

No heap bank, N-by-K copy, worker count, admission formula or workspace capacity
changes. The same roots, scales, cross-products, Hessian and worker observation
buffers remain live. Temporary norm state retains its existing disjoint lifetime
in the solution/factor banks. Scratch remains authoritative for canonical within
products and inference. Actual-plugin allocation audits retain exactly zero
allocator events and current growth, stable capacities and fresh/warm agreement.
Physical constrained-memory and cross-platform qualification remain deferred.

The [source-bound report](../../benchmarks/reports/development/ppml-joint-column-products-local-2026-09-23/README.md)
records all candidates, independent WLS and Poisson/Hessian/sandwich checks,
current-source phase attribution, stock rotations, fresh cases, control losses
and fixed-input follow-up. The new wide numeric-column Stata oracle explicitly
covers the bound-source path at N4800/K8, T1/T4, FW/PW and robust/clustered VCE.
Reference agreement does not replace these mathematical checks.

Public gains, remaining stock losses and control variation belong to the exact
artifacts in the report. This is a useful production improvement, not completion
of the per-case 0.90-times-stock objective. Close the product-loop experiment and address the
measured canonical two-FE projection and remaining whole-command costs next.
Historical failures, scientific exclusions, frozen definitions and protected
validation seeds retain their prior scope.
