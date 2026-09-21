# Resolve the small weighted command timing question

Local Mac development evidence, September 20, 2026. This is a precision
follow-up to control 56 in the [full native inventory](../two-fe-broad-native-controls-local-2026-09-20/README.md),
not a new fixest comparison or release qualification. Production remains
`cdfbcf58`, plugin `79141637b5252f27d6f86a721fb04cf5648df6b8d0d82ee26ea7e39b9b94de14`.
All 516 candidate source files and both measured estimator binaries were
reverified. The original baseline remains the artifact bound to `e9d593bd`.

The original ordinary-timer ratios of 1.05882 and 1.0625 remain failed checks.
The separate sixteen-command batch diagnostic is not used for this disposition.
Before new timing, `precision-plan.json` declared two independent five-pair
blocks of the same single public command, N=7,680, K=6, moderate AW, IID,
sixteen workers, seed 8675309. Inputs, command text, samples, estimator
identities and the entire existing timed body match the prior confirmation.

## Measurement and result

A benchmark-only C plugin brackets the existing timer with
`CLOCK_MONOTONIC_RAW`. It is never linked into or installed with fereg. Each
worker calibrates 128 empty brackets before the ready handshake and settling
period. The ordinary Stata timer and its metadata remain unchanged; no
calibration time is subtracted. Original and instrumented workers, their patch,
both controller manifests and the clock source/binary identity are retained.

| Five-pair block | High-resolution median paired ratio | Ordinary-timer median paired ratio |
|---|---:|---:|
| A | 0.995731 | 1.000000 |
| B | 1.038921 | 1.000000 |

Both predeclared precision blocks meet the unchanged 1.05 native-control
threshold. Average empty-bracket overhead is below one microsecond in every
worker; all maximum calibration observations are below 100 microseconds.
No overhead correction is applied. Every simultaneous ordinary/high-resolution
reading differs by less than two milliseconds. The Stata clock fixture checks
invalid calls, nested starts, repeated stops and three 50-ms sleeps, measured
as 51.31--51.34 ms. The fixture's explicit terminal sentinel passes.

This resolves the control's timing question using more precise measurements
of the same workload. It does not establish a small-case speedup: block B is
3.89% slower and one individual pair is 9.84% slower. All observations and
earlier failures remain visible. No numerical or routing change was made in
response to the rounded timer result.

## Scientific and implementation checks

All twenty application calls pass. All ten native pairs preserve the six
scientific CSV bundles bitwise, exact samples and status/DoF, and identical
native allocation peaks. The ordinary comparison checks also pass. The audit
verifies the original input/command identity and each generated controller
fingerprint. `precision-audit.json` contains every clock/calibration reading,
paired ratio, result hash, memory comparison and scientific-file equality.

Two initial clock builds failed before any measurement: the first lacked an
explicit SDK root, and the second exposed the SDK scalar-store pointer
signature. Both diagnostics and raw-log hashes are preserved in
`build-failures.json`. The successful clock hash is
`b91a01793356b15f120ab010b43466cc7471a8635db555859e46dab139669f20`.
Direct fixture launches produced no expected fixture receipt; acceptance uses
only the subsequent usual Python-controlled launch with its verified sentinel.

The broader milestone remains open pending the fresh holdouts and final
primary comparison blocks. The earlier allocation-estimate coverage gaps are
unchanged. No installed package, remote state or qualification changed.
