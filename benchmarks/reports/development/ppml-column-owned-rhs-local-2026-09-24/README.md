# Column-owned canonical PPML RHS: local evidence, 2026-09-24

## Decision

Adopt `ownedrhs2` under [ADR 0175](../../../../docs/adr/0175-column-owned-canonical-ppml-rhs.md). Replace the worker-private retained-group-by-RHS reductions with one column-owned bank in the existing low two-categorical-FE canonical product engine. Precisely admit the earlier topology phase while the old projection buffer remains live. Admit the complete numeric plan before reclaiming that buffer. There is no new solver, tolerance, public route or persistent weight cache. Independent current-weight certification remains unchanged.

The main fresh case falls from **1102 to 860 ms**, versus **964 ms** for comparable stock fixest: 22.0% below the native baseline and 10.8% below stock. All five native pairs improve and all five beat stock; only three pairs reach a full 10% lead. The gain is admitting existing direct work within the existing budget, not making every already-admitted solve substantially faster. Six of twelve selected multi-FE medians meet the 10% stock target, two beat stock by less, and four lose. These are selected development cases, not a package-wide win rate. The broad objective remains unfinished.

## Exact source and timing boundary

Baseline source is **101d6b5dbf26a6a072f6b442f2c2dd4d088f99cd**. All 550 captured `profilecurve5` inputs match that commit. Candidate inventories capture that base plus `crates/fereg-core/src/ols/canonical_products.rs`; all **551 final captured inputs match** the final computational worktree. The final test-only extension changes the artifact hash, so its fresh timings are reported separately from `ownedrhs1`. Complete captured source, recipes, inventories and immutable audits are archived.

| Artifact | Role | Plugin SHA-256 |
|---|---|---|
| profilecurve5 | Committed baseline | 569c601e8143e0744d9359f44d9fc966a09b244ee2a29a51ad0e1840a5b2bbd0 |
| topologyphase2 | First built topology-only candidate; initial unit failures preserved | 5f9ba543828b6af7e07eac9aa8aa2d7e38eff0be9161f660f09997d1a4d45d82 |
| topologyphase3 | Corrected topology-only tests; no meaningful public gain | bbf26600b5459190949d3995efa64c44df95848f7ef06ddd69c4b7d062dd1b11 |
| ownedrhs1 | Column ownership prototype; development measurement | c0287a7bd48e094be74a17899482c796b0dbb6e16a5209d132e6b127d49de95b |
| ownedrhs2 | Accepted final source with new disconnected WLS oracle | f27b1719718146d638493caa5a6769f90f3e828e89c9a793a553caced27ec3c9 |

The first staging attempt (`topologyphase1`) could not find Cargo in PATH and failed before compilation; no plugin or fits exist for it. Its staged changed source, base identity and failure log remain. Production builds use Rust 1.85.1 for aarch64-apple-darwin with the existing fat-LTO release settings and passed ABI/panic checks. Focused unit builds use release optimization with LTO disabled and 16 codegen units, isolated per artifact; they are scientific checks, not performance claims.

The comparator is pinned, unmodified fixest 0.14.2, source archive SHA-256 `17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`, with glm.tol=1e-10 and fixef.tol=1e-11. All ranked cases are comparable positive-outcome unweighted public PPML, robust inference and saved fitted means. Defaults failing accuracy elsewhere remain separate. Ordinary fixest weights are not used as a Stata FW oracle. Native threads match the declarations; BLAS helpers use one thread.

The existing harness measures the whole public estimation command after a separate <=7680-row warmup and excludes common data import. These are warm local Mac timings using licensed Stata 19 MP, not cold-start or qualification claims. No compilation, profiling, scientific auditing, archiving or competing fits ran alongside headline measurements. Every completed fit has clear captured overlap/error lists and a terminal sentinel. Clean sampling does not prove absence of ambient operating-system load.

## Fresh distributions, seed 202609393

Five rotating baseline/candidate/stock repetitions; low mode unless stated. Every case requests robust inference and saved means. Values are median [min–max] milliseconds. Paired C/B is the full range across matching repetition numbers. Case numbers follow this table from zero.

