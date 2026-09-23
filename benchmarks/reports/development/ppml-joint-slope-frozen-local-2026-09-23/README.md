# Frozen joint slope solves: computational comparison and candidate selection

Local macOS arm64 development experiment, September 23, 2026. Reject the generic
augmented-partition organization; select the direct finite-block organization
for a production ordinary-iteration experiment. **Neither prototype is adopted
or routed in production.** Whole-command improvement and the broad 0.90-times-
fixest objective remain unproven. The next required work is the production
adapter, boundary/oracle checks and public candidate/baseline/stock rotations.

## Identities and scope

The accepted baseline is `scrread1`, plugin
`e6d3bd7035556bb77d501b4aa5de2a030002af10068c32b97ce0913e1ba46c4f`,
matching all **534 computational inputs** at `7bf3c402` and the subsequent
documentation checkpoint `36d65a0c`. Production computational inputs remain
unchanged. The isolated diagnostic capture plugin `jointfreeze1`, SHA-256
`4799fde7d15f0391c57796a549d75f1e6e17add662a641ee793bf0b3a6ec7edf`,
adds input capture to that source. Its times are not benchmark results.

Seed **202609329** supplies actual first changing-weight fused response/control
frames from public PPML, weight generation 2. The original FE design and normalized
weights are captured literally. The response is column zero, followed by K
controls; these are the accepted session's warm representatives, not untouched
raw covariates. Their differences from the generated inputs are in the FE span.
Both algorithms and the independent oracle receive exactly those same values.
Older control-only `map-2` frames are not used as Newton responses.

Cases: two intercept-plus-slope partitions N30720/K6; unbalanced three FE with
slopes on two partitions N30720/K6; the larger version N61440/K10; and a small
N2400/K6 two-partition oracle. Five licensed diagnostic captures pass process rc,
Stata rc and terminal sentinel, including an excluded N1200 attempt described
below. These are accepted-baseline captures, not candidate Poisson validation.

Exact isolated sources, compiler output and executable hashes are in
`augmented-identity.json` and `direct-identity.json`. Each snapshot has a distinct
Cargo target and a release/fat-LTO executable. Eight frozen test invocations
(four per candidate) pass. Each invocation runs T1/T4/T8, tolerances 1e-7 and
1e-12, and five alternating baseline/candidate pairs. Inputs and workspaces are
constructed outside the solve clock; owner construction time and selected buffer
payloads are reported separately. Current-weight geometry, numerical coupling,
solving, reconstruction and checks are inside the clock. Controllers run
sequentially with no assistant-launched fit, compilation or archive alongside
the probes. No whole-machine contention monitor was attached to these diagnostic
kernels; monitored public rotations remain required before adoption.

The baseline proxy combines the production K+1 absorption kernel with a small
Gram-based finite regression. It does not execute the complete canonical finite
rank/inverse/publication machinery. These are computational diagnostics, **not
public PPML headline timings or release performance evidence**.

## Algorithms and decision

The accepted kernel already has current-weight rank-aware group geometry,
coefficient-space Schur CG, symbolic topology reuse, independent column stopping,
fast response/control fusion, and fresh residual acceptance. The original
repeated-generic-projection diagnosis is no longer an accurate description of
this path. The remaining ordinary calculation still projects K+1 columns before
finite regression.

The first prototype appends controls as one global slope-only partition and
uses the accepted generic group geometry and coefficient solver on one response.
It reconstructs finite coefficients through the global QR basis after solving.
The organization includes generic global weighted QR, additional group products,
a large global certificate group and a serial one-RHS recurrence. It loses the
baseline's RHS parallelism. On the large T8 case, ordinary proxy median is
**54.568 versus 21.954 ms**; on the two-partition T4 case, **16.561 versus 6.821**.
Some serial unbalanced cases win, but the intended threaded targets lose. Reject
this organization; do not tune guards around it. Phase-level attribution among
its added costs was not separately instrumented, so source-level attribution is
an explanation of required work, not a measured additive decomposition.

The second prototype keeps FE geometry separate from the finite block. With Q0
the eliminated partition, B the remaining local FE bases and Xs the scaled weighted
controls, form Z=(I-Q0 Q0')Xs, G=B'Z and H=Z'Z once for the current problem. Solve

    [ S_FE  G ] [a] = [ B'(I-Q0 Q0')y ]
    [ G'    H ] [b]   [ Z'y              ]

