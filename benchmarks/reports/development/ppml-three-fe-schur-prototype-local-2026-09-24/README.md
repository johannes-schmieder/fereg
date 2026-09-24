# Three-FE coefficient checkpoint comparison

Local macOS arm64 development experiment, 2026-09-24. Under
[ADR 0162](../../../../docs/adr/0162-three-fe-schur-checkpoint-prototype.md), select
the centered explicit-Schur organization for production integration. The GS
alternative is rejected. **The prototype is not adopted in production**, and
the whole-public <=0.90-times-stock goal remains unfinished.

## Sources and immutable evidence

Production computational source is `f6ed8db6`, unchanged by documentation commit
`67af142c`. Baseline `multifeseed1` plugin SHA-256:
`bbb28cd029cf0eb77e9f8a0b0c206412b663cb2e641fe050395152082f96a744`.
Current-source capture artifact `threecapture1` is `67af142c` plus the isolated
frame writer, plugin `d0fc1765e648770ca53238c43c205b3aeb727e3bd94a2d75dd2fdbca36f3b263`.
Public prototype `threeprototype1` is `67af142c` plus the captured route/helper,
plugin `edeaaa98ac2a0961883d4b70a5eb3a9385e128a6329a9c308eca476a126ae8f3`.
The builder's instrumentation seam applies this source overlay; there are no
extra profiling clocks in that public artifact. Each plugin's complete source,
identity, build log and ABI/panic-containment receipt is captured.

Three separate release Rust probe builds contain the baseline and one candidate
in the same binary, with distinct Cargo targets and complete source hashes.
They are kernel experiments, not production unit-suite receipts. The pinned
stock fixest 0.14.2 archive hash remains
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
The experiment also records the installed stock native-library hash, source
`demeaning.cpp` and exact R invocation. Stock is unmodified.

The [archive](receipts-content-addressed.zip) contains 5,415 logical receipts,
1,199 distinct payloads and 8,158,727 bytes, SHA-256
`4924a10d1b2e6596184ec732238a225fafe05fce4df421a47954de737488c66e`.
The [restore tool](restore_receipts.py) verified/restored all receipts. These
include the three kernel implementations, public overlay, complete source
snapshots, input declarations, work logs, public exports, independent oracles,
all repetitions and failures. Generated data/output binaries and raw licensed
logs are omitted with hashes; paths are sanitized. No historical receipt changes.

## Identical frozen inputs and computational comparison

Four development designs use seed 202609347: unbalanced fast three FE
N23040/K8/T4; low three FE N14400/K7/T4; fast three FE N12000/K3/T1; sparse
three FE N24580/K10/T4. All four public capture fits pass their explicit rc and
terminal sentinel. Eighteen absorption frames are captured. Select the first
multi-column initial and strict frame per case, giving eight immutable batches.
Run each at T1/T4 with five alternating baseline/candidate pairs.

Preserved controller failures include a missing generator section and a sparse
N24576 remainder that violated the generator's singleton guard; both occurred
before estimation. The sparse declaration was corrected to N24580 before fits.
A follow-up log-name collision and an evidence-summary launch without NumPy also
failed before their intended work and were rerun with corrected launch settings.

The exact captured weights, RHSs, codes and original-input certificate tolerance
are shared by both native solvers. Initial tolerance is 1e-6, strict 1e-12.
Copies are outside the paired call clocks. Candidate preparation, reconstruction
and full canonical certification are inside them. The original RHS is never
overwritten. No headline public runs overlap compilation, profiling, oracle
execution or archiving; their process monitors have no overlaps/errors.

The pinned stock general solver prepares weighted group quantities once, iterates
over FE coefficients, accelerates them, and schedules/stops columns independently.
For three-plus FE it can use an initial accelerated phase, a two-FE phase and
further acceleration. A stock iteration is not a native LSMR step, Schur matvec
or GS sweep. The stock inner probe calls the actual C++ entry point and verifies
its output equals stock `demean` on the same inputs, excluding R topology setup
from its inner clock. It uses the comparable public reference's `fixef.tol=1e-11`.
Thus initial native/stock inner timings do **not** represent equal accuracy.

