# PPML frame-owned reduced operator: integration closeout

Local macOS arm64 development evidence, 2026-09-24. **Reject production adoption
and restore the accepted solver.** This completes an implementation experiment,
not the broader objective of whole-public PPML <=0.90 times comparable stock
fixest on designs with at least two FE partitions.

The frame-owned operator is scientifically successful on the tested boundaries,
but the promising development gains do not carry over strongly enough to the
remaining-loss families. Fresh wide low improves 137 to 136 ms, unbalanced low
loses 98 to 101, larger low improves 185 to 183 and 70-control low improves 357
to 351. Serial low improves 108 to 101 and narrow fast 60 to 57, but their
accepted baseline already clears 0.90 stock on these fresh designs. Do not
fit guards to preserve those favorable subcases or tune consumed seed202609378.

## Source and evidence

Accepted production computation remains `2c34503d`, artifact `jointpanels2`.
The worktree is restored to `86124f72`, which also contains the separately
accepted test-only checkpoint-accounting repair. No experimental operator,
routing, memory allowance, generation plumbing or associated test remains in
production. `restoration.json` verifies an empty tracked computation diff and
removal of the captured new module. Historical receipts are unchanged.

- `jointpanels2` plugin SHA-256: `4d4c669d6716583b14be437e2732133d61f733840b610414429d8df5614168b1`.
- `frameoperator1` plugin SHA-256: `dc5e77d6c1254244f8bba126f8164829ae9e1549e47ad1918ea5c7087b77ffa7`.
- `frameoperator2` plugin SHA-256: `71f2924e0ceb59be4450181620cfcecfbefc03148353ee2b93a7b88ace3be68d`.

Both candidates are isolated builds of `86124f72` plus four captured overlays:
`fe/borrowed_map.rs`, its new `frame_operator.rs` module,
`ols/linear_session.rs`, and `ppml/prepared_irls.rs`. All **547 candidate2
computational inputs** matched the tested source before restoration. Both builds
pass the actual-plugin ABI/self-test. No diagnostic instrumentation is present
in these public timing artifacts.

The [archive](receipts-content-addressed.zip) contains **9,735 logical receipts**,
2,372 payloads and 18,735,071 bytes, SHA-256
`4dc511e30529fd6ec894db2f6691a7e06e64dcdb4b1894f347c36701b9caef39`.
The [restore tool](restore_receipts.py) verifies the container and every logical
payload. It was used to restore all receipts. Complete baseline/candidate
sources, declarations, controller/oracle scripts, test logs, fixture protocols,
public exports, distributions, memory/iteration metadata and restoration proof
are included. Raw licensed logs, datasets and binaries are omitted with hashes;
machine paths are sanitized. No compilation, profiling, archiving or competing
fits accompanied headline measurements. All monitor reports are empty and all
build/test/fit/audit/archive/restore processes are terminal.

## Implemented organization

The [preceding fixed-frame experiment](../ppml-frame-operator-local-2026-09-24/README.md)
compared edge and dense operators and selected matrix reuse for integration.
This implementation borrows the existing immutable FE group indexes; it adds
no edge arrays or observation-sized cache. A small q-by-q matrix stores positive
Schur couplings. For eliminated group g and retained labels i,j, its contribution
is `w_gi*w_gj/w_g`, evaluated by dividing the larger mass first. One rounded
contribution updates both symmetric entries. Applying the Laplacian through
coefficient differences preserves the constant-vector null space without
subtracting nearly equal diagonal products.

The existing component-scaled PCG still owns RHS state, independent column
readiness, iteration/progress limits, reconstruction and the complete fresh
original-weight certificate. This is an operator replacement, not a new solver
family or a direct factorized solve. There is no acceptance from a cached
residual or prior-weight solution.

A valid sample/topology/weight generation permits reuse across compatible
response, control tile and checkpoint calls. Response changes alone do not
invalidate FE geometry. Every absorption revokes prior authority before fallible
work and restores it only after success. Invalid norm selection also revokes it.
Unframed calls rebuild and grant no reusable authority. In-place rebinding of
sample/topology is unavailable. A failed preparation cannot leave an active
matrix. Positive coupling underflow declines the optional matrix; existing
normalization errors remain errors.

