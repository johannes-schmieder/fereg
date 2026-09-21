# ADR 0089: load the finite RHS for slope-only OLS

**Status:** Accepted for local development under the approved optimization plan
**Date:** 2026-09-21
**Decision owner:** Johannes Schmieder

The existing bounded numeric input pipeline may construct the finite RHS for
one ordinary slope-only partition with one to three slopes. Its validated
recipe records whether a trailing constant exists. The worker copies the
outcome and ordered continuous sources into the admitted RHS buffer and adds
a constant only when the compiled model requests one.

The outer input-team and inner recipe guards share the same effect check.
Observation count, worker count, numeric-width, complete-candidate and memory
guards remain in force. Missingness, sample classification and final RHS value
validation remain authoritative; declined or stale reuse follows the existing
native path. SPI reads stay on Stata's main thread. The representation adds no
observation-scale allocation and changes neither weight nor inference semantics.

The [local evidence](../../benchmarks/reports/development/slopes-loader-local-2026-09-21/README.md)
includes weighted full-output comparisons, independent checks, pipeline/SPI
tests and constrained whole-fit admission. It preserves the ineffective first
guard experiment and failed fixture/input-generation attempts. Final timing
blocks, Linux confirmation and complete allocation proof remain separate.
