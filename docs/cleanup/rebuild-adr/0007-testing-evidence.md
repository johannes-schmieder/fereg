# ADR 0007: Testing and evidence for the rebuild

**Status:** Superseded by ADR 0248
**Date:** 2026-09-29

Current authority: [active ADR index](../../adr/README.md). Historical text below
is subordinate to that disposition and the approved incremental plan.

## Context

The rebuild needs executable scientific checks and compact reproducible evidence
without recreating the historical evidence infrastructure.

## Decision

- Run the lean fmt/Clippy/Rust/size/mock-SPI entry point in push/PR CI.
  Keep licensed Mac Stata checks manual; use SCC for larger campaigns.
- Require independent OLS/Poisson property tests, pinned reference parity,
  direct weak-graph/CMG regressions and legacy differential tests.
- Use the declared smoke/standard/full benchmark tiers, fixed seeds, fresh
  processes and repeated medians; compare matched accuracy and defaults.
- Gate automatic-policy adoption through G-M3, G-OLS and G-PPML. Report explicit
  LSMR/CMG comparisons separately; slower cells do not remove those options.
- Enforce the plan's exact gate thresholds, no hidden timeouts and same-machine
  thread determinism. Report missed final performance targets as follow-ups.
- Store compact benchmark summaries in bench/history; preserve original
  evidence and its identities until passing replacement coverage permits M7.

## Consequences

- A process exit code without required fixture checks is not scientific success.
- No ignored failure is a passing gate; known baseline repairs remain pending.
- The coverage mapping is a requirements index; MILESTONES is the only progress log.

## Alternatives rejected

- A no-regression fence per optimization recreates route proliferation.
- A historical tag without verified replacement tests cannot justify cleanup.