Admission requires the existing two-FE PPML graph, owner RHS capacity <=64,
q squared <=N, and estimated assembly work
`N + q*q + sum_g choose(distinct_neighbors_g,2) <= 2*N*compatible_RHS`.
These are structural work limits, independent of seeds and case labels. A
fallible matrix and two q-sized scratch banks fit inside an independently
optional reservation admitted after the existing numerical solvers. Memory
refusal preserves those solvers. Actual capacities enter the session fingerprint.
The complete reservation is retained even if the finer assembly-work check
later refuses construction.

Candidate1 admitted public MAP only. Inspection showed that fast public LSMR
uses the same canonical categorical projector; candidate2 also admits that
checkpoint role. Public solver identity and ordinary joint LSMR are unchanged.
CMG, slopes, one/three-plus FE and ordinary OLS do not receive this matrix.

## Science, failures and scope

All **300 public applications and independent public-state checks pass** across
the three screens. All **100 native paired comparisons pass**; final candidate2
accounts for 228 applications and 76 native pairs. Sample, original-design score,
model-space and objective replay are checked independently. Large-cell checks
are not complete covariance oracles per cell.

The final source passes **75 distinct focused Rust tests**, including reciprocal
weighted-contrast oracles across changing frames/widths/threads and disconnected
component masses differing by 160 orders of magnitude; symmetry/null-space and
work-refusal tests; exact optional budget refusal; canonical fallback, strict
rank-zero/basis refresh, inference/reconstruction and publication error tests.
All **six licensed Stata fixtures pass** with explicit terminal sentinels and
return codes. They cover independent explicit-dummy Poisson, Hessian and sandwich
checks, FW multiplicity, PW/cluster inference, requested outputs, categorical and
slope paths, one-FE controls and warmed allocation audits. No full suite, physical
memory cap, Linux, release or qualification claim is made. Planned OLS timing
scripts were not run after rejection; restoring the accepted sources protects
that implementation instead of claiming unmeasured candidate OLS performance.

The initial coupling-underflow unit fixture used weights rejected earlier by the
established normalization check. It was corrected to use valid normalized weights
whose pair coupling underflows. The rejection policy was not weakened. Two test
selectors matched zero tests; corrected selectors ran and passed. A requested
stop of the large checkpoint debug fixture found it already finished successfully;
no test was terminated. Original logs and controller failures are preserved.

**One fresh comparator discrepancy remains unresolved.** In disconnected case4,
all five stock comparisons exceed the coefficient envelope at one coordinate:
maximum absolute error 2.324539e-10 and maximum tolerance ratio 1.088198. Sample,
covariance, means, objective and public-state checks pass. Baseline and candidate
exports agree exactly there, and the matrix is structurally excluded. This is
not evidence attributing error to either package. Keep the stock comparison
unranked; do not relax its tolerance or invent an accuracy exception. The other
95 direct stock comparisons pass.

Maximum candidate2 native differences across its screens are coefficients
1.102e-16, covariance 4.236e-21 and means 3.198e-14; reported objectives are exact.
All 100 native pairs retain identical outer and reported inner iteration counts.
The fresh regressions therefore do not arise from an extra reported IRLS step.

## Whole-command distributions

