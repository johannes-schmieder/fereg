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
no agents or competing builds run during clean timings.

Runtime changes require `./scripts/run_tests.sh` and mapped scientific/transaction
checks. Build/package changes require focused tooling and native build/ABI checks.
Documentation uses link checks. Reuse valid evidence for unchanged artifacts;
run broad suites at integration boundaries or when concrete risks require them.
After ten substantive unsuccessful repairs or one working day on one blocker,
record the reproducer and findings and ask Johannes for direction.

GitHub testing remains paused by Johannes's 2026-10-05 instruction. Mac remains
the default qualification host; his 2026-10-07 request additionally authorizes
bounded Linux and Windows build/load checks without resuming GitHub workflows.
No platform claim follows from another host's checks. See
`docs/development_workflow.md` for entrypoints.
Arithmetic-preserving changes with zero exact replay differences and unchanged
solver signatures may use instruction counts or function-level CPU samples on
predeclared workloads, plus no wall-time regression beyond max(5%, 1 ms) on the
standard workloads. Report sampling uncertainty; gains below the allowance are
observations, not failures. Result-changing work requires declared scope,
independent witnesses and quiet qualification-host timing. Release performance
claims require quiet Mac timing. Identify artifacts and preserve observations;
inconclusive evidence never passes a claim it cannot support. Experiments need
bounded stopping rules.

The three-FE LSMR hot path lives in pinned `within`/`schwarz-precond` dependencies
from `johannes-schmieder/within`. Apply these scientific and evidence rules there;
integrate changes through a pinned revision bump.

## Repository hygiene

Keep source, compact fixtures, current documentation and maintained test/benchmark
tools. Generated builds, datasets, raw logs and profiles belong in ignored output
directories and may be removed after their useful summary is recorded. Use one
current build cache; retire temporary source copies and artifacts after use.
Johannes explicitly authorized deleting completed campaign captures and caches;
historical committed source/documents remain in Git, ignored raw data does not.

Retain the runtime complexity ratchet at `tests/reference/complexity.json`;
explain scoped increases and lower it for reductions. Do not minify or move code
into exclusions. Code reachable only from its own tests is deleted, not gated;
independent reference implementations belong in fereg-oracle or fereg-testkit.
No arbitrary coverage percentage is a gate.
Do not introduce planning/status ledgers or receipt-folder proliferation.
Never commit credentials, licenses, personal paths, raw logs, datasets or new
files above 1 MB (existing reference bundles excepted). Paper work belongs in
fereg-paper. Native distribution still requires the licensing decision described
in `docs/licensing.md`; development is not publication authorization.

Changes to sample, rank, omission or separation semantics require Johannes's
decision before implementation. A failed or unverified prerequisite blocks
dependent work; independent work may continue.
