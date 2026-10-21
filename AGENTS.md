# AGENTS.md — fereg

fereg is a Stata OLS/PPML package with one Rust computational runtime.
Stata owns syntax, metadata and transactions; Rust owns loading, fixed-effect
operations, estimation and inference.

## Authority and implementation

Follow Johannes's direct instructions, then `docs/SPEC.md`, then the current
architecture and development documentation, then tests and code. Resolve real
scientific ambiguities with Johannes. Historical plans and Git revisions are
reference material, not active tasks or authority.

- `crates/fereg-core`: estimation, FE operations and inference.
- `crates/fereg-plugin`: host, FFI, loading and publication.
- `crates/fereg-testkit` and `crates/fereg-oracle`: scientific test support.
- `stata/` and `plugin/`: frontend and C bridge.

The former legacy runtime and engine selector are retired. Maintain one runtime;
compare changes with independent mathematics and, when useful, the prior revision.

## Scientific safeguards

Preserve supported OLS/PPML features, explicit LSMR/CMG, both mean-bias modes,
public memory controls, original factor slots/base flags and unsafe-CG rejection.
Never weaken scientific tolerances, silently change models or copy a known bug
for parity. Keep pinned references and independent numerical witnesses.

Preserve deterministic repeatability at fixed worker count and seed; require
scientific agreement and stable discrete decisions across worker counts.
Bitwise identity across platforms or processor features is not required.
Processor-dependent fast paths may use the fastest supported arithmetic when
independent certificates and scientific witnesses pass and discrete decisions
stay stable. Keep the portable path supported and tested; same-machine
repeatability at fixed worker count and seed remains required.
Arithmetic-preserving changes retain scientific bits and solver signatures.
Declare affected scope and independent witnesses before algorithm/arithmetic/
storage changes; unaffected scientific results remain exact. Timing, build
identity and observed memory are not scientific bit targets.

Keep one substantive native call per estimation, validation before publication,
atomic failure/rollback and Break checks. SPI stays on Stata's main thread;
workers access Rust-owned memory. Catch panics at FFI and keep safe crates unsafe-free.
Unsupported explicit requests fail clearly. Use general resource/worker policies
and mathematical solver families; never add benchmark-specific routing fences.

## Development and validation

The incremental Auto-routing workstream authorized on 2026-10-17 develops in
this checkout on `codex/auto-routing`, starting at `f35f3a4d`; its successor
CMG consolidation workstream (2026-10-19) continues on the same branch and on
CMG `codex/host-execution-foundation`. Commit and push those branches; do not
reset main, merge to main, or update release packages as part of this workstream. Other work retains the normal main-branch convention.
Use coherent, reviewable commits. Migrate consumers and remove
superseded implementations together. Push completed authorized work after checks;
never force-push or rewrite history. Report failed or missing checks honestly.
Use bounded Astra assistance for difficult problems and an independent high-effort
review of architectural changes. The lead coordinates implementation and tests;
controlled timings exclude competing builds, tests, estimation jobs and agent
tool workloads on the measured host. Ordinary OS/desktop activity is recorded
measurement noise, not an automatic reason to abort a panel. Independent
analysis on separate infrastructure may continue
without accessing or changing the measured artifacts or consuming host resources.

Johannes's 2026-10-18 validation decision replaces the mandatory A/B/C routing
campaign and recursive historical proof checks with one fixed-candidate
qualification round through `bench/qualify.py`. Keep the suite compact: one
manifest, parallel SCC shards, a fixed budget and one report. Record revisions,
build settings and hashes of the actual binaries, harness, inputs and references;
verify them once per job and use lightweight drift checks during execution.
Do not add receipt chains, repeated history walks or automatic sampling ladders.

Johannes's later 2026-10-18 direction makes Linux the primary performance test
and requires practical Mac qualification on an active desktop. Recover complete
Mac cells with valid artifact, scientific and observation evidence; rerun only
tests affected by externally observed interference, within fixed per-cell and
total retry caps. Ordinary desktop noise stays in the measurements. Do not
restart the whole battery, retry a scientific/native failure, or use a slow or
inconclusive result to trigger more sampling. Retain failed attempts and all
their observations; historical run failures remain failures. Freeze the recovery
schedule, observer policy and budgets before execution, then assess it once.

