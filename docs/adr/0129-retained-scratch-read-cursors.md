# ADR 0129: bounded immutable retained-scratch read cursors

Status: accepted for local development, 2026-09-23.

## Decision

Retained finite QR, its direct dropped-span check, and fresh/frozen residual
reconstruction keep one read-only scratch cursor per scan. It retains an 8 KiB
encoded page instead of issuing a positional file read for each narrow row.
Requests may cross pages, exceed page width or move backwards. Only requested
values are decoded and checked for finiteness, in the original order. Existing
scratch bounds, poison and I/O validation remain authoritative. Failed page
loading revokes its valid length before I/O, so partial bytes cannot be reused.

The cursor immutably borrows the private scratch owner. Safe Rust prevents its
coexistence with mutation, compaction or frame replacement through that owner.
It is local to a scan, with no mutable shared file cursor or state retained across
weight generations. Concurrent readers own separate encoded pages. Numerical
operations, row/FMA order, rank thresholds, certificates and failure publication
remain unchanged. No public routing, weight, sample, separation, DoF or inference
policy changes. Existing ADR 0070 moment tiles remain unchanged.

## Memory and scope

One cursor has an 8 KiB stack page plus a small descriptor, replacing the same
encoded temporary previously constructed for each positional read. Its longer
lifetime can increase overlapping stack use; this uses existing runtime/stack
headroom, not an unreported heap or N-by-K bank. There is no additional heap
allocation, worker or file. Heap admission/capacity reports remain unchanged;
the captured baseline/candidate tracked peaks agree exactly. This is not physical
memory-cap qualification or a proof of complete stack coverage.

Dense retained paths keep their prior row access. Public one-shot OLS does not
use the new cursor. Inference adapters still use their existing per-row reads;
changing those consumers requires a separate coherent extension. This change
does not add persistent caching to the general scratch owner.

## Evidence

The [source-bound report](../../benchmarks/reports/development/ppml-scratch-reader-local-2026-09-23/README.md)
records fresh public low-memory gains of 24.7–36.0%, 202 distinct focused Rust
checks, four licensed fixtures, independent Poisson/Hessian pilots and 69 exact
native scientific export pairs. Tests bound positional reads by encoded page
count, cover narrow/wide rows, negative zero, backward access, new-scan writes,
parallel readers, corrupt/truncated data, weighted QR and rank deficiency.
One multiple-slope stock covariance failure and one disconnected stock
nonconvergence remain unranked. Most candidate cells still lose to stock.

Fresh phase measurements reduce finite work substantially while absorption
stays nearly unchanged. The next algorithmic target remains joint finite-plus-FE
ordinary solving. This milestone does not close the broader 0.90-times-stock
objective, static allocation drift, constrained-fit/CG failures or platform gates.
