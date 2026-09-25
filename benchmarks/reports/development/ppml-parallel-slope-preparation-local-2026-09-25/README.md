# Parallel current-weight PPML slope preparation — 2026-09-25

Retain `geoteam1`: prepare independent weighted group QR bases and group-pair
numeric banks on existing workers. Fresh threaded slope medians improve
**7.2–15.0%**. This does not close the broader 0.90-times-stock objective.
[ADR 0184](../../../../docs/adr/0184-parallel-ppml-slope-preparation.md) specifies
ownership, scratch refusal and unchanged scientific authority.

## Source, comparator and timing scope

Accepted baseline `frameado3` has the runtime inputs of 132b69b6 and native
plugin SHA-256
`4a326b8d3b210084620478dfd857ffdca672db5aacfc7acbd82fbc3194b98af8`.
Candidate `geoteam1` captures base 51775b89 plus three explicit overlays:
`fe/map.rs`, `fe/slope_coefficients.rs` and
`fe/slope_coefficients/coupling.rs` under `crates/fereg-core/src`.
All **554** captured runtime inputs match the final worktree; the other 551
match the baseline. Candidate plugin SHA-256 is
`149871641cdce3d36e4551c9b1313352821ae8bce51375177e5f89fe4e3f4e7e`.
The isolated release build passes the actual-plugin ABI/self-test and injected
panic-containment harness.

Stock is pinned unmodified fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with glm.tol=1e-10, fixef.tol=1e-11, matched threads, BLAS1, robust inference
and requested fitted means. Each fresh process performs an untimed warm fit.
The command clock includes native loading, estimation, checks, inference and
outputs; host dataset import precedes it. These are local Mac development
measurements, not cold-start, other-platform or qualification results.
No compilation, profiling, audit, archiving or competing fit accompanies
headline rotations. Every process-isolation check passes.

## Measured diagnosis and change

`jointphases1` profiles the unchanged accepted numerical implementation at
6a1eb064; `geophases1` profiles the candidate. Both are isolated diagnostic
builds. Each runs twice per case with its own alternating uninstrumented science
control. Their clocks are excluded from headline rankings. The tables below
compare separate diagnostic blocks, not paired kernel speed estimates.
Nested spans must not be added as mutually exclusive command phases.

| Actual ordinary work, tuning seed347 | Geometry before / after ms | Pair preparation before / after | FE products before / after | Whole joint boundary before / after |
|---|---:|---:|---:|---:|
|Three FE slopes N61440/K10/T8 fast|9.998 / 1.894|5.869 / 3.250|18.086 / 17.747|68.870 / 57.716|
|Same, low|10.121 / 1.908|5.889 / 3.170|18.022 / 17.892|67.813 / 56.327|
|Two slope partitions N30720/K6/T8|10.915 / 2.518|7.223 / 6.918|1.381 / 1.411|38.296 / 29.932|
|Three FE slopes N30720/K6/T4|6.437 / 1.868|3.096 / 1.718|9.158 / 9.148|38.787 / 32.804|
|Two slope partitions N30720/K6/T1|10.415 / 10.753|6.896 / 6.788|1.357 / 1.370|55.751 / 56.311|

For the large fast fit, constructing projected controls plus the finite Hessian
costs only about 6 ms across ordinary calls. Geometry and FE work were a more
valuable first intervention than rewriting those moment formulas. The large
fit has four ordinary joint calls and 43 FE products; the small two-partition
fit has six calls and 71 products. Counts and numerical outputs stay unchanged.
The remaining large compact operator cost is not removed by this change.

Before solving RHSs, existing worker observation banks serve as QR scratch.
Disjoint groups write their bases directly into the already admitted geometry
bank, preserving each group's arithmetic and rank rule. Recursion divides work
by basis elements and handles empty groups. A complete precheck declines this
route if any group's declared channel block exceeds the worker's N-element
scratch; the original serial scratch remains available. One worker also retains
serial preparation. Pair numeric banks are disjoint and can be prepared in
parallel without a floating-point reduction. Two partitions have only one pair,
which explains their unchanged pair-preparation phase.

