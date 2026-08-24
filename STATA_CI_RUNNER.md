# Licensed Stata/Rust Mac runner

fereg uses one private repository-level GitHub Actions runner on the Mac
Studio for licensed Stata/MP 18 and target-native Rust/plugin checks. Workflow
code executes with the privileges of the logged-in macOS user.

## Security boundary

- The repository and workflow writers must remain trusted.
- No self-hosted workflow may run on a public pull request, fork, or untrusted
  branch.
- Never expose credentials, runner tokens, Stata license material, home
  directory contents, unrelated local files, or confidential research data.
- Use synthetic fixtures unless Johannes explicitly authorizes otherwise.
- The runner and the `varcomp_kss` runner share a lock; do not run licensed
  Stata concurrently.
- Development occurs on `main`. Local Codex performs coding locally; GitHub
  Actions supplies tests and evidence, not background coding or self-modifying
  repository orchestration.

## Validated host

- Apple Silicon Mac Studio `Mac14,14`, M2 Ultra, 24 CPU cores, 192 GB RAM.
- macOS 26.5.2 at the recorded setup checkpoint.
- Stata/MP 18 at
  `/Applications/Stata/StataMP.app/Contents/MacOS/stata-mp`.
- Rust 1.85.1 with `rustfmt`, Clippy, and both Apple targets.
- Apple clang/Xcode.

This host validates native arm64 Stata and a universal macOS plugin. Rosetta
loader evidence is not native Intel-host certification.

## Local profile interface

From the repository root:

```bash
./ci/run_stata_ci.sh version
./ci/run_stata_ci.sh smoke
./ci/run_stata_ci.sh syntax
./ci/run_stata_ci.sh quick
./ci/run_stata_ci.sh mata-unit
./ci/run_stata_ci.sh plugin-build
./ci/run_stata_ci.sh plugin-load
./ci/run_stata_ci.sh numerical-small
./ci/run_stata_ci.sh differential
./ci/run_stata_ci.sh benchmark-small
./ci/run_stata_ci.sh full
```

Profiles are defined in `ci/stata_profiles.json`. Each run stages tracked files
in a fresh temporary tree, isolates Stata ado locations, binds fixtures to the
freshly built plugin, and stores sanitized local output in the ignored
`.ci/stata/run/` directory.

Stata can return shell status zero after an uncaught Stata error. Acceptance
therefore requires the explicit Stata return code, per-fixture status files, and
exactly one terminal `END_FIXTURE|<name>|rc=0` sentinel.

## Durable GitHub workflows

- `.github/workflows/stata-ci.yml` — main-push Rust quick plus licensed-Stata
  quick; manual dispatch selects any profile; publishes exact-SHA receipts.
- `.github/workflows/fe-interaction-reference-probe.yml` — bounded exact-pinned
  offline reference probe.
- `.github/workflows/hosted-qualification.yml` — manual release qualification
  on hosted runners.

There is no `pull_request` trigger for the private Mac runner.

For a substantive source SHA, accept the checkpoint only when
`.ci/stata/results/<SHA>.json` records:

```text
tested_sha   == <SHA>
status       == success
stata_status == success
rust_status  == success
```

Also inspect `stata_rc`, `rust_rc`, fixture counts, plugin SHA-256, and
gate-specific evidence. `.ci/stata/latest.json` is only a convenience pointer;
receipt-only `[skip ci]` commits are not new source checkpoints.

Actions artifacts retain full test logs, fixture statuses, the candidate plugin,
its checksum, and the receipt for a bounded retention period. Committed
receipts are sanitized and must not contain machine paths or license data.

## Runner operation

Installation:

```text
/Users/johannes/actions-runners/stfixest-stata
```

Runner name:

```text
macstudio-stata-mp18-stfixest
```

Labels:

```text
self-hosted, macOS, ARM64, stata, stata-mp, stata18, fereg
```

The installation directory, runner name, and LaunchAgent service identifier
retain their original `stfixest` spelling. They are host-local legacy
identifiers only; changing them would require deregistration and service
replacement and is not part of the repository/package rename.

Manage it from the installation directory:

```bash
./svc.sh status
./svc.sh stop
./svc.sh start
```

The service is a per-user LaunchAgent, so the macOS GUI user must remain logged
in. When `svc.sh status` is ambiguous, inspect:

```bash
launchctl print \
  "gui/$(id -u)/actions.runner.johannes-schmieder-StFixest.macstudio-stata-mp18-stfixest"
```

Use repository Settings → Actions → Runners and the bounded runner stdout/stderr
logs for diagnosis. Registration/removal tokens are short-lived secrets and
must never be committed or logged.

See `docs/development_workflow.md` for commit and acceptance discipline.
