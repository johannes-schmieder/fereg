# Development

Scientific behavior is defined by [SPEC](SPEC.md); ownership and boundaries by
[ARCHITECTURE](ARCHITECTURE.md). Work on main in coherent commits and use
independent oracles to establish correctness. The repository has one runtime.

## Local checks

Python tooling uses Python 3.11 or newer. Scientific Python helpers require
NumPy, pandas and SciPy. Rust uses the pinned toolchain and Cargo.lock.

```sh
./scripts/run_tests.sh
./scripts/build_macos.sh
./ci/run_stata_ci.sh full
```

The full entrypoint covers Rust, Clippy, allocation/FFI audits, runtime complexity
and the mock SPI bridge. Licensed Stata profiles also include `smoke`,
`differential`, `fereg-regression`, `ppml-scientific` and `ppml-hidden`.
See [the Stata runner](../STATA_CI_RUNNER.md). Target mapped tests during development;
run the applicable integration checks before committing runtime changes.
GitHub testing is paused. Mac remains the default qualification host; Johannes
additionally requested bounded Linux and Windows build checks on 2026-10-07.
Existing GitHub build definitions remain inactive.

`scripts/build_linux.sh` and `scripts/build_windows.ps1` build the native plugins
and run their binary audits and actual-plugin loader checks. The private Windows
controller's `stata-do` profile uses `windows-ci.do` for an exact-source build,
checksum verification and licensed OLS/PPML smoke checks. A successful build is
not a full scientific or performance qualification for that platform.

Use `scripts/run_plugin_replay.sh` for real-plugin independent dense science and
fixed-thread repeat checks. A prior binary may additionally serve as an exact
regression comparator. It is not a second installed runtime or scientific authority.
Declare intentional numerical scope before changes; preserve unaffected results.

## Native and frontend boundaries

SPI runs on the main thread. Rust workers use owned memory. Keep panic containment,
fallible allocation, Break handling, validation-before-publication and rollback.
Original factor slots/base flags, independent certificates and both bias modes
remain mandatory. Different workers need scientific agreement and stable decisions;
a fixed deterministic worker configuration must repeat exactly.

After editing `_fereg_ppml_result_mata.ado`, rebuild its three `.mo` files and
receipt using the actual Stata 18 compiler with `scripts/build_ppml_mata.py`.
A newer compiler's version statement does not ensure Stata 18 compatibility.
Staging validates the source/object hashes. There is no runtime compilation fallback.

## Outputs and performance

Use `target/` for one current build/test cache and `dist/` for the current plugin.
Remove temporary captures, profiles, copied source and obsolete builds after
recording useful results. Keep large generated inputs and outputs out of Git.
The compact benchmark entrypoint is documented in [bench](../bench/README.md).

Time identified artifacts during quiet periods without our builds or agents.
Coordinate unrelated workloads without terminating them. Predetermine workloads,
measurement budgets and stopping rules for acceptance claims. Retain unsuccessful
observations until the experiment is closed; missing or uncertain results are
not successes. The default per-workload allowance is max(5%, 1 ms); report costs
as well as gains. No ongoing obligation to rerun retired historical campaigns exists.

Native distribution remains subject to [licensing](licensing.md).

Report slow workload families, memory, failed cells and scaling alongside aggregate
time. Unexplained material scaling cliffs fail qualification. Changing the default
max(5%, 1 ms) allowance requires Johannes's decision before the affected implementation.

The complexity inventory counts runtime Rust source after removing whitespace,
comments and test-only items. It follows declared modules, including non-test cfg
variants. Per-crate lines, long files/functions and sensitive dispatch conditions
ratchet; decreases lower the inventory and growth needs exact scoped reasons.
The inventory was relocated unchanged from the completed program; its historical
growth explanations remain attached to the same measurements.
