# AGENTS.md — fereg

fereg is a Stata package (`fereg` for OLS, `feppml` for PPML) with a Rust
computational core loaded as a Stata plugin. Stata owns syntax, metadata and the
user-facing transaction; Rust owns data loading, fixed-effect absorption,
estimation and inference.

The repository is in a clean-core rebuild. The plan of record is
`docs/cleanup/PLAN.md`; progress is tracked only in `MILESTONES.md`.

## Sources of truth (in order)

1. Direct instructions from Johannes.
2. `docs/SPEC.md` — what results must be (samples, weights, DoF, VCE, outputs, PPML).
3. `docs/adr/` — accepted decisions (short; see `docs/adr/README.md`).
4. `docs/ARCHITECTURE.md` — how the new core is structured.
5. `docs/cleanup/PLAN.md` and `MILESTONES.md` — the plan and where we are.
6. Tests and code.

Until milestone M1 is complete, `docs/SPEC.md` and `docs/ARCHITECTURE.md` may
not exist yet; follow this plan's explicit decisions and the existing scientific
contracts meanwhile. New rebuild ADRs are staged in `docs/cleanup/rebuild-adr/`
until M7; they supersede corresponding legacy policies only where stated.
Preserve legacy ADRs and scientific documents for the SPEC/coverage migration.

If these conflict on econometrics, public behavior or architecture: stop, add
the question under "Blockers" in `MILESTONES.md`, and ask Johannes.

## Layout

- `crates/fereg-engine` — the new core. All new numerical work goes here.
- `crates/fereg-plugin` — Stata plugin host: FFI, SPI loading, result publication.
- `crates/fereg-oracle` — dense oracles, random design generators,
  property and differential tests (dev only).
- `legacy/` — the frozen legacy core. Read it to understand old behavior and to
  harvest code by copying. Change it only for tasks in `MILESTONES.md` that say
  so. Never optimize or extend it. It is deleted in milestone M7.
- `stata/` ado/sthlp/Mata · `plugin/` C bridge · `tests/stata/` Stata fixtures ·
  `bench/` benchmark suite · `docs/` SPEC, ARCHITECTURE, ADRs, plan.

## Architecture rules (non-negotiable)

1. One substantive plugin call per estimation. Publish results only after the
   complete result has been validated; any error leaves no partial results.
2. Stata SPI calls happen only on Stata's main thread; worker threads touch only
   Rust-owned memory. Catch panics at the FFI boundary. `unsafe` is allowed
   only in the plugin's FFI module; every other crate forbids it with the
   crate-level attribute `forbid(unsafe_code)` in its root file.
3. One implementation per concept: one prepared-data representation, one FE
   engine interface, one finite WLS solver, one VCE module, shared certification.
   Direct, Schur-PCG, LSMR and CMG backends use that interface. Retain public
   `solver(auto|lsmr|cmg)` for OLS and PPML. Never add parallel "fast/low",
   "legacy/categorical" or per-VCE pipelines.
4. Honor explicit solver requests within documented mathematical support.
   Automatic selection depends on model structure and measured convergence,
   with comparative evidence for the default policy. Do not key routes on N,
   regressor count, thread count, weight kind or memory. Size-dependent constants
   are allowed as parallel chunk sizes inside kernels. Document allowed
   fallbacks and report requested/used solvers and reasons; unsupported explicit
   requests fail clearly.
5. Consolidate shared work. LSMR and CMG are retained supported backends;
   neither must win every benchmark to remain explicitly selectable. Changes
   to automatic selection and additional algorithms must earn their place on
   the benchmark suite overall (geometric mean) without a regression beyond
   its noise band. Never fence a gain into a new size-specific special case.
6. Every backend reconstructs residuals from coefficients (r = y − Dα) and
   checks D'Wr relative to SPEC scales. On failure, fail closed with a clear
   error. These checks alone do not establish forward accuracy on ill-conditioned
   designs; retain independent explicit-design/high-precision weak-graph and
   estimator-level accuracy tests.
7. Results are deterministic and identical across thread counts on the same
   machine: reductions use fixed partitions that do not depend on the number
   of workers.
8. Single memory mode: estimate memory once at the top, fail early with
   `r(909)` when clearly insufficient, otherwise use fallible allocation. No
   nested optional-memory admission.
9. Size limits (enforced in CI by `scripts/check_sizes.sh` and
   `clippy::too_many_lines` with threshold 150): files ≤ 1,500 lines,
   functions ≤ 150 lines. They apply to `crates/fereg-engine` and
   `crates/fereg-oracle` now, and to every crate after M7. No `#[cfg(test)]`
   branches inside production control flow; tests reach internal paths through
   explicit overrides (for example `fe::PlanOverride`).
