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

The Auto-routing, CMG and ownership/cell workstream completed on 2026-10-24.
Johannes authorized cleanup, publication of the updated Mac beta and merging
`codex/auto-routing` into main. Subsequent work uses the normal main-branch
convention unless Johannes specifies another branch. Commit and push coherent,
reviewable changes; never force-push or rewrite history. GitHub Actions remain
paused; use `[skip ci]` for this publication and cleanup.

Runtime changes require `./scripts/run_tests.sh` and mapped scientific/transaction
checks. Build/package changes require focused tooling and native build/ABI checks.
Prose-only edits use local link/reference checks and `git diff --check`.
Reuse valid unaffected evidence. Run broad checks at integration boundaries,
not repeatedly for unchanged artifacts. Keep the runtime complexity ratchet.
Use bounded Astra assistance for difficult problems and independent high-effort
review for architectural changes; the lead coordinates implementation and tests.

The accepted OLS Auto defaults cover categorical-intercept designs: many-FE
uses the geometry policy (bounded MAP pilot, Jacobi-to-additive LSMR, requested
certificate); two-FE uses the selected structural CMG gates. Ownership and deferred cells require exactly two
categorical-intercept terms, a global intercept and Auto solver/preconditioner.
Preserve explicit developer injections and unaffected paths. See SPEC for the
current behavior and VALIDATION for completed evidence and remaining limitations.

Performance work uses one fixed candidate, current inputs/references, a bounded
schedule and one report through `bench/qualify.py`. Protect original and accepted
Auto with max(5%, 1 ms), retain independent scientific witnesses, fixed-worker
repeatability and cross-worker scientific agreement. Declare numerical scope
before arithmetic/storage changes; unaffected fields and discrete decisions stay
exact. Public activation requires exact selected-hidden/public agreement.
Never sample until a pass, discard unfavorable timings, or silently replace an
inconclusive candidate. Diagnose harness failures with bounded repairs; discuss
scientific failures or inconclusive decisions with Johannes.

Linux is the primary performance host. Compare arms within the same verified
isolated allocation on general eligible SCC nodes; slot counts alone do not prove
isolation. Physical hosts are independent Linux units. Mac behavior requires
focused Mac checks; ordinary desktop activity is retained measurement noise.
Exclude competing builds, tests, estimation and agent tools from measured hosts.
Freeze any external-interference exclusion/retry rule before timing; native or
scientific failures and adverse timings never authorize resampling.
Arithmetic-preserving work can reuse timing at a workstream boundary and use
instruction counts or function-level samples with reported uncertainty.
Do not add receipt chains, recursive history checks or automatic sampling ladders.
After ten unsuccessful substantive repairs or one working day on one blocker,
record the reproducer/findings and ask Johannes for direction.

SCC authentication: after a batch authentication failure, run plain `ssh scc` in
an interactive PTY, complete prompts and phone 2FA, keep it open and verify
`ssh -o BatchMode=yes scc hostname` before retrying. A sandbox socket denial is
not an authentication failure. Never run workloads on login nodes or agent tools
on measured nodes. The pinned `within`/`schwarz-precond` dependencies follow the
same scientific rules; integrate dependency changes through reviewed pins.

Native publication permissions are resolved in `docs/licensing.md`. Main is a
live Stata installation source: refresh its package with runtime changes, test
the actual installed binary and publish matching corresponding source. Keep old
releases immutable and report platform limitations honestly.

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
work belongs in fereg-paper. Native distribution follows `docs/release_process.md` and the resolved terms
in `docs/licensing.md`; unrelated development is not publication authorization.

Changes to sample, rank, omission or separation semantics require Johannes's
decision before implementation. A failed or unverified prerequisite blocks
dependent work; independent work may continue.
