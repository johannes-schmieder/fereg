# ADR 0033: aggregate certified automatic LSMR/CG batch reports

**Status:** Accepted for local development
**Date:** 2026-09-10
**Decision owner:** Johannes Schmieder

## Context

Automatic OLS already permits a finite LSMR candidate that misses its tighter
internal certificate to continue through CG within the original iteration and
memory budgets. CG independently certifies the returned residual. Low-memory
fits nevertheless reject the result when one projection batch finishes with
LSMR and another finishes through this CG continuation: their report merger
requires the used-solver fields to agree. Johannes requested this next fix.

## Decision

Permit the existing report merger to combine automatic LSMR and automatic CG
reports when both are certified, the CG report carries its fallback flag, and
both retain identical, nonempty requested/effective LSMR preconditioner
metadata. An aggregate containing such a continuation reports requested
`auto`, used `cg`, and fallback true, regardless of batch order. The effective
preconditioner continues to describe the attempted LSMR stage, as it already
does in the individual continuation report.

Retain existing aggregation of maximum per-batch iterations and certificate
norms, summed timing and checkpoint retry fields, maximum pilot fields, and
warnings in batch order. A single certified batch or batches with matching
engines retain their prior report. Existing explicit CMG/CG aggregation is
unchanged. Uncertified results, changed solver requests, mismatched
preconditioners, unflagged CG routes and other mixed engines remain errors.

This corrects report aggregation after completed certified solves. It changes
no solver selection, RHS/reference scale, tolerance, recurrence, iteration
budget, memory lifetime, result wire or numerical output. In particular, it
does not treat a failed LSMR certificate as successful or suppress the CG
continuation and its warning. Public fits previously rejected solely by this
metadata check may now complete the remaining scientific and transaction
checks and return results.

## Evidence and limits

The regression test must fail on the prior merger and pass for every order of
three certified LSMR/CG batches after the fix. Invalid reports must remain
rejected before changing the aggregate. Validate the original weighted
automatic Stata fixture against an independent explicit-dummy fit and recheck
the larger low-memory reproductions against the pinned reference. Unaffected
successful captures require exact scientific equality.

The local development receipt is
`benchmarks/reports/development/ols-mixed-report-local-2026-09-10.json`.
This is not frozen F0 or release qualification. The prior receipts remain
immutable evidence of the failed baseline. Parallel exact-repeat failures and
explicit-diagonal LSMR certificate failures are separate issues; this decision
does not alter their acceptance rules.
