# Joint categorical PPML in low memory: local development

Accept `catjoint2`, based on `e196efa3`, under
[ADR 0136](../../../../docs/adr/0136-low-memory-categorical-joint-ppml.md).
All **538 captured computational inputs** match the adopted source. Plugin SHA-256:
`2697e671ce99185f173ad5be28ee0ca32b98e6eb5744103c386046139a8d8736`.
Baseline is accepted `infscan1`, plugin
`b4af5b56185bcd6e13d544e58e1ed90b75db3e71f3ca3c87e98d7a8bdcd5803c`.
The objective remains whole public PPML **<=0.90 times comparable stock per case**;
this milestone does not complete it.

## Computational change and rejected alternatives

Low categorical MAP sessions with multiple effects and finite controls now use
the existing joint finite/FE Schur engine for ordinary directions. Its current-
weight local factors, optional group coupling and block-preconditioned CG avoid
repeatedly constructing the complete within design and inverse in each ordinary
IRLS step. Retained scratch supplies FE-equivalent controls through bounded tiles.
This is the production integration selected by the
[frozen experiment](../ppml-categorical-joint-probe-local-2026-09-23/README.md),
not a new recurrence or a benchmark-specific routing threshold.

Two-/three-FE sessions retain the established borrowed categorical projector for
canonical rank/bread work. One generic owner holds the owned FE design and joint
workspace; the canonical projector borrows immutable prepared membership. Four-
plus FE use the generic coefficient projector at canonical boundaries too.
Fast categorical LSMR, one FE, mu separation, other solver families and public
OLS retain their routes; slope behavior is unchanged. No tolerance, sample,
weight, rank, DoF, separation, certificate or transactional policy changes.

The initial `catjoint1` used the coefficient projector at every categorical phase.
It passed the focused truth checks but failed the existing dynamic-range test:
disconnected components with very unequal masses produced relative predictor
error **7.438e-10**, exceeding its **1e-10** envelope. Its plugin was
`201b3761992b8d922fae552fadc8d640198b5cbc246708d75437ef05b46f9d02`.
The initial PPML test group had 340 passes and this one failure.
A supplemental local projection criterion and a subsequent attempt to separate
its recurrence scheduling from fresh checks both failed to converge. Preserve
those source overlays and logs; neither numerical change remains in production.
Gauge/roundoff amplification is a hypothesis, not measured attribution.
Keeping the existing canonical categorical arithmetic resolves the original
accuracy test without weakening it.

Ordinary joint results carry no inverse or inference authority. Every accepted
direction still has fresh current-weight FE, finite-score and reconstruction
checks. The strict joint Newton witness must agree with the accepted state;
one independent canonical final transaction establishes rank, within design,
finite solve, bread and final-Hessian synchronization. Weight identity, basis
checks, numerical refusal, canonical retry and rollback remain authoritative.

## Public timing distributions

Times are **median [minimum, maximum] ms**, with robust inference and requested
fitted means. Generation, import, startup, warmup and receipt exports are outside
the clock; the public estimation, requested outputs and publication are inside.
Stock is unmodified pinned fixest **0.14.2**, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched application threads, serial BLAS
and the established robust N/(N-1) normalization. Low versus ordinary stock is
**not physically memory-matched**.

Five alternating rotations, development seed335, all low mode:

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Dense three FE N30720 K6 T4 | 559 [553,589] | 350 [342,361] | 245 [242,251] | 1.429 |
| Sparse three FE N38400 K8 T4 | 5375 [5336,5449] | 2049 [2032,2107] | 15782 [15761,15833] | Unranked |
| Strong two FE N30720 K6 T4 | 165 [163,172] | 131 [129,132] | 54 [52,59] | 2.426 |

Native reductions are **37.4%, 61.9%, 20.6%**; every native pair improves.
The sparse stock comparison fails five fitted means in every rotation, for both
baseline/stock and direct candidate/stock comparisons. Its timing is retained
descriptively and receives no performance ranking.

Three alternating rotations, **predeclared fresh seed336**, all low mode:

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Dense three FE N46080 K8 T8 | 3108 [3050,3168] | 1750 [1703,1851] | 10633 [10631,10640] | 0.165 |
| Sparse three FE N30720 K4 T1 | 7945 [7945,7963] | 2557 [2548,2562] | 19393 [19385,19396] | 0.132 |
| Unbalanced three FE N61440 K16 T8 | 762 [762,778] | 583 [578,589] | 252 [249,254] | 2.313 |
| Disconnected two FE N23040 K8 T4 | 140 [137,145] | 118 [116,120] | 61 [56,62] | 1.934 |
| Weak barbell two FE N7680 K4 T4 | 60 [59,61] | 42 [41,42] | 212 [212,215] | 0.198 |
| Four FE N15360 K6 T4 | 3670 [3656,3674] | 83 [82,85] | 169 [167,171] | 0.491 |

