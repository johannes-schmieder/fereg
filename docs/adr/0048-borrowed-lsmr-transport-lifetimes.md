# ADR 0048: shorten borrowed LSMR transport lifetimes

**Status:** Accepted for local development
**Date:** 2026-09-12
**Decision owner:** Johannes Schmieder

## Decision

The borrowed categorical LSMR adapter extracts RHS columns directly from its
unchanged input. It creates the row-major candidate only after the upstream
solve returns and the column-major RHS bank has been released. Every candidate
entry is filled from the upstream demeaned result. Release that result bank,
and unrequested coefficients, before independent FE certification. Requested
coefficients remain available for the existing normalized recovery product.

The original candidate copy was overwritten in full. Removing it changes no
arithmetic, input/reference scale, solver/preconditioner, iteration budget,
fallback, certificate, memory admission or publication rule. Allocations remain
checked and fallible; an output allocation failure still fails the transaction.
Retained low-memory sessions use the same adapter. Explicit fast simple OLS
LSMR uses a different owned adapter and is not covered by this decision.

## Evidence and limits

The [source-bound local report](../../benchmarks/reports/development/lsmr-transport-local-2026-09-12/README.md)
compares against `15bd21d`, with seventeen successful pairs and separately
retained failures. Three-seed automatic weak-graph native peaks fall about
10.1% at N30,720/K10 and 14.0% at N122,880/K65. Command medians are 60→60 ms
and 997→1013 ms; the wide median paired change is +0.9%. Adoption rests on
repeatable memory reduction within the declared command-regression envelope,
not a speed claim. The existing public model still understates both peaks.

Successful pairs preserve full scientific exports and non-time solver
decisions exactly. Sixty-two focused Rust tests and both arms of two independent
explicit-dummy Stata fixtures pass, including retained batches, coefficients,
weights, covariance, reference scales and constrained routing. Two additional
weak-graph requests fail the same certificate guards on both artifacts; these
are preserved failures, not passing oracles or fixed defects.

The associated P0 audit proves a separate LSMR recurrence undercount and locates
additional model/reporting gaps. This lifetime change does not repair them or
qualify memory admission. No dependency pin, installed plugin, SCC job, release
qualification or unrelated reconstruction work changes.
