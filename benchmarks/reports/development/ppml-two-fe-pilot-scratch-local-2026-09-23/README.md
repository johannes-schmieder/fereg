# Reusing idle two-FE group scratch for PPML projection

Local macOS arm64 development milestone, September 23, 2026. Adopt `canonloan1`
under [ADR 0146](../../../../docs/adr/0146-two-fe-pilot-scratch-lending.md).
The reusable categorical owner lends already-admitted group buffers to parallel
initial checks and MAP pilot sweeps. A wide development case improves 11%; four
fresh affected cases improve about 5–13%. Dense categorical PPML still loses to
fixest. The per-case objective of whole public PPML at most **0.90 times stock**
remains unfinished.

## Identity and measurement

Baseline `jointcolpar2` matches `f5351210`, plugin SHA-256
`92e878f6d394242480b3030123d97a23edf67a808bc0f48656d20e3af1b8743f`.
Candidate `canonloan1` overlays only `crates/fereg-core/src/fe/borrowed_map.rs`,
including the new Rust oracle. All **542 captured computational inputs** match
the adopted worktree. Candidate plugin SHA-256:

`2176ca5412f88643a8b5c09f8ccdf5b0fdcff9a3471d71d61a3f9d5fa0f833ed`.

Stock fixest 0.14.2 is unmodified, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched requested threads and serial BLAS.
Public PPML clocks include robust inference and fitted means; stock includes
response prediction. Startup, generation/import, warmup and exports are outside
both clocks. Native low mode versus stock is not physical-memory-matched.

Orders rotate baseline/candidate/stock in fresh processes. All headline attempts
have no detected overlap or monitor error. Compilation, tests, diagnostics and
archiving run outside headline blocks. Seed 202609347 is development evidence;
**202609352** was predeclared for confirmation and is now consumed. Frozen
77/16/111 definitions and protected cycle seeds 211/212 are unchanged.

## Baseline and measured cause

The initial baseline/stock block, three rotations, establishes:

| Low two-FE case | Baseline median [min,max] ms | Stock median [min,max] ms |
|---|---:|---:|
| N16320/K24/T8 | 190 [190,194] | 72 [67,74] |
| N30720/K6/T4 | 112 [111,114] | 57 [54,58] |
| N8160/K12/T4 | 67 [65,67] | 36 [35,37] |
| N20160/K24/T8 | 178 [178,185] | 78 [77,82] |

Source inspection shows that ordinary parallel group storage is reserved only
at N>=16384. Wide RHS work below that boundary therefore uses serial projection.
The reusable two-FE owner already has sufficiently large group arrays for its
later PCG solve. Before current-weight PCG initialization they are scratch,
not authoritative geometry. This supports scoped lending without adding memory.

Isolated RAII diagnostics `canon2ledger1` and `canonloanledger1` have plugin hashes
`dc2d05dd24b98f727bc3fb5b004ba9ba6e38c5758e6f63b72d2153a9364d9207` and
`4179fed7a5e5d3216689505965f2fd0fddac3eb8ac9af1302e042e3cf9bd49a7`.
For N16320/K24/T8:

| Diagnostic work | Baseline | Candidate |
|---|---:|---:|
| Serial projections, calls / ms | 108 / 36.160 | 36 / 5.416 |
| Parallel fused projections, calls / ms | 0 / 0 | 72 / 17.024 |
| Complete symmetric sweeps, calls / inclusive ms | 36 / 60.575 | 36 / 28.174 |
| Categorical certificates, calls / inclusive ms | 28 / 14.878 | 28 / 12.539 |
| PCG calls / inclusive ms | 9 / 17.530 | 9 / 18.905 |
| Canonical projection wrappers, total inclusive ms | 94.386 | 60.549 |
| Native root, ms | 223.016 | 178.500 |

These rows nest. The probes are intrusive: the baseline sweep wrapper has about
24 ms outside its serial projection children, versus about 3 ms in the candidate.
Much of this is added logging overhead, not an attributed production saving.
Use the uninstrumented rotations below for speed claims. The source and call
counts establish the scheduling change; the diagnostic root difference does
not measure its headline gain. The N8160 diagnostic is nearly tied despite the
public improvement, reinforcing the limit of single-fit diagnostic clocks.

