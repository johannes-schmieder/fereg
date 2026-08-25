# fereg

fereg is a Rust-first Stata package for high-dimensional fixed-effect OLS/WLS.
The public `fereg` command is active in the untagged 0.9.0 release-candidate
source. `feppml` remains installed but deliberately returns `r(498)`; PPML is
outside this release-candidate scope.

## Project navigation

- [`AGENTS.md`](AGENTS.md) — governing implementation specification.
- [`STATUS.md`](STATUS.md) — sole mutable progress record and exact resume point.
- [`docs/README.md`](docs/README.md) — documentation index.
- [`docs/development_workflow.md`](docs/development_workflow.md) — local
  development and exact-SHA acceptance procedure.
- [`STATA_CI_RUNNER.md`](STATA_CI_RUNNER.md) — licensed Mac runner and Stata
  profile operation.
- [`docs/adr/`](docs/adr/) — accepted architectural and scientific decisions.

Development is consolidated on `main`. Historical feature/Codex branches and
self-modifying development workflows are not part of the current process.

## Current boundary

`fereg` supports the frozen OLS 1.0 command surface: numeric and fixed-width
string fixed effects, FE interactions and absorbed slopes, analytic/frequency/
probability weights, singleton policies, iid/robust/multiway-cluster inference,
DoF controls, time-series materialization, saved outputs, prediction, `test`,
and `estat summarize`. Native computation is certificate-checked and published
through a receipt-last Stata transaction with rollback on failure or Break.

The implementation includes `memory(fast|low|auto)`, allocation/RSS reporting,
bounded scratch cleanup, per-action native thread teams, and deterministic
execution controls. Source qualification is not complete until `STATUS.md`
records exact-SHA local, SCC/Linux, macOS, Windows build/mock, scientific,
performance, memory, and packaging receipts.

## Local development

Use the pinned Rust toolchain and repository scripts:

```bash
./scripts/run_tests.sh
./scripts/build_macos.sh
./ci/run_stata_ci.sh quick
```

Stronger licensed profiles include:

```bash
./ci/run_stata_ci.sh differential
./ci/run_stata_ci.sh full
```

The Stata profiles require the licensed environment described in
`STATA_CI_RUNNER.md`. SCC/Linux qualification follows the project-scoped
workflow in the release plan. Windows licensed Stata and native Intel-host
macOS qualification are explicitly deferred; build/mock and Rosetta evidence
do not imply native licensed certification.

## Design rule

Stata is the control plane; Rust is the data and computation plane. Each
estimation makes one substantive native call. Stata SPI access stays on the
calling thread, and results become visible only after computation, scientific
certificates, output writes, and the final receipt all succeed.