The first candidate uses coefficient-space GS with Irons–Tuck acceleration.
Group masses and normalized RHS sums are prepared once, independent columns
retain their own stopping state, and the final full certificate stays fresh.
It passes 60/80 calls but reaches its 256-sweep work cap on all twenty sparse
calls. Dense low/narrow strict T4 medians worsen 14.250→31.701 and
7.403→18.700 ms; unbalanced roughly ties 14.806→15.005. Do not tune this probe.

The second candidate eliminates the largest FE partition. Its reduced systems
have 243, 154, 130 and 259 coordinates, versus 23,040, 14,400, 12,000 and 24,580
observations. It builds weighted group couplings and the reduced Schur matrix
once, then uses diagonal scaling and per-column CG without observation scans
inside the recurrence. Original-input reconstruction recovers the eliminated
group coefficients and residuals before the independent canonical certificate.

The first Schur version subtracts large global moment matrices. It passes the
dense cases but rejects the sparse case's tiny negative diagonal. The corrected
version sums group-centered covariance contributions, recognizes exactly constant
group indicators by topology, and never clips a negative diagonal. It passes
**all 80/80 frozen calls**. Extra unidentified FE directions remain in the system;
the independent SVD oracle finds reduced ranks 241, 152, 128 and 243.

Strict checkpoint medians in milliseconds, baseline and corrected candidate from
the same five-pair block:

| Case | T1 baseline | T1 candidate | T4 baseline | T4 candidate | Stock T4 inner, separate block |
|---|---:|---:|---:|---:|---:|
| Unbalanced | 45.790 | 11.141 | 14.930 | 5.009 | 12.9 |
| Low dense | 37.411 | 5.916 | 14.144 | 3.645 | 20.7 |
| Narrow dense | 19.959 | 3.110 | 7.453 | 2.088 | Unranked discrepancy |
| Sparse | 233.779 | 45.215 | 43.619 | 14.758 | Not completed |

Both setup and checking remain measurable. The first strict T4 corrected calls
spend 1.365/0.472/0.446/0.725 ms constructing geometry and
0.719/0.950/0.611/0.982 ms on the full fresh certificate. Those are call-level
observations, not headline phase distributions. Summed column work increases:
baseline LSMR steps are 116/133/79/259; Schur matvecs are 125/183/110/990.
The gain comes from cheaper operators, not fewer nominal iterations. The sparse
baseline includes one selective RHS retry. Raw logs preserve each RHS count and
step; no equal-operation interpretation of these counts is intended.

Independent weighted-design checks explicitly assemble dummy columns. Sparse
direct solving and a separately implemented iterative projection are cross-checked
against dense least squares using SVD after analytic elimination of the first
indicator partition. The latter is authoritative for extra null directions.
Every corrected strict output passes; its largest normalized residual-distance
error is approximately 5.5e-13. The original-weight runtime certificate also
passes independently at 1e-12. Initial results retain their looser registered
projection contract and are not promoted to strict accuracy claims.

Stock's narrow strict inner result has residual-distance error 2.0245e-9, above
this probe's 1e-9 comparison threshold, confirmed by the independent dense solve.
Keep it unranked at this boundary. This is not a public-command discrepancy:
the public fits below all pass their existing envelopes. The sparse stock loop
records 8.0892 seconds per initial serial inner solve, then is intentionally
terminated before its disproportionate 10-by-5 repetition schedule completes.
The -15 process result, first measurement and missing conditions remain visible.
It is not restarted. The current native initial result uses a looser tolerance,
so the large timing difference is not an equal-accuracy win claim.

## Whole-public-command pilot

Three baseline/prototype/stock rotations, seed347, robust inference and fitted
means. Native and stock both include prediction work; startup, data import,
warmup and exports are outside the clocks. Requested threads match, BLAS is
serial, and stock uses `glm.tol=1e-10`, `fixef.tol=1e-11`. Native low mode does
not impose a matching physical memory cap on stock.

