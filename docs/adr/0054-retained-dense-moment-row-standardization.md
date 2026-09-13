# ADR 0054: Retained dense moment row standardization

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-13

For scaled retained dense finite designs with at most 64 active columns, cache
each observation's selected standardized suffix once per existing Gram-row
partition. Use the cache only when the layout assigns more than one left column
per partition; its final tail may contain one. Reuse the physical 64-column
cache bound without changing any estimator's rank or routing thresholds.

The partition owns a fixed 64-double stack array and retains the same Gram/RHS
rows, observation order, weight normalization and fused multiply-add expressions.
Resolve selected columns through their actual stored mapping. Wider, unscaled,
single-column partitions, retained scratch and one-shot prepared OLS sources
keep their previous loops. The separate helper is non-inlined to keep its array
outside excluded calls.

No full data pass, observation-sized copy, heap allocation, Gram partial bank,
solver setup or cross-frame cache is introduced. The local payload is 512 bytes
per partition, conservatively at most 32 KiB across eligible partitions. Native
heap instrumentation does not measure that stack payload. Original scales,
current-weight norms, independent certificates, retained QR, strict basis
transactions and main-thread SPI boundaries remain unchanged.

The [source-bound report](../../benchmarks/reports/development/ppml-moments-local-2026-09-13/README.md)
records 16 pairs. Complete/FW/PW median paired command gains are 22.0%/24.0%/24.4%,
with lower finite and strict solving. Full b/V and admitted native peaks are
exact. RSS moves both ways; the largest protected command increase is 10 ms.
The private audit retains zero events/live/peak growth and stable capacities.
Sixty-two focused tests and both independent Poisson/sandwich oracle arms pass.
These results establish this bounded local adoption, not broader-width/scratch
benefits, a new automatic solver region or release qualification.
