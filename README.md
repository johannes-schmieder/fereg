# fereg

fereg is a Rust-first Stata package intended to provide high-dimensional
fixed-effect OLS/WLS (`fereg`) and PPML (`feppml`).

The repository contains substantial native preparation, solver, inference,
protocol, and validation infrastructure. It is still a development snapshot:
`fereg`, `feppml`, public `estimate-ols`, and public `estimate-ppml`
deliberately fail closed with `r(498)` and post no estimate.

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

The accepted hidden OLS candidate covers a bounded numeric surface and extensive
private integration tests, including categorical absorbed-FE interactions,
iid/HC1/clustered inference, result transport, and rollback. This evidence does
not enable the public estimator.

The active OLS work is the option-A absorbed-slope correction documented in
`STATUS.md` and ADR 0006: successful inference must retain positive residual
degrees of freedom, the positive-DoF slope fixture must be recaptured and
resealed, and the old over-absorbed fixture must remain fail-closed.

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
`STATA_CI_RUNNER.md`. Linux and Windows build entry points exist, but a build on
one platform is not validation for another.

## Design rule

Stata is the control plane; Rust is the data and computation plane. A future
estimation command must make one substantive native call and publish results
only after the calculation, scientific certificates, and transaction checks
all succeed.