| Design | Baseline | Candidate | Stock | C/stock median | Paired C/B |
|---|---:|---:|---:|---:|---:|
| Disconnected N55680/K102/T8 | 1102 [1070–1123] | 860 [836–882] | 964 [959–969] | 0.892 | 0.7622–0.8037 |
| Disconnected N43200/K96/T8 | 554 [550–555] | 552 [544–561] | 747 [731–752] | 0.739 | 0.9819–1.0109 |
| Disconnected N53760/K104/T4 | 979 [969–987] | 975 [974–978] | 1683 [1680–1693] | 0.579 | 0.9868–1.0093 |
| Connected N22560/K142/T8 | 716 [712–727] | 718 [711–720] | 692 [687–701] | 1.038 | 0.9861–1.0112 |
| Connected serial N17280/K138/T1 | 1233 [1231–1236] | 1233 [1226–1239] | 2746 [2741–2751] | 0.449 | 0.9919–1.0049 |
| Unbalanced N24480/K28/T4 | 95 [93–96] | 94 [93–95] | 113 [113–115] | 0.832 | 0.9688–1.0215 |
| Connected narrow N22560/K2/T4 | 42 [42–43] | 42 [41–43] | 32 [31–32] | 1.312 | 0.9535–1.0238 |
| Disconnected N44160/K10/T8 | 122 [120–124] | 124 [121–126] | 79 [77–82] | 1.570 | 1.0081–1.0328 |
| Connected fast N22560/K10/T4 | 60 [59–61] | 61 [59–63] | 57 [54–58] | 1.070 | 0.9672–1.0500 |
| Two slope partitions N22560/K10/T4 | 105 [104–107] | 105 [104–107] | 113 [108–115] | 0.929 | 0.9720–1.0288 |
| Three FE unbalanced N24480/K10/T4 | 93 [92–94] | 92 [91–93] | 95 [91–96] | 0.968 | 0.9785–1.0000 |
| Weak graph N15360/K8/T1 | 137 [136–138] | 136 [136–137] | 931 [930–934] | 0.146 | 0.9855–1.0074 |

The four stock losses are wide connected T8, narrow K2, disconnected K10 and fast K10. Each loses all five stock pairs. The three-FE control loses one stock pair despite a lower median. Every individual observation is retained, including native regressions:

| Case | Candidate slower than baseline / 5 | Native ties / 5 | Candidate slower than stock / 5 |
|---|---:|---:|---:|
| 0 | 0 | 0 | 0 |
| 1 | 2 | 1 | 0 |
| 2 | 2 | 0 | 0 |
| 3 | 3 | 0 | 5 |
| 4 | 2 | 1 | 0 |
| 5 | 1 | 1 | 0 |
| 6 | 2 | 2 | 5 |
| 7 | 5 | 0 | 5 |
| 8 | 4 | 0 | 5 |
| 9 | 2 | 0 | 0 |
| 10 | 0 | 1 | 1 |
| 11 | 1 | 2 | 0 |

Disconnected K10 rises from 122 to 124 ms and loses all native pairs (1–4 ms); its route, iterations and tracked/model memory are unchanged. Fast mode rises 60→61 ms, with four native losses; it is excluded from the changed engine. These are retained small losses, not reclassified or hidden. The two-slope and three-FE medians are 105→105 and 93→92 ms. None is evidence of universal nonregression. No guard was tuned on these fresh cases.

## Measured attribution and memory

In fresh case 0, both artifacts take four outer iterations. Native inner reported work units drop 47→32; these mix algorithms and are not operation counts comparable with fixest. Standard phase medians locate the improvement:

| Phase | Baseline ms | Candidate ms |
|---|---:|---:|
| Preparation | 105.90 | 105.91 |
| FE projection | 290.90 | 244.49 |
| Finite solve | 115.68 | 58.91 |
| Strict/final solve | 324.09 | 182.69 |
| Objective/trials | 88.76 | 89.11 |
| Inference | 50.04 | 50.51 |
| Publication | 36.60 | 36.99 |

Strict time contains nested work and must not be added to nested phases. The code and allocation-boundary oracle establish the reduced RHS admission cost; the public phase shift and inner work counts support the interpretation that admitting the existing canonical direct work drives the gain. There is no new instrumented-stock or identical-frozen-frame operation-count experiment in this milestone, so no more granular cross-package attribution is claimed.

With N rows, P eliminated groups, Q retained groups, W response/control columns and T group workers, the former RHS implementation allocated T*(QW+3W) doubles, then a separate QW merged bank. The new implementation owns one QW bank and at most QW overlapping solved coordinates. Each column visits groups/rows in fixed order and computes its original mean with compensation. Complete setup, independent certificate and reconstruction bounds still enter maximum-of-phases admission; the certificate retains its existing worker reductions. Current-weight factors remain fresh for each call.

Before component discovery, admission charges the actual parent/descriptor tables, bindings and fixed allowance while retaining the old projection buffer. It still checks the minimum future plan, then the complete topology-dependent work and numeric plan. Only after those pass may it reclaim the old buffer if necessary. Capacity-checked fallible allocation remains. Refusal before reclamation preserves the old contents and capacity.

