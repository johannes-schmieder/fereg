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

Work on main in coherent, reviewable commits. Migrate consumers and remove
superseded implementations together. Push completed authorized work after checks;
never force-push or rewrite history. Report failed or missing checks honestly.
Use bounded Astra assistance for difficult problems and an independent high-effort
review of architectural changes. The lead coordinates implementation and tests;
clean timings exclude competing builds, tests and agent tool workloads on the
measured host. Independent analysis on separate infrastructure may continue
without accessing or changing the measured artifacts or consuming host resources.

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
the default qualification host; his 2026-10-07 request additionally authorizes
bounded Linux and Windows build/load checks without resuming GitHub workflows.
No platform claim follows from another host's checks. See
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
independent witnesses and quiet qualification-host timing. Release performance
claims require quiet Mac timing. Identify artifacts and preserve observations;
inconclusive evidence never passes a claim it cannot support. Experiments need
bounded stopping rules.
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
For an unchanged artifact, a predeclared bounded continuation may target only
inconclusive cells; retain and combine all applicable observations, preserve
the timing allowance and confidence standard, and account for repeated looks.
Do not sample repeatedly until a pass. A changed artifact requires reassessing
which evidence remains valid. See `docs/development_workflow.md` for the protocol.
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
(candidate plugins, A/B builds and extra Cargo target directories, worktrees,
generated data, logs, profiles) under one `target/tmp/<task>/` directory, never
one directory per run or tag, and delete it before reporting the task done,
after its hashes and summaries are in commits or committed documents. Committed
documents never cite ignored paths as retained evidence.
`scripts/clean_target.sh --apply` removes everything else; run it at the start
of a session when no other task is using `target/`, and whenever `target/`
exceeds 15 GB. Johannes authorized deleting completed captures and caches;
historical committed source/documents remain in Git, ignored raw data does not.

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
