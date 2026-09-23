# Implicit current-weight one-FE PPML products — local development

Accept `groupcanon3` for **low-memory one-intercept-FE PPML** under
[ADR 0139](../../../../docs/adr/0139-implicit-one-fe-ppml-products.md). Current-weight
group centers replace the retained projected scratch matrix; canonical rank,
QR fallback, reconstruction and inference consume original-minus-center rows.
The immutable prepared owner survives through final inference without another
finite-design copy. Fast execution keeps its existing dense representation.

This is a production integration of the
[frozen grouped-frame experiment](../ppml-grouped-canonical-probe-local-2026-09-23/README.md),
not another component-only result. It improves complete low-memory commands,
but **does not complete the target of public PPML <=0.90 times stock fixest**.
All measured one-FE cells still lose to comparable stock. Preserve the small,
serial and excluded-path timing losses below.

## Identities and measurement

Base source: `1da01ceca264344807c96ae24365eb7df51c2260`. Baseline `canontile1`
is the previously accepted production artifact, plugin SHA-256
`5ede5b83f42a4a291e56dab2b27a94c3711197ac3a32d3ef6d64870461a1b205`.
Adopted `groupcanon3` plugin:
`ba667b523455f10fa47be89e1385e928d810797467973d750c42a0749eaddb3e`.
All **541 captured computational inputs** match the adopted source. Source and
build identities for the baseline and all three development artifacts are
preserved. `groupcanon1` was rejected by source review before public execution;
`groupcanon2` is the measured dual-mode prototype, not the adopted routing.

