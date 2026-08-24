# Development workflow

This document is the operational entry point for maintainers and local Codex
sessions. It does not replace the scientific specification.

## Authority and source of truth

Use this order when sources disagree:

1. `AGENTS.md` — governing implementation and release specification.
2. Accepted decisions in `docs/adr/`.
3. `STATUS.md` — sole mutable progress record and current resume point.
4. Exact-SHA CI and scientific receipts under `.ci/`, `build_evidence/`, and
   `benchmarks/reports/captured/`.
5. Other design documents, tests, and source comments.

Historical commits and receipts remain evidence of what was tested, but a
superseded scientific policy is not current acceptance evidence.

## Branch and commit discipline

- Work directly on `main`. Do not create feature, Codex, recovery, or temporary
  branches unless Johannes explicitly requests one.
- Start by fetching `main`, inspecting recent commits, and reading `AGENTS.md`,
  `STATUS.md`, and the relevant ADR.
- Make small, coherent commits and push after each meaningful implementation,
  repair, test/oracle family, CI fix, or documentation checkpoint.
- Do not use tiny no-op trigger commits or self-modifying workflow controllers.
- Do not knowingly push a deliberately failing scientific checkpoint. Ordinary
  source pushes may use CI as an additional compiler/test environment, but
  discovered failures must be repaired promptly.
- Keep raw licensed logs, machine paths, credentials, license data, unrelated
  local files, and confidential data out of Git.

## Local validation

Use the pinned Rust toolchain and the repository entry points:

```bash
./scripts/run_tests.sh
./scripts/build_macos.sh
./ci/run_stata_ci.sh quick
```

Use targeted Rust/Python/Stata tests while iterating. Before closing a
substantive gate, run the relevant stronger profiles, normally including:

```bash
./ci/run_stata_ci.sh differential
./ci/run_stata_ci.sh full
```

The Stata commands require the licensed Mac environment documented in
`STATA_CI_RUNNER.md`. Do not claim a test was run when it was not.

## GitHub CI and exact-SHA acceptance

The durable workflows are:

- `.github/workflows/stata-ci.yml` — normal main-push quick qualification and
  exact-SHA receipt publication; manual dispatch supports stronger profiles.
- `.github/workflows/fe-interaction-reference-probe.yml` — exact pinned
  offline reference probe for its bounded contract.
- `.github/workflows/hosted-qualification.yml` — manual release-oriented hosted
  qualification only.

After every substantive source push:

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
- Public `stfeols`, `stfepois`, `estimate-ols`, and `estimate-ppml` remain
  fail-closed until their documented activation gates are exact-green.
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
- `docs/adr/0006-positive-residual-dof-for-absorbed-slopes.md`
- the code, tests, manifest, and receipts named by the active checkpoint.

Update `STATUS.md` only after exact-green, reachable evidence exists.
