# P2: fresh categorical one-FE projection, adopted locally

Adopt [ADR 0050](../../../../docs/adr/0050-categorical-one-fe-initial-projection.md)
against accepted native `7663a87614e37bea73e5dbc037c3ad0c7afccdda`.
Fast automatic OLS now sends a fresh intercept-only interaction through the
existing simple one-FE initial projector. This removes its raw-input certificate
and enables the existing OLS norm scheduling, without another numerical engine.

## Identity and scope

[Identities](identities.json) bind all 152 native inputs, the two-file
[implementation/test patch](implementation-reviewed.patch), the oracle and both
isolated universal plugins. Baseline is `c9db0dce...`; candidate is
`ea63bae26377af9ae561c03df8acdb95039461c6124258fe8e05a54955960415`.
Both sources use the locked dependencies and Rust 1.85.1. Static binary audits
and actual native arm64 ABI/self-tests pass; x86 runtime was not exercised.
Installed dist is unchanged and was not used for either arm.

The preceding [reporting prototype](../solver-memory-reporting-expanded-local-2026-09-13/README.md)
is excluded from both arms. Its 152 source hashes and exact thirteen-file patch
were verified before reversing only that patch from main. Its immutable report,
patch, frozen source and measured plugin remain available. It is still unadopted
because coverage and weak-graph timing cost remain unresolved; this experiment
does not close P0's reporting gap.

## Coverage and cost audit

| Actual call | Before | After / remaining cost |
|---|---|---|
| Simple one-FE automatic fast OLS, all weights/VCE | Direct fresh-input projection | Unchanged |
| One categorical intercept-only effect, including `g#h`, automatic fast OLS | Ordinary MAP entry, initial raw certificate, projection, final certificate | Existing fresh-input entry, same projection and final certificate; OLS norm scheduling |
| Current within residual / score reconstruction | Ordinary entry can certify and return without projecting | Unchanged; warm zero-iteration behavior tested |
| Explicit MAP/CG/LSMR/CMG, saved-FE retention | Existing requested-family/retention paths | Unchanged; saved and slope controls retained |
| Low-memory raw outcome and column blocks | Ordinary calls with `initial=false`; retained LSMR where eligible | Unchanged; fresh block marking and duplicate norm/copy passes remain candidates |
| Multiple intercept effects / slopes | Existing weighted Schur, borrowed MAP/CG/LSMR/CMG or slope algorithms | Unchanged |
| PPML ordinary/checkpoint/separation calls | Retained engines or explicitly selected canonical MAP | Does not enter this Auto-only shortcut |

For an affected ordinary raw input, initial group certification falls from one
to zero; projection and final independent certification remain one each. Building
the removed group statistics makes two membership-ordered visits to the N-by-P
RHS values (scale and compensated means): nominally `16*N*P` value bytes read,
where P is the actual RHS width. This is a source-derived payload count, not
measured hardware traffic. No full RHS copy is removed by this patch.

Raw and residual weighted norms are still computed. At the existing N>=16384
and P<=64 guard, they use fixed 4096-row scaled-sum-of-squares partitions and
the canonical merge through existing workspace. Smaller/wider cases keep the
ordinary norm implementation. Certificate reduction reuses the existing bounded
OLS partials. Original reference scales and stable rank fallbacks are retained;
large raw cross-products are never subtracted. Raw weight validation and
normalization, topology validation, final sample order and memory admission
are unchanged. All selected solver iteration/pilot/retry/fallback fields match;
no fallback occurs in these captures. Broader discarded-work counters remain P0.

## Complete command, phase and memory evidence

The [prospective protocol](protocol.md) requires >=10% complete-command gain on
an affected primary and investigation of protected increases above max(10ms,5%).
There are **17 pairs / 34 measured commands**: nine unprofiled primary pairs,
seven controls and one separate phase trace. Primary cases rotate three seeds
and arm order. Each arm uses a fresh Stata/MP 19 process on the local M2 Ultra
after a small untimed warm-up. No build/test runs alongside timing. Native
thread requests are distinct from Stata's eight-processor license cap; macOS
affinity is uncontrolled. These are local development findings, not external
comparator, SCC, cross-platform or release claims.

All primary cases absorb `g1#c2`; times are seconds. Paired percentages need not
equal ratios of independently computed medians.

