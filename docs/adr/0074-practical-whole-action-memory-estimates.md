# ADR 0074: Practical whole-action memory estimates

Status: Accepted policy direction by the owner; deployment requires the checks below.

## Decision

Keep conservative allocation admission separate from an expected-use model.
The development milestone is a phase-aware prediction within the larger of
10 percent of observed memory and 64 MiB on a fixed calibration set and
independent holdouts. This is an empirical engineering target, not a hard RSS
bound or a replacement for admission. Scientific contracts are unchanged.

Count simultaneous parent/frame ownership once, and take maxima across
sequential phases. Retain failed-attempt peaks. Include input construction,
PPML generations, inference, recovery, output publication and worker lifetime.
Account for Stata's resident allocation separately from native allocation;
OS available memory already excludes resident memory and must not have that
baseline subtracted again. Requested stacks, allocator retention and opaque
runtime storage keep explicit uncertainty and headroom.

The shared 11-by-2 memory result has presence/value cells and independent
schema versioning. Missing measurements remain unavailable. Partial predictions
must not publish a combined peak or additional reserve as complete estimates.
An estimated-complete practical model may retain strict proof limitations;
those limitations are disclosed separately. Current measured peaks are
validation observations and must never become their own prediction denominator.

Calibration fixes named ownership/lifetime models, not dataset-specific
multipliers. Require the predeclared synthetic calibration/holdout cases and a
bounded unchanged German five-percent native OLS case before claiming the
accuracy target. Preserve failed receipts. Formal release qualification and
cross-platform claims remain separate decisions.
