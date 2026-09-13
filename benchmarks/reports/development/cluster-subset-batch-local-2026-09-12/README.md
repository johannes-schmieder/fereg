# P3: bounded shared cluster-subset score passes

Adopt the scratch-score batch in [ADR 0045](../../../../docs/adr/0045-batched-low-memory-cluster-scores.md)
for local development against `c37686b96b633db43eb1d7210f5ca3e7f9520e93`.
The original two-/three-way experiment did **not** reach its 10% command target.
Adoption follows a separately specified, successful many-way investigation;
those evidence tiers remain separate rather than changing the original criterion.

## Mechanism and bounded coverage

A scratch-backed OLS score row now feeds every nonempty cluster subset during
one original-row traversal when the complete compensated group bank fits.
The bank includes group values, compensation, presence flags, subset ranges
and the temporary mixed-radix prefix codes. Their combined size must not exceed
one displaced `sample_rows * sizeof(usize)` ordering array. Frequency-expanded
effective N is never used to enlarge that allowance. Planning precedes every
bank reservation and score read; failure to fit leaves no partial bank behind.

This is an action-local owner, released before final bread/publication work.
It neither retains a second N-by-K score matrix nor caches state across fits.
Per-group row order, lexicographic group order, cardinality/mask covariance
merge order and the existing independent per-group bread arithmetic remain.
One-way and dense/strided consumers keep their existing routes; a shared
finishing helper also removes unused scratch-group vectors on the one-way
bounded-table path. Larger or unrepresentable products retain the accepted
per-subset table/sorting alternatives. No SPI, solver, rank, DoF, weight,
PSD, admission or transaction policy changes.

[Admission replay](bank-admission-replay.json) binds sample hashes and exported
cluster counts to the source byte formula. The five-way small/large banks need
93,983/204,479 bytes against 245,760/981,064-byte order budgets. Their covariance
score traversals fall 31→1; the original three-/two-way cases fall 7→1 and 3→1.
These are source-derived traversal counts, not instrumented physical I/O or
hardware traffic. Score residualization and its independent certification still
run before covariance construction; this change does not remove that work.

## Command evidence and the adoption decision

There are **26 pairs / 52 measured commands** in isolated local Stata/MP 19
processes on the M2 Ultra, following a small untimed warm-up. Repeated cases
rotate arm order and seeds. Native thread requests are separate from Stata's
licensed processor count. Affinity is uncontrolled; no external-comparator,
SCC, release or cross-platform claim is made.

| Evidence tier / low-memory case | N / K / native threads | Command medians, seconds | Median paired gain |
|---|---|---:|---:|
| Original primary: FW three-way | 122880 / 10 / 4 | 0.303 → 0.283 | 5.9% |
| Original primary: PW two-way | 245760 / 16 / 16 | 0.993 → 0.988 | 0.7% |
| Fresh follow-up: unit five-way | 30720 / 4 / 4 | 0.054 → 0.043 | 20.4% |
| Fresh follow-up: AW five-way, missing | 122880 / 10 / 4 | 0.441 → 0.365 | 17.4% |

Each row has three pairs. Ratios of displayed medians and median paired ratios
are different statistics. [Original protocol](protocol.md) and
[many-way follow-up protocol](manyway-protocol.md) preserve the decision sequence.
The original targets miss 10%, and two-way command gains remain unconvincing.
The five-way control's exploratory 58→43 ms result motivated two new shapes
with three fresh seeds each, prospectively requiring 10% gains in both shapes
and no material protected regression. Both meet that criterion. The general
bounded mechanism is adopted on that evidence; it is not a claimed 10% win on
ordinary two-way clustering.

The initial implementation repeatedly decoded each subset key. Its preserved
single pilot is 0.964→0.982 seconds, a 1.9% slowdown. The revised prefix recurrence
pilot is 0.958→0.956. Neither pilot is included in repeated estimates or treated
as a general speedup. [Initial source identity](initial-prototype/identity.json)
and its separate numeric archive are retained.

The five-way native medians fall 0.0457→0.0346 and 0.4146→0.3392 seconds, while
input/outer costs remain close. A separate large-AW trace is 0.443→0.363 command
and 0.4176→0.3375 native. Preparation is 0.00787→0.00774; score construction plus
its certificate remains 0.06285→0.06134. This locates most savings after score
construction, consistently with sharing covariance consumers. There is no
standalone hardware/physical-I/O counter, and overlapping timers are not summed.
All non-time solver diagnostics, including fallback/retry decisions, are exact;
no solver fallback is introduced by the bank. High-cardinality rejection cost
is included in its complete-command control.

