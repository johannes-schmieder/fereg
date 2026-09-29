# ADR 0005: Determinism across thread counts

**Status:** Accepted
**Date:** 2026-09-29

## Context

Reproducibility should not require a separate user option or solver family.

## Decision

- Results must be identical across thread counts on the same machine.
- Use fixed reduction partitions independent of worker count and deterministic
  merge order. Worker scheduling may change without changing arithmetic order.
- Apply this requirement to every supported backend and dependency adapter,
  as well as preparation, finite solves and covariance.
- Keep workers on Rust-owned data; all SPI operations remain on the main thread.
- Remove the deterministic option once the new default is cut over.

## Consequences

- Tests must compare multiple thread counts, including explicit LSMR and CMG.
- Cross-platform bitwise equality is not asserted by this decision.
- Parallelism and determinism must be measured together before route adoption.

## Alternatives rejected

- Merely deterministic scheduling at a fixed worker count is insufficient.
- A separate deterministic slow path duplicates the supported engine.
