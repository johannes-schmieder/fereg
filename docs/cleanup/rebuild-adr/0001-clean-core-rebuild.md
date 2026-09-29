# ADR 0001: Clean-core rebuild and legacy retirement

**Status:** Accepted
**Date:** 2026-09-29

## Context

The existing core has duplicated preparation, storage and solver routes.
Johannes approved the amended issue-28 rebuild while retaining LSMR, CMG
and PPML mean bias correction.

## Decision

- Build `crates/fereg-engine` beside a frozen legacy core in the same repository.
  Harvest reviewed code by copying with provenance; do not depend on legacy
  production code from the new engine.
- Keep the Stata control plane, one substantive native call, safe Rust core,
  main-thread SPI and panic/transaction boundaries.
- Use a temporary plugin engine switch and a dev-only differential dependency.
  Cut over OLS and PPML separately only after G-OLS and G-PPML.
- Retire legacy in M7 after replacement coverage passes. Preserve old contracts,
  tests and evidence until their requirements are mapped into SPEC and checked.
- Track progress only in MILESTONES.md; the plan is docs/cleanup/PLAN.md.

## Consequences

- The verified legacy-v0.5.0-beta tag preserves the original source/evidence.
- Scientific behavior and failure atomicity remain mandatory during migration.
- Legacy changes before retirement are limited to the named safety/maintenance
  tasks; the new engine receives substantive development.

## Alternatives rejected

- Continued growth of guarded legacy routes retains the duplication.
- Immediate replacement or deletion loses the executable comparison boundary.
