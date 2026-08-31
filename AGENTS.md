# fereg agent instructions

## Purpose and scope

This file governs work throughout the repository. It is an operating guide,
not a second scientific specification or a project-status log. Keep it short
and durable; put detailed contracts in `docs/` and current progress in
`STATUS.md`.

The project is a Stata package with a Rust computational core. Preserve that
boundary and the evidence-backed behavior already accepted by the project.

## Sources of truth

Use each source for its intended purpose:

1. Direct user instructions and the current user-approved release decision.
2. Accepted decisions in `docs/adr/`, including ADR-0007 for the 0.5.0 beta.
3. Focused contracts in `docs/`, frozen benchmark manifests, and reference
   pins in `references.lock.toml`.
4. This file for agent conduct and cross-cutting implementation invariants.
5. `STATUS.md` for the sole mutable progress record and current resume point.
6. Tests and source for implemented behavior; exact-SHA receipts for claims
   about a particular source revision or artifact.

If these sources genuinely conflict on econometrics, compatibility, the public
API, or architecture, stop and ask Johannes. Do not silently choose a policy.
Historical receipts show what was tested; they do not define current policy.

Start with:

- `STATUS.md`
- `docs/development_workflow.md`
- `docs/architecture.md`
- `docs/scientific_contracts.md`
- the ADR and focused contract relevant to the task

## Non-negotiable architecture

- Stata is the control plane: parse syntax and metadata, own the user-facing
  transaction, and publish results.
- Rust is the data and computation plane: load observations through the SPI,
  construct native state, preprocess fixed effects, solve, estimate, and form
  inference results.
- Do not materialize large design matrices, factor expansions, group IDs, or
  estimation loops in ado/Mata merely for implementation convenience.
- Keep one guarded substantive plugin call per estimation. The C bridge is an
  ABI adapter, not a scientific implementation layer.
- Do not add a silent slow path that bypasses the native architecture.
- Keep the safe core under `#![forbid(unsafe_code)]`. Confine unavoidable
  unsafe operations to the reviewed FFI boundary.
- Catch Rust panics before they cross the C ABI. Never unwind through C or
  Stata.
- Call Stata SPI functions only on Stata's main thread. Worker threads may use
  only Rust-owned data.
- Validate outputs completely before committing them to Stata. Errors,
  interruption, or allocation failure must leave no partial public results.
- Unimplemented public features must fail closed with a clear error.

Detailed boundaries live in `docs/architecture.md`, `docs/data_plane.md`,
`docs/plugin_protocol.md`, `docs/threading.md`, and `docs/memory_model.md`.

## Scientific and performance discipline

- Never weaken samples, tolerances, weight semantics, VCE, degrees of freedom,
  convergence certificates, failure atomicity, or reference designs merely to
  make a test pass.
- Reference parity is necessary but not sufficient. Retain an independent
  mathematical, explicit-dummy, or otherwise separately implemented oracle.
- Keep external references commit-pinned. Do not substitute a floating branch
  or a convenient local checkout for a frozen reference.
- Treat numerical disagreements as scientific questions: localize the stage,
  identify the invariant, and adjudicate with focused evidence.
- Preserve row/sample semantics, missing-value behavior, collinearity status,
  and deterministic result ordering unless an accepted decision changes them.
- Performance and memory are release contracts. Make claims only from the
  frozen benchmark suite and measured receipts.
- Do not sacrifice hot-path performance for cosmetic abstraction. Measure any
  change to data layout, allocation, parallelism, or solver control flow.

See `docs/scientific_contracts.md`, `docs/weight_semantics.md`,
`docs/fe_solver.md`, `docs/ols_core.md`, and `docs/performance_contract.md`.

## KISS implementation style

- Make the smallest coherent change that preserves the contracts above.
- Prefer direct, explicit control flow at transaction, FFI, and numerical
  boundaries.
- Reuse an existing representation or helper before introducing another one.
- Extract an abstraction when it removes demonstrated duplication, enforces an
  invariant, or makes a tested boundary clearer—not for hypothetical reuse.
- Keep public interfaces narrow. Do not expose developer actions as supported
  Stata commands.
- Use names that reveal units, ownership, sample scope, and weighted versus
  unweighted state where those distinctions matter.
- Delete dead code and stale compatibility paths once their callers and tests
  are gone. Do not retain commented-out implementations.
- Avoid unrelated cleanup in a focused change, especially in a dirty worktree.

Comments should explain why the code is shaped a certain way: a scientific
invariant, safety condition, ownership constraint, numerical safeguard,
platform quirk, or non-obvious performance choice. Remove comments that merely
restate the next line, narrate control flow, preserve obsolete plans, or repeat
documentation. Public documentation should describe observable behavior and
failure conditions, not internal chronology.

## Working in the repository

- Inspect `git status --short` before and after work. Preserve unrelated user
  changes and untracked files.
- Work on `main`; do not create branches unless Johannes requests one.
- Do not push, publish, stage a release, or mutate remote state without an
  explicit request.
- Keep commits coherent and evidence-backed. Do not create no-op CI triggers
  or self-modifying workflow controllers.
- Keep `STATUS.md` as the only mutable status/handover document. Update it only
  after the named evidence exists and is reachable.
- Do not commit credentials, license data, machine-specific paths, raw licensed
  logs, or unrelated generated artifacts.
- Preserve historical receipts and captured evidence. Never rewrite them to
  make a later claim look applicable to an earlier source SHA.

## Validation and release evidence

Use targeted tests while iterating, then the repository entry points appropriate
to the change. The standard local commands are:

```bash
./scripts/run_tests.sh
./scripts/build_macos.sh
./ci/run_stata_ci.sh quick
```

Stronger scientific or release gates may require the `differential` and `full`
Stata profiles. Licensed-runner requirements and exact-SHA acceptance are in
`docs/development_workflow.md` and `STATA_CI_RUNNER.md`.

- Do not claim a test, platform, benchmark, or Stata profile was run when it
  was not.
- A process exit status alone is not acceptance when the contract also requires
  fixture receipts, explicit return codes, or terminal sentinels.
- Release claims attach to the exact tested source SHA and artifact identity.
- Run `git diff --check` and inspect the final diff before handing off work.

## Documentation map

- Architecture and data flow: `docs/architecture.md`, `docs/data_plane.md`
- Scientific behavior: `docs/scientific_contracts.md`, `docs/ols_core.md`
- Solver, memory, and performance: `docs/fe_solver.md`,
  `docs/memory_model.md`, `docs/performance_contract.md`
- Supported surface: `docs/compatibility_matrix.md`, `docs/user_guide.md`, and
  Stata help under `stata/`
- Development and release: `docs/development_workflow.md`,
  `docs/release_process.md`, `STATA_CI_RUNNER.md`
- Current state: `STATUS.md`
