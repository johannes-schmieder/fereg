# ADR 0007: fereg 0.5.0 beta-qualification contract

**Status:** Superseded by ADR 0008
**Date:** 2026-08-24  
**Decision owner:** Johannes Schmieder

## Context

The OLS implementation has moved from a guarded development surface to the
public `fereg` command. The first public milestone is a source-qualified,
candidly labeled 0.5.0 beta. The
scientific comparator remains the isolated closure pinned in
`references.lock.toml`:

- reghdfe 6.14.1 at `29b2b203f534413369d675d0c0f674d61291f536`;
- ftools 2.50.0 at `d6e24d66fd02ba1b37325c82c5040444c5b4c636`;
- require 1.3.1 at `f4760b86e55cf73b82041ce59cf20e8e8224afb0`.

Earlier Phase 0–1 documents used exploratory 1.2–1.5x performance language or
an overall 1.5x geometric-mean threshold. Those values no longer govern the
beta.

## Decision

1. All release-facing package, diagnostic, help, manifest, and artifact
   identities use version 0.5.0. Public estimation uses action protocol 1.3,
   OLS result wire 1.3, and model wire 2.1. Compatible diagnostic actions may
   retain their narrower field sets.
2. Public `fereg` covers the frozen OLS 1.0 surface. `feppml` remains installed
   but must return `r(498)` without posting results. PPML correctness or speed
   cannot block OLS qualification beyond that fail-closed regression test.
3. Qualification applies to one exact source SHA. The repository may contain
   coherent local milestone commits, but development is not pushed until the
   complete stack is release-level. No tag or GitHub release is created by the
   0.5.0 source checkpoint.
4. The governing performance gates, evaluated only after scientific parity,
   are:
   - geometric-mean candidate/reference runtime ratio no greater than 0.40
     (equivalently at least 2.5x faster);
   - at least 80 percent of approved cases at least 2x faster;
   - the registered single-thread comparison at least 1.5x faster;
   - no unexplained case more than 25 percent slower than the comparator.
5. The governing memory gates are:
   - candidate peak RSS no worse than the comparator in at least 80 percent of
     approved cases;
   - no unexplained case above 1.25 times comparator RSS;
   - observed native allocation within 25 percent of the registered model;
   - no repeated-call leak; and
   - a constrained case where low mode succeeds while fast mode fails
     preflight.
6. Licensed qualification covers macOS Apple Silicon Stata 18 and SCC/Linux
   Stata 18 and 19. The macOS x86_64 plugin slice is exercised under Rosetta,
   but native Intel-host Stata remains deferred. Windows x64 is built and
   mock-SPI qualified through hosted MSVC; licensed Windows Stata is deferred
   and must be disclosed prominently.
7. Release and source artifacts are deterministic and staged outside the
   tracked tree. Native plugins are checksum-verified before inclusion.
   Developer-only Stata adapters, raw logs, reference PDFs, and build caches
   are excluded. The artifact set includes dependency audit information,
   checksums, SBOM/license notices, and accurate platform caveats.

## Consequences

- Historical receipts and documents remain immutable evidence even where they
  retain the former project name or superseded phase language.
- `benchmarks/manifests/ols_v1.json`, its verifier, and current documentation
  must encode the 2.5x overall gate rather than the exploratory 1.5x value.
- A green local correctness lane is necessary but insufficient. Source-qualified
  status requires exact-SHA local, SCC, macOS, Linux build/load, Windows
  build/mock, memory, performance, and packaging receipts.
- If any remote qualification fails, 0.5.0 remains unqualified and untagged.

## Supersession

ADR 0008 preserves this ADR's scientific, runtime, platform, artifact, and Git
requirements, but replaces its mandatory deterministic-default solver policy
and comparator-relative RSS gates. The four completed MAP performance cases at
source `df53822b3263d7425a9a2a42b16b58f7b306dc91` are retained as the frozen
pre-redirection implementation baseline.