Every fit requests robust inference and fitted means. Stock is the pinned,
unmodified fixest 0.14.2 library, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`, with
`glm.tol=1e-10` and `fixef.tol=1e-11`, matched requested threads and serial BLAS.
Use native baseline/candidate/stock rotations, not diagnostic-source timings.
Development seed202609347 has three candidate1 and two candidate2 repetitions.
Fresh seed **202609378 is consumed/frozen**, with five repetitions and all cases
predeclared before execution. These are local development results only.

B is accepted `jointpanels2`, C is the candidate named in each block. Values are
milliseconds. Case definitions and complete phase/memory metadata accompany
these distributions in `summary.json` and each captured plan.

### development: frameoperator1

| Case | B repetitions | C repetitions | Stock repetitions | B median | C median | Stock median | C/stock |
|---|---|---|---|---:|---:|---:|---:|
| 0: strong_d6/dense low N16320/K32/T8 | 120;120;121 | 112;109;110 | 89;92;87 | 120 | 110 | 89 | 1.236 |
| 1: strong_d6/unbalanced low N16320/K12/T4 | 68;66;65 | 65;65;63 | 46;46;43 | 66 | 65 | 46 | 1.413 |
| 2: strong_d6/dense fast N12000/K3/T1 | 51;49;50 | 48;53;47 | 42;39;41 | 50 | 48 | 41 | 1.171 |
| 3: strong_d6/disconnected fast N20160/K12/T4 | 70;71;74 | 72;72;71 | 66;69;69 | 71 | 72 | 69 | 1.043 |
| 4: three_fe_dense/unbalanced fast N23040/K8/T4 | 83;83;83 | 82;84;84 | 88;88;84 | 83 | 84 | 88 | 0.955 |
| 5: strong_d6/dense fast N16320/K6/T4 slopes2 | 71;72;69 | 72;73;69 | 66;65;68 | 71 | 72 | 66 | 1.091 |
| 6: strong_d6/dense low N16320/K6/T4 slopes2 | 74;72;73 | 72;73;73 | 64;66;66 | 73 | 73 | 66 | 1.106 |
| 7: strong_d6/dense low N15360/K16/T1 | 100;100;102 | 93;92;95 | 151;153;151 | 100 | 93 | 151 | 0.616 |

### development2: frameoperator2

| Case | B repetitions | C repetitions | Stock repetitions | B median | C median | Stock median | C/stock |
|---|---|---|---|---:|---:|---:|---:|
| 0: strong_d6/dense low N16320/K32/T8 | 120;119 | 109;108 | 85;88 | 119.5 | 108.5 | 86.5 | 1.254 |
| 1: strong_d6/unbalanced low N16320/K12/T4 | 66;68 | 61;62 | 48;46 | 67 | 61.5 | 47 | 1.309 |
| 2: strong_d6/dense fast N12000/K3/T1 | 49;51 | 46;42 | 35;36 | 50 | 44 | 35.5 | 1.239 |
| 3: strong_d6/disconnected fast N20160/K12/T4 | 70;69 | 70;70 | 70;68 | 69.5 | 70 | 69 | 1.014 |
| 4: three_fe_dense/unbalanced fast N23040/K8/T4 | 82;81 | 83;80 | 84;87 | 81.5 | 81.5 | 85.5 | 0.953 |
| 5: strong_d6/dense fast N16320/K6/T4 slopes2 | 69;70 | 70;68 | 66;62 | 69.5 | 69 | 64 | 1.078 |
| 6: strong_d6/dense low N16320/K6/T4 slopes2 | 71;72 | 72;71 | 67;66 | 71.5 | 71.5 | 66.5 | 1.075 |
| 7: strong_d6/dense low N15360/K16/T1 | 95;99 | 93;95 | 152;150 | 97 | 94 | 151 | 0.623 |

### fresh: frameoperator2

| Case | B repetitions | C repetitions | Stock repetitions | B median | C median | Stock median | C/stock |
|---|---|---|---|---:|---:|---:|---:|
| 0: strong_d6/dense low N20160/K34/T8 | 131;140;137;139;132 | 136;135;136;137;128 | 106;111;109;106;109 | 137 | 136 | 109 | 1.248 |
| 1: strong_d6/dense low N14400/K20/T1 | 108;107;108;106;108 | 100;101;102;101;100 | 162;163;160;165;162 | 108 | 101 | 162 | 0.623 |
| 2: strong_d6/unbalanced low N25920/K16/T4 | 98;98;99;98;99 | 101;98;106;99;104 | 71;78;71;78;72 | 98 | 101 | 72 | 1.403 |
| 3: strong_d6/dense fast N15600/K6/T1 | 60;60;62;58;61 | 56;60;60;57;56 | 73;65;67;65;72 | 60 | 57 | 67 | 0.851 |
| 4: strong_d6/disconnected fast N24000/K14/T4 | 91;90;91;89;91 | 89;97;89;92;91 | 80;81;82;77;81 | 91 | 91 | 81 | unranked |
| 5: strong_d6/weak_slope_barbell fast N10560/K6/T1 | 42;40;47;46;44 | 41;41;41;44;41 | 1116;1132;1104;1109;1109 | 44 | 41 | 1109 | 0.037 |
| 6: three_fe_dense/dense low N17760/K10/T4 | 74;73;75;75;76 | 76;77;74;76;76 | 223;212;214;216;213 | 75 | 76 | 214 | 0.355 |
| 7: strong_d6/dense low N19200/K11/T4 slopes2 | 105;106;104;105;104 | 105;107;108;106;105 | 102;104;104;105;97 | 105 | 106 | 104 | 1.019 |
| 8: strong_d6/dense fast N19200/K9/T4 slopes2 | 93;94;96;93;95 | 94;94;93;94;95 | 99;98;100;98;103 | 94 | 94 | 99 | 0.949 |
| 9: strong_d6/dense low N48000/K20/T8 | 186;187;182;185;181 | 195;182;178;190;183 | 160;158;167;163;157 | 185 | 183 | 160 | 1.144 |
| 10: strong_d6/dense low N24000/K70/T8 | 359;356;357;356;357 | 356;353;351;338;348 | 293;300;287;290;287 | 357 | 351 | 290 | 1.210 |
| 11: one_fe/dense fast N17760/K6/T4 | 30;32;32;34;31 | 32;31;33;32;32 | 20;19;21;21;21 | 32 | 32 | 21 | 1.524 |

Fresh serial improves in every pair, while narrow never loses. Fresh wide has a
+5-ms loss; unbalanced has four losses and one tie. The larger low design has
three losing pairs despite its 2-ms lower marginal median. Width70 improves all
five pairs, but its 1.7% median gain is small relative to the remaining stock gap.
Excluded three-FE and low-slope medians lose 1 ms; those changes have no established
causal attribution. Do not summarize this distribution as universal improvement.

## Memory and residual costs

The production candidate payload is `8*(q*q+2*q)+120` bytes on this platform,
including its owner and optional-pointer descriptors. A coexisting canonical
owner adds another eight descriptor bytes where modeled. Borrowed indexes are
not cloned; weighted preparation and operator application allocate no new bank.
No tolerance, sample, rank/DoF, weight, separation or failure-atomicity policy
changes. Optional state ends with the linear session before inference.

Fresh wide modeled selected bytes rise 29,699,735→29,757,655; tracked peak delta
26,675,284→26,733,196. The larger N48000 case rises by 323,328 modeled and 323,320
tracked bytes. Unbalanced rises by 95,168/95,160. Scratch-file bytes are unchanged.
Small descriptor changes also appear in excluded controls. These are measured
native allocations and modeled bounds, not process-RSS or physical-cap claims.
The static allocation inventory's documented drift remains an open failure.

Current public phase telemetry narrows the conclusion without proving a detailed
operator attribution. In fresh wide repetition0, baseline/candidate projection
times are 36.39/37.80 ms and strict-final solve 35.64/36.01; unbalanced is
22.70/24.58 and 23.24/24.35. Outer/inner counts are unchanged. These are named
inclusive command phases; do not sum overlapping accounting. The earlier
instrumented 2c34503d profile attributed only about 12.5 ms of its wide diagnostic
to all Schur operator applications, already too little to supply a 34–40% public
reduction. Those timings belong to that earlier profile, not this artifact.

## Decision and next experiment

Close this matrix-operator integration. Restore the accepted computation and
retain both implementations solely in immutable evidence. A safe generation
cache and faster frozen solves do not by themselves justify production complexity
when the remaining-loss families gain little or regress. Do not add a benchmark
selector, tune q thresholds on fresh seed202609378, or accumulate another matrix
application variant here. This does not reject frame ownership as a general
mechanism or establish that stronger certificates make the speed gap inevitable.

The next larger experiment should change **canonical weighted-product ownership**:
use the existing joint FE/finite structure to avoid constructing, writing and
rescanning complete residualized control panels merely to obtain Gram, rank,
bread and reconstruction products. Compare raw weighted moments plus certified
FE coefficient products against current retained panels on identical frames.
Keep canonical rank/status/QR/span authority, with explicit cancellation bounds
and fallback to materialized rows when required. Fresh residual, score, objective,
model-space, reconstruction and final-Hessian acceptance remain independent.

First establish which products already exist in the joint owner, their exact
weight/basis generations and their numerical authority. Then implement one
coherent product-oriented candidate and test explicit WLS/Poisson/Hessian and
adversarial near-collinearity before public timing. A reusable small-system
factorization may support that organization; another isolated operator micro-tune
is not the next milestone. Remaining accepted fresh wide/unbalanced budgets to
reach 0.90 stock are 38.9 and 33.2 ms. The broad goal remains unfinished.
