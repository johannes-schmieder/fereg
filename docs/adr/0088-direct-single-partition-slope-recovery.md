# ADR 0088: direct single-partition OLS slope recovery

**Status:** Accepted for local development under the approved optimization plan
**Date:** 2026-09-21
**Decision owner:** Johannes Schmieder

## Decision

OLS saved-effect recovery may solve a single slope partition directly when the
sample has at least 16,384 rows and at most four channels. Independent groups
use the existing scaled, compensated MAP moments and rank-revealing elimination.
This changes neither the rank rule nor the representative for dependent channels.
The requested channel-major outputs still pass the existing pointwise target
reconstruction and fresh residual certificate. A failed attempt leaves the
borrowed target untouched and falls through to the established recovery routes.
PPML retains its existing recovery policy.

With multiple workers, the direct route computes fresh group certificates in
bounded worker workspaces. A shared scalar helper preserves the serial moment
and projected-norm calculation. Group results are reduced in their original
order, including the RMS FMA sequence; focused tests require exact certificate
agreement across serial and parallel evaluation.

Admission adds a target copy, both coefficient layouts, the certificate reduction
bank and bounded worker descriptors/payloads to the existing finishing envelope.
Construction owners are released before finishing. Actual capacities are checked;
failed attempts remain represented in the recovery peak. A budget that excludes
this route may still admit the existing MAP fallback.

The cached fit absorber also checks a final residual before projecting it. It
scans that input's norms and freshly evaluates its QR span certificate. If the
certificate passes, the input is returned unchanged with zero projection steps.
Otherwise projection and a fresh final check proceed as before. Original input
norm handoff remains unit-weight only; no prior residual certificate is reused.

## Evidence boundary

The [local recovery checkpoint](../../benchmarks/reports/development/slopes-recovery-local-2026-09-21/README.md)
binds production and diagnostic plugins, paired screens, weighted saved-output
checks, exact certificate comparisons, admission/fallback tests and protected
licensed fixtures. Serial narrow fits and weighted two-way clustered outputs
still trail fixest in the reported screen. Full graph coverage, final repeated
blocks, solver recalibration and Linux confirmation remain open. This decision
is not release or complete memory qualification.
