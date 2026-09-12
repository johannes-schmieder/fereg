# P1 PPML ado preparation: adopted local development experiment

Against `8be388e280eb64af5de9830204a63bee8de3b33d`, remove the three nested
PPML `sortpreserve` wrappers and the optional ado completeness-marker,
`markout` and `count` passes. Retain candidate/weight-expression handling,
authoritative native classification and every publication validation. See the
[cost audit](cost-audit.md) and [ADR 0041](../../../../docs/adr/0041-ppml-ado-preparation.md).

Both arms use the **same universal plugin**, SHA-256
`3e7c91ec7c2d590d01f45fc99a02459367dfe56bdf5727d9250fe98a09aadf6f`.
All 150 tracked native source/build inputs match its previously measured frozen
snapshot. [Identities](identities.json) bind both ado sets, the
[isolated implementation patch](implementation.patch), fixtures and harness.
The installed/dist plugin was not replaced. These are local M2 Ultra/Stata 19
development findings, not new comparator, SCC, cross-platform or release claims.

## Command and phase measurements

There are **25 completed pairs / 50 measured calls**: nine public primary pairs,
ten stratified controls and six follow-up low-memory pairs. Each uses a fresh
Stata process after a small untimed warm-up, with three primary seeds and rotated
order. Native thread requests are separate from Stata's eight-processor license
cap; macOS affinity is uncontrolled. The [protocol](protocol.json) predates timing.

Medians in seconds. Paired changes are medians of within-pair ratios and need
not equal ratios of the displayed medians.

| PPML target | N / K / native threads | Command baseline → candidate | Median paired change | Outer Stata baseline → candidate |
|---|---|---:|---:|---:|
| Complete one-FE robust | 983040 / 50 / 16 | 10.247 → 10.205 | −2.4% | 0.271 → 0.101 |
| FW, exposure, sparse missing | 245760 / 50 / 4 | 3.418 → 3.380 | −1.1% | 0.092 → 0.052 |
| PW, offset, two-way cluster, sparse missing | 245760 / 50 / 16 | 3.080 → 3.068 | −0.4% | 0.074 → 0.042 |

Outer time is each command's measured total minus its native total. On the large
target its reductions are 161/169/180 ms across the three pairs. Corresponding
complete-command changes are −376/+52/−244 ms: the unchanged native work varies,
so the whole-command effect is less certain than the consistently lower outer
cost. The predeclared alternative adoption criterion of a repeatable 20 ms
command reduction with lower ado pass work is met; no uniform 3% gain is claimed.
Native preparation medians remain about 0.86–0.89/0.22–0.23/0.22 seconds. No
native kernel gain, iteration reduction or setup/fallback saving is attributed
to this change. Overlapping phase medians must not be summed.

Primary native peaks remain exactly 2,522,610,814 / 633,492,209 / 645,341,968
bytes. Native peaks agree in 20/25 pairs; the other graph controls vary between
−81,904 and +217,240 bytes, all within their unchanged models. All observed
action peaks fit the unchanged selected models. Primary sampled RSS medians
are 1.963→1.962, 0.528→0.528 and 0.574→0.544 GiB; these are process measurements,
not a claimed native peak reduction. The explicit removed Stata marker occupies
N bytes; no unmeasured sort-buffer or hardware-traffic saving is invented.

## Protected controls and the low-memory investigation

Weak/dense/bottleneck/cycle/disconnected/unbalanced controls retain exact science
and solver decisions. Their worst command increase is weak 140→146 ms (+6 ms).
Small/narrow improves 29→25 ms, restricted 63→62 ms and saved/separated 31→30 ms.

The original low-memory FW control is **957→1085 ms (+128 ms / 13.4%)**. Two
independent seeds give 726→796 and 1053→892 ms. Four subsequent same-input,
rotated-order pairs give 999→971, 963→953, 754→776 and 1066→1021 ms. Across all
seven low-memory pairs, the median paired change is −1.0%; the six follow-ups
alone give −1.9%. The original slowdown is retained, not replaced by repeats.
Most variation lies in the unchanged native finite/strict scopes. The precise
external cause is unestablished; the evidence does not show a consistent
regression or justify claiming a low-memory speedup. Every comparison keeps
the same memory mode, solver work, science and successful certificates.

## Validation and disposition

All 25 pairs have byte-identical full b/V, sample hashes, status, DoF, weight,
cluster, separation, certificate and receipt exports. Solver iterations,
line searches, strict retries and fallback decisions are exact; requested saved
outputs also match exactly. [Validation](validation.json) records:

- Four distinct candidate licensed fixtures: new native-preparation boundary,
  existing scientific contract, full hidden transaction and time-series policy.
  Each has r(0) and its terminal sentinel. The new fixture also passes baseline.
- Analytic Poisson cell-mean/offset oracles; explicit expected rows/drop reasons;
  all 27 missing codes; FW/PW and numeric/string clusters; cluster missingness
  before singletons; sort metadata and original order on successful and failed
  requests; saved-output namespace and prior/null-estimate rollback.
- 17 transaction source-contract tests and five profile-registration tests pass.
- The initial weighted-varlist draft failure and the pre-measurement RSS sandbox
  failure are disclosed. Every timed command uses the corrected frozen candidate.

Adopt the wrapper removal. This closes #27's P1 ado audit, with modest measured
command gains located in outer preparation. Batched adjustment/candidate loading,
P0 instrumentation/model gaps, remaining P2–P7 work and the later research backlog
remain separate tasks. No fresh input checks, scientific certificates, native
rank policies or memory admission rules are relaxed.

[Comparisons](comparisons.json), [summary CSV](summary.csv),
[numeric archive](numeric-evidence.zip) and [checksum](archive.json) preserve all
pairs. Replay with `FEREG_LOCAL_BENCH_ROOT` bound to the isolated source/data
directory and `STATA_MP_PATH` set, using
`benchmarks/local_fixest_gap/ppml_ado_compare.py --part primary|controls`.
The archived recheck records identify their seeds and ordering. Raw licensed
logs, local paths, binaries and synthetic datasets stay outside Git.