## Adopted mechanism

For an admitted reusable two-FE owner, lend `left_work` and `left_weights` only
when ordinary parallel banks are empty, the installed team has multiple workers,
and N times active RHS count is at least the existing 16384 work threshold.
The largest partition's capacity covers both partitions. Use existing group
projection and certificate kernels; restore both vectors through a scoped
destructor before a recurrence, return or unwind. PCG reconstructs current-weight
denominators and moments before using those banks. No cached numerical state
gains authority across weights or calls.

The shared certificate dispatcher considers N times RHS, still requiring actual
group storage. Initial checks, four-sweep pilot, PCG selection, current-weight
certificates, rank/inverse, scores, objective, reconstruction and final-Hessian
requirements remain. There is no tolerance, sample, weight, DoF, separation or
publication change. Serial and already-provisioned cases retain their work;
generic slope, three-FE and one-shot OLS algorithms are unchanged. Shared reusable
initial checks may also benefit on fast categorical execution.

A global N-times-RHS allocation-policy change was considered but not implemented:
it would change admission and peak storage unnecessarily. There is one measured
production candidate, no rejected build or solver layer, and no threshold tuning
from validation cases.

## Public timings and remaining losses

Development screen, three rotations, median [min,max] milliseconds:

| Low two-FE case | Baseline | Candidate | Stock | Candidate/stock |
|---|---:|---:|---:|---:|
| N16320/K24/T8 | 188 [186,194] | 167 [166,170] | 68 [67,71] | 2.456 |
| N30720/K6/T4 | 115 [113,116] | 113 [111,114] | 57 [57,57] | 1.982 |
| N8160/K12/T4 | 65 [65,67] | 60 [59,62] | 37 [33,38] | 1.622 |
| N20160/K24/T8 | 178 [177,196] | 178 [177,189] | 82 [76,82] | 2.171 |

Wide paired changes are -27/-20/-18 ms; smaller wide changes are -3/-6/-7 ms.
The unaffected larger-row cases have mixed pairs, including +1 and +11 ms.

Fresh seed352, five rotations, low categorical two-FE:

| Case | Baseline | Candidate | Stock | Candidate/stock |
|---|---:|---:|---:|---:|
| N10080/K10/T4 | 66 [65,68] | 60 [58,60] | 33 [30,36] | 1.818 |
| Unbalanced N15120/K24/T8 | 157 [153,161] | 137 [135,143] | 60 [54,62] | 2.283 |
| Disconnected N3840/K12/T4 | 42 [42,43] | 40 [40,41] | 26 [24,27] | 1.538 |
| Weak barbell N12480/K12/T4 | 89 [87,89] | 78 [77,80] | 902 [897,906] | 0.086 |
| Serial N10080/K10/T1 | 79 [77,83] | 77 [75,80] | 66 [61,68] | 1.167 |
| Larger-row N24960/K10/T4 | 121 [117,124] | 121 [119,123] | 61 [59,62] | 1.984 |

All twenty pairs in the first four rows improve, by 1–24 ms. Median reductions
are 9.1%, 12.7%, 4.8% and 12.4%. Serial paired changes include +3 ms and a tie;
the larger-row control has three +2-ms losses. These are visible controls,
not claims of universal non-regression.

Focused fresh controls, three rotations, all N15360/K12/T4:

| Control | Baseline | Candidate | Stock |
|---|---:|---:|---:|
| Fast categorical two FE | 93 [93,96] | 91 [91,93] | 54 [53,54] |
| Low two FE, slopes on both partitions | 101 [101,105] | 102 [101,103] | 106 [99,106] |
| Low three FE | 157 [154,164] | 156 [155,161] | 219 [217,224] |
| OLS categorical two FE | 29 [28,29] | 29 [29,29] | — |
| OLS two FE, slopes on both partitions | 79 [78,81] | 79 [78,82] | — |

The slope PPML median loses 1 ms. Three-FE PPML includes a +7-ms pair; OLS
includes +1 and +4-ms pairs. Exact exports and unchanged scientific metadata
do not establish timing equivalence; retain these distributions.

