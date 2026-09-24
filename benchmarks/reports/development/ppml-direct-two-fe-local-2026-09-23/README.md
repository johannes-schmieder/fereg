# Direct component-scaled two-FE PPML absorption

Local macOS arm64 development milestone, September 23, 2026. Adopt `directtwo3`
under [ADR 0147](../../../../docs/adr/0147-direct-component-scaled-ppml-schur.md).
The categorical PPML owner enters Schur PCG without four mandatory symmetric
MAP pilots, and balances disconnected components using their current masses.
All thirty fresh target pairs improve; reductions in medians are about 7–14%.
Dense cases still lose to stock fixest. The objective of whole public PPML at
most **0.90 times stock per comparable case** remains unfinished.

## Source and measurement identity

Baseline `canonloan1` matches `432808e4`, plugin SHA-256
`2176ca5412f88643a8b5c09f8ccdf5b0fdcff9a3471d71d61a3f9d5fa0f833ed`.
Candidate `directtwo3` is based on that checkpoint with three overlays:

* `crates/fereg-core/src/fe/borrowed_map.rs`
* `crates/fereg-core/src/ols/linear_session.rs`
* `crates/fereg-core/src/ppml/memory.rs`

All **542 captured computational inputs** match the adopted worktree. Candidate
plugin SHA-256 is
`9b027385d827c8e71ee4997ef10dc94d3c5a626c4666a7344e74090488122bd9`.

Stock fixest 0.14.2 is unmodified, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched requested threads and serial BLAS.
The public PPML clock includes robust inference and fitted means; stock includes
response prediction. Startup, generation/import, warmup and exports are outside
both clocks. Native low mode versus stock is not physical-memory-matched.

Orders rotate baseline/candidate/stock in fresh processes. No headline attempt
has detected overlap or a monitor error. Builds, tests, diagnostic fits and
archiving are separate. Seed 202609347 supplies development cases;
**202609353** was predeclared for fresh confirmation and is now consumed.
The fixed-input control follow-up changes neither source nor routing. Frozen
77/16/111 definitions and protected cycle seeds 211/212 are unchanged.

## Baseline, rejected experiments and adopted equations

The initial baseline/stock block has three rotations:

| Low categorical two-FE case | Baseline median [min,max] ms | Stock median [min,max] ms |
|---|---:|---:|
| N16320/K24/T8 | 175 [165,176] | 69 [69,71] |
| N30720/K6/T4 | 113 [111,115] | 59 [58,61] |
| N8160/K12/T4 | 63 [62,63] | 36 [31,38] |

Use the later paired blocks for gain estimates, rather than subtracting their
candidate times from this earlier block. The wide baseline shifts to 162 ms in
the paired development block.

Inspection establishes that the admitted reusable two-FE owner previously chose
PCG after four symmetric MAP pilots regardless of measured contraction. Those
pilots therefore do not supply route-selection evidence. Direct entry can remove
their observation passes while retaining initial certification, reconstruction,
fresh original-weight acceptance and reserved MAP continuation.

Direct entry alone is insufficient. `directtwo1`, plugin
`79dbda7c22d93030d1702bd4864d9e9fd4b2246bc90d6284adc94103be8023a4`,
fails the existing six-step small dynamic-range PPML test. Its disconnected hot
and cold components have weights differing by roughly thirteen orders of
magnitude. The inner certificates pass, but the second strict mean movement is
about 1.5244e-8, above the unchanged 1e-8 threshold. The baseline has one strict
rejection and succeeds; the prototype has two and exhausts the six-step bound.
The archived baseline/candidate traces expose the difference. A finishing
symmetric sweep and a fused centered-Schur RHS probe both fail to repair it;
neither is retained. These prototypes have no headline timing claim.

For disconnected component c, the adopted recurrence solves

    (S_c / m_c) a_c = b_c / m_c,

where m_c is the largest current right-group mass in that component. The Jacobi
diagonal is divided by the same m_c. A positive scalar constant within a
disconnected block preserves its symmetric positive semidefinite operator,
null space and fitted projection, while keeping low-mass blocks represented in
the shared CG energy reduction. Arbitrary per-group row scaling would not have
that property and is not used.

