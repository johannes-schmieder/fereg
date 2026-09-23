# Joint finite/FE ordinary PPML directions: accepted development milestone

Local macOS arm64 development evidence, September 23, 2026. Accept `jointprod2`,
based on `b2b59f62`, under [ADR 0130](../../../../docs/adr/0130-joint-ppml-slope-directions.md).
All **536 computational inputs** match the measured source. Candidate plugin:
`5ca6e1a81a548aef3c733146f04929b68da849493c35574a0361973a94aa8730`.
Baseline `scrread1` matches accepted production at `7bf3c402`, plugin
`e6d3bd7035556bb77d501b4aa5de2a030002af10068c32b97ce0913e1ba46c4f`.

This milestone implements a different ordinary solve organization, with 6–14%
fresh public gains and a modest memory cost. It does **not** complete the target
of public time at most 0.90 times scientifically comparable pinned stock fixest.
Several current cases still lose substantially. Earlier timings and successes
remain bound to their original artifacts.

## Adopted computation

Use one joint finite/FE block-preconditioned Krylov direction instead of K+1
complete ordinary FE projections and a separate finite inverse. Eliminate the
largest FE partition through existing local rank-aware QR; couple the finite
block directly to the existing FE Schur operator. Reuse prepared weighted FE
geometry under explicit generation authority, recompute all finite products
for each call, and use existing symbolic FE coupling when admitted.

Borrow retained FE-equivalent controls and reuse the owner's normalized column
bank for projected controls. The response representative advances by the change
in original working response; the joint residual never becomes its anchor.
Ordinary output has a distinct type without within-inverse or bread authority.
Fresh FE certificates, original-input finite scores and reconstruction remain
mandatory. Canonical initial rank, strict transaction, final score/objective,
model-space and Hessian construction retain their scientific duties. The ADR
states which redundant ordinary inverse construction is replaced and why.

This is the production successor to the
[frozen comparison](../ppml-joint-slope-frozen-local-2026-09-23/README.md),
which rejected generic global-partition augmentation and selected the direct
finite block. It is not a revival of residual-space extrapolation. The frozen
baseline already beat stock on warm inner frames; public final-state work,
loading and stronger acceptance still matter. Public OLS still needs its within
design/bread for a one-shot fit and is not rerouted by a direction-only gain.

## Public rotations

Stock is unmodified pinned fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched application thread counts and
single-thread BLAS. PPML includes robust inference and saved fitted means, using
the existing comparable normalization. Timers include the whole public command:
loading/preparation, solving, inference and outputs; process startup, import,
warmup and exports are outside. Native low versus ordinary stock is not a
matched-memory comparison.

Each main cell has three rotating baseline/candidate/stock applications. Values
are median [minimum, maximum] milliseconds. The initial preflight refused to
start while another project's Stata process was present. A captured wait ended
before measurement. All 175 eventual applications pass; no timeout, application
failure, detected overlap or monitor error is omitted. No compilation, profiling,
archiving, independent oracle computation or assistant-launched competing fit
ran alongside these headline rotations.

Development seed **202609329**:

| Case | Baseline ms | Candidate ms | Stock ms | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Two slope partitions N30720 K6 T4 | 163 [162,164] | 147 [147,149] | 118 [112,119] | 1.246 |
| Unbalanced three FE N30720 K6 T4 | 166 [164,169] | 153 [153,154] | 118 [115,120] | 1.297 |
| Larger unbalanced three FE N61440 K10 T8 | 372 [362,383] | 335 [331,337] | 227 [226,229] | 1.476 |

Fresh seed **202609330**, case definitions declared before development timing,
with no candidate changes between development and confirmation:

| Case | Baseline ms | Candidate ms | Stock ms | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Two slope partitions N23040 K8 T1 | 176 [173,178] | 163 [158,164] | 290 [290,293] | 0.562 |
| Unbalanced three FE N38400 K6 T4 | 211 [209,212] | 198 [195,200] | 154 [153,155] | 1.286 |
| Wider unbalanced three FE N46080 K12 T8 | 354 [350,358] | 306 [303,311] | 958 [957,961] | 0.319 |
| Multiple slopes N7680 K6 T4 | 60 [60,60] | 54 [53,54] | 60 [60,61] | 0.900 |
| Weak barbell N15360 K6 T4 | 76 [74,76] | 68 [68,70] | 1263 [1253,1269] | 0.054 |
| Disconnected N15360 K6 T4 | 99 [99,101] | 91 [90,93] | 76 [75,78] | 1.197 |

