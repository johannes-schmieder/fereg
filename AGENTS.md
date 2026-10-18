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
this checkout on `codex/auto-routing`, starting at `f35f3a4d`. Commit and push
that branch; do not reset main, merge to main, or update release packages as
part of this workstream. Other work retains the normal main-branch convention.
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

Johannes's 2026-10-17 incremental-routing decision makes explicit solver timings
diagnostic opportunity comparisons, not promotion gates. New versioned campaigns
protect the original uninjected Auto at `f35f3a4d` and the last accepted branch
Auto with the existing per-workload max(5%, 1 ms) allowance. Qualify the existing
many-FE and two-FE improvements separately, in that order, with separate untouched
confirmations. Johannes's subsequent priority correction requires completing the
existing many-FE finalist before advancing two-FE. Its first C witness attempt
failed operationally before native science or timing; preserve that failure and
repair the harness, then use a reviewed, versioned confirmation with genuinely
fresh inputs. Do not retry or relabel the accessed old C panel. A further tooling
failure is an engineering blocker to diagnose and surface, not permission to
abandon many-FE. Scientific failure or inconclusive performance retains the
incumbent and requires Johannes's direction. Keep the public default disabled
until valid confirmation passes. Restrict new activation to
OLS `solver(auto)` and `preconditioner(auto)` on categorical intercept designs.
Preserve historical protocol decisions and scientific/resource/transaction gates.
Activate passing defaults only on this branch. Finish this increment with bounded
CMG/RHS/overlap cost diagnosis and a successor proposal; it does not authorize an
unreviewed predictive payback algorithm or increased speculative work budgets.

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
scaling and Linux performance, followed by focused Mac selection/confirmation.
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
evidence; old excluded attempts remain excluded. Freeze discovery/selection and
untouched confirmation separately; an inconclusive finalist does not replace
the public incumbent. See `docs/development_workflow.md` for the protocol.
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