Right-level component roots join levels sharing a left group. They are immutable
for the owner's sample/topology. Current group masses, RHS moments and component
scales are refreshed on every absorption. The raw operator and group totals keep
the original normalized weights; only recurrence equations are component-scaled.
Cheap readiness recovers the original group mean and retains its original mass
factor. Reconstruction and the full acceptance certificate use original weights.

An initial component prototype fails to compile because four readiness-call
arguments were missing; its log is retained. `directtwo2`, plugin
`55a575899faa81a67d50d31d7f70a6c2722f0eb556efc6140456d79a4eb04c11`,
then passes the scientific boundary, but four literal memory expectations need
the new bank counts. Review also finds that initializing components in the
shared constructor would change OLS storage without its exact admission.
The final candidate adds a PPML-specific constructor and updates the exact PPML
memory equations. Ordinary OLS retains its previous constructor and admission.
`directtwo2` has no headline timing claim.

The final route retains initial early success, checkpoint-LSMR selection and
retry policy, rank authority, finite scores, model-space checks, objective,
strict profile, final Hessian and failure atomicity. A PPML checkpoint's existing
MAP fallback may use direct Schur. The graph is borrowed in place rather than
removed and restored, preserving ownership through errors and unwinding.
There is no tolerance, sample, weight-semantic, DoF, separation or public API
change. One-/three-FE and slope algorithms are unchanged.

## Whole-command timings

Development screen, three rotations; median [min,max] milliseconds:

| Low two-FE case | Baseline | Candidate | Stock | Candidate/stock |
|---|---:|---:|---:|---:|
| N16320/K24/T8 | 162 [161,163] | 151 [146,152] | 73 [70,75] | 2.068 |
| N30720/K6/T4 | 115 [114,116] | 103 [100,105] | 60 [57,60] | 1.717 |
| N8160/K12/T4 | 63 [60,63] | 55 [54,56] | 38 [37,39] | 1.447 |

Paired changes are -11/-15/-11, -10/-16/-11 and -9/-5/-7 ms respectively.
The small one-rotation explicit-oracle pilot has strong two-FE N1440/K3/T4 at
17→14 versus 15 ms stock, disconnected at 16→16 versus 15, and weak barbell
N960/K3/T4 at 15→13 versus 19. Single pilot observations are not confirmation.

Fresh seed353, five rotations, low categorical two-FE:

| Case | Baseline | Candidate | Stock | Candidate/stock | 0.90× stock target |
|---|---:|---:|---:|---:|---:|
| Wide N16320/K32/T8 | 224 [211,226] | 200 [198,204] | 86 [84,89] | 2.326 | 77.4 |
| N34560/K10/T4 | 161 [159,166] | 149 [141,153] | 82 [79,85] | 1.817 | 73.8 |
| Serial N12000/K10/T1 | 95 [92,99] | 85 [83,86] | 80 [76,82] | 1.063 | 72.0 |
| Disconnected N20160/K8/T4 | 95 [91,97] | 87 [83,88] | 53 [53,55] | 1.642 | 47.7 |
| Unbalanced N15120/K20/T8 | 120 [117,123] | 112 [111,115] | 51 [46,53] | 2.196 | 45.9 |
| Weak barbell N7680/K4/T4 | 36 [35,38] | 31 [30,32] | 174 [172,179] | 0.178 | 156.6 |

All thirty pairs improve. Median reductions are 10.7%, 7.5%, 10.5%, 8.4%, 6.7%
and 13.9%. The paired changes by row are:

* -22/-13/-26/-19/-25 ms
* -7/-25/-8/-12/-17 ms
* -12/-10/-15/-9/-9 ms
* -8/-8/-8/-5/-10 ms
* -7/-5/-12/-8/-8 ms
* -4/-6/-7/-5/-4 ms

Only the weak case clears the stock target in this fresh block. Dense wide and
unbalanced cases still need roughly 59–61% further native reduction to meet it.

Controls, three rotations, retain losses as well as gains:

