# Parallel fresh owned-FE certificate groups

Local Mac development milestone, 2026-09-23. Baseline `certcache3` matches all
529 computational inputs at `84de9f95`. Candidate `parcert1` is that revision
plus archived `certificate.rs` and `auto.rs` overlays; all 529 captured inputs
match the resulting computational source. Candidate plugin SHA-256:
`d7c556d44ec6eeae06b16715d60b5e194be83b91bede821ded1819ce81776439`.
Baseline plugin:
`21bb7a0d0a093e49e22b03e5923f4b13ab7bef2f727b7ebf4c4d30a640552e82`.

## Cost and implementation

The preceding geometry-cache milestone left fresh span projections and raw
moments as a large serial cost. [ADR 0117](../../../../docs/adr/0117-parallel-owned-certificate-groups.md)
evaluates independent effect-level groups on the existing team when an
automatic owned slope call has admitted geometry, at least 16,384 observations
and more than one installed worker. There is no new team or benchmark-specific
guard. Direct solvers, one-thread calls, smaller inputs and uncached contexts
retain serial evaluation.

Each worker writes disjoint group/RHS results. A sequential merge retains every
scalar contribution in effect/level/RHS order, including the RMS FMA recurrence.
Per-entry span and compensated raw-moment arithmetic, fresh residual norms,
rank, tolerances, checkpoint frequency, iterations, contraction and routing
remain unchanged. No recurrence, residual result, working response, score or
Hessian is cached. Current-weight geometry does not cross absorption calls;
the separate changing-weight reusable MAP implementation is unchanged.

Additional scratch is admitted independently of geometry. It holds at most
`max_effect_levels*(actual_RHS+1)` doubles for one certificate, reusing the bank
between effects and releasing it before return. Allocation/size failure uses
serial certification. Before CG, parallel scratch can be dropped while the
existing geometry cache remains; that cache in turn can be dropped if needed.
LSMR still releases the entire context. Optional work does not displace an
affordable solver, and earlier admitted attempts remain in the reported peak.

Separate diagnostic builds verify unchanged work counts: two-slope case 9/2
initial/final certificates and 36/3 projections; unbalanced three-FE case 8/2
and 55/5. Initial certificate time falls 113→40 ms and 124→43 ms, and final
certificate time 7.0→2.1 ms and 6.6→2.6 ms. Geometry construction remains about
9 ms across the two calls. Parallel group work accounts for 22/29 ms of the
initial certificate; fresh validation/norms and ordered merging remain outside
that group timer. All observed group calls use the requested four-worker team.
Projection sweeps remain approximately 109–116 ms and are now the largest
profiled native phase. These builds never enter headline comparisons.

## Alternating performance and visible losses

Three repetitions rotate baseline/candidate/stock order. Seeds 202609277–282
and the original/fresh/control cells were declared before measurement. The
first two cases establish the current baseline; unchanged candidate code then
runs fresh widths, low memory, weak native and excluded-path controls. No
compilation, profiling, oracles, archiving or competing fits run beside headline
benchmarks. Requested threads match and BLAS uses one thread.

Timers include the public command, native SPI loading, preparation, estimation
and publication; file import and diagnostic export are outside timing. No
saved outputs are requested here; licensed fixtures separately protect saved
slopes. Native low memory is a storage-mode request, not an equal-memory-budget
stock comparison. All times below are command medians in milliseconds.

| Case (N, K, threads) | Baseline | Candidate | Stock | Paired ratio min / median / max |
| --- | ---: | ---: | ---: | --- |
| Two slope partitions (61440,10,4) | 267 | 211 | 61 | .773 / .790 / .793 |
| Unbalanced three FE, two slope partitions (61440,10,4) | 327 | 238 | 75 | .728 / .731 / .760 |
| Fresh wide serial slope control (92160,30,1) | 994 | 985 | 907 | .979 / .992 / 1.019 |
| Fresh unbalanced three FE (92160,20,8) | 1939 | 1498 | 467 | .760 / .773 / .798 |
| Fresh low-memory slopes (30720,12,4) | 292 | 245 | 45 | .833 / .836 / .845 |
| Weak native slopes (19200,7,4) | 72 | 58 | unranked | .806 / .817 / .819 |
| AW robust slope control (30720,10,4) | 236 | 183 | — | .763 / .779 / .783 |
| PW clustered slope control (30720,10,4) | 238 | 185 | — | .773 / .781 / .782 |
| Categorical one-FE control (61440,10,4) | 20 | 20 | — | .950 / 1.000 / 1.111 |
| Categorical two-FE control (61440,10,4) | 79 | 79 | — | .987 / .988 / 1.025 |
| Public categorical PPML control (61440,6,4) | 207 | 207 | — | 1.000 / 1.005 / 1.024 |
| Explicit MAP slope control (30720,10,4) | 800 | 819 | — | 1.011 / 1.019 / 1.024 |
| Explicit LSMR slope control (30720,10,4) | 139 | 139 | — | .979 / 1.000 / 1.000 |

