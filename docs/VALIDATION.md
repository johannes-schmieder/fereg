# Validation status

The many-FE, two-FE structural-gate and ownership/deferred-cell improvements are
accepted. The OLS method-reporting correction changes diagnostic labels and
fallback flags only; solver selection, iteration budgets, numerical calculations
and PPML reporting are unchanged.

## Accepted results

| Change | Evidence and scope |
| --- | --- |
| Many-FE Auto | [Final compact result](../bench/results/incremental-many-20261018-compact.json): Linux qualification and practical Mac recovery passed all 126 protected comparisons per platform; public activation and independent science passed. |
| Two-FE structural gates | [Linux](../bench/results/two-fe-policy-gates-qualification-linux-20261022.json) and [Mac](../bench/results/two-fe-policy-gates-qualification-mac-20261022.json): all 84 protected bounds per platform passed. [Activation](../bench/results/two-fe-policy-gates-activation-20261023.json): exact public/hidden science, worker, gate, hierarchy and resource checks passed after the documented transport-accounting amendment. |
| Ownership and deferred cells | [Linux](../bench/results/ownership-qualification-final-linux-20261024.json): 10,440 calls, 58 settings and 12 distinct physical hosts, all 116 protected bounds passed. [Practical Mac check](../bench/results/ownership-qualification-practical-macos-20261024.json): 870 calls and all 116 paired-median comparisons within max(5%, 1 ms). The Mac check is descriptive, not a confidence-bound claim. |
| Public integration | [Final activation](../bench/results/ownership-public-activation-20261024.json): 174 actual hidden/public/public-repeat calls per platform, independent witnesses, fixed-worker exactness, cross-worker science, resource/gate/hierarchy checks, Mac/Linux ABI/load/panic checks and the full source suite passed. Six licensed Mac Stata fixtures and actual public OLS Break/rollback passed. |

Ownership/deferred cells activate only for OLS Auto/Auto with a global intercept
and exactly two categorical-intercept FE terms. Explicit injections and paths
outside the scope retain their contracts. The Break test interrupts at the
native pre-preparation checkpoint; it does not establish interruption inside a
cell constructor. Scientific behavior is specified in [SPEC](SPEC.md).

## Package checks

Beta.4 contains the method-reporting correction and current help files. Its
universal Mac plugin was built offline from clean source `1ac7125b` and the
vendored dependency closure with Rust 1.85.1. The dependency audit reported zero
advisories. Binary privacy/export/dependency checks and actual ABI, load and
panic-containment checks passed. [Package metadata](../dist/fereg-native.json)
records the exact binary and corresponding-source identities.

The default `./scripts/run_tests.sh` passed: 316 Python tests, Rust unit and
independent numerical integration tests, Clippy, FFI allocation audits, portable
checks, mock SPI transactions and the complexity ratchet. The final artifact
also passes installed-package admission. No new speed claim or timing campaign
is attached to this reporting-only change.

All 12 mapped Stata 19/MP fixtures passed on Apple Silicon. They cover the
public CG/CMG method names in Fast/Low modes at one/four workers, zero-step
certificates, mixed-block MAP fallback, weighted and wide OLS against independent
dummy-variable fits, explicit CMG-Schur and PPML against full-dummy Poisson.
Actual public OLS Break/rollback passed at the pre-preparation checkpoint.
A fresh local `net install` passed selftest and four reporting/numerical fixtures;
all 31 installed files matched the package. The documentation's 104 local links
also resolve.

## Limits

- Linux performance evidence applies to Linux. The practical Mac ownership/cell
  check reports paired medians, not confidence bounds or a universal speed claim.
- The distributed plugin contains Apple Silicon and Intel slices. Licensed
  installation testing is on Apple Silicon; native Intel-host Stata, Linux
  binary distribution and Windows distribution are not qualified by it.
- GitHub Actions remain disabled; the reported checks run locally.
