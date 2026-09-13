# P4 fresh raw reference norms: evaluated and rejected

The standalone prototype consistently reduces projection cost but fails its
prospective complete-command adoption threshold. Preserve the exact experiment;
restore accepted native `6ad8b201b73dcc1e550827a6b713011288292910`.
All 152 accepted native inputs were verified after reversing only this patch.
The selected within-norm reuse from ADR 0053 remains adopted.

## Prototype and identity

Fresh retained construction and the categorical projector separately compute
raw weighted norms of the same N-by-K input. The prototype tests structural and
weight-divisor compatibility before consuming that input. Eligible calls omit
the external norm scan and its two K-sized scratch allocations, then obtain raw
and within norms from the same successful projection. Both output shapes are
validated before writes; unexpected compatibility loss fails closed. The guard
is not authority for previously cached values. No index vector, last-result
accessor, supplied reference cache or observation-sized state is introduced.

Raw means, original reference scaling, the initial certificate, projection,
strict rank and independent final checks remain. Alternative divisors and
multi-FE/low/generic/slope paths retain stable calculation. The earlier rejected
fresh-input certificate shortcut remains absent throughout.

[Identities](identities.json) bind all 152 candidate inputs, the exact
[rejected patch](implementation.patch), locked dependencies and isolated
universal plugins. Baseline is accepted `6ad8b20`, plugin
`ef4f1a61a8f5c240e9a7a6de8b2aef835344ab51206e784d12b68c8480d09a75`;
rejected candidate is
`de024aac43810ef46fd533bd5429f35e0df97df05159f843dd7b127516d5a68e`.
Each baseline input matches the accepted Git revision. The changed candidate
crate visibly compiled for both architectures before measurement. Installed
dist was neither used as baseline nor replaced.

## Prospective command evaluation

The [protocol](protocol.md) requires a primary median paired command gain of
both 2% and 10 ms, with savings in the changed projection/construction scope.
The first PW screen saves 3.0%, authorizing two more seeds for every target.
All **14 pairs / 28 commands** remain: nine primaries and five controls.
Fresh local Stata/MP 19 processes on the M2 Ultra use a small untimed warm-up,
three rotated seeds/arm orders and isolated artifacts. Affinity is uncontrolled.
Native workers differ from Stata's license processor cap. No builds/tests ran
during timing. These are local development results, not comparator claims.

| Public target | N / K / workers | Command medians, seconds | Median paired change |
|---|---|---:|---:|
| Complete one FE, robust | 983040 / 50 / 16 | 9.485 → 9.403 | −0.9% |
| FW, exposure, sparse missing, robust | 245760 / 50 / 4 | 3.212 → 3.173 | −1.8% |
| PW, offset, sparse missing, two-way cluster | 245760 / 50 / 16 | 2.898 → 2.917 | +1.4% |

None reaches the predeclared 2% median paired gain. Ratios of independently
computed medians need not equal median paired changes. No additional seeds were
selected to try to cross the threshold; no private audit was run after rejection.

The changed projection scope improves in all nine primary pairs: complete saves
105/129/112 ms, FW 27/16/28 ms and PW 27/29/25 ms (seeds 104729, 8675309
and 20260819). Complete projection medians fall 1.0202→0.9064 s;
FW 0.4391→0.4142 s and PW 0.3364→0.3097 s. The pass reduction is observable.
Unchanged finite/strict work varies enough that the command gain does not meet
the adoption threshold. In particular, PW's repeat strict/final scopes increase
0.7601→0.8919 and 0.8243→0.8631 s, despite lower projection times. The report
does not assign those differences a speculative cause. Overlapping phase times
and independently computed medians must not be added.

All native action peaks remain exact across each pair and within unchanged
models. Primary sampled RSS medians increase: complete 1.9577→1.9591 GiB,
FW 0.5575→0.6221 GiB and PW 0.5456→0.5771 GiB. Some individual pairs fall;
neither peak nor RSS savings are established.
The [summary](summary.csv) and [comparisons](comparisons.json) retain every phase,
modeled/native/RSS value and solver/retry field. Protected command changes are
weak two FE +6 ms, low FW −31 ms, small one FE +1 ms, saved/separated +1 ms and
explicit strong two-FE MAP 0 ms. No public pair exceeds max(10 ms, 5%).

## Scientific validation and disposition

All fourteen full b/V exports are byte-identical; discrete scientific and solver
decisions are exact. Independent certificates and saved-output comparisons pass.
[Validation](validation.json) records 30 distinct candidate Rust tests in a fresh
source-specific Cargo target, both full-dummy Poisson/Hessian/sandwich Stata
oracle arms with explicit return codes and terminal sentinels, universal static
audits and the actual native arm64 ABI/self-test. No test failure or tolerance
change occurred in this experiment.

The new raw-norm test uses independent hypot accumulation on the original input,
changed values/weights, extreme scales and a warm zero-iteration return. Invalid
reference-output shape, alternate divisor and failed projection leave both
caller buffers unchanged; capacities remain fixed. The accepted fresh-session
rank/status/bread/residual test covers unit/FW/PW and alternate divisors, with
existing stale-rank, strict rollback and generation tests.

The [numeric archive](archive.json) retains all observations and source-bound
drivers. Raw licensed logs, datasets and plugins remain outside Git. The portable
oracle driver reads the Stata executable from `STATA_MP_PATH`; comparison drivers
use existing `benchmarks/local_fixest_gap` helpers and isolated baseline/candidate
trees selected by `FEREG_LOCAL_BENCH_ROOT`.

Reject this standalone change without claiming that raw norm reuse can never
help. It may be reconsidered as part of a demonstrably better construction
layout. The accepted within-norm optimization remains; broader P4 matrix/moment/
predictor and state work, other #27 packages and R1–R5 remain open. Unrelated
Veneto work and historical receipts are preserved. No installed-dist replacement,
agents, SCC, push or qualification occurred.
