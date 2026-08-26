# ADR-0002: dependency-free scoped thread team in Phase 0–1

**Status:** superseded by ADR-0007 and the 0.5.0 per-action Rayon pool.

The initial package implements the thread-team contract with
`std::thread::scope` so source checks and the C bridge can be validated in an
offline sandbox without resolving a crate graph. It supports explicit 16/32
thread requests and deterministic ranges but does not yet provide persistent
worker reuse across many kernels. Before Phase 2, benchmark a local Rayon pool
behind the same interface and adopt it if it materially improves total runtime
without changing SPI confinement or deterministic-mode semantics.

The 0.5.0 implementation completed that migration. Each substantive action
constructs an isolated Rayon pool at the requested effective thread count;
deterministic-mode tests require identical results across supported counts.
This ADR remains as the historical record of the earlier offline bootstrap.
