# ADR 0214: borrow-bound PPML numeric selection

Status: accepted for local development, 2026-09-26; not qualification.

## Decision

Bind authoritative numeric classification to an immutable borrow of the complete
PPML numeric input view: candidate mask, outcome and missing mask, source columns
and their roles, prior weights, offset/exposure and their masks. A private-field
proof object can be constructed only by the existing successful classifier.
Expose scalar counts for allocation admission; selection consumes the proof
object and accepts neither a replacement input nor unbound counts.

When active count equals candidate count, every candidate has survived the full
existing validation and deferred-error ordering. Construct the same row map and
drop ledger directly from the candidate mask after memory admission. Immutable
safe-Rust borrows prevent changes to any dependency while that proof is live.
This replaces a redundant numeric scan with an ownership guarantee, not with a
weaker or approximate scientific test. Counts alone never authorize this route.
For samples with numeric drops, retain the existing full row visitor and exact
classification/count agreement check.

This supersedes ADR 0038's second numeric scan only in the all-candidates-retained
case. Original-axis order, excluded-row behavior, deferred domain/weight error
precedence, zero/missing weights and all canonical weight and adjustment helpers
remain unchanged. There is no N-sized state before memory admission, no new
buffer, cache, generation reuse, thread pool or allowance. Existing vectors keep
their fallible reservation and candidate/original-axis capacity bounds.

The token ends at selection. It grants no authority over separation rebuilds,
changing weights, topology, rank, finite solves, projection, score, objective,
reconstruction, inference or final Hessian certification. Those independent checks
remain unchanged. The public Stata transaction, main-thread SPI, safe-core/FFI
boundary, failure atomicity and OLS path remain unchanged.

## Validation and admission

Compare selection, ledger, canonical reports, adjustment transformation and error
messages against the separate materializing preparation seam. Cover restricted
if/in, excluded invalid data, each numeric source role, actual drops, probability
and frequency weights, offsets/exposures, insufficient samples and memory rejection.
Equal shape must not permit transfer of validation to another input.

Run focused public science and requested-output checks, retain exact artifacts,
and separate instrumented attribution from stock command timings. Freeze fresh
cases before validation; preserve losses and unresolved comparator differences.
Accept only a measured useful whole-command improvement without weakening any
registered tolerance. Current progress belongs only in STATUS.md.

The [source-bound report](../../benchmarks/reports/development/ppml-classified-input-local-2026-09-26/README.md)
records classifiedinput1 against accepted directstart1. Clean diagnostics reduce
million-row selection from about 29.4 to 2.5 ms and core preparation from about
61 to 34 ms. The authoritative scan runs once, with unchanged rules. Public
million-row fast/low medians improve 2.3/2.1%; unchanged fresh seed 202609444
confirms 2.6/3.1% gains. Smaller losses and unresolved stock comparisons remain
visible. No source or guard is tuned on fresh validation. No new observation
buffer is introduced; tracked peak allocation and scratch deltas are zero.
