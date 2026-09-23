# Parallel owned-FE projection groups

Local Mac development milestone, 2026-09-23. Baseline `parcert1` matches all
529 computational inputs at `9a1d8db2`. Candidate `parproj1` is that revision
plus the archived projection, MAP, accelerated-MAP and automatic-routing
overlays. All 529 captured inputs match the resulting computational source.
Candidate plugin SHA-256:
`2ed364308f8c7e239e242414621e23cf6b59d1d9a50d6ab0abd09775e419f80a`.
Baseline plugin:
`d7c556d44ec6eeae06b16715d60b5e194be83b91bede821ded1819ce81776439`.

## Cost and implementation

The preceding certificate milestone left generic owned projection sweeps as
the largest profiled phase. [ADR 0118](../../../../docs/adr/0118-parallel-owned-projection-groups.md)
computes independent group coefficients on the installed worker team, then
subtracts fitted values from disjoint rows. Groups keep their original raw
weighted Gram, accumulation order, pivoted elimination and 1e-12 threshold.
Every entry retains its channel FMA order and one subtraction. No numerical
policy, fresh certificate, recurrence, contraction or route changes.

The automatic owned-slope path requires at least 16,384 rows, nonempty RHSs
and more than one installed worker. A fallible coefficient bank and worker
Grams are admitted after the existing geometry/certificate budgets. Refusal
retains serial execution. MAP releases the bank before LSMR/CG; CG admits its
own bank after choosing its affordable context. Earlier peaks remain charged.
No new worker team, output matrix copy or persistent weighted cache is added.
Direct solvers, serial/small fits, categorical borrowed paths and the separate
changing-weight reusable MAP retain their existing execution. Generic calls
use fresh current-call state, never state from an earlier PPML frame.

## Alternating performance and visible losses

Three repetitions rotate baseline/candidate/stock order. Seeds 202609283–288,
anchor/fresh/control cells and the stopping rule were declared before timing.
The candidate is unchanged across all screens. No compilation, profiling,
oracles, archiving or competing fits run beside headline benchmarks. Requested
threads match and BLAS uses one thread. Stock is the pinned, unmodified fixest
0.14.2 reference; source/library identities are archived separately from native
diagnostic artifacts.

Public-command timers include native SPI loading, preparation, estimation,
inference and publication. Data import and diagnostic export are outside
these timers. These cases request no saved outputs; licensed fixtures protect
saved slopes separately. Native low memory is a storage-mode request, not an
equal-budget stock comparison. Command medians below are milliseconds.

| Case (N, K, threads) | Baseline | Candidate | Stock | Paired ratio min / median / max |
| --- | ---: | ---: | ---: | --- |
| Two slope partitions (61440,10,4) | 213 | 174 | 53 | .785 / .817 / .825 |
| Unbalanced three FE, two slope partitions (61440,10,4) | 276 | 210 | 73 | .739 / .757 / .767 |
| Fresh wide serial slope control (92160,30,1) | 1011 | 1011 | 920 | .994 / 1.017 / 1.026 |
| Fresh unbalanced three FE (92160,20,8) | 1648 | 1055 | 472 | .634 / .640 / .672 |
| Fresh low-memory slopes (30720,12,4) | 254 | 209 | 40 | .820 / .829 / .835 |
| Weak native slopes (19200,7,4) | 60 | 52 | unranked | .839 / .847 / .867 |
| AW robust slope control (30720,10,4) | 196 | 161 | — | .809 / .821 / .845 |
| PW clustered slope control (30720,10,4) | 197 | 159 | — | .792 / .803 / .827 |
| Categorical one-FE control (61440,10,4) | 20 | 21 | — | .952 / 1.050 / 1.050 |
| Categorical two-FE control (61440,10,4) | 81 | 80 | — | .988 / .988 / 1.000 |
| Public categorical PPML control (61440,6,4) | 219 | 217 | — | .973 / .991 / 1.005 |
| Explicit MAP slope control (30720,10,4) | 831 | 831 | — | .993 / 1.000 / 1.005 |
| Explicit LSMR slope control (30720,10,4) | 145 | 144 | — | .986 / .986 / 1.029 |

Seven eligible threaded rows gain 15.3–36.0% by paired medians. Stock still
wins all five directly comparable rows. The wide serial control loses 1.7%
by paired median, with +17/+25 ms pairs despite equal aggregate medians.
Categorical one-FE has two +1 ms pairs; PPML includes +1 ms and explicit MAP
and LSMR each include +4 ms. These losses remain visible; no guard, chunk or
tile tuning follows. Small excluded-path gains are not algorithmic claims.
Weighted controls retain their absorbed finite-column omissions. Ordinary
fixest weights are not used as Stata frequency-weight inference oracles.

## Science and boundaries