Fresh candidate/baseline median ratios are 0.926, 0.938, 0.864, 0.900, 0.895
and 0.919. Every paired native ratio in these six cells is below one. The
multiple-slope result reaches exactly 0.90 on the rounded public timer medians;
its margin should not be overstated. Current large states and five independent
small pilots pass. This does not adjudicate away earlier multiple-slope
covariance failures or disconnected reference nonconvergence at other inputs.
Default-reference accuracy failures remain separate historical evidence.

Excluded-path controls use fresh seed330 and the same rotations:

| Case | Baseline ms | Candidate ms | Stock ms | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Unbalanced slopes N30720 K6 T4 low | 224 [218,225] | 222 [217,224] | 118 [117,120] | 1.881 |
| Sparse categorical two FE N61440 K10 T4 | 245 [243,254] | 245 [243,251] | 136 [135,137] | 1.801 |
| Unbalanced categorical three FE N30720 K10 T4 | 179 [175,180] | 178 [177,181] | 115 [111,115] | 1.548 |
| Dense categorical three FE N30720 K6 T4 | 206 [204,207] | 203 [202,203] | 242 [242,243] | 0.839 |
| One categorical FE N61440 K10 T4 | 130 [128,130] | 128 [127,129] | 51 [50,54] | 2.510 |

The low slope owner adds only its eight-byte optional descriptor when this joint
path is excluded. The categorical controls retain exactly the tracked native
peaks. No algorithmic improvement is attributed to their small timing movement.
All 15 excluded native scientific-export pairs are exact. One-FE and sparse/
unbalanced categorical PPML remain important losses; dense categorical three FE
clears the current target on this captured case.

OLS native controls cover multipartition slopes, low three-FE categorical,
one-FE full outputs/robust VCE, and low-memory factor interactions. Initial
baseline/candidate medians are 68/70, 65/65, 17/17 and 69/66 ms. Preserve the
small initial slope loss. Five additional identical-input alternating pairs
produce 73 [71,74] / 73 [70,77] ms, with overlapping ranges. Input hashes match
the initial control. All 17 OLS native export pairs are exact. No broad OLS
speed claim follows from these controls. Frozen 77 primary, 16 holdout and 111
native-control definitions are unchanged.

## Cost attribution and remaining budget

These are native phase medians from the public receipts, not a diagnostic build
or a sum asserted to equal the median public timer. On development N61440/K10/T8:

| Phase | Baseline ms | Candidate ms |
| --- | ---: | ---: |
| FE projection | 153.760 | 109.043 |
| Finite solve/check work | 25.274 | 22.051 |
| Strict/final construction | 74.261 | 87.053 |
| Preparation | 33.682 | 33.646 |
| Line search/outer checks | 31.428 | 31.383 |
| Inference | 1.492 | 1.325 |
| Publication | 9.762 | 10.086 |

Outer iterations remain six and fallback counts remain zero. The inner report
changes from 48 to 82 because it now mixes joint Krylov counts with canonical
per-column maxima; these are not comparable operation counts. Algorithmically,
ordinary solving replaces K+1 complete projections with one joint direction;
Z preparation and strict/final K+1 projection remain real work. The predecessor
frozen receipts provide direct operation/kernel comparisons.

The projection envelope falls by 44.7 ms; strict/final work increases by 12.8 ms.
The retained controls are not reprojected to convergence at every ordinary step,
so their later canonical refresh starts farther from a current within design.
That is an algorithmic explanation consistent with the phase measurements, not
a separately instrumented decomposition of every strict call. The fresh K12
case similarly moves projection 171.7 to 114.4 ms and strict/final 64.0 to 81.0 ms.

