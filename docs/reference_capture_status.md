# Licensed-reference capture status

A local licensed Stata 18 executable is available for parser and installation
fixtures. These local captures are useful implementation evidence, but they do
not pin `reghdfe` or `ppmlhdfe` and therefore do not satisfy reference-parity
gates. Required reference captures remain explicit unresolved gates rather
than fabricated output.

| Evidence | Script / destination | Status |
|---|---|---|
| `reghdfe` version, timing, `ereturn list` | `benchmarks/stata/baseline_reghdfe.do` | `PENDING_STATA_CAPTURE` |
| `ppmlhdfe` version, timing, `ereturn list` | `benchmarks/stata/baseline_ppmlhdfe.do` | `PENDING_STATA_CAPTURE` |
| Peak RSS and machine metadata | benchmark runner extension described below | `PENDING_STATA_CAPTURE` |
| Stata processor/license macros | `stfixest, doctor verbose` | `PENDING_STATA_CAPTURE` |
| Model-wire v2 parser matrices | `tests/stata/parser/model_wire_v2_capture.do`; sanitized record under `benchmarks/reports/captured/darwin-arm64/stata18/model-protocol-v2/` | `LOCAL_STATA18_CAPTURE_AVAILABLE; PINNED_REFERENCE_PENDING` |

## Capture protocol

Use the exact commits in `references.lock.toml`, record the Stata executable
hash/version and operating system, and run each benchmark from a clean Stata
process. Preserve the generated dataset seed, command line, warmup policy,
three or more timed repetitions, median/dispersion, peak RSS, `ereturn list`,
effective sample, and log hash. Store immutable logs under
`benchmarks/reports/captured/<platform>/` and replace the pending markers only
after independent review.

Local parser logs must remain uncommitted. Sanitized records may include the
fixture hash, Stata major version, platform, matrix dimensions, and log hash,
but never license owner or serial-number text.

The absence of pinned reference captures means the Phase 2 exit gate is not
certified. It does not prevent source handoff, but it prevents estimator,
parity, or performance claims.

## Current local Stata blocker

On the local macOS Stata 18 installation, all seven parser fixtures plus the
postestimation fail-closed fixture reach `END_FIXTURE|...|rc=0`. The two
installation fixtures do not reach their sentinels because
`stata/_stfixest_load.ado` observes `c(os)` as `Unix`, selects
`stfixest_linux.plugin`, and fails to find it even though the built artifact is
`dist/stfixest_macos.plugin`. This loader issue is separate from model wire v2
and has not been suppressed or skipped.

Reproduce the full licensed-fixture result with:

```sh
/opt/homebrew/bin/python3 scripts/run_stata_tests.py \
  --stata /Applications/Stata/StataMP.app/Contents/MacOS/stata-mp
```

The runner must report missing sentinels for `phase01_smoke.do` and
`reload_loop.do`; a Stata batch process returning OS status zero is not treated
as fixture success.
