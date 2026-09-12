# ADR 0042: bounded candidate-run and mixed-source numeric loading

Status: accepted for local development, 2026-09-12.

## Decision

Reuse the existing main-thread numeric SPI reader for long retained runs and
numeric peers of fixed-width string sources. Shared OLS/PPML input loading uses
this route at 16,384 original observations; unrestricted all-numeric input keeps
its existing route at every size. Restricted loading fills the existing candidate
vector first. It reads only retained runs, in chunks of at most 65,536 rows.
Runs shorter than 64 rows use scalar reads without per-run slice/FFI allocations.
Small restricted/mixed inputs retain their combined row traversal.

String sources remain candidate-gated scalar reads in original encounter order,
followed by the same deterministic final coding. PPML offset/exposure loading
uses the shared candidate-run reader and its existing destination buffers.

## Invariants

The authoritative scientific classifier, original-row domain precedence, weight
kinds/scales, cluster missingness before singletons and final sample ordering
remain unchanged. Candidate-excluded source and adjustment cells are never read.
Completeness hints retain their original applicability. Shape checks precede
reads, all slice/position metadata reserves are fallible, and long retained or
excluded scans remain interruptible. SPI stays on Stata's thread.

There is no additional observation-sized allocation or row packing. Numeric
value and missing buffers keep their existing bounds; PPML allocation-free
preflight and post-admission materialization are untouched. Publication,
solver routes, tolerances, rank and inference policy are unchanged.

## Evidence and limits

The [local report](../../benchmarks/reports/development/candidate-input-local-2026-09-12/README.md)
binds baseline `5411b9b`, the isolated patch and two plugin identities. Across
three seeds, paired public command gains are 27.6% for restricted weighted OLS,
18.9% for OLS with string clustering and 6.3% for restricted PPML with an offset.
OLS phase timers locate its gain in loading. PPML does not expose a separate
input timer; unchanged finite-solve timings also vary, so its entire gain is not
attributed to loading. The complete-data adjustment-only comparison shows no
reliable command gain. It remains a shared implementation, not a separate
performance claim.

All 29 pairs retain exact scientific exports and solver/retry/fallback decisions.
Selected native peaks fit unchanged models; no general peak-memory reduction is
claimed. Focused plugin tests, explicit-dummy/Poisson Stata oracles and bounded
controls cover missing codes, run/chunk boundaries, candidate gating, strings,
AW/FW/PW, low memory, interruption and atomic failure. This is local development
adoption, not SCC, cross-platform or release qualification.
