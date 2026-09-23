# Disconnected PPML slope-MAP failure reduced

Local Mac development diagnosis, 2026-09-23. The disconnected three-FE,
two-slope-partition failure is reproduced on 935 retained observations. It
occurs in the first linear projection, with every weight exactly one, before
IRLS advances. The independent projection audit confirms slow convergence
and a correctly failed certificate. No production algorithm is changed or
new speedup claimed. This experiment closes at the user's requested pause.
See [ADR 0122](../../../../docs/adr/0122-disconnected-ppml-map-diagnosis.md).

## Identity and bounded screen

Production is `190946a1051000d12de59a06e49eb16930433f65`. All 529 computational
inputs match the previously accepted `ppmlgeom1` artifact, plugin SHA-256
`895b2ae2ce57094a1290255d6e6f02ae15f7e374441e340caf5052d490003875`.
The accepted cache and its performance evidence are unchanged. Its source
overlays remain in the preceding geometry report; this archive includes its
original identity and a fresh complete source comparison.

The predeclared reduction uses seed 202609305, disconnected three categorical
partitions, slopes `g1[z1]` and `g2[z2]`, numeric controls, T4, fast memory,
robust inference and saved means. Outcomes are positive and unweighted.
Exactly one native and one strict stock attempt per case were declared, with
a 20-second target-command timeout. The harness retains its separate startup
allowance. Both warmup datasets equal the full dataset at these small sizes.

| Candidate N / K | Retained N / FE rank | Native result | Strict stock command | Independent stock science |
| --- | --- | --- | ---: | --- |
| 960 / 3 | 935 / 645 | Warmup `r(459)`; no target timer | 1.195 s | All checks pass |
| 1920 / 4 | 1901 / 1117 | Warmup `r(459)`; no target timer | .554 s | Covariance fails three coordinates |

These four completed attempts have no timeout, overlap or monitor error.
The two native application failures precede the target invocation: their
receipt clocks are null, not zero or measured failed-fit times. Stock times
are single diagnostic observations, not distributions or performance rankings.
There is no candidate/baseline timing experiment in this milestone. The stopped
large disconnected timing block from the prior milestone was not resumed.

