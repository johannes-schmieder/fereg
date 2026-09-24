# Retained dense moment observation tile: rejected

Local Mac development, 2026-09-24. Under [ADR 0159](../../../../docs/adr/0159-dense-moment-tile-closeout.md),
reject `momenttile1` despite reliable isolated kernel gains. Fresh whole-command
results do not support global adoption. Production remains **`fb617b80`** /
`onefecentereddense5`, including the accepted ADR 0158 preparation improvement.
The <=0.90-times-stock objective remains unfinished.

## Identity and design

Baseline plugin SHA-256:
`887001e476148c757b10362e09bc5ae7349b147d0129ed1ad00e5e3f6dcefbfc`.
Rejected candidate plugin:
`23c8b2b3ddfd9d3793e8a356d42f356a7dfa45e7c07a042e6f56e0aa09cfd060`.
Candidate is `fb617b80` with exactly one changed captured input,
`crates/fereg-core/src/ols/finite_wls_kernel.rs`. All 544 baseline captured inputs
match the restored production worktree. See `closeout-verification.json`.

The existing scaled retained-dense helper for at most 64 active columns caches
eight observations, preserving each entry's observation/FMA order. It reduces
repeated Gram/RHS accumulator traffic. Selected columns, normalized weights,
reference scales, partitioning, QR/rank/inverse checks and strict certificates
are unchanged. Other sources and the helper's existing excluded paths retain
their computation. There is no new routing threshold or cross-frame cache.

The three local arrays occupy 4,224 bytes per active partition instead of 512:
8-by-64 standardized values and two eight-value weight arrays. There is no
new heap reservation or N-by-K copy. The initial declaration conservatively
rounded the payload bound to 4,352 bytes. Existing worker stack headroom applies;
this prototype does not establish physical-cap safety.

## Frozen-input kernel comparison

`kernel-source` contains both helpers and an ignored benchmark test. Each pair
borrows the **same immutable arrays in one binary**; all Gram/RHS entries are
bit-identical. Input recipes, complete source map, executable build/test logs,
seven alternating measured pairs after warmup and all raw times are archived.
Times are milliseconds. This is diagnostic evidence, separate from production
or stock headline timings.

| N/K/T | Baseline median | Candidate median | Ratio |
|---|---:|---:|---:|
| 30720/32/8 | 2.992 | 2.056 | 0.687 |
| 30720/12/4 | 1.431 | 0.958 | 0.669 |
| 30720/3/1 | 0.351 | 0.222 | 0.632 |
| 30720/64/4 | 8.816 | 6.269 | 0.711 |
| 61440/10/4 | 1.377 | 1.110 | 0.806 |
| 16320/6/1 | 0.375 | 0.221 | 0.589 |

The result establishes that accumulator traffic is avoidable kernel work. It
does not establish sufficient end-to-end benefit or identify every source of
whole-command variation.

## Whole public commands

Robust inference plus fitted means; stock unmodified pinned fixest 0.14.2,
archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
`glm.tol=1e-10`, `fixef.tol=1e-11`, matched application threads and serial BLAS.
Generation/import, startup, warmup and exports are outside command timing.
Each three-way rotation alternates arm order. No builds, tests, profiling,
archiving or competing fits accompany headline measurements. All overlap and
monitor checks pass. Every value below is an individual repetition in ms.

Development seed 347:

| Case | Baseline | Candidate | Stock | Median B→C / S |
|---|---|---|---|---|
| One FE N30720/K32/T8 | 92,99,91 | 90,87,90 | 107,121,106 | 92→90 /107 |
| One FE N16320/K12/T4 | 39,41,38 | 39,38,38 | 26,26,27 | 39→38 /26 |
| Unbalanced one FE N30720/K3/T1 | 59,58,55 | 50,56,57 | 29,26,25 | 58→56 /26 |
| Two FE N16320/K12/T4 | 71,71,71 | 70,70,70 | 56,53,52 | 71→70 /53 |
| Two slope partitions N16320/K6/T4 | 68,68,70 | 69,68,68 | 67,61, 63 | 68→68 /63 |

Fresh seed **371**, declared before kernel or public development results:

| Case | Baseline | Candidate | Stock | Median B→C / S |
|---|---|---|---|---|
| One FE N24960/K24/T4 | 67,70,70 | 69,70,71 | 61,60,64 | 70→70 /61 |
| Unbalanced one FE N40320/K16/T8 | 79,81,81 | 76,77,77 | 53,47,52 | 81→77 /52 |
| One FE N18000/K4/T1 | 44,45,40 | 44,45,47 | 20,19,22 | 44→45 /20 |
| Two FE N20160/K12/T4 | 81,77,74 | 79,76,78 | 60,57,56 | 77→78 /57 |
| Two slope partitions N20160/K8/T4 | 86,85,85 | 87,86,88 | 96,95,97 | 85→87 /96 |
| Protected low one FE N24960/K12/T4 | 51,53,52 | 53,51,52 | 32,32,33 | 52→52 /32 |
| Weak two FE N10080/K4/T1 | 43,41,37 | 40,46,43 | 685,688,689 | 41→43 /688 |