All fresh native and stock scientific comparisons pass. Native median reductions
are **43.7%, 67.8%, 23.5%, 15.7%, 30.0%, 97.7%**; all 18 native pairs improve,
with reductions spanning **12.4–97.8%**. Four fresh cells clear 0.90 times stock;
two still lose. Seed336 is consumed validation, not an unused tuning seed.

The public harness detects no competing process or monitoring error in any
headline attempt. No compilation, profiling, archiving or assistant-launched
competing fit accompanied these timings. The later trace and numerical
adjudication are separate diagnostics and cannot supply headline timings.

## Excluded paths and small-fit follow-up

Two native pairs per initial control, seed335:

| Control | Baseline | Candidate |
| --- | ---: | ---: |
| Fast categorical three-FE PPML | 198 [196,200] | 197.5 [194,201] |
| Fast one-FE PPML | 109 [107,111] | 104.5 [104,105] |
| Low two-partition slope PPML | 158.5 [158,159] | 158 [157,159] |
| Fast two-partition slope PPML | 131.5 [131,132] | 131 [131,131] |
| OLS slopes | 67 [67,67] | 69.5 [67,72] |
| OLS low categorical | 64 [64,64] | 68 [67,69] |
| OLS one-FE robust/full outputs | 17.5 [17,18] | 17 [17,17] |
| OLS low generated factors | 73.5 [67,80] | 69 [68,70] |

All scientific exports are byte-identical. The two OLS median losses received
five **predeclared identical-input** follow-up pairs, with no seed/cell change:
slopes **71 [69,72] -> 70 [70,73]**, low categorical
**71 [67,75] -> 71 [68,76]**. Individual losses remain. Initial and follow-up
distributions remain separate; no universal timing non-regression claim follows.

The independent-oracle pilot's small two-FE case initially took 17/21/15 ms
for baseline/candidate/stock. Its five-rotation identical-input follow-up is
**17 [17,18] / 17 [16,18] / 15 [14,15] ms**, candidate/stock **1.133**.
All scientific and independent Poisson/Hessian checks pass. Retain the first
loss and this follow-up rather than fitting a small-case guard.

## Sparse comparator adjudication

The disputed development cell has N38400, eight finite columns and **8,084
explicit FE columns**. Neutral-intercept Poisson Newton fits using independent
SciPy column-equilibrated explicit sparse-design LSMR and LSQR, each at 1e-14,
converge in eight steps. Final scaled scores are **7.60e-14** and **3.38e-14**.
Both pass the independent public-state stationarity, model-space and objective
replay tests. Their coefficients, covariance and means agree under the extra
1e-9/1e-11 stability envelope; maximum mean difference is **6.39e-10**.

Both independent fits agree with baseline and candidate coefficients, robust
covariance, means, likelihood and deviance under the unchanged public envelopes.
The candidate's maximum mean error is at most **7.11e-10**. Stock misses five
means by up to **1.260e-6**, about **4.895** times their allowed error. This
localizes the discrepancy to stock mean accuracy on this cell; keep it unranked.
Stock's coefficients, covariance and objective still pass. Do not rewrite older
receipts or generalize this finding to other comparator disagreements.

This diagnostic forms the finite Hessian at independently solved means, with
fresh explicit FE projection and a full-rank finite design. It makes **no full
FE rank or DoF claim** and does not lift the existing small oracle's bounds.
An earlier 1e-13 LSMR attempt hit the declared 100-step stricter score cap at
3.16e-13. Recording its terminal state exposed failure of the extra cross-
tolerance mean stability check too. Those failures remain in the archive; the
predeclared LSMR/LSQR follow-up supplies the converged evidence. No production
or pairwise tolerance was relaxed.

## Remaining cost and memory

Dense development median native FE-projection time falls **322.665 -> 111.772
ms**; strict/final time remains **167.506 -> 170.329 ms**. Preparation grows
**8.755 -> 10.095 ms**, finite solve falls **18.927 -> 17.179 ms**, inference
is **0.600/0.613 ms**, and publication is **6.105/5.979 ms**. These named clocks
are diagnostic scopes, not an asserted disjoint partition of wall time.

The separate final-plugin trace accepts the strict joint witness and canonical
final transaction for both warmup and measured data. On N30720 it records strict
predictor change **2.88e-12**, coefficient change **2.32e-13**, unchanged basis
and accepted finalization. The remaining final phase is not repeated rejection.
The new four-FE route reduces projection **2228.725 -> 28.330 ms** and strict/
final **1369.294 -> 14.942 ms**. Its old generic repeated-MAP canonical path
was especially expensive. Raw inner counts mix MAP sweeps, joint Krylov steps
and per-column iterations and are not comparable work units.