using block-preconditioned CG. The finite preconditioner uses H's small Cholesky
factor; the FE operator is the accepted prepared Schur operator with its existing
observation fallback. Finite coefficients follow directly from b's scaling.
There is no generic global-partition QR or post-solve O(NK²) coefficient recovery.

Recurrence diagnostics schedule reconstruction, never accept a result. Reconstruct
from the original response, original controls and current weighted FE bases;
recompute all FE and finite moments; then run the accepted full fresh FE certificate.
The prototype's finite moment check is an ordinary-direction check, not a substitute
for canonical finite rank, inverse/bread or final PPML acceptance. Production must
retain initial canonical rank and strict/current-weight model-space, finite-solve,
score, objective, reconstruction and final-Hessian requirements. No tolerances or
scientific policy changed in production.

## Direct-block distributions

Total frozen ordinary proxy time at tolerance 1e-7, milliseconds, median [min,max].
Rows use baseline and candidate from the same alternating block.

| Case | Threads | Baseline | Direct block | Candidate/baseline |
| --- | ---: | ---: | ---: | ---: |
| Two slope partitions | 1 | 11.865 [11.768,12.351] | 9.057 [8.867,9.364] | 0.763 |
| Two slope partitions | 4 | 7.035 [6.874,7.181] | 6.279 [6.171,7.179] | 0.893 |
| Two slope partitions | 8 | 6.104 [5.917,6.300] | 5.786 [5.658,5.898] | 0.948 |
| Unbalanced three FE | 1 | 21.492 [21.348,21.966] | 9.700 [9.544,10.132] | 0.451 |
| Unbalanced three FE | 4 | 9.269 [9.201,9.394] | 6.689 [6.529,6.710] | 0.722 |
| Unbalanced three FE | 8 | 6.943 [6.843,7.110] | 6.013 [5.923,6.402] | 0.866 |
| Larger unbalanced | 1 | 71.178 [70.863,74.581] | 27.929 [27.332,28.676] | 0.392 |
| Larger unbalanced | 4 | 29.608 [29.306,30.690] | 17.014 [16.754,17.119] | 0.575 |
| Larger unbalanced | 8 | 23.018 [22.667,23.954] | 15.967 [15.456,16.475] | 0.694 |
| Small oracle | 1 | 1.062 [0.999,1.304] | 0.836 [0.822,1.080] | 0.787 |
| Small oracle | 4 | 0.561 [0.536,0.624] | 0.520 [0.494,0.567] | 0.927 |
| Small oracle | 8 | 0.561 [0.542,0.645] | 0.580 [0.549,0.638] | 1.033 |

Tight tolerance is not uniformly faster: two-partition T4 is **6.978/7.540 ms**
baseline/direct and T8 **6.207/7.241**. The small tight T4/T8 cases also lose.
The larger tight T8 case improves **30.576 to 18.447 ms**. All raw clocks and
iteration reports remain in the receipts; do not extrapolate a universal win
or use these losses to invent a seed-specific threshold.

On larger ordinary T8, median direct finite/coupling setup is 5.890 ms, recurrence
and reconstruction 6.384 ms, and the final fresh FE certificate 0.879 ms. Local
FE geometry and reference normalization precede these measured subphases. These
phase medians are not an additive public-command decomposition. The direct solve
retains nine recurrence iterations here; the baseline maximum per-column count
is also nine, but it solves eleven RHSs. Counts are not equivalent operation units.

## Stock comparison and accuracy