No solve, certificate, stopping tolerance or cache-generation rule changes.
Inactive geometry padding can differ; ranks and active basis entries retain
their original meaning and arithmetic. No accepted result reads inactive slots.
Canonical and ordinary PPML calls share the improvement. Categorical PPML and
public OLS retain their existing routes.

## Early public screen

Existing tuning seed202609347, three baseline/candidate/stock rotations.
Times are median [min,max] milliseconds. All comparisons are scientifically
rankable. Losses and ties refer to candidate versus baseline paired observations.

| Case | Baseline | Candidate | Stock | C/B | C/stock | Losses / ties |
|---|---:|---:|---:|---:|---:|---:|
|Large three-FE slopes fast|222 [220,227]|211 [208,213]|231 [227,233]|0.950|0.913|0 / 0|
|Large three-FE slopes low|239 [239,241]|225 [225,226]|232 [231,232]|0.941|0.970|0 / 0|
|Two slope partitions T8|102 [102,104]|93 [92,94]|80 [79,80]|0.912|1.163|0 / 0|
|Smaller three-FE slopes T4|113 [113,115]|104 [103,105]|117 [113,117]|0.920|0.889|0 / 0|
|Two slope partitions T1|149 [146,153]|148 [145,154]|315 [309,320]|0.993|0.470|1 / 0|
|Narrow categorical|36 [36,37]|36 [36,36]|31 [30,31]|1.000|1.161|0 / 2|

There are **15 wins, one loss and two ties** in 18 pairs. All twelve threaded
slope pairs improve. No threshold or numerical correction follows this screen;
the same candidate proceeds to fresh validation.

## Fresh public distributions

Seed202609416, five rotations, source and cases frozen before generation.
Sizes and data differ from fresh415 and from the development block; their timing
claims are not transferable across those blocks. All cases request robust
inference and saved means. Each reported loss/tie count is out of five pairs.

| Case | N/K/T; memory | Baseline ms | Candidate ms | Stock ms | C/B | C/stock | Losses / ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Unbalanced three-FE slopes|69120/10/8; fast|276 [275,278]|256 [251,261]|265 [260,267]|0.928|0.966|0 / 0|
|Unbalanced three-FE slopes|69120/10/8; low|301 [296,303]|279 [273,280]|265 [258,266]|0.927|1.053|0 / 0|
|Two slope partitions|36480/6/8; fast|120 [118,120]|107 [105,109]|84 [83,85]|0.892|1.274|0 / 0|
|Unbalanced three-FE slopes|34560/6/4; fast|127 [126,128]|117 [117,118]|129 [128,130]|0.921|0.907|0 / 0|
|Two slope partitions, serial|36480/6/1; fast|171 [169,172]|172 [170,172]|335 [334,336]|1.006|0.513|3 / 2|
|Narrow categorical|24000/2/4; fast|40 [39,42]|40 [39,42]|31 [28,32]|1.000|1.290|1 / 1|
|Weak slope barbell|14400/4/4; fast|41 [40,41]|38 [38,40]|714 [713,715]|0.927|0.053|0 / 1|
|Disconnected slopes|24960/8/4; low|115 [113,117]|106 [105,109]|138 [133,138]|0.922|0.768|0 / 0|
|Two slopes on each of two partitions|28800/10/8; fast|173 [171,174]|147 [146,148]|188 [181,189]|0.850|0.782|0 / 0|
|Four categorical FE|30000/8/4; low|191 [190,191]|188 [186,191]|481 [477,485]|0.984|0.391|1 / 1|

There are **40 wins, five losses and five ties** in 50 pairs. Threaded slopes
have 34 wins and one tie in 35 pairs. Serial slopes lose 1 ms at the median;
three pairs lose and two tie. The categorical route is unchanged, so its small
timing differences are controls, not a causal speedup claim. Five of ten fresh
medians clear 0.90 stock; scientific discrepancies are not hidden in that count.
All outer/inner iteration and step-halving counts match the baseline.

## OLS controls and independent science

