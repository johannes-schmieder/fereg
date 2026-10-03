# ADR 0011: Caller callbacks cover preloading admission

**Status:** Superseded by ADR 0247
**Date:** 2026-09-30

Current authority: [active ADR index](../../adr/README.md). Historical text below
is subordinate to that disposition and the approved incremental plan.

## Context

Native hosts must honor interruption while decoding and compiling model
metadata and planning storage, before loading observations or starting a
worker team. Existing callback-enabled estimation covers later numerical
phases. Preflight must share the existing validation and sole memory plan.

## Decision

- Add `ols::Call::new_with`, `ppml::Call::new_with`,
  `ModelWireV2::decode_specification_with` and `ModelSpec::from_wire_with`.
  Each borrows `FnMut() -> Result<()>` on its caller without Send/Sync bounds.
- Poll shared wire and typed metadata validation, column/source compilation,
  requested products, metadata accounting and storage estimates. Preserve
  callback errors directly; never retain callbacks or construct a worker team
  during preflight. Existing entries use the same implementation with no-op
  callbacks, preserving validation and error order when callbacks succeed.
- Reuse the compiled column view for layout and perform one admission before
  observation loading. No new Stata options or numerical policies are added.

## Consequences

- Hosts can connect their main-thread interruption API throughout preflight.
  Interrupted metadata owners are dropped; a fresh call is independently
  validated and admitted. Existing Rust clients remain source compatible.
- Boundary cancellation, original-error identity, non-Send caller affinity,
  exact fresh estimates and allocator release require executable checks.
  Native/Stata transaction binding remains separate milestone work.

## Alternatives rejected

- Starting a worker relay before admission adds an unplanned owner and permits
  host access from the wrong thread.
- A second metadata compiler or memory planner duplicates validation and can
  diverge from the estimation path.