| Primary | N / K / threads | Command medians | Median paired gain | Native phase medians |
|---|---|---:|---:|---:|
| Unit IID | 983040 / 50 / 16 | 0.846 → 0.693 | 18.0% | 0.4650 → 0.3139 |
| AW robust, sparse missing | 245760 / 50 / 16 | 0.310 → 0.274 | 11.3% | 0.2046 → 0.1691 |
| PW two-way cluster | 245760 / 50 / 4 | 0.521 → 0.478 | 8.6% | 0.4126 → 0.3694 |

Every primary pair improves: IID saves 144/156/152 ms, AW 32/35/42 ms,
PW 48/45/38 ms. Initial solver setup/solve medians are respectively
0.05332/0.10792 → 0.00916/0.02474, 0.01514/0.03017 → 0.00276/0.00843,
and 0.01493/0.04172 → 0.00492/0.02037. Input and outer work stay close.
The separate AW trace shows initial absorption 0.06000→0.02004 seconds,
preparation 0.02619→0.02504, and total fit 0.10881→0.07056. Score-design
certification stays 0.02723→0.02749. Do not sum overlapping scopes or medians.

Primary native peaks fall only 48 bytes: effectively unchanged at 948.67,
238.94 and 242.12 MB, within unchanged modeled budgets of 1671.19, 417.81 and
421.74 MB. No observation-sized allocation is added. Process RSS is a distinct
10-ms sampled measure: IID 1.424→1.426 GiB, AW **0.392→0.502 GiB**, and PW
**0.394→0.491 GiB**. The weighted RSS increases are retained costs with no
established attribution; native allocation equality does not establish RSS
equality or a universal resident-memory bound.

Protected command differences are small interaction −1 ms, saved interaction
+1 ms, interacted slope +2 ms, low-memory FW/pool1 0 ms, width65 interaction
−5 ms, weak AW −10 ms, and simple IID −2 ms. Native peaks on all but the weak
control are exact. The unaffected weak route's peak changes 28.52→27.71 MB;
that single observation is not a causal memory/speed claim. Both weak arms
exceed their unchanged 13.03 MB model. The low-memory control is unchanged at
8.77 MB versus a 6.70 MB model, with 4.90 MB scratch. These remain P0 modeling
gaps, not passing memory qualification cells. Full results are in
[comparisons](comparisons.json), [summary](summary.csv) and the checksummed
[numeric archive](archive.json).

## Scientific validation and disposition

All seventeen full b/V comparisons are exact (maximum absolute difference zero).
Samples, status, DoF, FE/cluster/subset tables and non-time solver decisions
are exact. Original rank/FE/score/bread/covariance/determinant certificates pass.
Numerical certificate diagnostics use their original tolerances rather than
being mistaken for discrete decisions. Saved values match under unchanged
tolerances. No tolerance, weight kind, normalization, sample or publication
contract changes.

[Validation](validation.json) records four distinct focused Rust tests, including
the new independent anchored group-mean fixture across widths 1/17/65, unit,
constant, varying and 12-order weights, 1/7 workers and large between-group
values. Residuals are bit-identical across worker counts; warm values return
with zero iterations. Retained and explicit-MAP paths match their ordinary
entry points. Existing categorical frame/recovery and PPML Auto-versus-MAP
identity tests also pass.

The independent [Stata oracle](../../../../tests/stata/development/ols_categorical_one_fe.do)
passes on both isolated artifacts with explicit r(0) and terminal sentinels.
It compares full slope b/V and residuals to `regress` with interaction dummies:
unit/AW/FW/PW, IID where legal, robust and clustered inference, 1/7 workers,
literal frequency expansion, common rescaling, exact collinearity and
FE-absorbed columns. Initial test-scaffold/compiler setup errors and an oracle
assertion using the wrong public MAP code are disclosed in validation; the
corrected science assertions were not weakened. The initially mixed trace
summary was corrected before adoption; all observations remain archived.

Adopt the fast categorical extension. Remaining P2 work includes evaluating
constant-weight specialization and fresh low-memory blocks. P5's centered tiles,
saved-state recovery and broader P3/P4/P6/P7 work remain separate. No agents,
dependency update, cluster jobs, qualification or push occurred. Unrelated
Veneto changes and historical evidence are preserved.

Replay with the existing `benchmarks/local_fixest_gap` modules on `PYTHONPATH`,
`FEREG_LOCAL_BENCH_ROOT` pointing to isolated baseline/candidate trees and
`STATA_MP_PATH` set. The portable scripts change only path discovery relative
to the measured runners. Licensed logs, binaries and datasets remain outside Git.