Protect the original uninjected Auto at `f35f3a4d` and the last accepted branch
Auto on every workload with max(5%, 1 ms). Retain the 42-cell routing coverage,
independent numerical witnesses, exact/scoped science, fixed-worker repeatability,
cross-worker agreement and scientific/resource/transaction gates. Explicit solver
timings are optional diagnostics; supported explicit solvers and PPML remain
covered by the feature tests. Public activation requires exact scientific and
solver-signature agreement with the selected hidden invocation. Keep the public
default disabled until applicable qualification passes; activation is confined
to this branch and OLS `solver(auto)` with `preconditioner(auto)` on categorical
intercept designs.

Finish the existing many-FE finalist before advancing two-FE. Historical failed
attempts stay failed; retain valid completed science and unchanged current inputs
when repairing timing setup. A harness failure requires diagnosis and a bounded
repair, not abandonment of the candidate. If repairs do not resolve it, or science
fails or timing is inconclusive, stop dependent work and discuss the evidence
with Johannes before abandoning the candidate, changing the protocol or advancing
to the next step. Never silently select a replacement or sample until success.
The incremental Auto increment closed with the M2 report. By Johannes's
2026-10-19 decision the optional CMG construction-work cap and its accounting
are removed; no work cap, dimension-only work bound, time cap or predictive
payback policy replaces them. The successor workstream consolidates the CMG API
and fereg's CMG integration and parallelizes construction on the host team,
keeping upgrade iteration 16, cutoff 5, one sampled tree, seed 0 and the
existing eligibility, small-direct criterion, restart and fallback semantics
fixed. Two-FE public activation remains unauthorized until a separate
fixed-candidate qualification.

Johannes's 2026-10-21 successor decision authorizes measured CMG setup
optimization in fereg/CMG and replaces the fixed iteration-16 requirement and
predictive-payback prohibition for two-FE OLS Auto only. Develop a deterministic
upgrade decision using active and genuinely reusable RHS work, admitted workers,
graph structure and solver progress; do not use elapsed-time routing or restore
the removed construction-work cap. Preserve public memory controls, cutoff 5,
one sampled tree, seed 0, eligibility, small-direct selection, certification,
restart and fallback semantics unless a further scientific decision is discussed.
Use Claude Opus 5.5 for setup review and GPT Pro plus Claude Fable for independent
upgrade-policy proposals. Keep setup and policy comparisons separate. Diagnosed
repairs are authorized without a fixed repair-count cap; failed evidence remains
failed and unchanged artifacts are never resampled to obtain a pass. Enable the
qualified two-FE default on the development branch only after the fixed-candidate
42-cell Mac/Linux round and exact public-versus-hidden equivalence pass. Main,
release packages and GitHub testing remain unchanged. Discuss scientific
questions, substantive rule conflicts and inconclusive qualification with Johannes.

Runtime changes require `./scripts/run_tests.sh` and mapped scientific/transaction
checks. Build/package changes require focused tooling and native build/ABI checks.
Prose-only documentation/instruction changes use link/reference checks and
`git diff --check`; when executable code and artifact inputs are unchanged,
reuse valid runtime evidence without rerunning builds or the full test suite.
Test/tooling changes still need their applicable checks. Reuse valid evidence;
run broad suites at integration boundaries or when concrete risks require them.
After ten substantive unsuccessful repairs or one working day on one blocker,
record the reproducer and findings and ask Johannes for direction.

GitHub testing remains paused by Johannes's 2026-10-05 instruction. Mac remains
the qualification host for Mac behavior and performance. His 2026-10-14 decision
authorizes isolated SCC benchmarking as primary evidence for algorithm discovery,
scaling and Linux performance, followed by focused Mac timing and integration.
Verify node isolation and compare artifacts within the same allocation; slots
alone do not establish exclusivity. Bounded Linux and Windows build/load checks
remain authorized. No platform claim follows from another host's checks. See
`docs/development_workflow.md` for entrypoints.

If automated SCC SSH fails authentication, first run plain `ssh scc` in an
interactive terminal/PTY without `BatchMode=yes`. Complete any login prompts;
Johannes can approve two-factor authentication from his phone if requested.
Keep the interactive connection open while retrying automated access. Verify
with `ssh -o BatchMode=yes scc hostname` (expect the SCC login host, for example
`scc1`), then retry the original SCC command. A failed batch-mode login alone
does not establish that SCC access is unavailable. If interactive login also
fails, report the actual prompt or error and request the needed user action.