| Case | Baseline tracked peak bytes | Candidate tracked peak bytes | Change | Candidate modeled bytes |
|---|---:|---:|---:|---:|
| 0 | 138,211,259 | 140,542,539 | +2,331,280 | 172,069,650 |
| 1 | 105,196,548 | 105,196,548 | +0 | 127,037,922 |
| 2 | 133,715,437 | 132,525,853 | -1,189,584 | 167,023,658 |
| 3 | 72,885,687 | 72,885,687 | +0 | 92,413,790 |
| 4 | 52,588,526 | 52,588,526 | +0 | 72,606,994 |
| 5 | 25,119,996 | 25,119,996 | +0 | 30,822,345 |
| 6 | 12,007,438 | 12,007,438 | +0 | 16,036,154 |
| 7 | 39,708,425 | 39,708,425 | +0 | 46,188,995 |
| 8 | 22,555,442 | 22,555,442 | +0 | 76,491,090 |
| 9 | 21,019,803 | 21,019,803 | +0 | 24,613,149 |
| 10 | 31,885,819 | 31,885,819 | +0 | 52,026,389 |
| 11 | 14,737,257 | 14,737,257 | +0 | 17,165,284 |

All modeled values are unchanged. The main successful admission increases tracked whole-command peak by 2,331,280 bytes; case 2 decreases 1,189,584 bytes. Other tracked peaks are unchanged. Smaller RHS storage does not imply a smaller whole-command peak: newly admitted direct work can consume more of the existing allowance. These are tracked/model observations, not RSS or physical constrained-memory qualification. Column ownership limits concurrency to W and changes access order, which is why narrow, serial and excluded controls are retained.

## Development alternatives

Topology-only `topologyphase3`, seed 202609347, three rotations, did not yield a meaningful improvement. It is retained as an accounting correction within the combined design, not promoted as an independent speed result. The same development definitions then evaluated `ownedrhs1`:

| Case | Topology-only B / C / stock medians ms | Column-owned prototype B / C / stock medians ms |
|---|---:|---:|
| 0 | 1033 / 1038 / 940 | 1017 / 711 / 943 |
| 1 | 681 / 682 / 650 | 678 / 676 / 689 |
| 2 | 109 / 109 / 77 | 108 / 109 / 76 |
| 3 | 40 / 40 / 30 | 41 / 40 / 31 |
| 4 | 57 / 59 / 54 | 58 / 57 / 54 |

Development case 0 is disconnected N53760/K100/T8; the remaining cases are connected N21600/K140/T8, disconnected N42240/K10/T8, narrow N21120/K2/T4 and fast N21120/K10/T4. Prototype case 0 ranges are baseline 1010–1024, candidate 708–716, stock 938–944 ms. Every native pair improves there. Full distributions, phases and every loss for both screens are in their summary/audit receipts. These timings belong to their own artifacts, not automatically to the final candidate.

The initial topology test demanded unconditional old-buffer reclamation. That was an invalid assertion: a T1 complete plan can fit while preserving the old buffer. The corrected test verifies both permitted retention and required reclamation, plus refusal without mutation. The initial 32-test selection had 30 passes and two failures: this assertion and the pre-existing `initial_full_width_lsmr_preserves_canonical_multiway_rank_policy` reporting assertion. The latter was independently rerun on committed `profilecurve5` and failed identically. It remains open; this milestone neither changes nor claims to repair it.

## Independent science and regression controls

Final source passes **35 distinct focused Rust tests**: ten canonical-product tests, 25 strict-filter tests and one final-product reuse test, with one duplicate across selections. The new four-component 100-control WLS construction crosses the old RHS memory limit while the new complete plan fits. Paired observations share explicit FE/control coordinates with reciprocal-weight errors, establishing independently known nonzero residual truth. Two changing weight frames, direct residual Gram checks, retained/streamed/tiled equivalence and one-byte budget boundaries pass. Extreme-scale, collinear, disconnected, rollback, stale-generation and current-Hessian checks remain.

All **five licensed science fixtures** pass with explicit process/Stata return codes and terminal sentinels: canonical products; disconnected nonzero Poisson truth; generated terms; multi-partition slopes; and three-FE OLS. They exercise FW/PW, offset, extreme weights, robust/cluster inference and requested saved outputs through their existing independent constructions. Two additional real sixteen-iteration allocation audits at T1/T4, pool 1, pass with zero measured allocation events/current growth, stable capacities and fresh/warm equivalence. These scoped audits do not supersede the known static allocation-inventory failure.