Fresh median **tracked / modeled MB**, decimal units:

| Case, in fresh-table order | Baseline tracked | Candidate tracked | Baseline model | Candidate model |
| --- | ---: | ---: | ---: | ---: |
| Dense three FE | 36.351 | 49.968 | 43.277 | 59.393 |
| Sparse three FE | 17.906 | 23.919 | 21.931 | 29.228 |
| Wide unbalanced | 57.509 | 73.532 | 66.743 | 85.174 |
| Disconnected two FE | 13.422 | 18.402 | 16.700 | 21.741 |
| Weak barbell | 3.323 | 4.435 | 9.765 | 10.947 |
| Four FE | 8.857 | 13.770 | 13.057 | 18.193 |

Tracked increments span **1.111–16.022 MB**. All PPML attempt peaks remain below
their selected model. Charge the coefficient/joint payload for the full session
and charge canonical-owner coexistence explicitly; no extra N-by-K design is
introduced. Optional-state refusal retains the previous route. Canonical and
joint owners are released before inference. Fixed-process pool1/T1, pool1/T4,
pool2/T4 plugin audits report exact zero events/growth and stable capacities.
The earlier mixed-thread Crossbeam reclamation failure remains open; this does
not repair static allocation drift or establish physical-cap qualification.

The comparable remaining target gaps are concrete: dense development needs
**350 -> <=220.5 ms**, strong two FE **131 -> <=48.6**, wide unbalanced
**583 -> <=226.8**, disconnected two FE **118 -> <=54.9**. Checkpoint arithmetic,
ordinary preparation and public-command overhead remain opportunities. Do not
repeat the rejected all-coefficient canonical or local-stopping probes without
a new numerical design. Next, measure and extend whole-command one-FE
concentration into low storage; it remains a broad loss in the preceding
source-bound inventory. Its stock ratio needs fresh measurement on this artifact.

## Validation and receipts

**347 distinct Rust tests** pass: 342 PPML plus five retained-session tests.
Tests cover weighted truth, current Hessian, changing frames, nested/deficient FE,
zero controls, narrow pools, forced predictor/basis movement, retry, rollback,
geometry refusal and the original dynamic-range case. The actual-plugin ABI/
injected-panic containment harness passes; the build has three existing plugin
warnings, not build errors.

**Nine licensed runs of seven fixtures** pass with process/Stata rc0 and terminal
sentinels. These include categorical and slope allocation audits, weak-mobility
categorical memory modes against explicit Poisson, multipartition slope and
one-FE oracles, PPML scientific contracts, and OLS memory modes. Raw licensed
logs stay local. The categorical memory fixture is an unchanged existing fixture
with a local terminal-sentinel wrapper, captured with its hash.

Public evidence comprises **175 passing headline/pilot applications**, **139
passing PPML states**, **24 small independent Poisson/Hessian checks**, **49
passing native PPML pairs**, **36 passing stock pairs and five preserved stock
mean discrepancies**, and **18 exact OLS pairs**. Direct candidate/stock audits
independently produce the same 36/41 result; baseline agreement is not used as a
substitute. Eight excluded PPML pairs have exact scientific exports. The separate
trace adds one passing application and public-state check. No application failure,
timeout or timing contention was dropped. The first direct-stock audit had a
physical/sample array-length assertion; its corrected loader uses the existing
independent sample reconstruction. The failed script remains archived.

The archive contains complete captured computational sources for baseline,
rejected `catjoint1` and adopted `catjoint2`; rejected local numerical overlays;
declarations, controllers, checks, timings, scientific exports, filtered trace
protocols and reproduction recipes. It omits data, binaries and raw licensed
logs, retaining their relevant identities/hashes. The 77 primary, 16 holdout and
111 native-control definitions remain unchanged. SCC, constrained-memory,
weighted constrained-fit/nonretaining-CG and older comparator failures are not
claimed repaired.

[Receipt archive](receipts-content-addressed.zip): **6,983 logical files**, **1,878
unique payloads**, **18,128,244 bytes**. SHA-256:
`85b9506cfc58c59c40526d4f8f9251e06d2cdeaafa0823e9dbef298652217ed6`.
All logical hashes were verified after restoration using
[restore_receipts.py](restore_receipts.py). Full distributions and counters are
in `public-summary.json`, `verified-experiment.json` and individual audits.
Original source hashes and sanitized archive hashes are recorded separately.
