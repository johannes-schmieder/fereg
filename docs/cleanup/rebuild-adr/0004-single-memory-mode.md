# ADR 0004: Single memory mode

**Status:** Deferred
**Date:** 2026-09-29

Current authority: [active ADR index](../../adr/README.md). Historical text below
is subordinate to that disposition and the approved incremental plan.

## Context

Fast/low storage twins multiply the existing execution and admission paths.
The approved target intentionally changes the public memory surface.

## Decision

- Use one prepared representation and one memory mode in the new engine.
- Estimate whole-call memory once, including the chosen backend, live outputs
  and workspaces. Fail early with r(909) when clearly insufficient.
- Use fallible allocations and preserve atomic failure on allocation errors.
  Remove nested optional-route memory admission from the new engine.
- Under engine(next), ignore memory() with an explicit note during migration;
  remove the option at M7. Legacy retains its existing modes until retirement.

## Consequences

- This supersedes legacy fast/low policy for the new engine only.
- Finite estimates do not authorize unchecked arithmetic or partial results.
- Memory tests cover checked sizing, clear early refusal and allocation failure;
  obsolete exact-byte route tests are mapped before retirement.

## Alternatives rejected

- Porting both storage pipelines defeats the approved consolidation.
- Removing memory checks entirely would violate failure and resource semantics.
