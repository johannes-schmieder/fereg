# ADR-0002: dependency-free scoped thread team in Phase 0–1

**Status:** provisional deviation from the requested Rayon implementation.

The initial package implements the thread-team contract with
`std::thread::scope` so source checks and the C bridge can be validated in an
offline sandbox without resolving a crate graph. It supports explicit 16/32
thread requests and deterministic ranges but does not yet provide persistent
worker reuse across many kernels. Before Phase 2, benchmark a local Rayon pool
behind the same interface and adopt it if it materially improves total runtime
without changing SPI confinement or deterministic-mode semantics.
