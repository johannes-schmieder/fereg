# Wider retained dense moments: adopted local checkpoint

Adopt [ADR 0057](../../../../docs/adr/0057-wider-retained-dense-moment-tiles.md).
Scaled retained dense moments above 64 active columns now reuse two fixed
64-value tiles inside each existing Gram-row partition. Every Gram/RHS entry
retains its original observation/FMA order, selected mapping, weight normalization
and reference scale. The up-to-64 cache, single-column partitions, unscaled and
scratch-backed sources, one-shot OLS and all rank/solver policies remain unchanged.

The accepted numerical baseline is `9ec47b88c9c78773fa4c8b7cba955ba397ba9ab3`.
Evidence-only `374d257` supplements its control coverage without changing native
source. Both comparison arms include the independent long-request and PSD-report
repairs. The earlier [two-pair probe](../ppml-wide-moments-probe-local-2026-09-13/README.md)
is not pooled with this new evaluation. The original PW K257 warm-up and full
request are used unchanged; no smaller substitute or discarded failing fit.

[Identities](identities.json) bind 176 source/ado/fixture inputs and isolated
universal plugins. Only `ols/finite_wls_kernel.rs` differs, through the exact
[patch](implementation.patch). Baseline plugin is
`411ed113101399adda2c893d0b3e51fa16cbc96e230871ec568e8b346a689126`; candidate is
`d1e8efc290851ace10f917ac3cb630415bfaa975a2da588dead4a85b87fa2abc`.
Core/plugin visibly recompiled for both architectures. Universal static audits
and actual arm64 ABI/self-tests pass; x86 runtime was not executed. Installed
dist was neither used as baseline nor replaced.

## Mechanism and cost

For each observation, a partition prepares at most 64 weighted left-column
values and 64 standardized right-column values, reusing the right tile across
the applicable Gram rows. Partitions wider than a tile continue through more
fixed tiles. This reduces repeated division and checked column reads without
changing the arithmetic order of any output entry.

[Source/shape-derived counts](logical-work.json) for full-rank scaled calls:

| Active columns / workers | Standardizations per observation, before → after | Gram/RHS FMAs, unchanged |
|---|---:|---:|
| 65 / 4 | 2210 → 223 | 2210 |
| 129 / 4 | 8514 → 447 | 8514 |
| 257 / 16 | 33410 → 2329 | 33410 |

Each partition uses **1024 stack payload bytes**, independent of width. There
is no new heap reservation, full observation pass, N-by-K copy, retained cache,
partial Gram bank, worker team or SPI call. Counts are logical operations, not
hardware traffic or instrumented total-fit passes. The full retained design
still exists: this is not P5's proposed centered-tile architecture.

## Complete-command and phase evidence

The original [protocol](protocol.md) and [resume note](resume-protocol.md) fix
three screen targets, two further seeds for all three after a successful screen,
seven controls and one separate private allocation audit. All **17 new pairs /
34 measured commands** finish successfully. Each uses a fresh local M2 Ultra /
Stata 19 process after a smaller untimed warm-up. Arm order rotates. No builds,
tests or statistical sampling overlap timing; native workers are distinct from
Stata's licensed processor count and affinity is uncontrolled. Local development
results are not comparator, cross-platform or qualification claims.

The `recheck` directory prefix denotes the two originally planned repeat seeds;
all three seeds belong to each primary estimate. Medians and median paired
ratios are different statistics and need not agree arithmetically.

| Primary | N / K / workers | Command medians, seconds | Median paired gain |
|---|---|---:|---:|
| Unit, one-FE robust | 122880 / 65 / 4 | 2.308 → 1.678 | 29.2% |
| FW, exposure/missing, one-FE robust | 61440 / 129 / 4 | 4.479 → 3.213 | 28.0% |
| PW, offset/missing, one-FE two-way cluster | 30720 / 257 / 16 | 16.106 → 15.171 | 5.4% |

Every primary pair improves. Finite-solve medians fall
**0.73024→0.30973 / 1.24682→0.40516 / 1.28166→0.75638 seconds**;
strict/final medians fall **0.68008→0.47348 / 1.07424→0.66018 /
1.55524→1.18155**. All targets meet the prospective >=5% and 20 ms command
criterion, with savings in changed finite work. Preparation remains about
0.11 seconds on each target; projection remains close. The PW target's much
larger inference (8.627→8.814 seconds) and publication (3.252→3.332) explain
why its total gain is smaller. That variation is retained in totals, not
attributed to the changed moment kernel. Overlapping phases must not be added.