All 93 headline calls pass without timeout, detected overlap or monitor error.
All 39 native scientific pairs and 52 repeat checks pass. Scientific CSV
exports and every non-clock solver field are exact, including certificates,
iterations, contraction, routing, preconditioner and fallback. All 15 direct
stock headline checks pass.

Three independent N2400 explicit-design pilots cover strong slopes, unbalanced
three FE and a weak positive-DoF barbell. Both native artifacts and stock pass
all three on the predeclared seed. Weak stock covariance maximum error is
4.72e-11 (0.358 times the unchanged envelope); native error is 1.63e-16.
This does not adjudicate the prior seeds' stock covariance failures. Weak
stock timings remain unranked, and disconnected discrepancies and stopped
campaigns remain untouched.

The public pilots are below the scheduling floor. Separate forced-kernel tests
compare against an independent weighted two-column closed-form projection at
1/2/4/8 workers, and compare scalar bits across dependent/zero channels, empty
levels, units from 1e-60 to 1e60, multiple sweeps and RHS widths across 32/64.
Validation-before-mutation, allocation/overflow fallback, exact capacities,
scheduling scope and exact-budget/one-byte-short admission are checked.
CG tests preserve affordable geometry/certificate choices, admit projection
storage only from the remainder and retain prior attempted peaks.

Fifty distinct Rust tests pass: six projection, 16 automatic route/admission,
eight certificate, three MAP, one accelerator and 16 independent FE/slope
oracle tests. Five harness tests and four licensed fixtures pass with explicit
terminal sentinels: multiple slope-only OLS, intercept/slope OLS, PPML saved
slopes and OLS transactionality. Both new release artifacts pass the actual
plugin ABI/panic harness. No optimized Rust unit-test run, full Stata profile
or qualification is claimed. All nine pilot and four diagnostic application
calls also succeed. An initial budget assertion expected the old peak when
the new bank fitted after geometry release; its corrected assertion checks
that additional admission explicitly. The failed initial log is retained.

## Diagnostic phases and memory

Isolated diagnostic builds preserve work counts: the two-slope case has 36/3
initial/final projections and 9/2 certificates; unbalanced three FE has 55/5
and 9/2. All 99 candidate projection calls on full data use four workers.
Candidate initial group work totals 18.7/23.6 ms and row subtraction 9.7/15.6 ms;
total projection timers are 33.5/47.3 ms. Baseline total projection timers
are 122.3/118.6 ms. Baseline also has fine-grained serial group clocks, while
parallel phases use join-level clocks; this asymmetry can inflate the diagnostic
contrast. Performance claims rely on the uninstrumented paired commands above,
not these phase ratios.

Fresh initial certificates stay about 39/50 ms and geometry about 9 ms across
the two absorption calls. Candidate preparation takes 11.9/17.5 ms, initial
absorption 127.9/143.6 ms, final residual absorption 10.7/12.9 ms, finite solve
1.2 ms and robust inference 3.3/3.1 ms. These are diagnostic native laps, not
mutually exclusive whole-command components or a loading/output decomposition.
Loading and publication remain in the public timer; no standalone output-cost
claim is made. Most remaining native time is still inside absorption.

Additional tracked heap on eligible rows, in table order, is 1.966, .178, .488,
.820, .116, .983 and .983 MB, identical across pairs. Excluded paths retain
exact tracked peaks. The admitted component bound is
`8*(L*C*R + T*C*C) + T*sizeof(ProjectionWorker)` beside the existing serial
workspace, geometry and certificate envelope. L/C are maximum effect levels/
channels, R actual RHS width and T installed workers. Worker rows retain a
256-byte stack tile. No new whole-action or constrained-memory guarantee is
claimed. The static allocation audit still exits 1 for documented source drift;
no successor inventory is generated.

## Receipts and next subsystem

`receipts.zip` preserves declared cells/seeds, controllers, source/artifact
identities, sanitized protocols and phase records, independent/exact audits,
build/test logs and fixture receipts. Data, binaries and raw licensed logs are
excluded. Baseline and candidate bindings verify all 529 computational inputs,
with per-entry archive hashes and a separate checksum. There is one candidate
and no post-screen numerical, scheduling or budget-policy revision.

Close this projection experiment. Return to broader public PPML coverage with
a small current-source screen of higher-dimensional/slope and saved-output
costs, where performance coverage remains incomplete. Establish comparability
and profile its own changing-weight lifecycle before choosing another change.
These OLS gains do not authorize reuse across IRLS weight generations. Do not
chase the small serial/control losses or restart stopped campaigns. Static
allocation drift, weighted constrained fits, nonretaining CG span instability,
factor-base provenance and SCC/Linux confirmation remain open. Frozen
77/16/111 definitions are unchanged. This is a local committed milestone,
not completion of the broad goal, push, release or qualification.

Archive verification: 3,374 hashed entries, 4,732,084 bytes, SHA-256
`2c27996c1f8b9620d6cb914662d8f7db5610e67bed0093b67d248a8ffdb3074c`.
