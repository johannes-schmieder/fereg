# Ordinary PPML joint-engine screen and retained dense Gram improvement

Development evidence, 2026-09-24; Mac arm64. **The broad whole-public PPML
<=0.90-times-stock objective remains unfinished.** Adopt only `densegram2` under
[ADR 0155](../../../../docs/adr/0155-dense-joint-gram-and-categorical-engine-rejection.md).
Reject the categorical routing/ownership experiments; their public timing gains
do not override their losses or independent boundary failure.

## Source and scope

Baseline `coeffcheck1` matches checkpoint `a1687ddb` in all 543 captured inputs;
plugin `d6c73d8493e58d2db2ef9ac656dcbe4b828752d99b45148df33539fea2b1c684`.
Candidate `densegram2` is that base with the dense Gram helper and its test,
and the existing extreme-weight regression test extended to fast storage.
All 543 captured inputs match the committed implementation. Plugin:

`a5768ce3774837d1968f21f956b9457cd35571760a6e2b0a01a88a1b26095a4c`.

Only dense joint finite Gram entries change reduction order. Eight fixed FMA
chains replace one dependent chain; the upper triangle, current normalization,
factorization, recurrence and every acceptance check remain. This benefits the
established fast multipartition slope engine. Categorical automatic routing,
explicit engines, low storage, OLS and the frozen 77/16/111 definitions remain.
There is no new heap bank; the helper uses eight f64 stack accumulators.

