# Licensed-reference capture status

The construction sandbox had no licensed Stata. The required Phase 0 captures
are therefore supplied as reproducible scripts and explicit unresolved gates,
not fabricated output.

| Evidence | Script / destination | Status |
|---|---|---|
| `reghdfe` version, timing, `ereturn list` | `benchmarks/stata/baseline_reghdfe.do` | `PENDING_STATA_CAPTURE` |
| `ppmlhdfe` version, timing, `ereturn list` | `benchmarks/stata/baseline_ppmlhdfe.do` | `PENDING_STATA_CAPTURE` |
| Peak RSS and machine metadata | benchmark runner extension described below | `PENDING_STATA_CAPTURE` |
| Stata processor/license macros | `stfixest, doctor verbose` | `PENDING_STATA_CAPTURE` |
| Factor/parser matrices | `tests/stata/parser/*.do` | `PENDING_STATA_CAPTURE` |

## Capture protocol

Use the exact commits in `references.lock.toml`, record the Stata executable
hash/version and operating system, and run each benchmark from a clean Stata
process. Preserve the generated dataset seed, command line, warmup policy,
three or more timed repetitions, median/dispersion, peak RSS, `ereturn list`,
effective sample, and log hash. Store immutable logs under
`benchmarks/reports/captured/<platform>/` and replace the pending markers only
after independent review.

The absence of these captures means the Phase 0 exit gate is not yet certified;
it does not prevent source handoff, but it does prevent an estimator release or
performance claim.