10. Unsupported features fail with a clear error; never silently fall back to a
    different model or estimator.

## Scientific rules

- `docs/SPEC.md` defines behavior. Never weaken tolerances, samples, weight
  semantics, DoF, VCE or certificates to make a test pass.
- Correctness needs both parity with the pinned references
  (`references.lock.toml`) and independent oracles (`crates/fereg-oracle`).
- Treat numerical disagreements as questions to localize, not to tune away.
- Reproduce and localize the reported CMG oracle failure before choosing its
  repair. Keep an executable regression; neither rewriting nor rerouting alone
  establishes that the explicit CMG backend is accurate.

## Workflow

- Work on `main`, one `MILESTONES.md` task at a time, in small commits named
  `M<n>.<k>: <summary>`. Push after each completed task once its local checks
  pass. Never rewrite history or force-push.
- After each task: run its checks, tick it in `MILESTONES.md`, add one log line
  (date, task, commit, checks, key numbers), commit, push.
- If blocked: record the blocker in `MILESTONES.md`, continue with the next
  independent task, and stop only when nothing independent remains.
- Do not create new status files, reports, receipt folders or ADRs outside the
  rules below.

## Testing

Always, before every commit (CI runs the same script on every push):

```bash
./scripts/run_tests.sh   # fmt check, clippy -D warnings, cargo test, size limits, mock-SPI tests
```

Engine changes additionally run the long property and differential tests:
`cargo test -p fereg-oracle --profile ci -- --include-ignored`.

**Local Mac (small and medium tests).** Licensed Stata/MP with an 8-processor
license on a 24-core Mac Studio (16 performance cores). Use it for Stata
fixtures (`./ci/run_stata_ci.sh <profile>`), plugin builds
(`./scripts/build_macos.sh`), the benchmark `smoke` and `standard` tiers, and
quick A/B timings. Native `threads()` may exceed Stata's processor count; for
timing keep native threads ≤ 16.

**SCC cluster (larger test campaigns).** Boston University SCC, project `welfgr`.
- Use 28-core nodes only: `#$ -pe omp 28` with `#$ -l cpu_type=E5-2680v4`
  (or `#$ -l num_proc=28`), plus `#$ -P welfgr`, `#$ -l mem_per_core=3G`
  (raise if a cell needs more) and `#$ -j y`.
- Wall time: never request more than 22 hours (`#$ -l h_rt=22:00:00` at most).
  Split longer campaigns into array tasks or several jobs.
- Stata on SCC: `module load stata-mp/19`. The Stata/MP license there has only
  4 cores, so `set processors` ≤ 4. fereg's native `threads()` is not limited
  by the Stata license: thread scaling is tested at 1, 2, 4, 8, 14 and 28 native
  threads.
- Rust: `CARGO_HOME`/`RUSTUP_HOME` under
  `/projectnb/welfgr/johannes/fereg-rust-1.85.1/`. Build the Linux plugin with
  `./scripts/build_linux.sh`.
- Run directories: `/projectnb/welfgr/fereg/runs/<run-id>/` for campaigns and
  `/projectnb/welfgr/fereg/dev/<run-id>/` for development jobs. Use job
  templates in `bench/scc/`. Record job IDs in the `MILESTONES.md` log.
- Timing jobs get a whole node (`-pe omp 28`); never time on a shared node.
- Raw outputs stay on SCC; commit only compact summaries to `bench/history/`.

## Evidence and documentation

- `MILESTONES.md` is the only mutable status document: task checklist, current
  task, blockers, one-line log entries. Keep it under 300 lines.
- ADRs only for changes to SPEC, the public API, these rules, dependencies or
  licensing, or the testing/evidence policy. At most one page. Never for
  optimizations, experiments or bug fixes.
- Performance claims must cite a summary in `bench/history/`.
- Preserve old contracts, ADRs, scientific tests and evidence until SPEC and
  `docs/cleanup/coverage.md` account for their requirements with passing
  replacement checks. Historical evidence remains reachable at the verified
  legacy tag; final working-tree cleanup occurs in M7.
- Never commit credentials, license files, personal local paths (the SCC
  project paths above are fine), raw logs, datasets or new files over 1 MB
  (the existing `ci/reference-bundles/` are the only exception).
- Comments explain why (invariants, numerical safeguards, safety), not what.
- The working paper lives in the sibling `fereg-paper` repository; paper text,
  figures and paper builds belong there, not here.
