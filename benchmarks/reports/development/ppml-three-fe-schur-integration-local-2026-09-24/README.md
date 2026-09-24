# Admitted three-FE Schur checkpoints

Local macOS arm64 development evidence, 2026-09-24. Adopt the current-call
centered Schur checkpoint under [ADR 0163](../../../../docs/adr/0163-admitted-three-fe-schur-checkpoints.md).
This completes production integration of ADR 0162's selected design; the broad
<=0.90-times-comparable-stock goal remains unfinished.

## Exact sources and receipts

Baseline is computational source `f6ed8db6`, artifact `multifeseed1`, plugin
SHA-256 `bbb28cd029cf0eb77e9f8a0b0c206412b663cb2e641fe050395152082f96a744`.
Candidate **`threeschur3`** is `145ab6d2` plus the captured five-file Rust overlay;
all **546 computational inputs** match the resulting worktree. Its plugin hash is
`a8366911510287757fb4644ff47ab04030521991a2067c3964abbf8c52ef4df0`.
The production artifact has no added profiling instrumentation. Each isolated
build has a distinct Cargo target and passes actual-plugin ABI/panic containment.

The pinned, unmodified fixest 0.14.2 archive remains
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Stock calls use `glm.tol=1e-10`, `fixef.tol=1e-11`, matched requested threads,
serial BLAS, robust inference and fitted means. Startup, import, warmup and
exports are outside the command clocks; requested prediction is inside both.
Low mode is native execution mode, not an equal physical memory cap on stock.

The [content-addressed archive](receipts-content-addressed.zip) holds 7,761 logical
receipts, 1,847 distinct payloads and 13,276,512 bytes, SHA-256
`18476514a1f36bb423ee8ca53306db8cb222ae99690747ba43457d788404f355`.
The [restore tool](restore_receipts.py) verifies every logical payload. Complete
baseline and three candidate sources, build identities, unit logs, scientific
fixtures, declarations, public exports, all repetitions, audits and scripts are
preserved. Raw licensed logs and generated data are omitted with hashes;
machine-specific paths are sanitized. Earlier receipts remain immutable.

## Organization and admission

The mathematical selection and same-input stock comparison are in the
[preceding experiment](../ppml-three-fe-schur-prototype-local-2026-09-24/README.md).
Production retains its centered covariance construction, eliminates the largest
partition and independently solves the remaining coefficient system for each RHS.
Reusable indexed group scratch replaces per-group growing vectors. Couplings are
held in one bounded sparse bank; fixed disjoint worker slices bound live Krylov
storage. There is no observation pass inside a reduced matvec, no weighted cache
across calls, and no change to ordinary joint IRLS.

Require q² <= 6N, where q is the total size of the other two partitions. The full
checked storage bound must fit checkpoint LSMR's already admitted transient
allowance. Every reservation is fallible and actual capacities consume that
allowance. The plan charges both reduced matrices, normalized weights, masses,
couplings, offsets, group/worker scratch, column outputs and conversion storage,
without taking credit for sequential lifetimes. Optional refusal retains LSMR
at its previous minimum budget. Configured work limits, finite checks and fatal
errors remain authoritative; the cap is min(configured maximum, 2q).

Acceptance still requires original-input reconstruction and the full fresh
original-weight certificate at the same tolerance. Numerical refusal frees the
candidate before untouched-input LSMR fallback. Failed work and certificates
remain in reporting. Mixed certified low-memory tiles now merge their actual
costs and certificates without treating successful Schur work as a failed LSMR
attempt. Ordinary OLS merging and the control-plane/FFI boundary are unchanged.

The structural gate bounds matrix size and per-iteration work, not every setup
cost. Per-group covariance assembly can be expensive in large dense group
systems; this block does not qualify all such shapes or four-plus FE.

## Development and fresh timings

Development seed **202609347**, three alternating baseline/candidate/stock
rotations. Each row lists all observations in repetition order, milliseconds:

