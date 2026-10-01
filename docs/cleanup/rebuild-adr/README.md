# Rebuild architecture decisions

This staged set implements issue #28, plan §4.2. These decisions describe the
approved target; they do not claim that the engine or its gates are complete.
Existing `docs/adr/` documents remain available for scientific migration.
M7 activates this directory only after SPEC, passing replacement tests and the
coverage mapping account for the old requirements.

Write an ADR for a change to SPEC, the public API, architecture rules,
dependencies/licensing, or testing/evidence policy. Optimizations, experiments
and bug fixes belong in code, tests and the milestone log. Keep each ADR to one
page using TEMPLATE.md. Status is Proposed, Accepted, or Superseded by NNNN.
Number sequentially from 0001 within this staged set; historical ADR numbers
remain distinct until migration.

| ADR | Decision | Status |
|---|---|---|
| [0001](0001-clean-core-rebuild.md) | Rebuild and legacy retirement | Accepted |
| [0002](0002-fe-engine-backends.md) | One FE interface and retained solver choice | Accepted |
| [0003](0003-scientific-contract.md) | Scientific contract and independent accuracy | Accepted |
| [0004](0004-single-memory-mode.md) | One memory mode | Accepted |
| [0005](0005-thread-determinism.md) | Determinism across worker counts | Accepted |
| [0006](0006-public-surface.md) | Public surface and derived outputs | Accepted |
| [0007](0007-testing-evidence.md) | Testing, benchmarks and evidence | Accepted |
| [0008](0008-dependencies-licensing.md) | Dependencies and GPLv3 direction | Accepted |
| [0009](0009-factor-domain-precedence.md) | Ordinary factor sample precedence | Accepted |
| [0010](0010-engine-size-guidance.md) | Engine size guidance and retained limits | Accepted |