Stock fixest is the pinned unmodified library from the existing harness archive,
SHA-256 `17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads and serial BLAS.
All ranked PPML fits request robust inference and fitted means. Timings are whole
public command clocks after the harness warmup, not process startup or fixture
creation. Input generation precedes fits; native loading and requested output
work remain inside the command. Default-reference accuracy claims are not reused.
No build, profiling, archiving or competing fit ran alongside these timing blocks.
The process monitors record no detected overlap.

## Retained public results

Development seed 202609347; three rotations. Times in milliseconds, listed in
repetition order. Baseline/candidate/stock execution order rotates each repetition.

| Design | Baseline | Candidate | Stock | Median change |
|---|---|---|---|---:|
| Two slope partitions N30720/K6/T4 | 121,121,122 | 120,121,120 | 123,112,122 | -0.8% |
| Unbalanced three FE, two slope partitions N23040/K32/T1 | 472,467,468 | 400,397,399 | 1356,1348,1344 | -14.7% |
| Two slope partitions N19200/K64/T8 | 338,330,337 | 296,295,298 | 379,371,368 | -12.2% |

Fresh **202609363 is consumed and frozen**. Cases and five rotations were declared
before generation or measurement; no tuning used them. Three fresh cases are
below the requested stock target, but the narrow first case still loses.

| Fresh design | Baseline median | Candidate median | Stock median | Candidate/stock | 0.90 stock target |
|---|---:|---:|---:|---:|---:|
| Two slope partitions N32640/K7/T4 | 141 | 139 | 114 | 1.219 | 102.6 |
| Unbalanced three FE, multiple slopes N34560/K12/T4 | 210 | 203 | 269 | 0.755 | 242.1 |
| Unbalanced three FE, two slope partitions N24960/K30/T1 | 444 | 387 | 1243 | 0.311 | 1118.7 |
| Disconnected two slope partitions N21120/K60/T8 | 348 | 310 | 472 | 0.657 | 424.8 |

Full fresh distributions (baseline; candidate; stock), milliseconds:

- K7: `[143,141,144,141,141]`; `[139,138,143,138,142]`; `[114,114,111,110,115]`.
  Paired differences `[-4,-3,-1,-3,+1]`; 1.4% median improvement.
- K12: `[209,212,206,212,210]`; `[203,203,203,202,207]`; `[269,268,263,272,269]`.
  Differences `[-6,-9,-3,-10,-3]`; 3.3% improvement.
- K30: `[438,438,452,444,445]`; `[380,387,390,390,384]`; `[1228,1247,1240,1256,1243]`.
  Differences `[-58,-51,-62,-54,-61]`; 12.8% improvement.
- K60: `[352,343,341,354,348]`; `[308,310,317,313,301]`; `[476,471,463,472,477]`.
  Differences `[-44,-33,-24,-41,-47]`; 10.9% improvement.

Three-rotation controls: one FE 44→44 vs26 (one +1-ms pair); categorical two FE
79→78 vs64; low slope mode 99→97 vs193. These routes do not call the changed
kernel. Both OLS controls have exact exports: categorical 35→35 with a +1-ms
pair, slopes 68→69 with differences `[+2,+1,+1]`. That small consistent control
loss is retained, not attributed or hidden by an aggregate win rate. The second
small independent-oracle pilot also has a +2-ms single-pair loss. No universal
non-regression or small-fit improvement claim is made.

## Science, state and memory

The final candidate passes **66 distinct Rust tests**, the build's ABI/panic
containment test and **four licensed fixtures**, including independent explicit
Poisson/Hessian/sandwich checks for fast slopes, unit/FW/PW, T1/T4, robust/cluster,
categorical controls, checkpoint rollback and separation. Ordinary fixest weights
are not used as a frequency-weight inference oracle.

The retained blocks contain **132 successful public applications**, **120 passing
public-state audits**, **six independent Python oracle comparisons**, **46 passing
native pairs** and **40 passing direct candidate/stock comparisons**. No application
failure, timeout, scientific exclusion or detected overlap. Fifteen native export
pairs are exact. Maximum native differences: beta 1.110223e-16, covariance
5.929231e-20, means 3.552714e-14, likelihood 4.547474e-13, deviance zero.

All forty PPML pairs have identical outer/inner iterations, fallback/strict-rejection
counts, rank, DoF, step halving, **tracked peak allocation, modeled selected bytes
and scratch bytes**. Independent weighted WLS tests cover frame changes, rank,
reconstruction and invalidation. The newly extended fast/low extreme-component
truth test passes without changing its 1e-10 envelope or bounded iteration policy.
There is no physical-memory-cap, Linux, Windows or qualification claim.

## Frozen solver comparison: promising inner work, insufficient public result

Capture artifact `jointcapture1` records all 22 ordinary/strict frames from four
successful development public fits: dense N16320/K16, unbalanced N15360/K10,
narrow N12000/K3 and weak N7680/K4. Captures bind identical weights, outcome,
means, controls, FE codes, tolerance and cap. Step halving and strict rejection
are zero. Compare the existing joint LSMR owner (including its retained warm state)
with zero-start coefficient joint solving across each complete frame sequence.
Construct owners outside the clock; current weighted geometry, solve and fresh
native acceptance are inside. These are diagnostic inner clocks, not public
headlines or stock comparisons. The coefficient call omits the public session's
additional original-input checks.

First replay: the existing coefficient solver loses dense T1, 35.962 versus
32.230 ms summed over frames, despite winning the other seven scopes. The second
replay rotates LSMR, old coefficient Gram and eight-chain coefficient Gram within
each frame, with three repetitions and forced coefficient geometry invalidation.
All 396 calls pass native certification; no detected external contention.

| Frozen scope | LSMR ms | Old coefficient ms | Revised coefficient ms |
|---|---:|---:|---:|
| Dense T1 | 31.940 | 36.879 | 23.239 |
| Dense T4 | 20.468 | 17.239 | 13.556 |
| Unbalanced T1 | 16.762 | 15.664 | 11.748 |
| Unbalanced T4 | 11.961 | 9.166 | 7.879 |
| Narrow T1 | 14.574 | 7.173 | 6.711 |
| Narrow T4 | 11.307 | 5.557 | 5.360 |
| Weak T1 | 4.734 | 3.997 | 3.654 |
| Weak T4 | 4.275 | 3.019 | 2.877 |

Dense T1 median Gram work falls 16.096→3.118 ms across the captured sequence.
Independent grounded explicit-design weighted LU, cross-checked with separately
equilibrated SciPy LSMR, passes all **42 strict outputs**; maximum strict residual
projection error is 1.695339e-11. Earlier ordinary frames retain their captured
scheduled tolerances and are recorded separately, not relabeled strict.

## Rejected public organizations and independent boundary failure

The source-bound prototypes are preserved, not installed:

- `jointcoeffproto1`: auto two FE uses the existing generic coefficient owner,
  dense Gram improvement and separate borrowed canonical projector. Public medians
  include dense 80→74, unbalanced 69→76, narrow 50→52, weak 31→37 ms. It passes its
  ordinary public science screen but has mixed performance.
- `jointcoeffproto2`: retain accelerated canonical checkpoints. Two licensed
  fits fail atomically with r(3499), because the publication guard did not admit
  fast MAP ordinary directions plus LSMR checkpoint identity. No timings collected.
- `jointcoeffproto3`: correct that prototype envelope and remove an unused numeric
  binding. Dense 82→77, unbalanced 67→74, narrow 50→45, weak 31→32; dense K16/T1
  98→105, K64/T1 600→493 and K64/T8 325→279 ms. Reliable serial losses remain.
- `jointcoeffproto4`: feed original controls through the existing dense bank and
  reuse their freshly computed norms for the independent compensated original
  scores. Dense 82→82, unbalanced 68→76, narrow 50→44, weak 34→32; medium-width
  serial loss persists. This is not adopted either.

The first, third and fourth public screens total 189 applications, all with
passing captured public science. **Those screens missed an important boundary.**
Extending the existing low-mode extreme-weight truth fixture to fast storage
finds a failure in the proposed fast categorical joint path. It has disconnected
components with roughly thirteen orders of weight disparity. Both the original
canonical coexistence and component-balanced checkpoint attempts fail the
unchanged 1e-10 predictor envelope. The original-control successor returns
1.234325308629991e-9. An isolated `a1687ddb` replay with the same added fast fixture
passes. The entire categorical extension, including publication guard and norm
sharing, is removed. The passing fast fixture is retained as a regression test.

The failure is localized to integration of this ordinary joint route; its exact
initial/ordinary/strict state origin is not yet established. Do not infer that
coefficient-space mathematics is invalid: frozen independent solves pass. Nor
should passing weighted normal equations alone override this predictor oracle.
The rejected all-coefficient canonical experiment from ADR 0136 remains separate.

## Measured remaining work and next milestone

Separate `coeffcheckledger1` / `jointcoeffledger1` diagnostics retain eight public
applications. Dense native root is 71.619→65.539 ms; the coefficient inner calls
cost 11.235 ms, but their full session wrapper costs 20.745 ms. Baseline joint
LSMR totals 23.700 ms. Unbalanced inner coefficient calls cost 10.368 ms while
the wrapper costs 22.107 ms, compared with baseline LSMR 19.734 ms. Canonical
reprojection also differs. These nested clocks cannot be added as disjoint costs.
Current native inference in those fits is only about 0.6–1.6 ms, so inference is
not the dominant explanation for these particular remaining gaps.

The next candidate should use coefficient-space **Newton increments behind the
existing categorical incremental-direction interface**, preserving independent
canonical response history and finalization. First trace the extreme-component
fixture through initial, ordinary and strict/final states with cold explicit
oracles. Then port the efficient operator/current-weight preparation, avoiding
the generic absolute-WLS wrapper and its duplicate passes. This is a concrete
ownership/solver redesign, not another dimension threshold or a claim that the
failed predictor state's cause is already proven. Keep sparse/weak automatic
LSMR wins and low-memory/OLS controls protected.

Static allocation source drift, constrained weighted fits, nonretaining-CG span
instability, prior one-FE reference and large-shift discrepancies, synthetic
three-FE LSMR refusal and deferred platform/cap work remain open. No SCC retry,
qualification, stopped campaign, push or release was performed. Seed 360 stays
frozen; 361 was declared but never generated/measured. Protected 211/212 remain
untouched. Do not transfer historical categorical wins to this artifact.

## Immutable receipts

`receipts-content-addressed.zip` contains 14,834 logical receipts, 2,881 unique
payloads, 23,685,753 bytes. SHA-256:

`9ce0122427002eb118dbc0775fe8bd4f7b35e577f327bdad83986717570529df`.

Run `python3 restore_receipts.py NEW_DIRECTORY` to verify and restore. The archive
contains source/artifact identities, captured source trees, declarations, build
and test receipts, frozen harnesses/oracles, public plans/results/distributions,
phase traces and rejected patches. Licensed raw logs, generated datasets, means
binaries and compiled artifacts are omitted with hashes. Paths are sanitized;
original and archived hashes are separate. Key restored files under
`joint-engine/` are `retained-summary.json`, `retained-metadata-deltas.json`,
`retained-maximum-native-differences.json`, `retained2-tests.json`,
`densegram2-licensed-results.json`, `frozen-summary3.json`, `frozen-oracle3.json`,
`ledger-summary.json`, and the prototype/baseline dynamic-range receipts.
