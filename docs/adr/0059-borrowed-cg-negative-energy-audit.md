# ADR 0059: independently audit negative borrowed-CG energy

**Status:** Accepted for local development
**Date:** 2026-09-13
**Decision owner:** Johannes Schmieder

## Context

The saved-output optimization probe exposed a baseline borrowed symmetric-CG
failure on a weighted constant. Its Hestenes energy subtraction overshoots zero
by two ulps. A 45-row weighted closed-form projection independently reproduces
that failure. Discarding the convergence rule or simply clamping an uncertain
potential would not establish a valid step.

## Decision

Keep the existing ordinary recurrence and its negative-roundoff trigger. When
remaining potential is below minus epsilon times max(previous,1), reject a
shortfall exceeding 64 epsilon times max(previous,decrease,1). Otherwise audit
the current and pending iterate's weighted energy with two scaled, compensated
column scans. Match the pending multiply/subtract operation exactly. Require
both current-energy/previous-potential and actual-decrease/recurrence-decrease
agreement within that arithmetic envelope. Failure remains invalid data.

Use the independently measured pending energy, retaining the existing epsilon
floor, Hestenes quotient and stopping rule. Nonfinite quotients remain fatal.
Do not change the actual iterate, direction, coefficient updates, raw reference
scales, fresh FE certificate or stricter saved-effect reconstruction condition.
The envelope checks consistency; it is not a convergence tolerance or a promise
to recover all accumulated roundoff. Larger uncertainty still fails closed.

Only affected RHSs incur the extra scans. Energy calculation uses fixed stack
arrays with no heap allocation, retained state, observation-sized copy or SPI.
Ordinary successful updates retain their arithmetic. Automatic routing, public
weight/inference semantics and PPML policies remain unchanged.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/cg-energy-audit-local-2026-09-13/README.md)
records the red/green closed-form fixture, 57 focused Rust tests, independent
saved-OLS and Poisson oracles on both artifacts, successful replay of the exact
original PW request, and twelve protected pairs with exact scientific exports,
solver decisions and native peaks. Largest absolute slowdown is 48 ms / 1.7%;
none exceeds the prospective review envelope. This is a correctness repair,
not a speedup or qualification claim.

Existing low/saved-output memory-model gaps remain open. The separate saved-Schur
prototype still requires recovery/fallback memory admission before adoption.
Historical failures, fixture corrections and discarded collector drafts remain
visible. No installed plugin, dependency pin or separate reconstruction work
is changed.
