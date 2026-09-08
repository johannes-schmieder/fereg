# Development workflow

This document is the operational entry point for maintainers and local Codex
sessions. Detailed scientific and architectural contracts live in the focused
documents under `docs/`.

## Authority and source of truth

Use each source for its intended purpose:

1. Direct user instructions and the current user-approved release decision.
2. Accepted decisions in `docs/adr/`, including ADR-0007 for the 0.5.0 beta.
3. Focused contracts in `docs/`, frozen manifests, and pinned references.
4. `AGENTS.md` for agent conduct and cross-cutting implementation invariants.
5. `STATUS.md` for the sole mutable progress record and current resume point.
6. Tests and source for implemented behavior; exact-SHA CI and scientific
   receipts for claims about a particular revision or artifact.

Historical commits and receipts remain evidence of what was tested, but a
superseded scientific policy is not current acceptance evidence.

## Branch and commit discipline

- Work directly on `main`. Do not create feature, Codex, recovery, or temporary
  branches unless Johannes explicitly requests one.
- Inspect local and remote `main`, recent commits, `AGENTS.md`, `STATUS.md`,
  and the relevant ADR without changing the worktree.
- Make coherent local milestone commits with targeted green evidence. Do not
  push incremental development commits during 0.5.0 qualification.
- Do not use tiny no-op trigger commits or self-modifying workflow controllers.
- Push only when Johannes requests it. For ordinary prototype work, attach the
  smallest relevant local or SCC development check. Release-level pushes still
  require the exact candidate SHA to pass the explicitly requested gates.
- Keep raw licensed logs, machine paths, credentials, license data, unrelated
  local files, and confidential data out of Git.

## Local validation

Use targeted Rust, Python, source-contract, or Stata tests while iterating. The
commands below are escalation points, not a checklist for every change:

```bash
./scripts/run_tests.sh
./scripts/build_macos.sh
./ci/run_stata_ci.sh quick
```

Run stronger profiles only when the requested task needs broader regression or
qualification evidence:

```bash
./ci/run_stata_ci.sh differential
./ci/run_stata_ci.sh full
```

The Stata commands require the licensed Mac environment documented in
`STATA_CI_RUNNER.md`. Do not claim a test was run when it was not.

For focused PPML development, use the active public command and the smallest
fixture that exercises the changed boundary:

```bash
./ci/run_stata_ci.sh ppml-scientific
```

`ppml-hidden` and `fereg-regression` remain useful focused regression profiles,
but they are not standing requirements or substitutes for the public surface.

### PPML solver development

Use the public `solver(auto|map|lsmr|cmg)` surface and its versioned solver
report. Start with the smallest local science, parser, protocol, or transaction
fixture that exercises the change. Auto follows the development-admitted policy
in ADR 0029; explicit-family measurements must not silently alter that policy.

Cross-estimator objective adjudication follows
[ADR 0026](adr/0026-ppml-comparator-objective-adjudication.md). The pinned
`ppmlhdfe` fit's reported likelihood and deviance are the authoritative
reference pair after their own finite, domain, and Poisson-identity checks.
Prediction replay, KKT, and model-space projection remain diagnostics; a
predicted state is an additional objective witness only after independent
feasibility and stationarity certification. Fereg continues to require full
self-consistency and certification. Version the manifest, parser, adjudicator,
and focused tests together before collecting evidence under this policy, and
never reinterpret an older receipt under the new rules.

When SCC behavior is relevant, use small diagnostic samples first. Prefer
comparable 28-core nodes and run roughly 10--20 independent tasks in parallel;
each task may execute several small jobs sequentially. Escalate sample size and
repetitions only after local and small-cluster science checks pass. Development
results remain diagnostic until the frozen qualification contract is explicitly
run on an exact source and artifact.

## Test escalation and SCC lanes

This repository is in prototype and development mode. Unless Johannes
explicitly asks for qualification, release evidence, or a detailed campaign,
stop after the first proportionate green check:

1. Focused local unit, oracle, static, or licensed-Stata fixture.
2. SCC development smoke when Linux/SCC behavior matters, selecting exactly
   `ols`, `ppml`, or `shared`.
3. Focused benchmark only when a measured performance or memory answer is
   needed.
4. Frozen matrices and claim-grade receipts only after explicit authorization.

The SCC development job is `benchmarks/scc/fereg_dev_smoke_job.sh`. It requests
four slots, 30 minutes, and 4 GB per core. Stage one exact source snapshot into
`/projectnb/welfgr/fereg/dev/<run-id>/source`, set `FEREG_DEV_PROFILE` to the
changed boundary, and use a fresh run directory. Its summary and
`DEV_SMOKE.SUCCESS` marker are diagnostic evidence; they are not promoted into
qualification receipts. A normal development smoke should be checked directly
after submission and does not need a heartbeat.

The exact licensed-load artifacts needed before the 33-cell SCC prepare use
the closed workflow in [feppml platform load evidence](feppml_platform_load_evidence.md).
In particular, Linux has a standalone immutable SCC bootstrap so its build,
actual-loader, Stata capture, and qacct receipt do not depend on the full
qualification run that consumes them.

Any checkpoint that extracts or changes a mechanism used by public OLS must
also follow `fereg_regression_contract.md`. Run its closed local screen before
the checkpoint commit and its governing SCC profile before accepting the
checkpoint. Evidence remains diagnostic while the F0 manifest is explicitly
unsealed.

## GitHub CI and exact-SHA acceptance

The durable workflows are:

- `.github/workflows/stata-ci.yml` — manual licensed Mac Stata/Rust validation;
  dispatch defaults to the small `smoke` profile and publishes an exact-SHA
  receipt. Ordinary pushes do not start it.
- `.github/workflows/fe-interaction-reference-probe.yml` — manual exact-pinned
  offline reference probe for its bounded contract.
- `.github/workflows/hosted-qualification.yml` — manual release-oriented hosted
  qualification only.

After the release-level source push:

1. Record the complete source SHA.
2. Read `.ci/stata/results/<FULL-SHA>.json`.
3. Require `tested_sha` to equal that SHA.
4. Require `status`, `stata_status`, and `rust_status` all to be `success`.
5. Inspect `stata_rc`, `rust_rc`, fixture counts, plugin identity, and any
   gate-specific evidence.
6. Treat `.ci/stata/latest.json` only as a convenience pointer. A later
   `[skip ci]` receipt commit is not a new source checkpoint.

Stata process status alone is not authoritative: the explicit Stata return code,
fixture status files, and terminal PASS sentinel must agree.

## Scientific boundary

- Never weaken tolerances, samples, weight semantics, VCE, DoF, convergence
  certificates, or failure atomicity merely to match a reference.
- Reference parity is necessary but not sufficient; retain independent
  mathematical or explicit-dummy oracles.
- Ask Johannes before making a genuinely ambiguous econometric, compatibility,
  API, or architecture decision.
- Public `fereg` OLS and public `feppml` are active beta testing surfaces. The
  internal PPML transaction remains an implementation and qualification seam,
  not a second user-facing solver interface.
- Performance and memory claims require measured evidence on the frozen suite.

## Resume checklist

```bash
git checkout main
git pull --ff-only origin main
git status --short
git log --oneline --decorate -20
```

Then read:

- `AGENTS.md`
- `STATUS.md`
- `docs/adr/0007-fereg-050-beta-qualification-contract.md`
- the code, tests, manifest, and receipts named by the active checkpoint.

Update `STATUS.md` only after exact-green, reachable evidence exists.