Fresh seed202609417 gives five OLS controls at three pairs each: small serial
two FE, threaded two/three FE, low-memory three FE and two slope partitions.
Baseline/candidate medians are **51/53, 35/34, 52/52, 144/143 and 92/91 ms**.
The small serial control has ranges 50–55 versus 49–57 ms and one losing pair.
Its separately declared seven-pair unchanged-source confirmation uses the exact
same input hash and gives **53 [51,55] / 53 [50,54] ms**, with four losses and
three wins. Preserve both blocks. Across all 22 OLS pairs there are fourteen
wins, six losses and two ties; all scientific CSV exports match exactly.

Validation comprises:

- **17 distinct Rust tests pass**: thirteen MAP tests and four coefficient/joint
  tests. The separate ignored frozen-frame diagnostic is not claimed as run.
  Independent analytic weighted residuals cover deficient/zero/duplicate slopes,
  changing weights, disconnected products, large scales and threads; joint tests
  cover dense, streamed and column-oriented controls and failed-frame revocation.
  The new geometry boundary test adds 32 thread/weight/scratch configurations,
  including empty levels and exact active-basis comparisons.
- **Seven licensed fixture executions pass**, with explicit Stata return codes
  and terminal sentinels: two-/three-partition slope Poisson oracles, categorical
  canonical-product oracle, OLS three-FE oracle, fast allocation audit and two
  fixed-process low allocation audits. Explicit dummy Poisson and separately
  assembled Hessian/sandwich calculations cover unit/FW/PW weights, offsets and
  robust/clustered inference. Ordinary fixest weights are not the FW oracle.
- **244 PPML public states**, **156 baseline comparisons**, **68 direct
  candidate/stock comparisons**, **88 exact native scientific/mean pairs** and
  **22 OLS pairs** pass. Forty PPML states are diagnostic and excluded from
  headline rankings; 204 are headline PPML calls. There are also 44 headline
  OLS calls. No measured application, timeout or isolation failure occurs.

This is proportionate development validation, not a full Rust, frozen-matrix,
physical-cap, SCC, Windows or release qualification campaign.

## Memory and retained limitations

No persistent payload, descriptor, bank or admission formula is added. Existing
serial scratch stays retained for refusal. All 68 headline PPML pairs have exact
tracked allocation peaks and scratch-byte reports. Selected-memory telemetry
differs by -48 to +48 bytes on a few pairs; unchanged-artifact baseline repetitions
also exhibit that 48-byte variation. Do not claim universal bitwise equality of
memory telemetry. Peak RSS is not asserted equal.

The fast T1/T4 and low T1/pool1 and T4/pool2 audits each complete the unchanged
16-iteration warmed window with zero allocator events, zero current growth,
stable PPML/linear capacities and warm/fresh equivalence. Source geometry tests
exercise insufficient worker scratch and retain the serial result. These are
not constrained physical-memory qualification or repairs to the existing static
allocation inventory drift and constrained-fit/nonretaining-CG failures.

Fresh residual savings still needed for 0.90 stock are **17.5 ms** large fast,
**40.5 ms** large low, **31.4 ms** two-slope T8, **0.9 ms** smaller three-FE and
**12.1 ms** narrow categorical. Do not spend a long cycle on the 0.9 ms gap.
The measured compact ordinary FE product still costs about 18 ms on the large
tuning frame, while the small two-partition product costs only 1.4 ms. A bounded
single-RHS worker experiment can help the former. The harder two-partition gap
requires fewer/cheaper complete Poisson steps or canonical/final work, consistent
with the revised coefficient-space nonlinear plan. Do not reopen rejected dense
factors, paired lanes or retained-product layouts. Seeds416/417 are now frozen
validation, not tuning inputs. The original 77/16/111 definitions remain intact.

## Immutable receipts

The content-addressed archive contains **10,794 logical receipts / 2,290 payloads /
18,039,596 bytes**, SHA-256
`7763f4300e53a786471751c8ab1a53f1fb55611cc7b233152ff8a2e37b90c122`.
The sibling verification file and restore script validate every logical hash.
Receipts include declarations, captured controllers/source identities, full
artifact source inputs, phase recipes/protocols, paired exports, audits, native
test logs and sanitized fixture sentinels. Raw datasets, means, binaries and
licensed logs are omitted with hashes retained. Work root is
`target/ppml-joint-moments-20260925`; restored evidence is below its
`restored-receipts` directory. All measured operations are terminal.
