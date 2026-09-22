# Serial slope inference-cache column binding

Retain the bounded numeric-column binding under ADR 0099. Two unchanged
five-pair blocks show AW K10 robust/full gains of 4.4% and 2.9%, and PW K6
robust/full gains of 2.8% and 2.3%. Other weighted cases improve modestly;
unit-weight robust/full is essentially flat in block one and faster in block
two. These gains leave the remaining serial fixest losses open.

The successful serial cached-slope route now binds continuous/intercept
columns once before computing packed inference-cache means. The existing
64-column stack representation removes repeated model/source lookups. Each
mean retains its scale, multiplication order, row order, fixed block boundaries
and compensated merge. Cache layout, fresh score/covariance checks, memory
admission and heap ownership are unchanged. Unsupported column recipes and
widths use the existing evaluator. Threaded, explicit/shared, PPML and strided
routes retain their previous eligibility and arithmetic.

| Serial case | Block 1 candidate/prior | Block 2 candidate/prior |
|---|---:|---:|
| AW K10 robust/full | 0.9560 | 0.9710 |
| AW K10 two-way cluster/full | 0.9931 | 0.9689 |
| PW K6 robust/full | 0.9721 | 0.9766 |
| PW K6 two-way cluster/full | 0.9919 | 0.9917 |
| PW K6 robust/saved | 0.9901 | 0.9662 |
| Unit K10 robust/full | 1.0045 | 0.9679 |
| FW K10 robust/full | 0.9474 | 0.9918 |

Ratios are medians of same-repeat ratios. [All timings](timings.md) preserve
the twelve-cell three-pair screen and both seven-cell five-pair blocks.
The screen includes pinned stock fixest, threaded PW and wide AW controls.
N=245,760 and seed 104729 are unchanged. Ordinary fixest is excluded for FW
inference; its ordinary weights do not implement Stata frequency-weight
inference. AW/unit/FW have an intercept plus one slope; PW has three slopes
without an absorbed intercept. Timings include the full command.

Separate diagnostic fits compare the prior and candidate instrumented builds:

| Cache row pass | Prior | Candidate |
|---|---:|---:|
| AW K10 | 9.860 ms | 5.435 ms |
| PW K6 | 5.765 ms | 3.204 ms |

These single traces show roughly 45% less cache row-loop time. Allocation
and merge costs are small. The actual K50 fit uses the strided cache; its
packed-cache trace belongs to warm-up and is excluded. Diagnostic times do
not rank whole-command performance.

All 245 screen/confirmation calls pass with clean isolation. All 106 native
pairs preserve exact numerical exports, samples and requested saved arrays,
with equal tracked allocation peaks. All 28 non-timing solver fields match,
including routing, iterations and certificate diagnostics. Independent output
reconstruction and normal-equation checks pass. Nine diagnostic calls preserve
exact outputs across three artifacts in each of three cases.

The candidate passes four focused release fixtures in three test invocations:
bitwise cache means/values and identical reservations across all four weight
types, extreme scales, zero columns, intercept handling, sample/width boundaries
and multiple reduction blocks; allocation and nonfinite failures; complete
OLS-only eligibility/full-fit checks; and recovery admission/fallback checks.
Both new plugins pass the actual ABI test.

The prior plugin is `rrc1`, hash
`481f2aecc4a86f77925fc383ab5d0cfa71f7a58aa1b250bceb98f774a532e2f2`.
The retained candidate is `scb1`, hash
`c68c679ecb3790139ece883bfe93da422cae212e338d693891a631d8986e1dc9`.
The archive preserves 254 calls, four complete source identities, controllers,
filtered traces and scientific/isolation/solver audits. Integrity verification
covers 7,103 stored entries, 236 omitted local output arrays and four source
reconstructions. Raw licensed logs and binaries are excluded. The captured
sources include six earlier graph/window drafts that remain separately pending
Linux acceptance. Broader integration evidence must attach to the final source;
this checkpoint does not complete the milestone or authorize a release.