| Control | Baseline | Candidate | Stock |
|---|---:|---:|---:|
| Fast two FE N16320/K16/T4 | 127 [125,130] | 133 [126,134] | 61 [61,62] |
| Low three FE N30720/K6/T4 | 234 [234,236] | 225 [223,227] | 242 [240,245] |
| Low two FE, slopes on both, N30720/K6/T4 | 137 [136,140] | 139 [137,141] | 116 [116,117] |
| OLS categorical two FE N16320/K16/T4 | 34 [34,35] | 35 [34,36] | — |
| OLS slopes on both N30720/K6/T4 | 68 [68,68] | 69 [69,70] | — |

Fast pairs are -4/+9/+6 ms; slope PPML pairs are +1/+2/+1. OLS categorical
pairs are +1/+1/0 and slope pairs +2/+1/+1. Exact exports do not establish timing
equivalence. A predeclared source-unchanged five-rotation follow-up gives:

| Same control | Baseline | Candidate | Stock | Paired changes ms |
|---|---:|---:|---:|---|
| Fast two FE | 128 [126,129] | 130 [126,134] | 65 [58,66] | +5/-1/+1/+4/+4 |
| OLS categorical | 34 [34,35] | 35 [34,35] | — | 0/0/-1/+1/+1 |
| OLS slopes | 68 [65,69] | 69 [65,70] | — | -4/+2/+4/-4/+2 |

The larger fast median loss is not fully reproduced, but a 2-ms median loss
remains; its median paired loss is 4 ms. Do not describe it as removed. No guard
or algorithm is tuned to this follow-up. All its exports remain exact.

## Scientific checks and memory

**163 distinct final-source Rust tests pass**, including the repaired six-step
boundary, prepared IRLS/estimator, all 51 PPML memory tests, checkpoint Schur,
large thread reproducibility and exact OLS Schur admission versus independent
MAP. The new analytical N256 two-ring weighted-contrast oracle varies component
masses down to 1e-10, RHS widths 8/3/1/8, a zero RHS and a large FE contribution
at T1/T4. It also fails a one-iteration attempt, reuses the owner successfully
under fresh weights and verifies stable capacity. Ordinary construction has no
component roots. Production build ABI and panic-containment checks pass.

One test controller terminates with a failure status because an expensive
checkpoint test had been selected twice and its redundant second execution was
intentionally cancelled after the first passing receipt. `duplicate-test-stop.json`
preserves that action. All distinct selected tests have passing final-source
receipts; the two remaining unique OLS checks were run explicitly. This is not a
claim that every controller exited successfully.

**Six licensed fixtures pass** with process status, explicit Stata return code
and terminal sentinels:

* N4800/K8/pool4 explicit Poisson coefficients/means and separately assembled
  Hessian and robust/cluster sandwich, unit/FW/PW at T1/T4 (12 combinations).
* N4096/K8/pool4/T4 actual-plugin allocation and warm/fresh equivalence audit.
* Multipartition slope oracle and separation-method fixtures.
* Low checkpoint failure/publication transaction fixture.
* Generated factor/product terms, PW and numeric FE shifts 0/1000 against an
  explicit Poisson oracle. The separate historical 1e6-shift failure is not
  rerun or repaired by this fixture.

All **218 headline applications**, **186 public-state checks**, **9 independent
Python Poisson oracle results**, **72 native scientific pairs**, **65 baseline/
stock pairs** and **56 direct candidate/stock pairs** pass. Thirty native pairs
have exact exports. There are no scientific exclusions, application failures,
timeouts or detected contention in these blocks. Four diagnostic applications
are separate. Algorithmic residual differences in affected paths are judged by
the established scientific envelopes, not old-recurrence bit equality.

All 56 PPML native pairs retain outer iterations, halvings, fallback/strict
rejections, rank/DoF and scratch metadata. Forty-two pairs reduce reported inner
iterations; those reports count different work after pilot removal and are not
a package-to-package operation metric. Only inner counts and memory fields
change in the recorded scientific metadata.

PPML admits two words per smaller-partition level, at most **16R bytes** on
supported 64-bit platforms, for roots and current scales. Union ranks and later
component maxima use idle operator scratch. Connected owners discard their
temporary roots; disconnected owners retain both banks. No N-sized weight copy
or new RHS matrix is added. OLS construction/admission remains unchanged.

