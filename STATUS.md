# fereg 0.9.0 release-candidate status

**Last updated:** 2026-08-24
**Development branch:** local `main`; no development pushes
**Latest committed executable milestone:** `a0cc6ecb4aecc6e33bb668c9857ce920f8a5e5f9`
**Active milestone:** deterministic packaging and cross-platform qualification
**Release state:** public OLS candidate implemented; source qualification pending

`STATUS.md` is the mutable project-progress record. The 0.9.0 qualification
contract is ADR-0007. Historical receipts remain immutable even when they
describe earlier hidden or fail-closed checkpoints.

## Current accepted state

- Public `fereg` implements the documented OLS surface. The frozen 40-case
  semantic inventory is executable: multiple slope-only terms, string and
  interacted FEs, lag/lead/difference materialization, iweights, continuous
  DoF, saved outputs, prediction, `test`, and `estat summarize` are no longer
  blockers.
- The Stata public path retains one substantive plugin call. Rust owns sample
  construction and computation; SPI access stays on the Stata thread; the
  completion receipt is stored last; and failure/Break tests require rollback
  of `e()`, prior estimates, and requested output variables.
- `memory(fast|low|auto)`, scratch-backed low-memory execution, allocation
  preflight, bounded cleanup, allocation/RSS reporting, `timeit`, thread
  reporting, and deterministic per-action Rayon pools are implemented.
- `feppml` remains installed but is an explicit `r(498)` stub. PPML behavior
  and performance are outside this candidate.
- Candidate identity is `0.9.0`; public estimation uses action protocol 1.2,
  OLS result wire 1.2, and model wire 2.1. The reference closure is pinned to
  reghdfe 6.14.1 (`29b2b203...`), ftools 2.50.0 (`d6e24d66...`), and require
  1.3.1 (`f4760b86...`).

## Milestone exit criteria

The untagged 0.9.0 source checkpoint is qualified only when the exact candidate
SHA has green local Rust/Python/C/mock-SPI and licensed Stata evidence; native
Apple-Silicon and Rosetta slice evidence; SCC Stata 18/19 Linux scientific,
performance, memory, and clean-install evidence; hosted Linux/Windows build and
mock qualification; and deterministic audited source/release archives.

The governing performance and memory gates are those in ADR-0007 and
`docs/performance_contract.md`. No tag or GitHub release is part of this
checkpoint. Windows licensed Stata and native Intel-host macOS certification
are explicitly deferred and must remain visible caveats.

## Completed, evidence-backed baseline

- At committed source `a0cc6ecb4aecc6e33bb668c9857ce920f8a5e5f9`, the isolated
  licensed Stata/MP 18 Apple-Silicon quick lane passed 30/30. Its tested plugin
  SHA-256 was
  `10902ebea72314511c7ac03f830486c94fe13a5579da9e0cb260c3fe21547340`.
- The same implementation milestone passed repository checks, 232 Python
  tests, strict C, mock SPI, `cargo fmt`, Clippy with warnings denied, all-target
  Rust tests, and FFI tests. Full workspace counts were 278 core and 50 plugin
  plus integration tests.
- Licensed absorbed-slope checks passed IID, robust, aweight, cluster,
  `dof(none)`, and `dof(firstpair)`. The pure all-slope-only model now matches
  the pinned comparator's structural no-constant result.
- A constrained-memory local fixture passed with a 539,500,000-byte limit:
  fast mode returned `r(909)`, low mode succeeded, and auto selected low.
- The earlier accepted exact quick/full and captured hidden receipts remain
  useful historical evidence, but they do not qualify the current public
  source.

## Active checkpoint

Finish release identity and deterministic staging, then make a coherent local
candidate commit and rerun the exact-source local suite. After that, deploy the
committed snapshot—not Git history—to `/projectnb/welfgr/fereg`, qualify Linux
correctness under Stata 18 and 19, and run paired same-node performance and
memory jobs.

## Current blockers

1. Deterministic source/release ZIP staging, SBOM, dependency audit, native
   checksum inclusion, and developer-artifact exclusions are not yet green.
2. The exact post-packaging candidate SHA has not run the licensed local full
   profile, Rosetta slice lane, sanitizer/Miri/fuzz jobs where applicable, or
   repeated-call leak/store-failure matrix.
3. SCC Stata 18/19 smoke, correctness, clean-install, scientific-parity,
   performance, memory, and accounting receipts have not been accepted.
4. The governing 10-million-observation performance and memory gates are
   unmeasured. A local exploratory reference warmup was interrupted and is not
   evidence.
5. Hosted Linux build/load and Windows MSVC build/mock qualification are
   pending. Licensed Windows Stata and native Intel-host Stata remain deferred.
6. No exact-SHA release archives or final compatibility/evidence receipts exist
   yet, so no push, tag, or publication is permitted.

## Next evidence to record

Record a green deterministic packaging test and exact local candidate commit;
then attach exact-SHA local quick/full, native/Rosetta, SCC Stata 18/19,
scientific parity, performance, memory, build/mock, audit, and archive receipts.
Only after all feasible gates pass may the complete local commit stack be
pushed once to `origin/main` for exact-SHA hosted qualification.
