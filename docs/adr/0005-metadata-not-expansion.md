# ADR-0005: Stata parses metadata but does not expand columns

**Status:** accepted.

Stata resolves factor semantics and canonical names with `fvexpand` and
`_ms_parse_parts`; Rust later performs observation-scale coding. The parser may
materialize only narrowly classified complex time-series operators in a future
protocol revision. Broad `fvrevar`, dummy generation, and `egen group()` are
prohibited because they defeat the memory architecture.