Seven eligible threaded slope rows gain 16.4–26.9% by paired medians. All five
comparable stock rows still favor fixest. Explicit MAP loses 9–19 ms per pair;
wide serial includes +19 ms, PPML +5 ms, and categorical controls +2 ms.
These losses remain in the distribution; no guard or tile tuning follows.
The serial median and categorical/PPML differences are not claimed algorithmic
gains. Weighted controls preserve their absorbed finite-column omissions;
ordinary fixest weights are not treated as Stata inference oracles.

## Science and boundary checks

All 93 headline application calls pass without timeout, detected overlap or
monitor error. All 39 native pairs and 52 repeat checks pass unchanged scientific
envelopes. Scientific CSV exports and every non-clock solver field are exact,
including routes, iterations, preconditioners, fallback, contraction and fresh
certificates. All 15 direct stock headline checks pass.

Three independent N2400 explicit-design pilots cover strong slopes, unbalanced
three FE and the positive-DoF weak barbell. Both native artifacts pass all three.
Stock passes the first two; weak covariance misses two coordinates, maximum
absolute error 1.08998e-10 and 1.02173 times the unchanged envelope. Native
covariance error there is 2.81e-16; exact FE rank is 207 and residual DoF 2187.
The failed stock oracle is retained and weak timing stays native-only. Earlier
weak/disconnected discrepancies and the stopped rank-deficient weak campaign
are not reinterpreted or restarted.

The public small pilots are below the scheduling floor. The parallel kernel
itself is separately checked against an independent closed-form weighted
two-column projection, with duplicate/zero channels and an empty group, at
1/2/4/8 workers. Cross-thread results are bit-identical to the serial certificate.
Additional forced-kernel tests cover RHS widths 0/1/7/33, residual changes,
weights, rank deficiency and channel units from 1e-100 to 1e100. Exact-budget
and one-byte-short tests cover declining scratch without losing geometry,
releasing scratch before CG, releasing geometry when required, and preserving
results, routes and maximum attempted peaks.

Fifty distinct Rust tests pass: 16 automatic route/admission, eight certificate,
three MAP, one accelerator, 16 independent FE/slope oracle tests, plus six
existing parallel-boundary regressions selected by the initial focused filter.
Five harness tests and four licensed fixtures pass with explicit terminal
sentinels: multiple slope-only OLS, intercept/slope OLS, PPML saved slopes and
OLS transactionality. Release artifacts pass the actual-plugin ABI/panic test.
No optimized Rust unit-test run, full Stata profile or qualification is claimed.
All nine pilot and four diagnostic application calls also succeed; the weak
stock scientific failure is distinct from application success.

## Memory, receipts and next step

Tracked heap increases on the seven eligible rows by 1.065, .064, .170, .451,
.064, .532 and .532 MB respectively, consistently across pairs. Other controls
have identical tracked peaks. The component bound is
`8*max_effect_levels*(actual_RHS+1)` bytes, beside the existing admitted geometry
and workspace. Each worker uses the existing 768-byte raw-moment stack tile;
no worker-private observation bank or new thread pool is added. Existing
runtime/stack headroom remains authoritative. This is not a tighter whole-action
memory guarantee. The static allocation audit still exits 1 for source drift;
no successor inventory is generated.

`receipts.zip` preserves declared cells/seeds, immutable controllers, source and
artifact identities, sanitized protocols and phase records, independent audits,
exact solver checks, build/test logs and fixture receipts. Data, binaries and
raw licensed logs are excluded. Baseline and candidate bindings verify all 529
computational inputs each, with per-entry archive hashes and a separate checksum.
There was one candidate and no post-screen numerical or guard revision.

Close parallel-certificate tuning. Next investigate the dominant owned-FE
projection sweeps: independent group coefficient work followed by disjoint row
subtraction, with explicit admission and unchanged raw Gram/pivot/rank and
per-entry arithmetic. Preserve same-weight ownership and fresh certificates.
Do not chase small serial/control losses or enlarge PPML cache lifetimes.
Static allocation drift, weighted constrained fits, nonretaining CG span
instability, factor-base provenance and SCC/Linux confirmation remain open.
Frozen 77/16/111 definitions are unchanged. This is a local implementation
checkpoint, not broad goal completion, release or qualification.

Archive verification: 3,362 hashed entries, 4,699,664 bytes, SHA-256
`9004a9eccd30e3ce54ebb6a9e6c4dfd37993a475198f3d3ba116c5590ac4bb5a`.