The comparator is pinned, unmodified fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`. Default-reference accuracy failures
remain separate historical evidence. These stock comparisons are positive,
unweighted numeric PPML with matched samples, robust inference and fitted means;
no ordinary fixest-weight result is used as a Stata FW inference oracle.

Each process warms once and then times the complete public command. Data
creation/loading into the application precedes timing; command-side loading,
preparation, estimation, inference and requested output creation are included.
Baseline/candidate/stock order rotates. Decision and fresh blocks use five
rotations; excluded controls use two. No compilation, profiling, archiving or
other fits ran alongside the completed uncontended blocks. These are local Mac
development measurements, not frozen-suite or platform qualification claims.

## Adopted whole-command results

Milliseconds: median [minimum, maximum]. Decision seed 202609335 was already a
development seed. Candidate/stock is the ratio of their medians.

| Case | Baseline | Adopted | Stock | Candidate/stock |
|---|---:|---:|---:|---:|
| Low N30720 K6 T4 | 57 [56,59] | 51 [50,52] | 26 [23,26] | 1.962 |
| Low N61440 K10 T4 | 120 [116,123] | 108 [104,109] | 53 [52,54] | 2.038 |
| Low N30720 K32 T4 | 151 [149,155] | 126 [125,129] | 119 [115,123] | 1.059 |
| Fast N61440 K10 T4, excluded route | 103 [102,107] | 104 [102,105] | 53 [50,56] | 1.962 |
| Low small N3840 K3 T1 | 23 [17,23] | 21 [18,22] | 12 [10,13] | 1.750 |

All fifteen larger low native pairs improve. Median reductions are 10.5%, 10.0%
and 16.6%. The small cell has three gains, one tie and one loss of 5 ms; its
millisecond rounding/spread does not support a precise speed-margin claim.
Fast has two gains, a tie and two losses. Its native plugin median is essentially
unchanged, 87.054/87.051 ms; no fast-route improvement is claimed.

Fresh confirmation was declared before generation, seed **202609341**:

| Low case | Baseline | Adopted | Stock | Candidate/stock |
|---|---:|---:|---:|---:|
| N49152 K12 T4 | 117 [111,118] | 101 [96,105] | 52 [50,54] | 1.942 |
| N16384 K48 T4 | 157 [153,158] | 133 [132,136] | 107 [102,113] | 1.243 |
| Unbalanced N49152 K10 T1 | 128 [122,130] | 124 [121,128] | 75 [73,82] | 1.653 |

The two wider cells improve in all ten pairs, by 13.7% and 15.3% in medians.
Serial unbalanced improves 3.1% in median but loses two pairs by 2 and 3 ms.
This is not a universal timing win. One-FE imbalance describes group occupancy,
not connectivity between multiple FE partitions.

## Prototype disposition and preserved interruptions

`groupcanon2` enabled the representation in both memory modes. Its clean screen
had low medians 56/52, 118/103 and 149/128 ms, but fast narrow 103/105.
Fresh seed 202609339 produced low medians 96/87 (N49152 K8 T8), 161/157
(N61440 K10 T1), 152/128 (unbalanced N61440 K16 T4), and a small-fit loss 18/21.
Fast K65 improved only 175/172. Its narrow fast finite-solve phase increased
while projection savings did not compensate. The adopted routing therefore
retains dense fast moments; it is a storage-based decision, not a shape guard.

Two earlier blocks stopped when the contention monitor detected unrelated Stata
processes: `screen` after 40 completed attempts, and `ols-controls` after 14.
Each has one flagged overlapping attempt. All completed applications returned
success; their numerical exports and missing pairs remain visible in interrupted
audits. None of those 54 attempts is pooled into the tables or clean-block counts.
Successor blocks used new directories and declarations. No application failure
or timeout occurred in a complete block, and no scientific discrepancy was hidden
by excluding a timing.

Development failures are also preserved: the initial integration exposed an
obsolete dense-only profile line-search caller, and source review found a
single-source topology receipt that excluded genuine interaction FEs. Both were
repaired before the relevant accepted tests. Incorrect test-harness expectations
and a compile-only formatting attempt were corrected or reverted; no headline
binary used the reverted source. The frozen earlier indexed-Gram rejection remains
in its original report rather than being reinterpreted here.

## Science, regression controls and memory

The adopted source passes **422 distinct Rust tests**: 387 PPML, 25 finite-moment,
seven retained-session and three new implicit-frame tests. New checks cover
independent scalar group projection, exact known-linear WLS with duplicate and
FE-collinear controls, zero controls, genuine interactions, three changing weight
patterns at scales 1e-200/1/1e200, T1/T4, failed-frame invalidation, selected-column
compaction, owner identity, inference after input-owner release, final drop and
checked memory-size overflow. Existing rank, strict transaction and failure tests
remain active. Production-plugin ABI and panic containment pass.

The final artifact passes **14 licensed runs of 11 distinct fixtures**, including
two budget runs. Independent full-dummy Poisson/Hessian/sandwich checks cover
unit/FW/PW, offset, robust/clustered inference, generated terms and interaction
partitions. Fixed-process allocation audits at pool1/T1, pool1/T4 and pool2/T4
retain zero events/growth, stable capacities and warm/cold equivalence. Separation,
categorical and slope PPML, zero-control analytic group totals, and OLS memory
modes pass. The dual-mode prototype separately passed twelve licensed runs.

Complete blocks contain **328 successful applications**, 312 certified PPML
states, 104 native PPML pairs, 104 baseline/stock pairs and 104 additional direct
candidate/stock checks. Nine independent small Python Poisson/Hessian checks
belong to the prototype pilot. Of these totals, the **adopted source** contributes
160 applications, 144 PPML states, 48 native pairs, 48 baseline/stock pairs and
48 direct stock checks; its independent oracles are the licensed fixtures and
Rust checks above. All checked comparisons pass their unchanged envelopes.

Eight adopted-source OLS control pairs and eight excluded PPML pairs have exact
scientific exports. Two-rotation excluded PPML baseline/candidate/stock medians:

| Case, N30720 K6 T4 | Baseline | Candidate | Stock | Candidate/stock |
|---|---:|---:|---:|---:|
| Dense three-FE, fast | 183 | 187 | 246.5 | 0.759 |
| Dense three-FE, low | 343 | 350.5 | 247 | 1.419 |
| Two slope partitions, low | 156 | 157.5 | 120 | 1.313 |
| Two slope partitions, fast | 129.5 | 130 | 116 | 1.121 |

Keep these small native losses visible: seven pairs lose and one ties. Two
rotations establish a current opportunity screen, not fresh confirmation of a
10% family-wide win. OLS medians are 67.5/69, 68/66, 16/17 and 66/66.5 ms;
the small one-FE case loses 1 ms in both pairs. Thus exact scientific protection
is established, but universal timing non-regression is not claimed. Existing
one-/two-FE OLS and absorbed-slope algorithms were not replaced by this change.

For low N61440/K10/T4, tracked peak delta falls **34,603,573 -> 29,789,542 bytes**
(13.9%); actual retained scratch falls **4,915,200 -> 0**. The conservative modeled
peak rises **43,061,399 -> 44,342,215** because group state is added without
crediting removed buffers. For K32, tracked peak is 30,192,283/28,343,810,
modeled peak 34,492,681/36,220,441 and scratch 7,864,320/0. G-by-K centers may
be substantial when G approaches N; admission is required, not an assumption
that group storage is always cheap.

The one-byte optional-admission refusal oracle reports modeled peaks
1,757,593/1,754,649 and scratch 0/65,536, while both fits match independent
Poisson coefficients and means. This is a logical budget boundary, not an OS
physical-cap test. Existing static allocation-inventory drift, constrained
weighted failures, nonretaining-CG instability and platform deferrals remain open.

## Attribution and remaining work

Existing native telemetry, medians across the five adopted large-low runs:

| Phase, milliseconds | Baseline | Candidate |
|---|---:|---:|
| Complete plugin | 103.470 | 90.054 |
| Preparation | 13.946 | 13.956 |
| FE projection | 12.237 | 4.092 |
| Finite solve | 14.662 | 16.308 |
| Strict/final solve | 25.407 | 20.903 |
| Objective/line search | 9.407 | 9.372 |
| Publication | 11.897 | 10.113 |
| Inference | 1.724 | 1.805 |

These are phase marginals with nested scopes, **not an additive exclusive cost
ledger**. Reported outer/inner work remains 3/4 for this cell and 4/5 for K32.
The gain comes from representation and passes, not fewer outer iterations or
weaker checks. Source inspection and the frozen-frame experiment establish the
removed materialization/scratch work; timings also show the increased finite-row
cost. No instrumented comparator result is presented as stock timing.

At this stock median, the large-low target is **47.7 ms**, still 55.8% below 108.
K32 needs **107.1 ms**, another 15.0% reduction; fresh K48 needs 96.3 versus 133.
The next coherent milestone is shared accepted-state construction through
canonical reconstruction and final publication: reuse compatible current-frame
group/predictor/moment products, eliminate duplicate original-column and residual
reconstruction passes, and bind immutable outcome/prior terms once while keeping
an independent final objective replay. A likelihood-constant cache alone cannot
supply the needed reduction. Obtain exclusive spans before summing possible
savings. Preserve the fresh rank, score, objective, reconstruction and Hessian
acceptance purposes when redesigning intermediate checks.

Then refresh/adapt the multi-FE and slope screen around measured remaining costs.
Fast dense three-FE is promising; low categorical and slope construction remain
opportunities. The existing coefficient Schur/PCG and joint-direction machinery
already applies there. Do not restart residual extrapolation or closed serial
slope tuning. Public-command overhead remains a separate priority for small fits.

## Immutable receipts

`receipts-content-addressed.zip` preserves **13,321 logical receipts** in 2,770
payloads, 22,819,595 bytes, SHA-256
`bc9f2939ed78b28b9b0dc5598fcc7cd808fe6c25478a26ebdcb3d53edaf8087e`.
All logical receipts were restored and verified. Run
`python3 restore_receipts.py NEW_DIRECTORY` to reproduce that verification.

The archive includes declarations, exact source snapshots, build identities,
controllers, complete and interrupted progress, scientific exports/audits,
per-run timings and phases, test logs, sanitized licensed summaries, budget
receipts and rejected source evidence. Data/binary hashes and generation recipes
are retained; data, plugin binaries, credentials, machine paths and raw licensed
logs are excluded. No frozen 77/16/111 definitions changed; no SCC retry, stopped
campaign restart, push, release or qualification was performed.