## Memory, protected costs and science

Native peaks and modeled budgets are exact in all 17 pairs, with every observed
action delta inside its model. Primary action deltas remain 364.49 / 287.97 /
253.09 MB against 431.45 / 347.32 / 431.47 MB models. Raw live peaks, action
deltas, scratch and all phase estimates are separately retained in
[comparisons](comparisons.json) and the [numeric archive](archive.json).
There is no native-memory saving. Sampled RSS moves both ways: primary medians
are 0.352→0.349 / 0.335→0.342 / **0.334→0.381 GiB**. The PW increase is a
retained cost; a fixed stack bound does not prove process-RSS equality.

Protected changes are complete K50 **+89 ms/1.2%**, FW K50 −15 ms,
PW K50 −23 ms, weak two-FE K65 −15 ms, low-memory FW **+30 ms/3.3%**,
small/narrow 0 ms and saved/separated +1 ms. None exceeds max(10 ms, 5%).
These single-pair controls are not causal speed claims. `weak_d3` here means
a degree-three two-FE graph, as the actual worker command records.

The separate private PW K257 audit is **21.923→19.398 seconds (−11.5%)** and
is excluded from public aggregates. Both arms retain **zero allocation events,
zero live/peak growth over 16 audited iterations**, stable PPML/linear/scratch
capacities, fresh/warm equivalence and passing allocation-free certificates.
The [audit](allocation-audit.json) and complete runtime tables preserve its scope.
No allocation-free claim is inferred solely from source inspection.

Every scientific CSV, including full b/V, fit, sample/rank/status/DoF,
weight/cluster/separation tables and independent certificates, is byte-identical
in all 17 pairs. Saved outputs pass the original checks. Discrete solver choices
and work counts agree; fallback, step-halving and strict-profile-rejection counts
are zero throughout. No tolerance, stale-frame rule, QR/rank fallback, omitted
column policy or publication check changes. No fallback-heavy timing claim.

## Focused validation and disposition

[Validation](validation.json) records **62 distinct focused Rust tests**, including
an independently written exact scalar-moment oracle at 1/7/16/63/64/65/128/129/257
columns and 1/4/16 workers. It covers selected/gapped storage, unit/provided
weights, extreme finite scales, scratch/unscaled controls and nonfinite input
failure. Existing retained QR, strict rollback, current-weight norm and generation
invalidation tests pass. The initial `--lib` filters covered 61 distinct tests;
the matching numeric-sample integration oracle was then explicitly run after
all timing, completing the planned set. No failing test or fit was rerun.

The independent full-dummy one-FE Poisson/Hessian/sandwich fixture passes both
artifacts with explicit r(0) and terminal sentinels. The numerical moment oracle
and full-result comparisons complement that small independent estimator oracle;
none substitutes for formal qualification.

P6's bounded width-boundary exercise is evidenced jointly by this finite-column
coverage and the [weighted Schur report](../weighted-schur-local-2026-09-12/README.md),
which measures actual RHS widths 15/16/64/65 including the normalization constant.
These performance crossovers do not change scientific rank thresholds. Other
P6 generated-column/output/low-memory work remains open, as do broader P4/P7
passes, P5 and R1–R5.

Replay with the existing local harness, binding `FEREG_LOCAL_BENCH_ROOT` to
isolated baseline/candidate trees and setting `STATA_MP_PATH`. The archived
`compare.py` takes `screen`, `repeat --name unit_65|fw_129|pw_257`, `controls`
and `audit`; `run_oracles.py` runs both independent fixture arms. Reuse the
recorded original data hashes/recipes; the archive contains numeric exports,
worker copies and input identities. Raw licensed logs, local paths, datasets
and binaries stay outside Git. No agents, SCC, Windows, qualification or push.

The K257 phase costs identify covariance/eigensystem work as a concrete next
investigation. Broader PPML construction/predictor/state and stable centered
one-FE estimation remain separate hypotheses, not gains inferred from these tiles.