Stock uses the same pinned unmodified fixest 0.14.2 library/archive,
SHA-256 `17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Compare both public `demean` and direct `cpp_demean` on the frozen inputs, with
fixef tolerance 1e-11. The latter excludes repeated R topology preparation and
reports per-column iterations. The matched wrapper successor checks resolved
thread counts explicitly, with OpenMP ceiling eight and single-thread BLAS.
Stock runs form separate diagnostic blocks, not native/stock public rotations.

For the large T8 problem, matched stock `demean` plus finite recovery has median
**52 ms**; direct C++ entry plus recovery has median **50 ms**. Native baseline at
its tighter 1e-12 certificate is 29–31 ms across its two separate paired blocks.
Stock per-column counts are 8 or 9, versus native maximum 16 at the tighter
certificate, but fixest accelerated sweeps and native Schur products do different
work. This supports an important limited conclusion: **the accepted native inner
solver is already competitive on these warm frozen problems**. Remaining public
losses cannot be assigned wholesale to a slow inner recurrence. These inputs,
timers and checks do not establish whole-IRLS parity.

Independent explicit weighted-design checks use dense SVD on N2400 and sparse
LSMR on the three large designs, independently constructing every dummy and
level-by-slope column without native geometry or coupling. All strict baseline,
augmented, direct and stock outputs pass the registered coefficient and residual
envelopes. Direct maximum coefficient discrepancy is **3.19397e-14** and residual
discrepancy **1.46404e-11**. Large oracle LSMR terminates with normal-equation
residuals at approximately 1e-12; small SVD identifies one exact FE gauge null
coordinate. Output vectors, input hashes and full diagnostics are preserved.

All eight loose 1e-7 baseline/direct outputs miss at least one final-publication
numerical envelope. These are scheduled ordinary inner solves, not published
estimates; this evidence is why independent strict final acceptance must remain.
No candidate public Poisson/Hessian, changing-weight lifecycle, weak-connectivity,
rank-boundary, low-memory or OLS integration claim is made by these frozen checks.

## Memory and production boundary

The direct prototype owns a dense N-by-K projected-control bank, an M-by-K
FE/finite coupling bank, two K² banks and coefficient/observation scratch. It is
not a low-memory implementation. Selected numerical payload at larger T8 is
**15,156,176 bytes**, versus **17,301,448** for the batched baseline; at T1 it is
**15,156,176 versus 13,657,080**. These counts exclude common/input/topology and
other owner buffers and are not actual process or estimator peaks. Both owners
are live in the diagnostic process. No physical-cap or allocation qualification
is implied.

For production, reuse the existing slope owner's normalized column workspace
for Z where lifetimes permit, and borrow the retained control representation.
Do not duplicate an active design or FE owner merely to reproduce the prototype.
Introduce fallible bounded storage, independently checked original-input scores,
explicit frame/basis authority, failure invalidation and canonical fallback. Keep
strict/final construction canonical initially. Measure the complete public cost
before adopting routing; the small two-partition margin could disappear there.

## Preserved failed and corrected attempts

- The first N1200 oracle generator has only five firms and emits a modulo-zero
  warning in the existing graph formula. Its successful native capture remains,
  but it is excluded from intended-design evidence. N2400 was declared before
  generation and uses valid ten-firm dimensions. Frozen definitions are unchanged.
- Dense SVD's initial scalar-epsilon cutoff retained the known global FE gauge
  direction (smallest singular value 5.53e-15), creating an unstable oracle.
  The dimension-scaled machine-rank cutoff removes that exact mathematical null
  direction. The initial failure, corrected rank/spectrum and source are retained;
  no native rank or comparison tolerance changed.
- The first standalone stock-wrapper controller set `OMP_NUM_THREADS=1` for
  all nominal thread counts. Those T4/T8 clocks are not matched comparisons.
  The corrected controller verifies the resolved count. The repository public
  harness already sets OpenMP to the cell's thread count and is unaffected.
- Direct snapshot compilation reports two unused prototype helper warnings.
  There are no compile or frozen-test failures. These helpers will not be carried
  into a production implementation.

## Immutable receipts and next action

The restored content-addressed container verifies **2,323 logical receipts**, 695
payloads, 29,437,774 bytes, SHA-256
`9a29de4aded72296438cc7ad8213504b07577717db5449912db950d9652634c3`.
It preserves exact source snapshots, declarations, synthetic frozen frames,
controllers, all successful/failed diagnostics, distributions and independent
oracles. Licensed raw logs, binaries and Stata datasets are omitted; dataset hashes
are retained. Use `restore_receipts.py` with a new destination directory.

Proceed to the direct-block ordinary PPML adapter, preserving canonical initial
rank and strict/final acceptance. Then run public rotations and fresh seed330
confirmation with OLS/categorical PPML controls. Seed329 is consumed; seed330 is
predeclared but ungenerated. Retain broader final-state/inference redesign because
successful frozen inner work alone cannot close the public gap. Static allocation
drift, constrained-fit/CG failures, old reference discrepancies and deferred
platform gates remain open. No production source, public API, frozen definition,
release or remote state changed in this selection milestone.