Across topology-only, prototype, final fresh and oracle screens, **276 public states / 184 baseline pairs / 92 direct candidate-stock pairs pass**. Final-source fresh/oracle screens account for 186 public states. Fresh independent seed 202609394 adds two bounded explicit-design Poisson/Hessian/sandwich/mean oracles (N6240/K62/T8 disconnected and N6240/K82/T4 connected), passing for all three artifacts: **six independently checked fits**. No completed application failure, timeout, scientific discrepancy or monitor failure occurred. The setup and unit failures above remain separate and visible.

OLS controls use seed 202609394, N16320, three alternating pairs each and full requested outputs. **24 applications / 12 science-output pairs pass**. They are excluded-path regression controls, not new OLS speed claims:

| Case | Baseline ms | Candidate ms | Paired C/B |
|---|---:|---:|---:|
| One FE K8/T4 | 16 [14–16] | 15 [15–16] | 0.9375–1.1429 |
| Two FE K12/T8 | 28 [28–28] | 28 [27–28] | 0.9643–1.0000 |
| Two slope partitions K6/T4 low | 101 [101–101] | 101 [101–101] | 1.0000–1.0000 |
| Three FE K8/T4 low | 60 [60–61] | 60 [60–61] | 0.9836–1.0167 |

Samples, weights, rank, DoF, separation, model space, objective, score, projection, reconstruction, finite solving, final Hessian and failure atomicity retain their existing contracts and tolerances. Frozen 77 primary / 16 holdout / 111 native-control definitions are untouched. Seeds 202609393/394 are consumed and frozen; they must not become tuning cases. No stopped campaign, SCC retry, release or qualification was run.

## Next bounded milestone

Prioritize complete public narrow/small-disconnected PPML costs. Fresh narrow 42 ms versus stock 32 requires 13.2 ms (31.4%) reduction to reach 0.90×; disconnected 124 versus 79 requires 52.9 ms (42.7%). A recurrence-only tweak is unlikely to suffice. On the present artifact the disconnected case spends 31.55 ms in projection, 10.56 ms in finite solving, 23.42 ms in strict/final work, 14.59 ms preparing and 8.55 ms publishing; strict overlaps nested phases. First establish the remaining repeated full-input passes and current-generation ownership at those boundaries before selecting a coherent lifecycle redesign. Preserve independent fresh acceptance, but avoid separately reconstructing data or geometry already owned in the same certified state.

Then address the wide connected T8 tail: 718 versus 692 ms needs 95.2 ms (13.3%) to achieve 0.90×. Its projection 155.53, strict 138.42, inference 97.61 and publication 66.90 ms identify substantial remaining work, while the existing certificate worker banks can still dominate admission. A tiled or column-owned certificate reduction is a concrete candidate only after verifying its unchanged independent residual purpose and complete phase budget. This is separate from the already-rejected full dense ordinary-direction experiment, which should not be rerun without a materially different organization.

Fast 61 versus 57 ms needs 9.7 ms (15.9%); slopes 105 versus 113 and three-FE 92 versus 95 need 3.3 and6.5 ms to reach a full 10% margin. Keep those in the control screen, but do not spend another long cycle tuning a small residual gap before the larger losses. Extend to additional slope multiplicities, generated terms and imbalance only around evidence-backed mechanisms. Predeclare fresh cases, retain losses, and stop candidates that offer only inconsistent gains or disproportionate complexity.

Known weighted constrained-fit/nonretaining-CG failures, reporting assertion, static allocation drift, seed 378 comparator discrepancy and deferred Linux/T1/8/16/physical-memory/platform qualification remain open. Earlier wide latency tails remain in their source-bound reports; no universal latency claim follows from this screen.

## Immutable receipts

The adjacent `receipts-content-addressed.zip` contains deduplicated captured sources, artifact identities, source verification, declarations, controller sentinels, public result/covariance/phase receipts, scientific audits, complete timing distributions, unit logs, sanitized fixture protocols and executable recipes. Raw licensed logs and large binary data/output files are omitted with hashes; machine paths are normalized. `receipt-index.json` maps logical paths to payloads and `archive-hashes.json` records original and archived hashes. The adjacent SHA-256 file binds the container.

Use `python3 restore_receipts.py NEW_DIRECTORY` to verify every logical payload while restoring. The archive contains **11,179 logical receipts / 2,464 payloads / 27,446,995 bytes**, all verified and restored. SHA-256: `25e0f233a436e118c40dda0706e8bc7dbb52d72d2aa412041f3a534f719b0bfe`. No computational file changed after the final artifact capture.