Arithmetic-preserving changes with zero exact replay differences and unchanged
solver signatures may use instruction counts or function-level CPU samples on
predeclared workloads, plus no wall-time regression beyond max(5%, 1 ms) on the
standard workloads. Report sampling uncertainty; gains below the allowance are
observations, not failures. Result-changing work requires declared scope,
independent witnesses and controlled qualification-host timing. Mac release
performance claims require controlled paired Mac measurements, not a completely
idle desktop. Identify artifacts and preserve observations;
inconclusive evidence never passes a claim it cannot support. Experiments need
bounded stopping rules.
Johannes's 2026-10-14 timing decision replaces the strict per-process CPU
admission/whole-panel abort rule prospectively. Use randomized balanced blocks
of complete solves, fixed warmups and budgets, robust paired summaries and
uncertainty at the actual independent measurement unit. Record ordinary OS
bursts; verified competing compute or host faults may invalidate whole affected
blocks only under predeclared external-evidence rules and bounded attempts.
Never remove observations because of unfavorable timings or use minima to pass
qualification. Scientific failures and solver timeouts retain their declared
failure semantics. Keep max(5%, 1 ms), exact/scoped science and prior valid
evidence; old excluded attempts remain excluded. Freeze one candidate and its
qualification budget before collecting results; an inconclusive candidate does
not replace the public incumbent. See `docs/development_workflow.md` for the protocol.
Johannes's 2026-10-09 decision adds two tiers. A commit whose runtime production
tokens and artifact inputs equal its parent's
(`scripts/check_complexity.py --compare-production REV`) is an unchanged artifact
and needs `./scripts/run_tests.sh` only, except for the prose-only case above.
For arithmetic-preserving work, wall-time nonregression is measured once per
workstream boundary rather than per commit. A
declared ulp-scope change (affected fields named in the replay scope, all other
fields and discrete decisions exact, independent witnesses on the affected
fields, instruction counts within noise) reuses prior timing evidence.

Johannes's 2026-10-11 clarification: finish production edits, focused checks and
targeted profiling before starting the full timing panel on a stable candidate.
The current qualification round has one fixed measurement budget. The later
Mac recovery decision above additionally permits bounded retries of specifically
interfered tests while retaining valid completed evidence. It does not permit
sampling until a performance pass. A changed
artifact requires reassessing which evidence remains valid. See `docs/development_workflow.md` for the protocol.
Performance coverage targets material measured costs: negligible residual library
FMA calls outside hot loops are not by themselves a failure. Quantify residual
cost and preserve exactness/certification; this supersedes a literal zero-call
reading of the performance plan, without rewriting prior measurement outcomes.

The three-FE LSMR hot path lives in pinned `within`/`schwarz-precond` dependencies
from `johannes-schmieder/within`. Apply these scientific and evidence rules there;
integrate changes through a pinned revision bump.

## Repository hygiene

Keep source, compact fixtures, current documentation and maintained test/benchmark
tools. `target/` keeps only Cargo's build cache and the current replay baseline
(`target/replay-baseline/`); repository scripts may write their fixed default
outputs there and overwrite them each run. Put every other task output
(candidate plugins, comparison builds and extra Cargo target directories, worktrees,
generated data, logs, profiles) under one `target/tmp/<task>/` directory, never
one directory per run or tag, and delete it before reporting the task done,
after its hashes and summaries are in commits or committed documents. Committed
documents never cite ignored paths as retained evidence.
`scripts/clean_target.sh --apply` removes everything else; run it at the start
of a session when no other task is using `target/`, and whenever `target/`
exceeds 15 GB. Johannes authorized deleting completed captures and caches;
historical committed source/documents remain in Git, ignored raw data does not.
During an unfinished workstream retain only the current inputs, artifacts and
applicable evidence needed to resume; delete superseded harnesses, duplicate
build trees, archives and completed captures after recording compact summaries.

Retain the runtime complexity ratchet at `tests/reference/complexity.json`;
explain scoped increases and lower it for reductions. Do not minify or move code
into exclusions. Code reachable only from its own tests is deleted, not gated;
independent reference implementations belong in fereg-oracle or fereg-testkit.
Developer injections (`inject=`, `break_marker_token=`, `developer_injection.rs`)
are the one sanctioned test surface in shipped builds; other fault plans use
`fault_plan!` so non-test builds compile them out.
No arbitrary coverage percentage is a gate.
Do not introduce planning/status ledgers or receipt-folder proliferation.
Never commit credentials, licenses, personal paths, raw logs, datasets or new
files above 1 MB (existing reference bundles and the native plugins required by
Johannes's 2026-10-14 main-branch GitHub installation request excepted). Paper
work belongs in fereg-paper. Native distribution still requires the licensing decision described
in `docs/licensing.md`; development is not publication authorization.

Changes to sample, rank, omission or separation semantics require Johannes's
decision before implementation. A failed or unverified prerequisite blocks
dependent work; independent work may continue.