## Memory and protected costs

Native and scratch peaks are identical on all original and follow-up primary
pairs. Five-way native peaks stay 7,754,448 and 39,112,772 bytes; scratch stays
1,966,080 and 19,621,280. All primary and non-weak control peaks fit their unchanged
models. Sampled RSS is retained separately: small-five-way median rises roughly
0.066→0.070 GiB, while large-five-way falls roughly 0.121→0.100 GiB. No general
RSS saving is claimed.

Nine controls cover small/narrow, one-way, weak one-/two-way, high-cardinality
fallback, fast clustering, no FE and saved FE/residuals. The largest command
slowdown is one-way 254→259 ms (+2.0%, +5 ms); high-cardinality is +1 ms and small
is unchanged. These single pairs are protected diagnostics, not repeatability
claims. Saved outputs and every full scientific export agree exactly.

Weak controls retain a substantial **pre-existing memory-model discrepancy**:
one-way native 29,582,192→29,616,680 bytes versus a 13,460,424-byte model; two-way
29,869,362→31,584,590 versus 13,829,064. Two separate two-way rechecks give
32,240,734→30,196,682 and 29,951,282→32,058,302 bytes, with unchanged/slightly lower
command times. The direction varies and candidate values lie within the observed
baseline range, but the precise source of peak variation is not established.
These are **not successful model-bound checks**. The new two-way bank is only
69,287 bytes within a 245,760-byte ordering allowance; that local storage proof
does not repair the broader P0 accounting gap. All attempts remain archived.

## Independent science and identity

[Validation](validation.json) records thirteen distinct focused Rust tests:
legacy bit-exact clustered meats and scientific/fallible-reservation injection,
a 443-byte bank that rejects a 440-byte budget and admits 448, no row reads on
constructor allocation failure or undersized capacity, all 31 five-way subset
orders, and sparse/overflow fallback. Shared reducers preserve the independent
certificate rather than substituting the production meat for it.

The [licensed oracle](ols_cluster_batch.do) passes both artifacts with terminal
r(0): explicit FE dummies, unit/AW/FW/PW, full/deficient rank, exact sample/order,
and all slope coefficients/covariances. Three independent Stata regressions
supply the one-way subset covariances; their corrections are removed using
independently measured design rank, then combined with the registered common
minimum-cluster correction. Mata independently clips the raw slope covariance
under the accepted intercept/CGM policy, with an eigensystem reconstruction check.

The initial oracle failed on the accepted baseline because it omitted PSD repair
and confused covariance rank (16) with full design rank (18). A second draft used
the wrong eigenvector orientation. Both failed fixtures/diagnostics and their
log hashes remain; the final fixture uses column eigenvectors and unchanged
coefficient/V tolerances. No production scientific rule was altered to pass it.
A prior test launched from main is excluded from candidate evidence.

[Identities](identities.json) bind 150 native input files in each arm. Baseline
plugin `13d35042a36ca5222202e5d3f982fa5b9fa85f2bd8f8a367b845397e043f19e0`
matches the preceding accepted report and source. The final reviewed build is
byte-identical to measured revised plugin
`4c83f9e86568f18420a3d77d285392cdd1f64f6c669e450ac49942df3b41650e`.
Universal static audits and actual arm64 ABI/self-tests pass. Installed dist and
unrelated Veneto state remain intact. Nothing was pushed.

Numeric archives, summaries and checksums are separated into `initial-pilot/`,
`revised-pilot/`, `primary/`, `controls/`, `many-small/`, `many-large/`, `profile/`
and `memory-rechecks/`. Replay the scripts with the existing
`benchmarks/local_fixest_gap` modules on `PYTHONPATH`, an isolated
`FEREG_LOCAL_BENCH_ROOT` and licensed `STATA_MP_PATH`. Raw licensed logs, binaries,
local paths and synthetic datasets remain outside Git.

This adopts the low-memory multiple-subset consumer experiment under P3.
Interaction construction/order reuse, unequal-group scheduling, QR/low-memory
residualization, dense/PPML consumers and the P0 memory-model gaps remain open.
It does not complete P3 or roadmap #27.
