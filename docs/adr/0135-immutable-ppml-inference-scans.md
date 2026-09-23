# ADR 0135: immutable scratch scans for robust PPML inference

Status: accepted for local development, 2026-09-23.

## Decision

Extend ADR 0129's scan-local reader to the certified retained design consumed
by robust PPML inference. A fixed reduction block owns one immutable positional
reader and the reducer's existing row buffer. Read consecutive rows through the
reader's 8 KiB encoded page instead of constructing a positional read for every
row. This supersedes ADR 0129's inference exclusion only for this consumer.
Clustered PPML and public one-shot OLS retain their existing access paths.

The shared sandwich design and PPML adapter expose a statically dispatched,
fallible range visitor. The default visitor preserves pointwise row access;
dense sources retain it. The scratch specialization first validates row bounds,
row width and compacted identity positions, then reads in the reducer's original
order. The callback may overwrite the caller-owned row after consuming it.
Each parallel block owns its cursor; workers share neither mutable cursor state
nor a file position. The safe immutable borrow excludes writes, compaction and
frame replacement throughout the scan. No page survives that borrow.

## Scientific boundary

The mathematical reduction is unchanged. Fixed row blocks, multiplication/FMA
order, separate production and certificate scalar construction, separate meat
storage, forward/reverse compensated merges and final sandwich acceptance retain
ADR 0014's requirements. The visitor changes how raw certified rows are read,
not which design or certificate is authoritative. Existing scratch poison,
finite-value, bounds and I/O validation still apply, and callback/read failures
propagate before publication. No change to tolerances, rank, samples, weights,
DoF, separation, outer scores/objective or final-Hessian synchronization.

Independent Poisson/Hessian and weighted Stata oracles complement exact native
export comparisons. Reader tests cover page boundaries, rows wider than a page,
negative zero, fresh writes, invalid ranges/compaction and callback interruption.
The actual two-meat reducer matches dense input exactly across weights, block
sizes and thread teams. Existing corruption, truncation and allocation-failure
tests remain active.

## Memory and evidence

One active block retains an 8 KiB stack page and a small descriptor, replacing
the temporary previously constructed for each row read. There is no new heap
bank, N-by-K design, thread or file. Longer stack lifetimes use existing runtime
headroom; this is not physical-cap qualification or complete stack accounting.
Tracked and modeled peaks agree with baseline on all measured changed low-mode
cells. One excluded fast categorical attempt has a 463,312-byte tracked peak
increase; its model is unchanged and five follow-up pairs have matching peaks.
Keep that observation and the previously open mixed-thread audit limitation.

The [source-bound report](../../benchmarks/reports/development/ppml-inference-scan-local-2026-09-23/README.md)
records exact artifacts, diagnostic read counts, every timing distribution and
regression controls. At N61,440/K10 the diagnostic reduces positional reads from
61,440 to 600 and T8 meat time from 32.314 to 1.378 ms, with identical matrices.
Fresh whole-command low-mode gains are 4.9–13.0% in five cells; a sixth retains
a 0.8% median loss despite cheaper inference. Initial fast control losses become
small gains on predeclared identical-input follow-up; both distributions remain
evidence. This does not establish universal timing non-regression or complete
the broader 0.90-times-stock PPML objective.

The remaining large deficits require work on repeated projection, joint finite
directions and whole-command one-FE ownership. Further cursor/block-size tuning
is not the next milestone. Static allocation drift, constrained-fit/CG failures,
historical comparator discrepancies and deferred platforms retain their scope.