| Case | Baseline | Candidate | Stock | Median candidate/stock |
|---|---|---|---|---:|
| Unbalanced 3FE N23040/K8/T4 | 94,95,96 | 82,80,79 | 84,81,87 | 0.952 |
| Low 3FE N14400/K7/T4 | 92,90,91 | 61,60,60 | 123,120,121 | 0.496 |
| Serial 3FE N12000/K3/T1 | 85,84,82 | 55,57,57 | 194,188,192 | 0.297 |
| Excluded narrow 2FE N12000/K3/T1 | 46,49,45 | 46,46,46 | 38,33,41 | 1.211 |
| Excluded two slope partitions N16320/K6/T4 | 69,68,69 | 69,68,68 | 65,65,66 | 1.046 |

All nine target pairs improve; median reductions are 15.8%, 34.1% and 32.1%.
The earlier prototype's excluded narrow regression does not recur in this block;
the excluded slope median improves one millisecond. This does not imply a stock
win for either case.

Fresh seed **202609375 is consumed/frozen**. Nine cases and five rotations were
declared before collection. The declaration amendment only updates the candidate
source identity after a test-only assertion correction; no fresh result existed
and no case, seed or repetition changed.

| Case | Baseline | Candidate | Stock | Median candidate/stock |
|---|---|---|---|---:|
| Unbalanced 3FE N24960/K10/T4 | 126,123,121,118,118 | 99,94,99,95,92 | 100,100,98,99,101 | 0.950 |
| Low 3FE N15840/K9/T4 | 113,109,109,109,109 | 73,73,72,72,74 | 201,201,200,202,201 | 0.363 |
| Serial 3FE N13200/K4/T1 | 101,98,104,101,98 | 55,60,61,62,61 | 254,256,254,250,270 | 0.240 |
| Sparse 3FE N26880/K12/T8 | 234,221,214,228,223 | 162,165,161,164,157 | 7294,7305,7308,7197,7305 | **Unranked** |
| Excluded narrow 2FE N13200/K4/T1 | 48,48,52,44,48 | 44,46,50,45,50 | 47,46,47,48,42 | 0.979 |
| Excluded disconnected 2FE N22080/K14/T4 | 77,79,79,78,79 | 80,77,79,80,77 | 70,75,75,74,74 | **Unranked** |
| Excluded low 2FE N18000/K28/T8 | 115,118,119,122,116 | 118,115,116,121,118 | 83,78,80,80,81 | 1.475 |
| Excluded weak 2FE N9600/K5/T1 | 41,39,38,39,38 | 35,36,41,41,39 | 761,770,762,771,765 | 0.051 |
| Excluded two slope partitions N17280/K8/T4 | 76,76,75,75,74 | 74,75,76,77,75 | 87,87,88,88,87 | 0.862 |

The four three-FE target medians improve **21.5%, 33.0%, 39.6%, 27.4%** against
the native baseline. All twenty target pairs improve. Sparse native agreement
passes; its stock discrepancy prevents a comparative speed claim. Excluded
native medians improve or tie, but individual paired losses remain: up to +3 ms
in disconnected, wide low and weak controls, and +2 ms in narrow/slopes.

OLS controls reuse the frozen seed374 designs, five native pairs. Categorical
N10080/K8/T4 is baseline 17,17,18,17,18 versus candidate 17,17,16,17,18 ms.
Two slope partitions N10080/K6/T4 are 14,14,15,14,14 versus 15,14,15,15,14 ms.
All scientific exports are exact. Categorical medians tie at 17 ms, and the
earlier 143-ms prototype tail does not recur; its cause remains unexplained.
Slope OLS retains a **one-millisecond median loss**. No universal non-regression
claim follows from this bounded control block.

## Scientific results and failures

All **200 public applications** complete: 45 development, 135 fresh and 20 OLS.
No timeouts, application failures or detected contention occur. All **180
independent PPML public-state checks**, **60 native PPML comparisons** and **ten
OLS comparisons** pass. Native maximum absolute differences are coefficient
1.925e-12, covariance 6.001e-17, means 2.402e-9, likelihood 7.276e-12 and deviance
zero. Non-bitwise PPML exports are expected for the changed recurrence.