Fresh unbalanced paired differences are -3,-4,-4 ms; fresh slope differences
are +1,+1,+3. Weak-case differences are -3,+5,+6. Wide differences are +2,0,+1.
These mixed results reject global adoption; no favorable width/thread guard is
added and no further threshold/partition sweep is pursued. The new seed is
consumed/frozen, including its failures to improve.

The **unchanged accepted baseline**, not the rejected candidate, reaches the
0.90 target on development wide (92/107=0.860), fresh slopes (85/96=0.885) and
fresh weak (41/688=0.060). Other current baseline ratios are 1.148 for fresh
wide, 1.558 for unbalanced, 2.200 for serial, 1.351 for two FE and 1.625 for low.
These are case-specific source-bound measurements, not family-wide wins.
Low versus unconstrained stock is a timing aspiration, not equal physical
memory constraint evidence. The candidate's fresh slope ratio 87/96=0.906
would miss the target that the baseline clears in this block.

Focused OLS controls, baseline/candidate repetitions: one FE N10080/K6/T1
`[13,10,12]/[9,10,12]` ms; two FE N10080/K8/T4 `[18,16,16]/[16,18,17]` ms.
Their medians 12→10 and 16→17 are noisy; one-shot computation is unchanged and
all six scientific export sets are byte-identical. Do not claim universal
timing non-regression from these controls.

## Science, memory and failures

All **120 headline applications** succeed: 45 development PPML, 63 fresh PPML,
12 OLS. All 108 PPML own-state checks, 36 native pairs, 36 baseline-stock pairs
and 36 additional direct candidate-stock comparisons pass. Native coefficients,
covariance and means are byte-identical in all 36 PPML pairs. No current stock
scientific discrepancy or timeout occurs in this set; older large one-FE
stock-mean failures remain unresolved and unranked elsewhere.

**71 distinct Rust tests** pass (25 finite-WLS kernel tests and 46 prepared-IRLS
tests) in the isolated diagnostic tree. Its production source prefix matches
the candidate exactly; it adds only the test-only legacy helper and benchmark.
The independent scalar oracle covers mapped/unused nonfinite columns,
extreme reference/weight scales, cache boundaries 1–257 and T1/T4/T16. Existing
nonfinite and rank/inverse tests remain authoritative. The frozen comparison
is additional exact-arithmetic evidence, not a substitute for that oracle.

The candidate plugin passes actual ABI/panic containment and four licensed
fixtures with explicit zero return codes and terminal sentinels: generated
terms/interaction FE and all outputs; one-FE full dummy Poisson/Hessian and
sandwich; one-FE ordinary-iteration allocation audit; multipartition slope
Poisson/Hessian oracle. Unit/FW/PW and robust/cluster semantics are covered.
Ordinary fixest weights are not used as a frequency-weight inference oracle.

Every native PPML pair has identical tracked/modelled memory and scratch.
The added stack is absent from heap instrumentation. OLS RSS varies both ways,
as recorded, while scratch remains zero. No memory qualification follows.

The first isolated Rust test build omitted the fixture directory and failed
at its existing `include_str!`. The corrected source capture includes the
fixtures and passes; the failure and a subsequent Cargo lock wait are
preserved. No failed build is treated as a test pass. There are no failed
licensed fixtures, failed headline applications or unfinished processes.

## Next boundary

Retain ADR 0158. Do not equate a 19–41% small-kernel improvement with a broad
command win. The prior independent wide profile places about 6.46 ms in final
inference, but has not split that total into score-row access, independent
meat reductions, covariance products, PSD and Wald work. Measure those parts
before selecting a coherent replacement. Existing robust blocks use 16,384
rows, which limits available tasks at these sample sizes; that is a source
observation, not a measured scheduling diagnosis or permission to drop checks.
Common loading/preparation remains a second substantial target. This closeout
does not establish that the remaining gap is unavoidable.

The 77/16/111 frozen definitions, SCC/Linux/physical-cap deferrals, static
allocation drift, constrained-fit/CG issues and other known failures remain
unchanged. No push, release, full qualification or stopped campaign restart.

## Immutable receipts

`receipts-content-addressed.zip`: **4,985 logical files**, **1,389 unique
payloads**, 9, 638,986 bytes. SHA-256:
`6cf721557837d9b7c70757c9c2453da527947d5d7ddf173f0845ee991a98cf46`.
The adjacent digest and restore script verify the container and every logical
payload. The archive contains both full source maps/payloads, diagnostic source,
declarations, build/test/fixture receipts, all text scientific exports and binary identities, direct stock
audits, complete distributions, memory comparisons, rejected patch and restored
source verification. Raw licensed logs and binary datasets are omitted with
hashes; paths and license data are sanitized. Key entries live under
`dense-moment-tile/`: `public-summary.json`, `kernel-summary.json`,
`memory-output-summary.json`, `closeout-verification.json` and the three screen
directories. `restore_receipts.py` accepts a new destination directory.