Stock is unmodified fixest 0.14.2 with the existing public comparator settings
`glm.tol=1e-10, fixef.tol=1e-11`. Archive SHA-256 is
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`;
loaded library SHA-256 is
`bece258f6c6fc3fc47accea45c7b004b0c495d46170d78fa2d6302e66de5fbf9`.
The public native command includes loading/preparation/publication; the stock
timer includes fitting and response prediction. File loading and scientific
exports are outside both timers. BLAS is serial. Builds and audits were
separate from this screen.

## First failing frame and independent projection

An isolated `disconnected_capture1` build changes only `fe/map.rs` relative
to `190946a1`. It records call/check identifiers and, on exhaustion, the actual
weights, original RHSs, final residuals, codes and slope values. Its plugin
SHA-256 is `0da389acbf911ee0dded15038257d19cc2043d72581bf15c213320639a363dc9`.
The archive binds all 529 diagnostic inputs and contains the exact changed
source and transformation. Original identity fields are preserved; the
separate source comparison enumerates the effective diagnostic overlay.

One untimed licensed diagnostic reproduces `r(459)` with the expected fixture
terminal sentinel. This is successful failure reproduction, not successful
estimation. Actual-plugin ABI/panic checks pass for the diagnostic build.
Instrumentation clones the original RHS matrix and writes the failed frame;
it is neither production code nor allocation/performance evidence.

Call zero has 935 rows, five RHSs, levels `[192,131,40]`, channels `[2,2,1]`,
tolerance `1e-6` and iteration limit 16000. All weights are exactly `1.0`.
The fresh certificate infinity norm decreases as follows:

| Symmetric MAP sweeps | Relative infinity norm |
| ---: | ---: |
| 0 | .17972 |
| 256 | .0014574 |
| 1024 | .00064484 |
| 4096 | .00026106 |
| 8192 | .00017829 |
| 16000 | .000086184 |

The final certificate is 86.2 times above tolerance and still contracting.
The evidence identifies slow MAP convergence, not a roundoff plateau or reuse
of stale changing-weight geometry. It does not identify the best remedy or
establish that every disconnected topology fails.

The archived `projection-audit.py` reconstructs all 686 explicit FE columns
from the exact captured frame. Column-equilibrated dense SVD and independently
pivoted QR agree on rank 645 and residuals to `8.31e-14` maximum absolute
error. The retained singular-value condition ratio is 276.95, with a clear
rank gap (`.00680` versus `1.91e-15`). An independent local QR certificate
reproduces the native final infinity/two norms (`8.61836e-5`, `4.52784e-6`).
The independently solved residual's infinity certificate is `1.01e-15`.

The remaining MAP error relative to each original RHS norm is approximately
`[.0002864, .0027133, .0088280, .0172632, 4.12e-15]`. Its component outside the
FE span is below `9.88e-15` of each original norm. Thus the failed residuals
retain substantial FE components; publishing them would be incorrect. The
largest unweighted element error is .35839. No tolerance or iteration limit
was relaxed, and no alternate solver was trialed.

## Poisson and covariance adjudication

The independent explicit-design Poisson oracle converges in seven/eight Newton
iterations. Iterative singleton deletion removes 25/19 observations in one
round; sample hashes are retained. Both stock fits pass independent coefficient,
mean, log-likelihood and deviance comparisons plus fresh public score,
model-space and objective-replay checks. Structural residual DoF are 287/780.
Native fits publish no successful state to compare.

Case 0 also passes covariance, with maximum error `2.52e-12`. Its maximum
mean tolerance ratio is .826, so this is a passing check with a modest margin.
Case 1 fails the unchanged covariance tolerance on three coordinates: maximum
absolute error `5.48511e-10`, maximum tolerance ratio 2.572. This discrepancy
remains visible and the case is scientifically unranked.

`dense-hessian-audit.py` independently residualizes controls against the
current weighted explicit FE matrix using dense SVD and pivoted QR, then forms
the current-Hessian bread and robust `N/(N-1)` sandwich. At the independent
mean, case 1's SVD/QR covariance difference is `5.97e-19`; the existing LSMR
oracle differs from SVD by only `6.84e-16`. The covariance failure persists
when using stock's own published means: maximum error `5.48495e-10`, tolerance
ratio 2.572. Weighted FE rank remains 1117, condition ratio 79.25, with a clear
rank gap. Case 0 passes both versions of this dense check.

This localizes the unresolved comparison to stock inference versus fresh
explicit-design inference, rather than merely differing fitted means or
LSMR oracle error. It does not prove a particular internal stock mechanism.
No stock source instrumentation, tolerance adjustment or additional reference
fit was undertaken after this diagnosis.

## Evidence and stopping boundary

[receipts.zip](receipts.zip) contains 72 manifest entries (878,775 total
uncompressed bytes including the manifest), with original and sanitized entry
hashes. Archive SHA-256:
`4468cedff139071d3d9fc8356399630a5d9b3a35df4016d571d9d7481febcce2`.
It includes the predeclaration, all screen outcomes, controller/data hashes,
independent audits, identities, diagnostic overlay/builder, fixture protocol,
and the bounded synthetic captured frame. Its CSV SHA-256 is
`3302f351f421427986928e62a87758e80a414e6639738f1aa8f985458e16c8a2`.
No raw licensed logs, binaries, DTA datasets or private absolute paths are
included. The projection audit replays directly from the archived frame with
NumPy/SciPy; other audit helpers and the generation controller are preserved.

Production remains unchanged, so no new Rust suite or broad licensed profile
is claimed. The checks here are the four-arm reduced screen, independent
Poisson/sample/public-state audit, dense Hessian adjudication, captured-frame
SVD/QR/certificate audit, one licensed failure-capture fixture and diagnostic
ABI/panic checks. The archive and complete production source match verify.

Work pauses here by user request. The next solver milestone, if resumed, should
predeclare a certified convergence remedy for this bounded frame before any
large timing campaign. It must preserve fresh certificates, rank, memory
admission and failure atomicity. Existing generic nonretaining-CG issues and
ADR 0120's rejected extrapolation remain closed to automatic substitution.
Investigating the stock covariance discrepancy is a separate comparator task.
Seed 202609305 is consumed; 202609306 onward are unused by this experiment.
Frozen 77/16/111 definitions and all previously documented deferrals remain
unchanged. No qualification, remote operation, push or release occurred.
