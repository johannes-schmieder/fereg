# Threading contract

## Phase 0–1 implementation

`ThreadTeam` is a local, scoped standard-library thread team. `threads(0)` uses
`available_parallelism`; an explicit positive count is accepted and later
capped by the amount of useful work. Ranges are contiguous, nonempty, and
returned in worker order. The self-test exercises 1, 2, 8, and 32 requested
threads.

The construction package does not use a global process pool. This avoids hidden
state across Stata commands and prevents clashes with Stata/MP. A future
estimator call creates one local team and should reuse it across kernels inside
that one call.

## SPI prohibition

No worker may call `SF_*`, access Stata memory, read a Stata macro/matrix, write
an output variable, or poll Stata directly. The main thread copies data into
Rust-owned arrays, workers compute, and the main thread copies certified output
back.

## Determinism

`deterministic` fixes chunk ordering and serial merge order. It does not promise
bitwise identity across different architectures unless the later estimator's
floating-point kernels explicitly implement that stronger contract.

## Rayon decision

The supplied plan requested a local Rayon pool. This package uses a
dependency-free scoped team so the Phase 0–1 source can be audited offline and
linked without a third-party crate graph. ADR-0002 records this deliberate
implementation deviation. Before Phase 2, benchmark a local Rayon pool against
this interface; the public contract is the team abstraction, not the backend.