| Case | Baseline ms | Prototype ms | Stock ms | Prototype/stock |
|---|---|---|---|---:|
| Unbalanced 3FE N23040/K8/T4 | 96,97,98 | 83,84,84 | 80,87,87 | 0.966 |
| Low 3FE N14400/K7/T4 | 90,88,90 | 61,62,61 | 121,114,120 | 0.508 |
| Serial 3FE N12000/K3/T1 | 82,82,83 | 62,53,55 | 195,196,189 | 0.282 |
| Excluded 2FE N12000/K3/T1 | 41,41,49 | 48,50,48 | 41,39,39 | 1.231 |
| Excluded two slope partitions N16320/K6/T4 | 69,68,67 | 71,68,69 | 65,65,66 | 1.062 |

Target medians improve 13.4%, 32.2% and 32.9%; all nine target pairs improve.
The unbalanced case still needs another 5.7 ms to reach 0.90 times this block's
stock median. First-repetition strict-final phases fall 31.18→17.36,
35.99→15.66 and 32.24→12.02 ms. Outer iterations remain four. The ordinary fast
unbalanced projection phase remains about 24–25 ms, a concrete residual cost
after the checkpoint improvement. The changed inner iteration totals do not
measure comparable work.

The excluded narrow loss triggers a predeclared five-pair follow-up on unchanged
artifacts and inputs. Baseline times are 50,45,44,44,47; candidate
47,52,48,42,50 ms: medians 45→48, paired differences -3,+7,+4,-2,+3. The related
disconnected two-FE control has baseline 71,70,68,69,68 and candidate
68,69,68,67,66: medians 69→68. There is no universal non-regression claim or
attribution of the narrow loss to a specific compiler or operating-system cause.

OLS controls retain exact scientific exports. Categorical times are baseline
18,18,17 versus prototype 18,143,17 ms; the 143-ms outlier has no detected competing
process and is unexplained. Slope OLS is 14,15,14 versus 14,15,15. Preserve both
the tails and small losses; median equality alone does not close this boundary.

## Science, memory and adoption limits

All **77 public applications** finish successfully: 45 pilot, 20 excluded-path
follow-up and 12 OLS controls. All 65 PPML independent sample/stationarity/
model-space/objective checks, 25 native PPML pairs, 15 direct prototype-stock
comparisons and six OLS comparisons pass. Native maximum absolute differences:
coefficients 1.925e-12, covariance 6.001e-17, means 2.402e-9, likelihood and
deviance zero. No tolerance, sample or frozen benchmark definition changed.
There was no new licensed full-dummy Poisson/Hessian fixture or production unit
suite in this selection milestone; those remain required for integration.

The first public target runs show tracked peak reductions of 1,770,617,
5,819,898 and 637,818 bytes. The prototype leaves selected/scratch forecasts
unchanged because it has not yet integrated the additional admission logic.
**That unchanged forecast is not a proof that its new storage is admitted.**
The reduced matrix and construction scratch are quadratic in retained FE
coordinates; sparse couplings, column outputs and concurrent worker banks add
further live storage. Production must charge every overlap, use fallible checked
allocation, bound unsuitable reduced systems and preserve affordable fallback.
No physical-cap or cross-platform claim follows.

The public prototype also needs proper configured work limits, interruption,
fatal-versus-numerical error handling and accounting for rejected attempts.
Disconnected component scaling remains outside its new route. These are explicit
implementation requirements, not permission to weaken certificates. Production
source and package artifacts remain unchanged.

The next milestone is to integrate this selected solver with those boundaries,
then run focused changing/extreme-weight, null-space, budget/failure and independent
Poisson/Hessian tests plus fresh public confirmation and excluded-path controls.
Do not start a third algorithm search or tune the rejected GS probe. No new fresh
seed was consumed. The existing 77/16/111 definitions, static allocation drift,
constrained-fit/CG failures, SCC deferral and earlier unresolved comparisons stay
open. No push, release, stopped campaign restart or broad qualification occurred.