Direct candidate-stock comparisons pass 15/15 development and 35/45 fresh.
Every fresh sparse repetition fails the mean envelope: baseline-stock maximum
absolute error 3.611e-7, tolerance ratio 2.556, one observation. Every fresh
disconnected repetition fails the coefficient envelope: baseline-stock maximum
absolute error 2.392e-10, tolerance ratio 1.073, two coordinates. Both also fail
direct candidate-stock comparison. All stock public-state stationarity, span and
objective diagnostics pass; those diagnostics do not adjudicate the discrepancies
or replace the registered pairwise envelopes. Keep both cases unranked. This is
not proof of stock error, and neither comparison is repaired by this checkpoint.

**65 distinct focused Rust tests** pass on the final source, including new
constructed weighted-projection truth, rank-deficient and group-constant designs,
changing/common-scale weights, zero RHS, T1/T4, numerical rejection, configured
caps, thirteen fallible-reservation boundaries and exact budget refusal. All 48
prepared-IRLS tests pass, including strict rank, score, reconstruction, final
Hessian and transaction checks. The unchanged full-width two-FE checkpoint test
also passed on `threeschur2`; it was not repeated after the later test-only
assertion correction. See `unit-identity.json` for exact logs and source scope.

**Four licensed fixtures pass** against `threeschur3`, with explicit process rc,
Stata rc and terminal sentinels: categorical full-dummy Poisson/Hessian/sandwich,
multipartition slope oracle, one-FE dummy oracle, and fixed-process low-memory
joint allocation audit at pool1/T4. They cover unit/FW/PW, robust/clustered,
T1/T4 and saved outputs. The actual allocation window retains zero events/growth,
stable capacities and warm/fresh equivalence. The same four fixtures had passed
on `threeschur2` before its test-only successor build.

Preserved development failures: the first unit underflow test incorrectly
expected an already-invalid normalized weight frame to reach the new solver;
existing input validation rejects it. The initial integration missed mixed
Schur/LSMR low-memory report merging, then passed after the scoped merger fix.
A stale low-memory test expected LSMR even when Schur independently certified.
An existing two-FE fault-injection test also failed on the pinned native baseline:
its accelerator bypassed the injected LSMR boundary. The test seam now reaches
LSMR unless it explicitly requests a preceding Schur rejection; recovery assertions
recognize the restored certified accelerator. No production tolerance or acceptance
requirement changed. A predeclaration script first used a nonexistent local
filename and was corrected before creating the declaration or running fits.

## Memory, residual costs and next step

Modeled selected/scratch bytes and actual scratch bytes stay unchanged. The new
temporary bound is explicitly admitted inside the existing LSMR allowance;
unchanged forecasts are therefore supported by the implementation, unlike the
preceding prototype. First fresh target tracked peaks fall by 1,736,739,
6,717,357, 804,618 and 5,015,474 bytes. Excluded controls' tracked peaks are
unchanged in those first runs. All repetition-level peaks are in the archive.
There is no physical-cap or cross-platform claim.

First fresh strict-final phases fall 38.56→19.89, 46.59→18.82, 42.86→12.10 and
84.42→36.30 ms. Outer counts remain four/four/four/six. The unbalanced ordinary
projection phase is still 31.76 ms; its 95-ms median needs another 5 ms to reach
0.90 times stock. Nominal inner iteration totals increase on all four targets
and are not comparable operations.

Move to **wide low-memory two-FE canonical storage/products and reconstruction**:
its 118-ms median needs another 46 ms to reach the 72-ms target. Its first native
projection and strict-final phases are about 31.71 and 32.26 ms. Use the earlier
scoped current-source attribution to start a frozen-frame representation comparison;
do not spend another long cycle tuning a five-millisecond three-FE residual.
Retain the new three-FE wins as controls. Remaining narrow and development slope
losses, the two unranked comparisons, larger dense-group setup costs and broader
coverage remain explicit work. Do not tune guards on seed375.

Preserve 77 primary, 16 holdout and 111 native-control definitions, static
allocation-inventory drift, constrained-fit/nonretaining-CG failures, and SCC
deferral. No stopped campaign restart, push, release or qualification occurred.