## Independent validation and memory

* **156 distinct final-source Rust tests** pass across scratch ownership,
  reusable categorical projection, parallel projection, prepared IRLS/estimation
  and PPML memory. Production build ABI/panic checks also pass.
* The new weighted two-component ring oracle has N4096/RHS8, pairwise equal
  weights and constructed orthogonal residuals, three weight frames with unequal
  component masses, and a zero RHS. Exact serial/parallel residuals and full
  certificates agree; the mathematical residual error is below 1e-10. Injected
  error/panic cases poison both loaned banks, then verify pointer/capacity
  restoration and successful fresh-weight solves.
* **Five licensed fixture runs pass**, with process status, explicit Stata return
  code and terminal sentinels. The new local N4800/K8/pool4 categorical fixture
  checks coefficients/means against explicit Poisson and independently forms
  the Hessian and robust/cluster sandwich for unit/FW/PW at T1/T4. The other runs
  cover actual-plugin allocation, multipartition slopes, separation and failure
  atomicity. Local fixture recipes and hashes are archived.
* **189 headline applications**, **177 public-state checks**, **57 exact native
  export pairs**, **63 baseline/stock pairs** and **51 direct candidate/stock
  pairs** pass. No scientific exclusions, application failure, timeout or
  detected contention occurs. Eight diagnostic applications are separate.
  The harness has no small Python oracle block in this milestone; independent
  mathematical and licensed explicit-design checks above supply that boundary.

All 51 PPML native pairs retain iteration/halving/fallback, rank/DoF, modeled
peak and scratch metadata. Tracked peaks match in 48; the fast categorical
control changes by +61080, -520040 and -747008 bytes. The change introduces no
bank or admission increase, but measured tracked peaks are not all identical.
No physical-memory claim is made.

The actual-plugin pool4/T4 audit reports exactly zero events and zero current
growth during sixteen ordinary joint iterations, stable capacities and fresh/warm
agreement. That window does not directly audit canonical pilot allocation; the
Rust loan test covers its ownership/capacity boundary. Static allocation inventory
drift, weighted constrained-fit failures, nonretaining-CG instability, historical
comparator discrepancies and the separate 1e6-FE-shift failure remain open. SCC,
constrained-memory and cross-platform confirmation remain deferred.

## Next bounded design

Close scratch lending. In the candidate wide diagnostic, canonical projection
still takes about 60.55 ms: symmetric pilot sweeps account for 28.17 ms, PCG for
18.90 ms and categorical checks for 12.54 ms, with nesting. There are still nine
PCG calls following repeated pilots. Joint directions separately cost 60.17 ms,
including 29.47-ms finite-product tiles and 12.24-ms reconstruction; exclusive
joint recurrence is below 1 ms. More recurrence tuning is not supported here.

The next coherent experiment should test direct current-weight two-FE Schur
absorption without a mandatory four-sweep warmup, reusing compatible group
geometry across canonical RHS tiles. Preserve independent original-weight
acceptance and canonical rank authority, with disconnected/unequal-mass and weak
controls before routing it. This targets repeated whole-observation work rather
than another scratch threshold. It is a hypothesis, not an adopted guard removal
or a promise of parity. One-FE preparation/objective and wide inference remain
subsequent measured subsystems under the broader per-case target.

## Receipts

`receipts-content-addressed.zip` contains **7,673 logical receipts**, 1,783 unique
payloads, 11,657,691 bytes. SHA-256:

`2206eb5db6ee997399cf0cc15208997d031853da798f03bcca05a1193441e026`.

`restore_receipts.py NEW_DIRECTORY` checks the container, index and each logical
payload. Restoration was verified before handover. Captured sources, identities,
build/test results, declarations, controller receipts, audits, metadata, timings
and diagnostic spans are included. Generated data/mean files have content hashes;
licensed logs, binaries and machine-specific paths are excluded or sanitized.
`summary.json`, `metadata-details.json`, both ledger summaries and the licensed
result JSON files are the concise entry points. These are development receipts,
not qualification or a claim transferred to any other artifact.
