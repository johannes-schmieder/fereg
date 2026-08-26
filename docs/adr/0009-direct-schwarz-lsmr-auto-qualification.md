# ADR 0009: direct Schwarz-LSMR qualification for automatic routing

**Status:** Accepted
**Date:** 2026-08-26
**Decision owner:** Johannes Schmieder

## Context

The capped automatic LSMR route was materially faster than certified CG on
weak worker-firm graphs, but the largest cases could miss the frozen reghdfe
observation-residual comparison even after their FE and OLS score certificates
passed. A coarse LSMR reduction followed by ordinary CG polishing improved the
bulk solve but did not remove that path-dependent comparator discrepancy.

Reghdfe is not an independent projection oracle. Qualification therefore
required a path-neutral accuracy contract before using direct LSMR beyond the
old 65,536-coefficient cap.

## Decision

1. Bounded categorical fixtures are adjudicated against a sealed 100-digit
   explicit-design oracle. The fixtures cover chains, barbell graphs,
   disconnected components, high- and low-mobility AKM graphs, unequal
   weights, multiple right-hand sides, and absorbed slopes. Residuals and
   fitted values use `atol=1e-9`, `rtol=1e-7`; objectives and checksums are
   sealed as well.
2. An independent observation-space audit estimates the remaining projection
   of a returned residual onto the complete FE span. It is a test and
   diagnostic condition. The existing rank-aware FE certificate and OLS score
   certificates remain runtime publication conditions.
3. Calibration compares requested-tolerance and 100-times-tighter direct LSMR,
   Schwarz/diagonal/off preconditioners, forced restarted CG corrections of
   4/8/16 iterations, and the prior coarse-LSMR/ordinary-CG hybrid. Forced
   correction bypasses CG's initial-certificate return and restarts its
   recurrence from the LSMR result.
4. The frozen survivor is direct Schwarz-preconditioned LSMR at
   `max(1e-14, requested_tolerance * 1e-2)`, with no CG refinement. Automatic
   routing keeps one-FE direct demeaning and the four-sweep MAP pilot. A
   multiway case routes to direct LSMR when pilot contraction is at least
   0.008; the former coefficient cap is removed.
5. An automatic LSMR failure or failed certificate has one visible retry from
   the saved MAP-pilot state using certified CG. The retry must fit within the
   remaining iteration and memory plans.
6. For an LSMR-routed result, the independent oracle and 100-times-tighter
   stability check are authoritative for residual accuracy. Reghdfe residual
   distance remains reported but is not a hard gate. Exact sample, singleton,
   coefficient status, coefficient, VCE, DoF, RSS, fit-statistic, FE-score, and
   OLS-score contracts remain hard.
7. Execution and solver workspaces are planned jointly. The route-specific
   model includes CG, LSMR, Schwarz, RHS, topology, and thread-reduction peaks.
   If LSMR does not fit but CG does, auto selects CG; if neither fits, the call
   fails before large solver allocation. The existing internal memory-limit
   seam feeds this planner; no public `maxmemory()` syntax is added.
8. Comparator RSS ratios are diagnostic. Promotion instead requires modeled
   native allocation within 25 percent, clean constrained-memory selection or
   failure, and no repeated-call leak.

## Qualification evidence

The frozen final universal macOS plugin is identified by SHA-256
`2f3ff78a7e154eab72651adc24b3c0519cc4d1d4cd6cd6ed08288564ff25f16e`.
The sealed summary is
`benchmarks/expected/direct_lsmr_qualification_v1.json`.

- The complete 100,000-row matrix produced 148 timing records and ten green
  scientific preflights. Only the two already-quarantined high-mobility IT
  diagnostics failed closed.
- Fresh-seed one-million-row holdouts passed. Routed low mobility reduced warm
  median runtime by 73.8 percent at one thread and 65.6 percent at eight
  threads. Unrouted standard cases changed by -1.4 and +3.3 percent.
- The untouched ten-million-row freeze passed. Routed low mobility reduced
  warm median runtime by 76.9 percent at one thread and 50.7 percent at eight
  threads. Unrouted standard cases improved by 2.7 and 0.4 percent.
- Final routed ten-million-row allocation-model error was 0.4 percent at one
  thread and 20.3 percent at eight threads; repeated-call leak gates passed.
  The routed residual remained stable against the 100-times-tighter solve by
  a factor of more than four relative to its registered envelope.
- Repository checks, deterministic source/release staging, portable/static
  checks, strict Clippy and formatting, all workspace/all-target Rust tests,
  FFI tests, 269 Python tests, mock SPI, universal binary audit/loader, and all
  32 licensed Stata fixtures passed on the final implementation.

## Consequences

- Direct Schwarz-LSMR is a production automatic route, not merely an explicit
  escape hatch. Reghdfe residual bits are no longer allowed to veto an
  independently accurate routed projection.
- The coarse-LSMR/CG hybrid and forced-CG variants remain qualification arms;
  they are not extra production stages or public options.
- A future `maxmemory()` option must populate the existing joint planner. It
  may not introduce a second solver-routing system.
- This decision does not qualify PPML, other platforms, or a public release.