For the development large case, stock's 227 ms implies a 204.3 ms target versus
335 ms now: another 130.7 ms is needed. Inference at 1.3 ms cannot close that
loss. The next coherent subsystem is shared exact-state strict/final construction,
with explicit sample/design/weight/basis authority and fresh final acceptance,
then a deliberate low-memory adapter. Removing repeated projection, rank/factor
or Hessian construction must be justified at those boundaries; do not tune this
ordinary solver's thresholds to seeds or restart ADR 0128's rejected probes.
The broader one-FE concentration and categorical/inference plan remains active.

## Memory and validation

The additional joint payload is one N-vector, P-by-K cross-products, two K-by-K
banks, five P+K vectors and small finite/elimination workspaces. The original
N-by-RHS bank and retained design are reused. All storage is independently
admitted and included in actual capacities; refusal preserves the prior solver.
There is no low-memory N-by-K adapter hidden in this change.

Development tracked native peak increases are 272,096, 308,192 and 611,072 bytes.
The large peak changes from 57,726,518 to 58,337,590 bytes; its modeled admitted
peak changes from 64,651,196 to 65,262,268 bytes. Fresh increments range from
69,400 to 485,648 bytes. These are local tracked/modeled values, not physical-cap
or cross-platform qualification.

- **52 distinct Rust checks pass**: FE MAP/coefficient/certificate boundaries,
  prepared IRLS, reconstruction and optional-memory admission. One unrelated
  environment-dependent frozen-frame test is explicitly ignored by its harness.
- The analytic independent weighted-design test covers two disconnected
  components, a locally rank-deficient duplicate slope, permuted/scaled finite
  columns, four changing-weight frames, T1/T4, coupling on/off, changed and zero
  responses, a one-step failure, finite singularity, NaN weights, capacity and
  accounting. Failed frames revoke acceptance and geometry reuse authority.
- A session test verifies repeated changed frames against fresh canonical fits,
  strict bread, commit/rollback, subsequent joint solves and canonical recovery.
- **Five final-artifact licensed fixtures pass** with process/Stata rc and terminal
  sentinels: independent multipartition unit/FW/PW robust/cluster Poisson/Hessian,
  the new actual-allocation audit, saved slopes, separation methods and OLS memory
  modes. The 16-iteration audit passes T1/T4 with zero events/growth, stable
  capacities and fresh/warm equivalence. Two earlier `jointprod1` fixture runs
  are preserved separately.
- **Five small independent explicit Poisson/Hessian pilots**, N2400 except
  multiple slopes N1920, pass for both native builds and stock: 15 oracle checks.
  Candidate maxima versus oracle are coefficient 2.740e-13, covariance 6.462e-17,
  fitted mean 4.313e-10. All 141 PPML public states pass independent sample,
  current score, model-space and objective-replay checks. All 64 native pairs
  pass their scientific envelopes; bitwise recurrence equality is not required.
- Actual-plugin ABI/panic checks pass. Early test compilation/helper mismatches
  and the repaired helper's failed run are preserved, not counted as successful
  tests. They preceded the final artifact. No tolerance or design was relaxed.

Static allocation inventory drift, constrained-fit failures, generic nonretaining
CG instability, historical comparator discrepancies and deferred Linux/Windows
qualification remain open. No full qualification, SCC retry, push or release ran.

## Immutable receipts

`receipts-content-addressed.zip` restores **6,723 logical receipts** from 1,907
payloads (18,267,630 bytes), SHA-256:
`9bc10851c2710c2e2cbe152d33c584ac74c8fd1925f398d6d930f926b36a6ddf`.
The verification output is in `verification.txt`.

```bash
python3 restore_receipts.py /new/path/to/restored-receipts
```

The container preserves exact source snapshots/identities for baseline and both
production candidates, build/test/controller receipts, predeclared cells,
rotations, science exports, oracle implementations/results, phase and memory
summaries, failed attempts and the blocked preflight. Large data, binary means,
plugins and raw licensed logs are omitted; data/mean hashes and generation
recipes are retained. Paths are sanitized and original/archived hashes recorded.
`summary.json` contains distributions, every paired ratio and phase/memory data;
`verified-tests.json` lists the distinct successful Rust checks. Receipts are
local development evidence, not qualification sealing or promotion.
