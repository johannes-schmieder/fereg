# AGENTS.md — fereg

fereg is a Stata package (OLS and PPML) with a Rust computational core.
Stata owns syntax, metadata and the transaction; Rust owns data loading,
fixed-effect absorption, estimation and inference.

## Authority and scope

1. Direct instructions from Johannes.
2. `docs/SPEC.md`: scientific results and failure behavior.
3. Active decisions listed in `docs/adr/README.md` only.
4. `docs/ARCHITECTURE.md`: current module map and measured hotspots.
5. `docs/cleanup/PLAN.md`: approved M0–M7 tasks; `MILESTONES.md`: status.
6. Tests and code.

Historical ADRs are non-binding. Their requirements bind only where retained
in SPEC or an active decision. The previous clean-core rebuild is abandoned.
Do not resume its tasks, automatic Schur policy, callback APIs or M7 retirement.
Resolve a real scientific ambiguity with Johannes; record it in MILESTONES.

## Implementation boundary

- `crates/fereg-core`: working core moved from `legacy/fereg-core` in M0.3.
- `crates/fereg-plugin`: active next host, FFI, loading and publication.
- `crates/fereg-testkit`: active test support moved in M0.3.
- `crates/fereg-oracle`: independent dense oracles and generators; dev only.
- `legacy/`: baseline identity; frozen source is at `legacy-comparator-v1`.
- `stata/`, `plugin/`: shared frontend and native C bridge.

Copy the complete working implementation into next before simplifying it.
Preserve existing algorithms, dependencies, options and memory modes initially.
Next and legacy are separate native binaries; next is the M1 default for both
commands. Never fall back from next to legacy. Legacy is a useful comparator,
not an authority on correctness or a reason to retain unnecessary complexity.
The comparison is frozen plugin plus current ado; keep its native protocol
compatible. Implement scientific fixes in Rust where possible. Any necessary
next-specific frontend adjustment must prove legacy behavior unchanged.

## Scientific and architectural safeguards

- Never weaken scientific tolerances, change models silently, or reproduce a
  known bug for parity. Pin external references and use independent oracles.
  Record narrowly defined intentional differences and executable regressions.
- Preserve full supported OLS/PPML functionality, explicit LSMR/CMG and both
  mean bias-correction modes. Keep the existing unsafe-CG rejection and CMG fix.
- Bit identity across engines or different thread counts is not a scientific
  requirement. Preserve fixed-thread deterministic repeatability and fixed
  seeds; require scientific agreement and stable discrete decisions across
  thread counts. Keep the public deterministic option during M0–M1.
- The initial copy must be bit-identical on the replay corpus at one thread
  under controlled deterministic settings. A no-numerical-change commit must
  preserve scientific bits and available solver/route signatures versus its
  parent. Numerical fixes must leave unaffected cases/fields bit-identical.
  Timing, build identity and observed memory are not scientific bit targets.
- One substantive native call per estimation; validate before publication;
  failure leaves no partial results. Preserve existing rollback/Break checks.
- SPI runs only on Stata's main thread. Workers access Rust-owned memory.
  Catch panics at FFI. Safe crates forbid unsafe code; host unsafe stays in FFI.
- Unsupported explicit requests fail clearly. Do not hide estimator changes.
- Simplify one concrete source of duplication at a time and remove superseded
  implementations after qualification. Do not add N/width/thread/weight/memory
  route fences to protect individual benchmark cells. Kernel chunk sizes are
  distinct from algorithm-selection conditions.
- A shared representation, FE interface and memory mode are future scoped
  goals, not requirements to rewrite the working copy before it can run.

## Gates, timeboxes and workflow

Work on main, one named task at a time, in small `M<n>.<k>: <summary>` commits.
Push completed tasks after their local checks pass. Never force-push or rewrite
history. A failed or unverified gate blocks all dependent tasks; independent
work may continue. Never mark missing evidence as a passing gate.
After three substantive unsuccessful repairs or one working day on the same
blocker (whichever comes first), record the reproducer, findings and attempts,
then ask Johannes for direction. Do not silently expand a milestone.

Before each commit run the task's required checks. Runtime changes require
`./scripts/run_tests.sh` plus their mapped scientific/transaction checks.
Documentation-only changes use governance/link checks; build-tool changes use
the relevant tooling and native build/ABI checks. CI runs the full entrypoint
on every push; no failed required check may be hidden by this local scoping.
The M0 replay matrix defines
M1 qualification; add tests only for mapped gaps or concrete regressions.
Reuse allocation, interruption and rollback fixtures. Do not build new polling
or evidence frameworks. Re-run broader checks only for a relevant change or
unresolved concern. Keep exact-copy gates separate from scientific acceptance.

## Complexity and evidence

M0.5 establishes a CI ratchet: production lines per runtime crate, functions
above 150 lines, files above 1,500 lines, and N/thread/weight/memory dispatch
conditions. Increases need a narrow checked-in reason and scope; decreases
lower the baseline. No silent baseline refresh, minification or moving code
into exclusions. New functions/files follow the limits; inherited violations
do not force unrelated rewrites. There is no aggregate production-line cap.
Coverage identifies exercised and unexercised paths; no arbitrary percentage
is a gate. Instrumented builds are not timing or frozen-comparator artifacts.

MILESTONES is the only mutable status ledger: fewer than 300 lines, no line
longer than 200 characters; each log entry is one line. Link to detailed
summaries in `bench/history/` or CI artifacts. No receipt-folder proliferation.
ADRs are at most one page and cover durable scientific/API/architecture,
dependency/licensing or evidence-policy decisions, not experiments or bug fixes.
Performance claims cite compact benchmark summaries and artifact identities.
Preserve independent scientific witnesses; abandoned implementation scaffolding
need not be ported. Do not remove a retained requirement without replacement
coverage. Historical source/evidence remains reachable through Git.

Never commit credentials, license files, personal local paths, raw logs,
datasets or new files above 1 MB (existing reference bundles excepted).
Comments explain invariants and reasons. Paper work belongs in `fereg-paper`.
The existing licensing decision remains in force; development does not imply
permission to publish native release assets.

## Platform checks

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
