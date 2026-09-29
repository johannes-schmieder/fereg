## 0. How to execute this plan

1. If `MILESTONES.md` does not exist yet, start at **M0.1**. Otherwise read `AGENTS.md`, `MILESTONES.md` and `docs/cleanup/PLAN.md`, then take the **first unchecked task**.
2. Do exactly that task and keep the diff focused on it. Harvested legacy code is **copied** into the new crates (with a provenance line), never depended on. The only allowed dependencies on legacy code are the temporary dev-dependency in `crates/fereg-oracle` (differential tests) and the plugin's existing dependency on the legacy core, which serves the temporary engine switch. Both are removed at M7.
3. When a task deletes or moves files, update every kept file that *uses* the old path: code and include paths, scripts, CI, build and packaging, and config such as `.gitignore`. User-facing text (`README.md`, `docs/user_guide.md`, `stata/*.sthlp`) must not point to deleted files. Comments and history (`CHANGELOG.md`, `MILESTONES.md`) may still mention them; the old tree stays available at tag `legacy-v0.5.0-beta`.
4. Run the task's **Checks**. Rust checks run everywhere; Stata checks run on the local Mac; large campaigns run on SCC (see `AGENTS.md`). If a required check cannot run in your environment, mark the task `[~]` in `MILESTONES.md` with exactly what is pending, and continue with the next task that does not depend on it. A gate (`G-*`) is never marked passed without its evidence.
5. Update `MILESTONES.md`: tick the task and add one log line (date, task, commit, checks, key numbers). Commit as `M<n>.<k>: <summary>` and push to `main`.
6. Repeat until M9.3 is done. Stop and ask Johannes only for: (a) a genuine SPEC ambiguity about econometrics or public behavior; (b) the M3 go/no-go gate failing; (c) a gate that still fails after one focused fix attempt; (d) any wish to change a decision in §3. Record the question under "Blockers" in `MILESTONES.md` before stopping.
7. After M0, `docs/cleanup/PLAN.md` (a verbatim copy of §0–§8 of issue #28) is the plan of record. Amend it only by commit, with a log line explaining the change.
8. **Amendment approved by Johannes, 2026-09-29:** retain public `solver(auto|lsmr|cmg)` for OLS and PPML; keep the pinned `within` LSMR and CMG dependencies behind one shared FE interface; adopt GPLv3 as the distribution direction, subject to resolving the native Stata/SPI licensing boundary. Preserve independent accuracy checks and defer destructive scientific-documentation, test and evidence cleanup until requirements and executable coverage have been carried forward. The corresponding decisions and tasks below incorporate this amendment; the historical findings in §1 remain attributed to their original review.
9. **Cleanup prerequisite:** before deleting legacy scientific contracts, ADRs, tests, harnesses or evidence from the working tree, consolidate their requirements in SPEC and record their replacement tests in `docs/cleanup/coverage.md`. This is a requirements-to-tests mapping, not another status log. Replacement checks must have passed and the original evidence must remain reachable at the verified legacy tag. Perform final removal in M7 after G-OLS and G-PPML; a tag alone does not prove that a requirement survived.

## 1. Why: problems with the current codebase

Findings from a review of HEAD `b41f18e` (2026-09-28):

**Path explosion.** Each optimization was added as a new, narrowly guarded route while all existing routes were "retained" (248 ADRs in five weeks; the ADRs say "retain their [prior policy]" 139 times and mention fallbacks 175 times).
- Production code contains ~55 row-count gates at N = 16,384, 21 thread-count-versus-8 gates, width windows (3–8, 8–64, ≥16), and unit-weight-only, fast-memory-only and no-saved-FE conditions.
- Two OLS data representations × three VCE kinds give six near-identical pipelines (`estimate.rs:266-507`). There are at least five FE representations, ~55 absorb/recover entry points (15 just for FE recovery), fast/low-memory twins throughout, and a 1,900-line PPML IRLS function (`prepared_irls.rs:1005`).

**Performance cliffs (measured).** Same 1M-row worker–firm panel, core Rust API, 4 cores:

| Change | Route before → after | Time |
|---|---|---|
| 2 FE, 13 → 14 regressors | CMG → Schur-CG | 2.01 s → 1.00 s |
| 2 FE, 7 regressors, request 8 threads on 4 cores | CMG → Schur-CG | 1.14 s → 0.57 s |
| 2 FE, 7 regressors, add analytic weights | CMG → LSMR | 1.14 s → 2.02 s |

The fastest engine already exists, but the gates keep it away from most models.

**Reliability.**
- `cargo test --release --workspace --no-fail-fast` at HEAD: 1,457 passed, 13 failed. Nine allocation-audit tests fail only under the parallel test runner. Four fail deterministically:
  - the oracle test `favorable_auto_topology_uses_cmg_without_a_map_pilot_in_fast_and_low_modes` — low-memory CMG residuals are off by 9.1e-5 while reported as certified;
  - two memory-contract tests whose pinned byte counts are stale;
  - one checkpoint-Schur test where code, documentation and test disagree.
- Explicit `solver(cg)` publishes certified but inaccurate results. On an 800-row two-FE design, β is off by 5.5e-5 at the default tolerance with every certificate passing. At tolerance 1e-10 the FE step returns β = −0.001 instead of 0.6995, and only the later inference check stops publication. `auto` can reach the same CG kernel. The cause: the FE certificate checks only that D'Wr ≈ 0, not that y − r lies in the column space of D.
- Conservative memory admission silently vetoes fast routes. The generic Schwarz bound is 316 GiB where a topology-specific bound is 0.69 GiB, and large degree-two OLS times out at 720 s. The Sep 26 campaign had 28 command timeouts and 4 r(459) failures.

**The gap to fixest is mostly outside the numerics.** fereg is about 7× faster than reghdfe, but its median runtime ratio against default fixest is 1.16 for both OLS and PPML.
- In a 1M×50 one-FE profile, fereg's native work beats fixest (absorption 55 ms vs 156 ms; finite solve 31 ms vs 136 ms). But 307 of the 593 ms total go to one-value-at-a-time SPI reads, and Linux and Windows use a slower reader than macOS.
- Stata/MP's own threads add about 50% on small fits.

**Code and process weight.**
- 137K lines of production Rust plus 80K lines of tests, with files of 10K–16K lines.
- STATUS.md is 19.6K lines; about 4 GB of receipts are committed (2.8 GiB pack); 210K lines of Python harness; CI runs only when triggered by hand.
- The acceptance rule "no cell may regress; fence each gain to its measured envelope" is the root cause of the path explosion.
- CMG, licensed GPL-3.0-only, is statically linked into the MIT-licensed plugin.

## 2. Goal and end state

Build a **new, clean core** (`crates/fereg-engine`) beside the frozen legacy core. Move `fereg` and then `feppml` onto it through an engine switch in the plugin, then **delete the legacy core**. Keep the Stata front end, the transaction/rollback model, the public semantics (SPEC), and parity with pinned reghdfe and ppmlhdfe.

End state:
- One prepared-data representation, one FE engine interface with direct, coefficient-space Schur-PCG, `within` LSMR and CMG backends, one finite WLS solver, one VCE module, and shared independent result checks. Public `solver(auto|lsmr|cmg)` is retained for both estimators.
- Automatic algorithm choice depends on model structure and observed convergence, with comparative evidence for the default policy. Honor explicit solver requests within each backend's documented support. Do not recreate routing keyed on N, regressor count, thread count, weight kind or memory.
- Every backend reconstructs residuals explicitly from FE coefficients and checks D'Wr with the same scales and tolerances. These checks do not alone establish forward accuracy on ill-conditioned designs: retain independent weak-graph/high-precision oracles and estimator-level accuracy checks. Results are identical across thread counts. There is a single memory mode.
- At most 35K lines of production Rust (engine ≤ 25K, plugin ≤ 8K); no file over 1,500 lines and no function over 150 lines.
- CI on every push; property tests against dense oracles; a small benchmark suite (Mac + SCC) with compact summaries in git; `MILESTONES.md` as the only status document; ADRs only for real decisions.
- Performance targets against fixest tracked by the suite (§7).

Target layout:
```
AGENTS.md  MILESTONES.md  README.md  CHANGELOG.md  LICENSE  THIRD_PARTY_NOTICES.md
Cargo.toml  rust-toolchain.toml  references.lock.toml
crates/fereg-engine/   new core (safe Rust only)
crates/fereg-plugin/   Stata plugin host: FFI, SPI loading, result publication
crates/fereg-oracle/   dense oracles, design generators, property/differential tests (dev only)
legacy/                frozen legacy core + testkit (exists only until M7)
plugin/                C bridge + Stata SPI headers
stata/                 ado, sthlp, Mata
tests/stata/           Stata fixtures: parity_reghdfe, parity_ppmlhdfe, installation, parser, postestimation, documentation, bias
tests/mock_spi/        C bridge tests
bench/                 suite.toml, generators, runners, scc/, real/, history/ (summaries only)
ci/                    licensed Mac Stata runner
scripts/               build_macos.sh, build_linux.sh, build_windows.ps1, run_tests.sh, packaging
docs/                  SPEC.md, ARCHITECTURE.md, adr/, cleanup/PLAN.md, user_guide.md, compatibility.md
```

## 3. Decisions made by this plan

1. **Same repository.** The legacy core and testkit move to `legacy/`; new crates go under `crates/`.
2. **Single memory mode.** Drop the fast/low split. The new engine estimates memory once, fails early with `r(909)` when the estimate clearly exceeds available memory, and otherwise relies on fallible allocation. `memory()` is ignored (with a note) under `engine(next)` and removed at M7.
3. **Public surface.**
   - Kept: model syntax, `absorb()` (interactions, slopes, saved FEs), `vce()`, `dof()`, `keepsingletons`, `residuals()`, `noconstant`, weights, `solver(auto|lsmr|cmg)` for both `fereg` and `feppml`, `threads()`, `tolerance()`, `iterate()`, `verbose`, `timeit`, and `feppml`'s documented PPML options.
   - Removed: public `solver(map|cg|it|squarem)`, `preconditioner()`, `poolsize()`, `memory()`, and `deterministic` (determinism becomes the default). A hidden developer override remains available for internal plans and tests; it does not replace the retained public LSMR and CMG options.
4. **FE engine.** One shared FE interface with direct projection, Schur-PCG, `within` LSMR and CMG backends. `within` is the Rust library implementing modified LSMR with Schwarz preconditioning; it is not an additional public solver family. The initial `auto` candidate is direct projection for one FE and Schur-PCG for multiple FEs, with monitored LSMR fallback; its adoption requires G-M3 and estimator gates. Retain explicit `solver(lsmr)` and `solver(cmg)`. CMG inclusion in `auto` requires comparative evidence and a documented structural/convergence rule; retaining the option does not make it the default. All backends share preparation, output reconstruction, inference and independent certification. CMG is limited to designs with a valid graph-Laplacian/SDDM formulation; it is not a generic substitute for arbitrary slope or multiway systems. SPEC records current supported structures for both estimators and any required adaptation. Unsupported explicit requests fail clearly. Document allowed convergence fallbacks, report requested and used solvers and the fallback reason, and never silently substitute a backend.
5. **Derived outputs** (residuals, xbd, saved FEs) are certified at the requested `tolerance()`. There is no automatic re-solve to 1e-14.
6. **Determinism.** Results are identical across thread counts on the same machine (fixed-partition reductions).
7. **No new features** (IV, `c.x#i.g` RHS interactions, HAC, other GLMs) until M9 is complete. File them as follow-up issues.
8. **Push policy.** Codex pushes to `main` after every completed task whose local checks pass. Tags are created only at M0.1 and M9.3.
9. **Windows** gets build and mock-SPI checks only; there is no licensed Windows Stata.
10. **Dependencies and licensing.** Retain commit-pinned `within` (MIT) and CMG (GPL-3.0-only). Johannes accepts GPLv3 for fereg; preserving an MIT-only distribution is no longer a requirement. Update project-owned licensing, Cargo metadata, notices and source-distribution packaging consistently, preserving third-party notices. Before distributing native plugins, resolve the combination with proprietary Stata and its SPI, including whether permissions or an exception are required from relevant rights holders. Changing fereg's license cannot grant an exception for dependency code. Record the disposition in ADR 0008 and pass G-LICENSE before binary distribution/release; there is no requirement to relicense CMG to MIT. [GNU licensing guidance](https://www.gnu.org/licenses/gpl-faq.en.html#GPLIncompatibleLibs).
11. **Scientific continuity and cleanup.** Preserve independent explicit-design and high-precision weak-graph tests for every backend. Reproduce and localize the reported CMG failure before selecting its repair; a rewritten adapter or passing normal-equation check is not proof that it is fixed. Keep the old scientific documents, ADRs, tests and evidence until SPEC and passing replacement coverage account for them, then remove superseded working-tree files at M7 under §0.9.

## 4. Step one: new agent infrastructure (M0)

### 4.1 New `AGENTS.md` (write this text verbatim in M0.2)

````markdown
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
````

### 4.2 ADR system (M0.3)

- Preserve the existing ADRs in `docs/adr/` through specification and test migration. Stage the rebuild ADR set in `docs/cleanup/rebuild-adr/` at M0.3; activate it as `docs/adr/` only in M7.5 after §0.9 is satisfied. Original ADRs remain reachable at the verified legacy tag.
- The staged `README.md` (eventually `docs/adr/README.md`) contains:
  - when to write an ADR (the AGENTS rule);
  - the format (at most one page; status is `Proposed`, `Accepted` or `Superseded by NNNN`);
  - sequential numbering restarting at 0001;
  - an index table.
- The staged `TEMPLATE.md` (eventually `docs/adr/TEMPLATE.md`) has these sections: Title · Status · Date · Context (≤ 10 lines) · Decision (bullets) · Consequences (bullets) · Alternatives rejected (bullets).
- Write these ADRs, each `Accepted`:
  - **0001 Clean-core rebuild and legacy retirement.** A new core beside the frozen legacy core; cutover per feature class through an engine switch; legacy deleted at M7 (this plan).
  - **0002 One FE interface, retained solver backends.** AGENTS rules 3–6; shared direct/Schur-PCG/LSMR/CMG interface, public `auto|lsmr|cmg`, documented mathematical support and visible fallback. Initial automatic policy and any later CMG selection require comparative evidence.
  - **0003 Scientific contract.** `docs/SPEC.md` is authoritative. Correctness requires both parity with the pinned references and independent oracles; test tolerances as in SPEC.
  - **0004 Single memory mode.** Drop fast/low (§3.2).
  - **0005 Deterministic results across thread counts** (§3.6).
  - **0006 Public surface reduction with retained solver choice** (§3.3, §3.5): keep `solver(auto|lsmr|cmg)` for both estimators.
  - **0007 Testing and evidence policy.** CI on push; oracle property tests; the benchmark suite and its tiers; the Mac/SCC split; summaries only in git; acceptance on the suite's geometric mean with no fencing.
  - **0008 Dependencies and licensing.** Retain pinned CMG (GPL-3.0-only) and `within` (MIT); GPLv3 direction for fereg, updated notices/metadata/source packaging, and a recorded resolution of the Stata/SPI linking boundary before native-plugin distribution (G-LICENSE). Acceptance records the direction, not a claim that outstanding permissions are resolved. No vendored or patched dependencies in the end state; pinned toolchain.

### 4.3 `MILESTONES.md` template (M0.1)

```markdown
# fereg clean-core rebuild — milestones

Plan: docs/cleanup/PLAN.md (issue #28). Legacy snapshot: tag `legacy-v0.5.0-beta`.
Current task: M0.1
Default engine: fereg = legacy, feppml = legacy
Legend: [ ] todo · [x] done · [~] implemented, verification pending (say what)

## Tasks
- [ ] M0.1 Tag legacy, copy plan, create MILESTONES.md
- [ ] M0.2 ...   (one line per task in docs/cleanup/PLAN.md §6, in order)

## Gates
- [ ] G-M3 engine go/no-go   - [ ] G-OLS   - [ ] G-PPML   - [ ] G-FINAL
- [ ] G-LICENSE native-plugin distribution

## Blockers / questions for Johannes
- none

## Log (newest first; one line per task)
- YYYY-MM-DD M0.1 <sha> — checks: ...; notes
```

## 5. Target architecture (M1.2 expands this into `docs/ARCHITECTURE.md`)

**Data flow.**
```
ado parse → wire request → plugin: load columns via SPI (main thread only)
  → engine::spec::ModelSpec + engine::data::Loaded
  → data::prepare (candidate rows, missing values, weights)
  → topology::build (factorize, interactions, singletons to a fixed point, components)
  → ols::fit | ppml::fit
      fe::Engine on all RHS columns → wls::solve → vce::compute → outputs
  → plugin validates the complete result → writes shadows → ado posts e() atomically
```

**Crate `fereg-engine` modules** (budget ≤ 25K lines):
- `error` — error codes and Stata return codes.
- `threads` — team plus fixed-partition reduction helpers.
- `spec` — model wire parsing (harvested).
- `data` — sample construction and weights (harvested).
- `factor` — factorize and interactions (harvested).
- `topology` — singletons, components, one FE representation: codes + CSR membership + slope channels.
- `dof`.
- `fe` — `Design`, `Plan`, `direct`, `schur_pcg`, `lsmr`, `cmg`, `certificate`; all backends return the same coefficient/residual and diagnostic interface.
- `wls` — moments, guarded pivoted Cholesky, QR fallback, statuses.
- `vce` — iid, hc1, multiway cluster, PSD repair.
- `ols`.
- `ppml` — family, IRLS, separation, bias.
- `outputs`, `result`.

**FE engine.** For each right-hand-side column v, minimize Σ w_i (v_i − Σ_q x_qi′ α_{q,g_q(i)})². Effect q has L_q levels and m_q channels (intercept and/or slope variables).
- **Initial auto candidate, Q = 1:** a per-level m×m weighted solve. Rank-deficient blocks are handled per SPEC; the result receives the shared certificate.
- **Initial auto candidate, Q ≥ 2:** eliminate exactly the effect with the most parameters (per-level solves). Run PCG on the reduced (Schur complement) system over the remaining effects' parameters. This default is subject to G-M3 and estimator gates.
  - The operator is applied with observation passes over all active RHS columns at once.
  - Block-Jacobi preconditioner using per-level blocks.
  - Independent recurrences per column; warm starts supported (PPML).
- **Explicit LSMR.** `solver(lsmr)` selects the pinned `within` modified-LSMR backend directly, with Schwarz preconditioning on supported designs. Use the same adapter for automatic fallback; do not create a separate fallback implementation.
- **Explicit CMG.** `solver(cmg)` selects the pinned graph-Laplacian/SDDM CMG backend through a narrow adapter. Reuse the shared topology and handle the supported categorical core/nuisance structure without duplicating the estimator. Document OLS and PPML support, including any limits; unsupported slope or multiway formulations must fail clearly.
- **Convergence monitor and fallback.** For `auto`, stagnation or a projected over-budget Schur-PCG solve may transfer remaining columns to LSMR. Specify a finite total attempt/iteration budget in SPEC. Explicit requests start with their requested backend; any permitted numerical fallback must be documented, visible in diagnostics and independently certified. Unsupported mathematical structure is an error, not a fallback trigger. CMG inclusion in automatic selection requires separate comparative evidence.
- **Certificate.** Every backend computes r = v − Dα explicitly and checks D'Wr relative to the same SPEC scales. Saved FEs are α, normalized per SPEC within each connected component. Keep finite-regression/inference checks and independent high-precision weak-graph tests: reconstruction plus a small normal-equation residual is not a universal forward-error bound.
- **Plan choice** honors the public solver request and backend support. Automatic decisions depend on FE structure and observed convergence, not size/weight-kind/thread/memory gates. A test-only `PlanOverride` can force each supported backend at any size, including forced fallback failures.
- **M8 optimization (same math):** an edge-compressed pair operator using weighted cross-tabulations.

**PPML.** IRLS with μ-weights. Each iteration refreshes the weights, warm-starts α and applies step-halving on the deviance. `solver(auto|lsmr|cmg)` uses the same FE interface, WLS, VCE and certificate modules. Refresh or invalidate backend numerical state when weights/sample change; no stale factor may certify a new frame. Convergence, separation and final-Hessian acceptance follow SPEC.

**Plugin.** One entry point; an engine switch exists until M7. Errors map to Stata return codes; panics are caught; SPI is used only on the main thread.

**Threads.** One Rayon pool per call, sized by `threads()`. Reduction chunk sizes are compile-time constants, so results do not depend on the thread count.

## 6. Milestones and tasks

Every task: do → checks → tick `MILESTONES.md` → commit → push. "Rust checks" means fmt, clippy with `-D warnings`, and `cargo test --workspace`.

### M0 — Infrastructure reset (documentation, process, legacy safety; no new engine code)

- **M0.1 Tag, plan, milestones.**
  - Tag current HEAD `legacy-v0.5.0-beta` (annotated) and push the tag.
  - Copy §0–§8 of issue #28 verbatim into `docs/cleanup/PLAN.md`.
  - Create `MILESTONES.md` from §4.3 with every task of §6 and the gates of §7.
  - Checks: the files exist; the tag is on origin.
- **M0.2 New `AGENTS.md`.** Replace it with §4.1 verbatim. Checks: none beyond Rust checks.
- **M0.3 Stage rebuild ADRs.** Carry out §4.2 in `docs/cleanup/rebuild-adr/`; retain current `docs/adr/`. Record the GPLv3 direction and the unresolved Stata/SPI distribution questions in ADR 0008. Checks: the staged set contains README, TEMPLATE and 0001–0008; existing ADRs are unchanged.
- **M0.4 Transfer the resume record.**
  - Carry current unresolved scientific findings and relevant evidence links from `STATUS.md` into `MILESTONES.md` or the requirements mapping as appropriate. Freeze `STATUS.md` as historical input; only `MILESTONES.md` receives new progress updates.
  - Update `README.md` to point to `MILESTONES.md` and issue #28; adjust active status references in user docs and packaging.
  - Preserve `STATUS.md` and the M1.1 SPEC sources until M7 cleanup. Checks: the resume point and unresolved findings are accounted for; no source contracts or evidence were deleted.
- **M0.5 Inventory and prepare the lean harness; preserve scientific coverage.**
  1. Inventory kept tests, runtime fixtures, scripts and evidence in `docs/cleanup/coverage.md`; map each scientific requirement to its SPEC destination and replacement test. A pending replacement is not authorization to delete its source.
  2. Copy the 100-digit `weak_graph_decimal_oracle_v1.{csv,json}` into `tests/reference/` and verify identity; keep original receipts immutable. Copy the generators under `benchmarks/systematic/` and `benchmarks/two_fe_dense/` to `bench/legacy_generators/`, and `Veneto_Benchmark/scripts` to `bench/real/veneto/`. Copy the explicit-dummy Poisson, multiway-cluster and recession-cone Python oracles to `tests/oracles/`, and the manual AKM/PPML examples to `examples/`.
  3. Preserve the prior test entry points under explicit legacy names, then introduce the lean `scripts/run_tests.sh` and mirrored PowerShell entry point: fmt, clippy with the current feature flags, workspace tests, plugin FFI tests, mock-SPI/Linux mock-plugin audits, and size checks once available at M2.2. Keep the existing scientific fixtures runnable during migration; record their commands in the coverage mapping. Infrastructure checks tied to the old layout may be adapted, but their scientific assertions must survive.
  4. Keep existing evidence, development fixtures, Python tests, reference pins, capture manifests and their readers until replacement coverage passes. Defer deletion of obsolete scripts, profiles and config entries to M7.5. Packaging must continue to resolve all its inputs.
  - Checks: copied oracle identities match; Rust checks and the lean entry point run; existing scientific checks remain callable; plugin builds and local Mac `quick` remain runnable. Preserve and adjudicate any existing failures in M0.7–M0.8; do not claim a green baseline before those pass.
- **M0.6 CI.**
  - Add a `[profile.ci]` to `Cargo.toml` (inherits release, `lto = false`, `codegen-units = 16`, `debug-assertions = true`) for fast optimized test builds.
  - Add `.github/workflows/ci.yml` on push and pull_request, runner `ubuntu-latest`, pinned toolchain via `rust-toolchain.toml`, cargo caching. It runs `./scripts/run_tests.sh`.
  - Retain the old manual workflows until the M7.5 coverage cleanup; creating the new CI workflow does not dispatch them.
  - Keep `stata-ci.yml` manual on the licensed Mac; defer obsolete receipt-job removal to M7.5.
  - Move `STATA_CI_RUNNER.md` to `ci/README.md`, keeping runner setup and usage and dropping receipt and qualification history.
  - Checks: CI passes on the push, after M0.7 if needed.
- **M0.7 Make the legacy suite pass.**
  - Nine plugin allocation-audit tests fail only under the parallel runner; they all pass with `--test-threads=1`. Mark them `#[ignore = "process-global allocator audit; run with --test-threads=1"]`. Observed:
    - `estimate::tests::{action_metadata_matches_independent_deallocation_and_counts_spare_capacity, inference_residual_overlap_stays_within_existing_frame, numeric_dictionary_growth_stays_inside_construction_envelope, saved_slope_recovery_workspace_bounds_observed_allocator_peak}`
    - `ffi::tests::whole_action_stitches_prefix_and_legacy_reset_without_losing_early_peak`
    - `input_memory::tests::simultaneous_dictionaries_bound_independent_allocator_peak`
    - `ppml_estimate::tests::{core_publication_observation_has_no_obsolete_fit_or_generation_owner, publication_owners_match_independent_deallocation_and_exact_admission}`
    - `ppml_execution::tests::repeated_scoped_family_work_and_callbacks_allocate_nothing_after_warmup`
  - Fix the three plugin dead-code warnings. Some may be platform-specific: gate them with `cfg` rather than deleting code that macOS uses.
  - Refresh the two stale byte counts in `crates/fereg-core/tests/feppml_constrained_memory_contract.rs` only after confirming the +320,000,000-byte change is intended (`git log -S`).
  - Make `initial_full_width_lsmr_preserves_canonical_multiway_rank_policy` accept the checkpoint-Schur path (update the test, not the solver).
  - Run the named CMG low-memory oracle explicitly and preserve its failure details for M0.8. Keep it active; do not ignore it to claim a green scientific baseline.
  - Checks: `cargo test --workspace` and CI pass after M0.8 if the scientific failure remains. In that case continue directly to M0.8 with M0.7 pending; neither task is complete until the failure is adjudicated and the suite passes.
- **M0.8 Minimal legacy safety fixes** (the only legacy behavior changes before M7):
  - (a) Reject explicit `solver(cg)` with a clear error in `_fereg_estimate_public.ado` and `_fereg_estimate_ols.ado`.
  - (b) Map `CmgAutoBuildOutcome::DirectCgPreferred` to direct LSMR instead of CG (`prepared_fit.rs:1867`, `auto_direct_solver`).
  - (c) Reproduce and localize the reported CMG low-memory discrepancy on its source/design, then on the current baseline. Distinguish the CMG kernel, adapter, recovery and certificate. Preserve the independent oracle and a regression that directly exercises explicit CMG. If the implicated automatic route remains inaccurate, route that bounded legacy case to certified LSMR as a temporary safety measure; this does not count as fixing explicit CMG. Repair the implicated explicit path or fail it clearly until repaired, retaining the public option and unaffected supported cases. The new CMG backend must pass the regression before cutover.
  - (d) Add a legacy regression test: the 800-row design from §1, where explicit CG must now be rejected and auto must match the dense oracle.
  - Checks: Rust checks and the active CMG regression; explicit LSMR/CMG supported-case oracle checks and affected-path error/rollback checks; `./ci/run_stata_ci.sh quick` on the Mac. Record the diagnosis and any temporary unsupported case; do not weaken tolerances.

### M1 — Specification, architecture, oracles, benchmark suite

- **M1.1 `docs/SPEC.md`.** Consolidate the behavioral contract.
  - **Sources** (current tree or tag `legacy-v0.5.0-beta`): `docs/scientific_contracts.md`, `weight_semantics.md`, `ols_core.md`, `data_plane.md`, `factor_protocol.md`, `plugin_protocol.md`, `compatibility_matrix.md`, `user_guide.md`, `ppml_bias_correction.md`; old ADRs 0003–0006, 0016, 0017, 0019, 0026, 0055, 0182, 0201, 0227, 0246; `stata/*.sthlp`.
  - **Required sections:**
    1. public syntax and options (new surface, §3);
    2. sample construction, missing values, `if`/`in`;
    3. weights (aw/fw/pw; iw rejected);
    4. singletons;
    5. absorbed terms (interactions, slopes, saved-FE normalization);
    6. regressors (factor variables, TS operators, base/omitted statuses);
    7. OLS estimation;
    8. rank and collinearity rules;
    9. DoF policies and cluster nesting;
    10. VCE (IID, HC1, multiway cluster, PSD repair);
    11. fit statistics and `e()` contents;
    12. outputs and `predict`;
    13. PPML IRLS, convergence, start values;
    14. PPML separation methods and ledger order;
    15. PPML bias correction;
    16. certificates and error codes;
    17. transaction, atomicity and Break;
    18. threads and determinism;
    19. memory behavior;
    20. test tolerances.
  - Each section states the rules and names the tests that enforce them. Include the OLS/PPML support matrix for `solver(auto|lsmr|cmg)`, requested/used solver diagnostics, permitted fallback and iteration-budget semantics, plus independent weak-graph accuracy requirements. Reconcile the old contracts and ADRs through `docs/cleanup/coverage.md`; preserve current supported capabilities unless this plan explicitly changes them.
  - Fold any license or provenance facts from `docs/upstream_map.md` into `THIRD_PARTY_NOTICES.md`.
  - Retain the source docs and ADRs after consolidation; M7.5 performs cleanup only after replacement checks pass. Identify superseded files in the coverage mapping without rewriting historical evidence.
  - Checks: every retained public option and documented behavior appears in SPEC; removed options are explicitly listed; the contract-to-test mapping has no unexplained omissions.
- **M1.2 `docs/ARCHITECTURE.md`.** Expand §5: module map with line budgets, data flow, FE engine math, plugin seam and engine switch, error model, threading. Maximum 400 lines.
- **M1.3 `crates/fereg-oracle`.**
  - Copy the dense explicit-dummy OLS oracle (`crates/fereg-testkit/src/explicit_ols.rs`, `explicit_dummy.rs`; the legacy testkit keeps its copy until M7). Add a dense Poisson IRLS oracle, porting `tests/oracles/ppml_explicit_dummy_oracle.py`.
  - Add `designs.rs`, a seeded random design generator covering: N; number of FEs; levels; graph type (dense panel, AKM strong/weak mobility, chain/barbell, disconnected components); slopes; weight kinds; missing values; singletons; collinear regressors; clusters; zero outcomes for PPML.
  - Add comparison helpers using SPEC tolerances and retain the 100-digit weak-graph oracle data in `tests/reference/`. Check fitted values, residuals, identified coefficients and estimator outputs independently of each backend's certificate. Include the reported CMG discrepancy and CG counterexample as regressions; only supported backends are forced, and unsupported structures receive explicit rejection tests.
  - Add a dev-dependency on the legacy core with a `legacy_fit()` helper for differential tests.
  - Checks: oracle self-tests; legacy passes the OLS property tests on its default routes, and the old CG defect is detected by a (legacy-only, `#[ignore]`) test.
- **M1.4 `bench/` suite.**
  - `bench/suite.toml` defines cells, tiers and comparators. `bench/generate.py` ports the generators and writes `.dta` files with fixed seeds.
  - Runners: `bench/stata_runner.do` (fereg with `engine(legacy|next)`, reghdfe, ppmlhdfe) and `bench/r_runner.R` (fixest at defaults and matched accuracy: OLS `fixef.tol=1e-10`; PPML `glm.tol=1e-10, fixef.tol=1e-11`).
  - Pin fixest 0.14.2 in `references.lock.toml` (source archive SHA-256 `17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`).
  - Protocol: a fresh process per command, one warm-up, five repetitions, median. Record fereg's input/native/total times from `e(fereg_runtime)`. Check coefficients against a reference fit (fereg at tolerance 1e-12) and mark disagreements.
  - Per-command timeout: 30 minutes in `standard`, 2 hours in `full`. A timeout is recorded as such and never retried automatically.
  - `bench/compare.py` writes `bench/history/<date>-<sha>-<platform>-<tier>.csv` (≤ 100 KB) plus a printed table: ratios, geometric means, worst cell, timeouts.
  - **Cells** (default N = 1e6, K = 10, IID, unit weights):
    - OLS: `ols-1fe`, `ols-2fe-dense`, `ols-2fe-akm-strong`, `ols-2fe-akm-weak`, `ols-3fe` (AKM plus year), `ols-2fe-k2`, `ols-2fe-k50`, `ols-2fe-aw`, `ols-2fe-cluster`, `ols-2fe-outputs` (residuals plus saved FEs), `ols-1fe-slope`, `ols-2fe-slope`, `ols-2fe-n1e5`, `ols-2fe-n1e7` (SCC).
    - PPML: `ppml-1fe`, `ppml-2fe-gravity`, `ppml-3fe-gravity`, `ppml-3fe-cluster`, `ppml-2fe-zeros-separation`, `ppml-2fe-slope`, `ppml-3fe-n1e7` (SCC).
    - Real data: `veneto-akm` (SCC; data never in git).
  - **Tiers:**
    - `smoke` — four cells at N = 1e5, T = 4, under 5 minutes on the Mac.
    - `standard` — all non-SCC cells at T = 1 and T = 8 on the Mac, under 60 minutes.
    - `full` — all cells, plus a thread ladder 1/2/4/8/14/28 for `ols-2fe-akm-strong`, `ols-3fe` and `ppml-3fe-gravity`, on SCC as array jobs of at most 22 hours each.
  - Checks: `smoke` runs end to end on the Mac with the legacy engine.
- **M1.5 SCC tooling.**
  - `bench/scc/setup.sh`: stage a commit snapshot, build the Linux plugin, and install the pinned comparators into the run root (never into the repository): reghdfe, ftools and require from `ci/reference-bundles`; ppmlhdfe and fixest at their `references.lock.toml` pins.
  - `bench/scc/job.sh`: 28-core node, `h_rt` ≤ 22:00:00, `stata-mp/19` limited to 4 processors, native threads per cell.
  - `bench/scc/submit.sh` and `collect.sh`.
  - Checks: one `smoke` job on SCC finishes and its summary is collected.
- **M1.6 Legacy baseline.**
  - Run `standard` on the Mac and `full` on SCC with the legacy engine and all comparators.
  - Commit the summaries; record the job IDs and headline numbers in `MILESTONES.md`.
  - These are the numbers every later gate compares against. Add a small declared solver-comparison subset for explicit LSMR and CMG on their supported difficult graphs; keep its counts separate from default-policy aggregate scores.

### M2 — New engine skeleton and harvested foundations

- **M2.1 Move legacy.**
  - `git mv crates/fereg-core legacy/fereg-core` and `git mv crates/fereg-testkit legacy/fereg-testkit`.
  - Update the workspace members, the path dependencies in `crates/fereg-plugin`, `crates/fereg-oracle` and `fuzz/Cargo.toml`, and any script that names the old paths.
  - Checks: Rust checks; the plugin builds; behavior unchanged.
- **M2.2 Create `crates/fereg-engine`** with the crate-level `forbid(unsafe_code)` attribute and the modules `error`, `threads` (team plus fixed-partition reductions) and `spec` (the model-wire types and parser harvested from legacy `metadata.rs`, `protocol.rs` and the request structs).
  - Add `scripts/check_sizes.sh` (files ≤ 1,500 lines) plus `clippy.toml` (`too-many-lines-threshold = 150`), with `too_many_lines = "deny"` for `fereg-engine` and `fereg-oracle`. Call the size check from `run_tests.sh`.
  - Checks: harvested unit tests pass; wire round-trip tests pass.
- **M2.3 Harvest `data`** (sample, weights, model columns; from legacy `data/`) and **`factor`** (`factorize.rs`, `fe/interaction.rs`).
  - Strip memory-admission and instrumentation code; keep the unit tests.
  - Checks: the differential test against legacy gives exact equality of candidate rows, drop reasons and weights on random designs.
- **M2.4 Harvest `topology`**: one representation `FeTopology` (codes, CSR membership, slope channels, components), singletons to a fixed point, connected components, and `dof` (from `topology_dof.rs`, `dof.rs`).
  - Checks: the differential test against legacy gives exact equality of retained samples, singleton counts, components and DoF on random designs.

### M3 — FE engine (coefficient space)

- **M3.1 `fe::Design` and `fe::Plan`.** Build one design from `FeTopology` and weights. Honor `auto|lsmr|cmg` and documented backend support; automatic policy uses structure/convergence only. Define common coefficients, residuals, requested/used solver and fallback diagnostics. `PlanOverride` forces supported backends in tests.
- **M3.2 Direct projection for Q = 1** (intercept and/or slopes; rank-deficient level blocks per SPEC).
- **M3.3 Schur-PCG for Q ≥ 2** (§5): exact elimination of the largest effect, block-Jacobi PCG on the reduced system, multi-RHS passes, independent recurrences, warm start.
- **M3.4 Certificate and coefficients.** Compute residuals explicitly from α, check D'Wr, and normalize α per SPEC within each component.
- **M3.5 Retained backends and convergence monitor.** Implement narrow adapters for pinned `within` LSMR and CMG. LSMR is directly selectable and reused for automatic fallback; CMG is directly selectable for documented graph-compatible structures. Both use M3.4's reconstruction/certificate and shared preparation/output paths. Implement bounded, documented, visible fallback and unsupported-request errors. Retain the CMG regression; choosing CMG automatically requires comparative evidence.
- **M3.6 Property tests and micro-benchmarks.**
  - At least 2,000 random designs, forcing every supported backend via `PlanOverride`, against the dense oracle: weak/disconnected graphs, slopes, all weight kinds and collinear FEs. Retain the high-precision weak-graph fixtures. Verify explicit-request dispatch, unsupported errors, forced fallback, diagnostics, failure atomicity and same-machine determinism across thread counts, including both dependencies.
  - Micro-benchmark FE-only projection on `ols-2fe-akm-strong`, `ols-2fe-akm-weak`, `ols-3fe` and `ols-2fe-slope` (N = 1e6, K = 10, T = 1 and 8, Mac) against legacy's `auto`. Compare explicit LSMR and CMG on their supported cells and use the evidence to assess the proposed automatic policy.
- **G-M3 go/no-go:**
  - All supported backends pass property/high-precision tests, including the CMG regression; no backend is accepted solely on its internal certificate.
  - The proposed automatic policy's projection time is ≤ 1.1× legacy's on every micro-benchmark cell, or MILESTONES records a concrete fix within this architecture. Explicit LSMR and CMG remain supported even when another backend is faster; their results and costs are reported separately.
  - If this fails after one focused attempt, stop and ask Johannes.

### M4 — OLS on the new engine

- **M4.1 `wls`.** Blocked deterministic moments; guarded pivoted Cholesky with QR fallback; coefficient statuses per SPEC. One implementation, harvested from legacy `finite_wls_kernel.rs` and `rank.rs` and simplified.
- **M4.2 `vce` and fit statistics.** Scores, bread, IID, HC1, multiway cluster, DoF policies, PSD repair (harvested `sandwich.rs`, `sandwich/qualified_cluster.rs`, `ppml/psd.rs`); fit statistics per SPEC.
- **M4.3 Outputs.** Residuals, xb, xbd, d, and saved FEs from α.
- **M4.4 `ols::fit(&ModelSpec, &Loaded, &Options) -> Result<OlsFit>`.**
- **M4.5 Oracle and differential tests** across weights × VCE × slopes × outputs × interactions × factor/TS regressors.
  - Checks: exact equality with legacy for samples, statuses and DoF; SPEC tolerances for numbers.

### M5 — Plugin seam and OLS cutover

- **M5.1 Engine switch.**
  - Add a hidden ado option `engine(legacy|next)`, passed as request key `engine=`, dispatched in `crates/fereg-plugin`.
  - When the option is omitted, its value comes from the global macro `FEREG_ENGINE` if set (developer and test use only), otherwise from the current default engine.
  - `engine_next.rs` converts `OlsFit` into the existing result shadows; the Stata layer is otherwise unchanged.
  - If the ado validation rejects the new engine's solver or engine codes, extend it minimally; the wire is simplified in M7.2.
  - Under `engine(next)`, `memory()` is ignored with a note.
- **M5.2 Stata fixtures with `engine(next)`** on the Mac: parity_reghdfe, installation, parser, postestimation, documentation.
  - Add `FEREG_ENGINE=next` support to `ci/run_stata_ci.py`: when the environment variable is set, the runner defines the Stata global `FEREG_ENGINE` before each fixture.
  - Exercise public `solver(auto|lsmr|cmg)`, supported designs and outputs, invalid/unsupported requests, fallback diagnostics and rollback. Do not silently translate an explicit solver request to `auto`.
- **M5.3 Benchmarks.** `standard` on the Mac and `full` on SCC, comparing next, legacy, fixest and reghdfe. Commit the summaries.
- **M5.4 G-OLS, then flip the `fereg` default to `engine(next)`.** Update `MILESTONES.md` "Default engine".

### M6 — PPML on the new engine

- **M6.1 `ppml::family` and IRLS.**
  - Step control and convergence per SPEC.
  - Start values: global plus analytic one-FE group start only.
  - FE solves reuse `fe` for public `auto|lsmr|cmg`, with warm-started α and refreshed weights. Validate invalidation/rebuild of each backend's numerical state and unchanged strict/final-Hessian acceptance.
- **M6.2 Separation.** Harvest `ppml_separation` (FE-only zero groups, simplex, IR, and Mu if SPEC requires it), the singleton interplay and the ledger order.
- **M6.3 PPML inference and outputs.** Robust/cluster through `vce`; mu, eta, xb, d, saved FEs; ll, deviance, pseudo-R².
- **M6.4 Bias correction.** Harvest `ppml/bias.rs` and `bias_support.rs` onto the new linear algebra.
- **M6.5 Tests and seam.**
  - Poisson oracle and differential tests.
  - The plugin adapter for PPML, plus the hidden `engine(legacy|next)` option (with the same `FEREG_ENGINE` default) on the `feppml` ado path.
  - ppmlhdfe parity fixtures and the bias fixtures with `engine(next)`, covering all retained solver options on their supported structures, saved outputs, changing weights, visible fallback and unsupported-request rollback.
- **M6.6 Benchmarks and cutover.** Run the benchmarks, pass G-PPML, and flip the `feppml` default.

### M7 — Delete the legacy core

- **M7.1 Remove legacy after coverage acceptance.** Require G-OLS and G-PPML, passing replacement tests for the mapped scientific requirements, and preserved CMG/weak-graph regressions. Then delete `legacy/`, the engine switch, the `engine()` option and the oracle's legacy dev-dependency. Keep the independent oracles and their fixtures.
- **M7.2 Clean up the plugin.**
  - Remove the tracking global allocator, allocation audits, memory-report tables and legacy result rows.
  - Define result wire v2 with only the matrices the ado needs, and update the ado validation to match (a single path).
  - Regenerate or remove the compiled PPML Mata validator.
- **M7.3 Dependencies and licensing.** Retain pinned CMG and `within` (including the preconditioner dependency as needed). Remove the vendored `rayon-core`, `[patch.crates-io]` and genuinely unused crates after replacement checks. Update project-owned `LICENSE`, Cargo license metadata, `THIRD_PARTY_NOTICES.md`, source packaging and `references.lock.toml` for the GPLv3 direction, preserving third-party terms. Resolve and document Stata/SPI permissions in ADR 0008; do not distribute native plugins or mark G-LICENSE passed while unresolved.
- **M7.4 Ado cleanup.**
  - Remove only the dropped options (§3.3); retain and document `solver(auto|lsmr|cmg)` on both commands.
  - Merge the duplicated parsing across `_fereg_estimate_public`, `_fereg_ols` and `_fereg_estimate_ols`.
  - Delete the hidden adapters (`_fereg_ols`, `_fereg_prepare_ols_test`, `_fereg_estimate_ols_test`, `_fereg_ols_hidden_numeric_v1`).
  - Collapse the PPML wrappers.
- **M7.5 Final sweep after §0.9.** Verify the requirements-to-tests mapping, passing replacement checks and reachability of the original evidence at the legacy tag. Then remove superseded working-tree scientific docs/ADRs, frozen `STATUS.md`, bulk evidence and obsolete harnesses listed in §8; retain all fixtures and scripts still used by the new checks. Activate `docs/cleanup/rebuild-adr/` as `docs/adr/` and update links. Remove obsolete CI/receipt jobs, capture-reader config, script references and ignore/attribute entries together; keep every reference/dependency pin still in use. Retarget `fuzz/` to the new spec parser and factorization, and enforce size limits for every crate.
  - Checks: coverage has no unmapped requirement or pending replacement check; historical evidence is reachable; searches find no active use of deleted paths; CI and all Mac Stata fixtures pass; SCC smoke passes; `MILESTONES.md` records the line-count report (≤ 35K production lines).

### M8 — Performance pass on the clean core

- **M8.1 Input path.**
  - Give Linux and Windows the same direct-accessor plus pipelined reader as macOS, after verifying SPI safety.
  - Read each Stata variable once.
  - Measure with the `input` time.
- **M8.2 Host threads.** Restrict Stata's own processors during `fereg` and `feppml`, restored on every exit path including Break; measure.
- **M8.3 Faster FE operator (same math).** Edge-compressed pair operator, blocked multi-RHS kernels, allocation-free iterations; property tests unchanged.
- **M8.4 Thread scaling** 1–28 on SCC; fix phases that do not scale.
- **M8.5 Targets.** `full` suite against the §7 targets. Record results; open follow-up issues for remaining gaps. Never add special routes.

### M9 — Documentation and release readiness

- **M9.1 User docs.** Update `README.md`, `docs/user_guide.md`, `stata/*.sthlp` and `docs/compatibility.md` (replacing the old compatibility matrix). Document retained `solver(auto|lsmr|cmg)`, backend support, requested/used diagnostics and fallback behavior. Add a `CHANGELOG.md` entry covering removed options, GPLv3 licensing and behavior changes.
- **M9.2 Final sync.** Final SPEC/ARCHITECTURE/ADR sync. `MILESTONES.md` gets a closing summary with the final numbers; file follow-up issues (§3.7 features, M8 gaps).
- **M9.3 Release.** Require G-LICENSE and the preceding release checks. Update packaging for GPLv3 source/notices obligations and the documented Stata/SPI disposition; build the Mac, Linux and Windows plugins; run a clean-install test on the Mac and on SCC; tag `v0.6.0-beta`; close issue #28.

## 7. Gates and targets

- **G-M3** (go/no-go): see M3.6.
- **G-OLS:**
  1. The OLS oracle property tests pass (≥ 2,000 designs).
  2. parity_reghdfe, installation, parser, postestimation and documentation fixtures pass with `engine(next)` on the Mac, including explicit LSMR/CMG, supported/unsupported structures, outputs, fallback diagnostics and rollback.
  3. Differential results against legacy are exact for samples, statuses and DoF, and within SPEC tolerances for numbers.
  4. On the OLS benchmark cells using the default automatic policy, next/legacy geometric mean ≤ 1.0 and no cell above 1.25×. Explicit-backend comparisons are reported separately and do not remove a retained solver merely because it is slower on a cell.
  5. No `engine(next)` timeouts on SCC `full`.
- **G-PPML:** the same checks with the Poisson oracle, ppmlhdfe parity and the PPML cells, including each retained solver's changing-weight state, strict checkpoints and final-Hessian acceptance.
- **G-LICENSE:** the retained GPL-3.0-only CMG and MIT `within` dependencies are reflected in consistent project metadata/notices and corresponding-source packaging; the native Stata/SPI combination and any required rights-holder permissions are resolved and documented in ADR 0008. This gate precedes native-plugin distribution/release; GPL relabeling alone does not pass it.
- **G-FINAL** (end of M8):
  - **Hard requirements:** CI passes; no timeouts; no certified-but-wrong results in property tests; results identical across thread counts; size limits met.
  - **Performance targets against fixest** (report if missed and file follow-ups rather than blocking M9):
    - matched accuracy: geometric mean ≤ 0.8×, no cell above 1.2×;
    - defaults, end to end including data transfer: geometric mean ≤ 1.0×.

## 8. Inventory: keep, harvest, delete

**Keep in place:** `stata/` (edited in M7.4 and M9.1), `plugin/` C bridge, `tests/stata/{parity_reghdfe,parity_ppmlhdfe,installation,parser,postestimation,documentation,bias}`, `tests/licensed_stata/` (Break test used by the CI runner), `tests/mock_spi/`, `tests/fixtures/`, `fuzz/`, the `ci/` Stata runner and `ci/reference-bundles/`, `references.lock.toml`, build scripts, `LICENSE`, `CHANGELOG.md`.

**Harvest by copying into `fereg-engine`** (about 22K legacy lines, simplified on the way):

| Target module | Legacy source |
|---|---|
| `data` | `data/` |
| `factor` | `factorize.rs`, `fe/interaction.rs` |
| `topology` | `fe/singleton.rs`, `fe/components.rs`, `fe/partition*.rs`, `fe/topology_{finalize,registry,source,rhs}.rs` |
| `dof` | `ols/topology_dof.rs`, `ols/dof.rs` |
| `spec` | `metadata.rs`, `protocol.rs` |
| `vce` | `sandwich.rs`, `sandwich/qualified_cluster.rs`, `ppml/psd.rs` |
| `wls` | `ols/finite_wls_kernel.rs`, `ols/rank.rs` (numerics only) |
| `ppml` | `ppml/family.rs`, `ppml/certificate.rs`, `ppml/residuals.rs`, `ppml_separation/`, `ppml/bias*.rs` |

Plugin FFI, SPI input and result code stays in `crates/fereg-plugin` and is cleaned in M7.2. The pinned CMG and `within` dependencies remain. Their legacy adapters may supply reviewed mathematical mappings and tests for the new narrow adapters; do not copy the old routing/pipeline duplication. Explicit LSMR and CMG capabilities and regression fixtures must survive.

**Legacy implementations rewritten or dropped after their required behavior is carried forward:**
- `fe/borrowed_map*`, `fe/map.rs`, `fe/auto.rs`, `fe/accelerated_map.rs`, `fe/cmg*.rs`, `fe/lsmr*.rs`, `fe/single_slope.rs`, `fe/slope_coefficients*`, `fe/topology_map.rs`, `fe/topology_slope_map.rs`, `fe/projection.rs`;
- `ols/prepared*.rs`, `ols/linear_session.rs`, `ols/canonical_products*`, `ols/implicit_centered*`, `ols/slope_absorber.rs`, `ols/bias_*.rs`;
- `ppml/prepared*.rs`, `ppml/categorical_*`, `ppml/one_fe_profile.rs`, `ppml/profiled_intercept_curve.rs`, `ppml/memory.rs`, `ppml/metadata_memory.rs`, `ppml/irls.rs`, `ppml/solver.rs`, `ppml/result.rs`;
- `memory*`;
- the plugin's `*_memory.rs`, `instrumentation.rs` and `memory_report.rs`.

**Delete only at M7 after §0.9 and the coverage checks:** frozen `STATUS.md`, superseded legacy ADRs/docs, `benchmarks/`, `build_evidence/`, `output/`, `Veneto_Benchmark/` (needed scripts copied), obsolete development/Python/manual tests and harness scripts, `vendor/`, and `legacy/`. Preserve every still-used scientific fixture, independent oracle and CMG/weak-graph regression; activate the staged rebuild ADRs. Keep original evidence reachable at the verified legacy tag without rewriting it. The dependency cleanup retains CMG and `within`.