The whole-route modeled peak usually rises by 48 bytes for owner metadata, with
larger rises of 2,096, 2,352 and 5,424 bytes where component admission dominates.
The fresh disconnected tracked peak rises 5,424 bytes. Threaded fast controls
also show unrelated varying tracked peaks (initial +419,474 to +635,978 bytes;
follow-up -503,730 to +484,527); do not attribute all peak differences to the new
component banks. The full per-pair metadata is archived.

The actual-plugin audit records zero allocation events/current growth during
sixteen ordinary joint steps, stable capacities and fresh/warm equivalence.
That window does not cover canonical PCG/component construction; the exact
memory equations and Rust capacity tests cover its modeled boundary. No physical
memory-cap or cross-platform claim is made.

## Diagnostic attribution and remaining work

Isolated `directtwoledger3`, plugin
`6091752be87101126c3213ccaf802c0deb84f37a47c9516dafd8c9f5ae814d8e`,
adds RAII spans to this candidate. Its four applications pass their audits.
The prior baseline diagnostic `canonloanledger1`, plugin
`4179fed7a5e5d3216689505965f2fd0fddac3eb8ac9af1302e042e3cf9bd49a7`,
is preserved in the [preceding source-bound report](../ppml-two-fe-pilot-scratch-local-2026-09-23/README.md).
That baseline's computational implementation matches `canonloan1`; its diagnostic
overlay and timings are separate from both stock and headline artifacts.

For development N16320/K24/T8:

| Diagnostic work | Prior baseline | Candidate |
|---|---:|---:|
| Symmetric MAP sweeps, calls / inclusive ms | 36 / 28.174 | 0 / 0 |
| Categorical certificates, calls / inclusive ms | 28 / 12.539 | 19 / 8.312 |
| PCG, calls / inclusive ms | 9 / 18.905 | 9 / 21.409 |
| Canonical projection wrappers, inclusive ms | 60.549 | 28.129 |
| Native root, ms | 178.500 | 145.651 |

These are intrusive single-fit diagnostics with nested rows; the earlier probe
also executes more logging calls. Do not use their difference as a headline
speedup or sum nested durations. Source and operation counts establish removal
of pilots. Original full initial and final acceptance remain.

The candidate wide diagnostic spends 58.939 ms in six joint-direction calls:
finite-product tiles cost 29.690 ms, norms 5.689 ms and reconstruction 11.138 ms;
the exclusive recurrence is only 0.995 ms. Canonical rank work costs 17.218 ms,
inference 4.766 ms, loading 2.742 ms and core preparation 2.834 ms. At N30720/K6/T4,
joint work is 35.228 ms, canonical projection 15.877 ms and rank work 10.612 ms.
These are phase leads, not proof that every measured cost is avoidable.

Close this direct-entry experiment. The next coherent design should address
joint-direction preparation and canonical product ownership, not tune pilot
counts or recurrence thresholds. A current-weight matrix-free joint operator
could avoid repeated O(NK²) product formation, using prior factors solely as
preconditioners; a blocked current-weight product engine is the competing
candidate if matrix-free observation passes are too costly. Both must retain
fresh original-input scores/reconstruction and canonical rank/final-Hessian
authority. No claim is made that either alone closes the remaining stock gap.

Static allocation inventory drift, weighted constrained-fit failures,
nonretaining-CG instability, historical comparator discrepancies and the separate
1e6-FE-shift failure remain open. SCC, physical constrained-memory and
cross-platform confirmation remain deferred.

## Immutable receipts

`receipts-content-addressed.zip` contains **9,134 logical receipts**, 2,143 unique
payloads and 15,969,197 bytes. SHA-256:

`9c1c08a4ce809fbfcebeae69652dc56f4bbd61d71464012cfe42cb21e66b2cc7`.

`restore_receipts.py NEW_DIRECTORY` validates the container, index and every
logical payload. Restoration was verified before handover. Captured source
snapshots, artifact identities, build/test receipts, rejected probes, declarations,
controller/audit records, timings, metadata and diagnostic spans are included.
Data and means have content hashes; raw licensed logs and binaries are excluded,
and machine-specific paths are sanitized. Concise entry points are `summary.json`,
`metadata-details.json`, `ledger-summary.json` and the three final licensed-result
JSON files. These are local development receipts, not qualification or claims
transferred to a later source/artifact.
