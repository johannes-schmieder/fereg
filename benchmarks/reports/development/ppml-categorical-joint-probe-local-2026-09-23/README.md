# Categorical PPML opportunity and frozen joint directions

This is a completed development experiment and implementation-priority decision,
not adoption of a new production solver. Production remains `b9f2acbf`, with all
**537 computational inputs** matching accepted `infscan1`, plugin SHA-256
`b4af5b56185bcd6e13d544e58e1ed90b75db3e71f3ca3c87e98d7a8bdcd5803c`.
No production routing, tolerance, certificate, rank, memory or inference policy
changed. The experiment supports extending the existing joint finite/FE Schur
engine to low categorical PPML before further small inference optimizations.

## Whole public command baseline

Three alternating native/stock rotations, development seed202609335, T4,
robust covariance and fitted means. Stock is unmodified fixest 0.14.2 from
archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11` and serial BLAS. Generation, import,
startup, warmup and exports are outside the public command timer. No compilation,
profiling, archiving or competing fits accompanied the headline screen; its
contention checks pass. Low versus stock is not physically memory-matched.

Times are median [minimum, maximum] ms:

| Design / mode | fereg | Stock | Comparison |
| --- | ---: | ---: | --- |
| Dense three FE, N30720/K6, fast | 187 [180,187] | 247 [246,248] | Pass; ratio 0.757 |
| Same dense design, low | 555 [543,562] | 246 [243,249] | Pass; ratio 2.256 |
| Sparse three FE, N38400/K8, fast | 292 [288,294] | 15977 [15896,16036] | Unranked: fitted means |
| Same sparse design, low | 5345 [5335,5354] | 15823 [15759,15914] | Unranked: fitted means |

All **24 applications** and independently checked public states pass. Six dense
stock pairs pass; six sparse pairs do not. Sparse sample, coefficients,
covariance, likelihood and deviance comparisons pass, but five fitted means
exceed the unchanged envelope. The fast comparison's largest absolute difference
is 1.25964e-6, maximum tolerance ratio 4.892. This report does not adjudicate the
public discrepancy or reclassify these timings as wins.

The dense low command needs **333.6 ms**, or **60.1%**, removed to reach
0.90 times its matched 246-ms stock median. Fast dense is a protection case.

## Identical current-weight frames

An isolated `catfreeze1` artifact captures ordinary IRLS iteration two from
three low-mode fits. It records current Hessian weights, original working
response, retained FE-equivalent controls and categorical topology. Three
licensed capture runs pass process status, Stata return code and sentinel.
Capture plugin SHA-256:
`ea0aefcedc3fb3f13cc9a384757a746be6e3d87eb3d2ef264a0a71c067934a0b`.
This artifact is diagnostic and supplies no headline timings.

Frames are dense N1440/K3, dense N30720/K6 and sparse N38400/K8. A separate
release test binary compares four organizations at T1/T4/T8, tolerances 1e-7
and 1e-12, with five rotating repetitions: **360 native kernel attempts**.

- Fused: current categorical projector over response and controls together,
  followed by finite FWL recovery.
- Blocked: current projector over response separately, then pool-bounded
  controls, followed by the same recovery.
- Joint dense and joint stream: the existing coefficient Schur/PCG owner with
  its full finite-block preconditioner, current-weight QR/coupling preparation,
  reconstruction and independent fresh certificates.

The stream arm uses the actual streamed preparation over an in-memory row
source. It does **not** measure scratch I/O or demonstrate a whole-fit low-memory
bound. Input construction and persistent-owner constructors are outside kernel
timers and reported separately. Required per-call copying/recovery, current
numeric preparation, solves and acceptance are inside. Each repetition changes
the weight-generation identity, forcing current numeric preparation. No
assistant-launched competing build, fit or archive accompanied kernels; no
whole-machine contention monitor was used for these diagnostic runs.

Five stock demeaning/FWL repetitions per frame/thread setting follow the native
block: **45 stock kernels**. Stock uses `tol=1e-11`; native strict uses 1e-12.
Independent output envelopes, rather than equal iteration labels, establish
comparability. These separate diagnostic blocks are not alternating public
candidate/baseline/stock evidence.

Selected strict medians in ms:

| Frame / threads | Current fused | Current blocked | Joint dense | Joint stream | Stock |
| --- | ---: | ---: | ---: | ---: | ---: |
| Dense 1440/3, T1 | 7.843 | 10.884 | 0.701 | 0.702 | 6 |
| Dense 1440/3, T4 | 7.827 | 11.051 | 0.683 | 0.704 | 3 |
| Dense 1440/3, T8 | 7.752 | 11.092 | 0.796 | 0.743 | 3 |
| Dense 30720/6, T1 | 259.662 | 313.343 | 15.513 | 15.425 | 160 |
| Dense 30720/6, T4 | 74.349 | 96.080 | 12.876 | 13.451 | 52 |
| Dense 30720/6, T8 | 65.138 | 86.744 | 12.586 | 13.443 | 28 |
| Sparse 38400/8, T1 | 1730.866* | 2253.123* | 65.424 | 63.828 | 4963* |
| Sparse 38400/8, T4 | 523.274* | 747.555* | 62.032 | 64.099 | 1873* |
| Sparse 38400/8, T8 | 416.442* | 594.275* | 59.611 | 61.531 | 1277* |

`*` Sparse baseline and stock outputs fail the independent residual envelope
below. Retain their raw timings without assigning comparative ranks. Small
stock kernels also have coarse timer resolution. Full distributions, constructors,
phase fields, iteration counts and certificates are in the immutable receipts.
The native binary is
`56d575786379c5c5174ebf0c6fd0f8e4a7e3e35cdf18fcff944df0bb2b90df50`.

The dense T4 blocked-to-stream reduction is about **7.1-fold**. The joint system
avoids independently absorbing every finite control to compute an ordinary
direction. This is evidence for computational reorganization, not a prediction
of a sevenfold public improvement. Canonical rank, projected design and Hessian
construction remain necessary. Raw iteration counts represent different work:
for example, dense T4 blocked reports 76 across its two calls, joint reports 41
Krylov steps. They are not comparable operation counts. Reported zero separate
certificate time in some arms means checking is embedded in solve time, not free.

## Independent science and failures

The small frame uses explicit weighted dummy-design SVD. Larger frames use an
independently assembled, column-standardized sparse design with SciPy LSMR at
1e-14, independent of native FE geometry and the Schur recurrence. All six strict
joint beta/residual comparisons pass the predeclared envelopes. All five strict
arms pass on each dense frame.

On sparse data, strict fused and blocked residuals differ from the oracle by
up to approximately 1.81e-8, and stock by 2.504e-6, exceeding the same residual
envelope. Their beta comparisons pass. Joint dense/stream maximum residual
differences are 1.087e-10 and 7.296e-11 and pass. Production certificates passing
does not erase an independent discrepancy. These frozen results do not by
themselves adjudicate the separate nonlinear public fitted-mean discrepancy.
All ordinary-tolerance residual comparisons fail the strict output envelope;
ordinary frames are inner diagnostics, never publication substitutes.

The oracle controller returns failure after recording all three strict
discrepancies. `verified-experiment.json` verifies the expected successes and
preserved failures; it is not an all-science-pass receipt. First attempts are
also preserved: the native probe supplied K+1 stored columns to a K-admitted
stream owner and correctly failed dimension validation; two stock calls used
invalid empty slope arguments for a categorical frame. Corrected adapters use
K controls and omit absent slope arguments. No solver/tolerance change repaired
these harness errors. The first oracle stopped at a discrepancy; its successor
records all results before returning failure, with unchanged envelopes.

## Memory, scope and decision

At dense T4, the probe's selected joint payload count is 6,858,576 bytes for
dense and 6,561,072 for stream. Sparse T4 is 9,289,488 / 8,917,616 bytes. These
count selected retained FE/certificate/accelerator capacities only, not the
complete owner, fit lifetime, stack, scratch or RSS. They cannot establish a
production budget. Constructor costs and thread dependence remain in receipts.

The coherent next candidate is bounded categorical joint directions using the
existing shared owner, with explicit whole-fit admission and canonical fallback.
It must independently establish current-frame strict Newton acceptance, final
rank/bread/Hessian authority, fresh public performance and excluded-path controls
before adoption. The current fast categorical LSMR route has not been replaced.
No production improvement, allocation-audit repair, OLS non-regression, fresh
seed336 validation or physical-cap result is claimed here.

The implementation plan and resume point live only in `STATUS.md`. Historical
allocation drift, mixed-thread audit failure, constrained-fit/CG issues,
comparator discrepancies and platform deferrals retain their existing scope.

## Immutable receipts

The archive contains **2,605 logical receipts**, **844 unique payloads**, and
**18,451,149 bytes**. SHA-256:
`c8311ae0da61e792c694b15893eb8a00baca46c4c92308f97e073b74b038cfd3`.
Container and every logical receipt were restored and verified. It includes
exact production/capture/probe source snapshots, declarations, all attempts,
frozen synthetic CSV frames, outputs, independent checks and reproduction
recipes. Raw licensed logs, native binaries and generated binary Stata inputs
are excluded; omitted inputs retain hashes. Machine paths are sanitized with
original and archived hashes retained. Earlier reports are unchanged.
