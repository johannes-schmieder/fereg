# ADR 0008: Retained dependencies and GPLv3 distribution direction

**Status:** Accepted
**Date:** 2026-09-29

## Context

The existing plugin links MIT within and GPL-3.0-only CMG. Johannes approved
retaining both and accepts GPLv3 for fereg. Acceptance here records that
direction; native Stata/SPI distribution permissions have not been resolved.

## Decision

- Retain commit-pinned within, its needed preconditioner dependencies, and CMG.
  Keep the pinned toolchain and reproducible reference identities.
- Update project-owned licensing, Cargo metadata, third-party notices and
  corresponding-source packaging consistently for GPLv3 distribution.
- Preserve original third-party terms. Do not claim an exception for another
  rights holder's code or require CMG to become MIT.
- Before native-plugin distribution, determine the Stata/SPI combination's
  licensing requirements and obtain any required permissions from the relevant
  rights holders. Record evidence here and pass G-LICENSE before release.
- Remove unused/vendored/patched dependencies at M7 only after replacement checks.

## Consequences

- New-engine/oracle within pin: 268a1f5d4fe71ec3521cb02ef977620389a2cf40,
  [upstream repair PR 441](https://github.com/py-econometrics/within/pull/441).
  Fixed reductions, weighted Gram accumulation and raw-coordinate construction
  preserve the shared determinism/rank contracts; optional iteration polls
  support caller-thread interruption through a scoped relay. Explicit additive
  `ParallelReduction` is required; upstream Auto is not the deterministic policy.
- Frozen legacy retains within 1ac3522a251c2622dbeab451415924f9ebe117b9 until M7;
  legacy CMG remains 1d39c113f0c397f29653d92672090e5a42f80921. New-engine CMG pins
  980064c30b02f6e403f9739619e31ab7bcfa3d9d for optional caller-buffer PCG iteration polls.
  Frozen and rebuilt identities are separately recorded in `references.lock.toml`.
- G-LICENSE is unpassed: SPI terms, the proprietary host boundary and any
  necessary exception/permission coverage still require examination.
- Development may continue; changing a LICENSE label alone does not pass the gate.

## Alternatives rejected

- Removing CMG solely to preserve MIT contradicts the approved solver decision.
- Assuming fereg's own license grants rights over dependencies is insufficient.
