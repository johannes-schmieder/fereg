# ADR-0004: Stata SPI is main-thread-only

**Status:** accepted.

All `SF_*` calls are isolated in the C bridge and Rust `SpiSink` on Stata's
calling thread. Native workers may operate only on copied Rust buffers. This is
a hard safety contract even if a future Stata version appears tolerant of
concurrent access.
