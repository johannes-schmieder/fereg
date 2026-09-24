# PPML ordinary joint controls: current gaps, dataflow change and validation

Local macOS arm64 development milestone, September 23, 2026. Adopt `jointorig2`
under [ADR 0144](../../../../docs/adr/0144-original-controls-for-streamed-ppml-directions.md).
Borrow original prepared controls for low-memory joint directions, reuse each
normalized row across its Hessian tile, and bind narrow numeric original-input
scores. This removes repeated ordinary scratch reads without another N-by-K
matrix. It does **not** complete the whole-public-command <=0.90-times-stock goal.

## Source and measurement identity

Baseline `ckptlow5` matches `7a68c163`, plugin
`c07ad1086ef53f4fa7e79b6c2ee6a8b2a4c61decfce757077d57006f65337eef`.
Final candidate `jointorig2` overlays two computational files on that commit:
`fe/slope_coefficients/joint.rs` and `ols/linear_session.rs`. All **541 captured
computational inputs** match the adopted worktree. Candidate plugin SHA-256:

`ebf09233b4f56e01cd18accd7c0d6183f46b329025edbc33b077de7c0f9b91de`.

Stock is pinned, unmodified fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched requested threads and serial BLAS.
PPML times include the public estimation command, robust covariance and fitted
means; stock includes response prediction. Startup, data generation/import,
warmup and exports are outside both clocks. Memory(low) versus stock is not a
physical-memory-matched comparison. OLS controls use their captured surface.

Rotations alternate baseline/candidate/stock within each repetition. The controller
records application status, sentinels and process isolation. All completed
headline attempts have no detected competing fit/build or monitor errors.
Diagnostic plugins, tests, compilation and archival work run separately from
headline fits. No qualification campaign, remote run, SCC retry or push occurred.

## Current opportunity and measured attribution

The initial current-source screen uses seed 202609347 and three rotations:

| Public case | fereg median [min,max] ms | Stock median [min,max] ms | Science |
|---|---:|---:|---|
| One FE N61440/K10/T4 low | 92 [89,92] | 52 [51,55] | Unranked: fitted means |
| Same, fast | 99 [96,99] | 51 [51,53] | Unranked: fitted means |
| Two FE N30720/K6/T4 low | 126 [123,126] | 59 [56,60] | Comparable |
| Same, fast | 98 [96,101] | 59 [58,59] | Comparable |
| One FE N8192/K3/T4 low | 19 [19,20] | 14 [14,15] | Comparable |

The two large one-FE comparisons fail the unchanged mean envelope at five rows.
Coefficients agree to about 1e-16 and covariance/objective comparisons pass.
An independent group-intercept calculation conditional on each fit's own beta
uses log-sum-exp group totals. Native maximum mean discrepancy is <=3.2e-14;
stock is 7.14e-7, maximum tolerance ratio 1.611. This localizes the mismatch to
stock fitted means; it does not turn these timings into ranked wins or losses.
Both packages' separately audited public stationarity/model-space states pass.

Separate diagnostic builds `narrowledger1/2` and `jointorigledger1` instrument
the whole native command and nested phases. The timed fit is distinguished from
warmup; exclusive spans subtract the union of children and sum to the native
root. These are single-fit diagnostics, not stock-comparator headline results.
For low two-FE N30720/K6/T4, six joint directions yield:

| Diagnostic work | Baseline ms | Original-control candidate ms |
|---|---:|---:|
| Whole native plugin | 117.989 | 109.888 |
| Joint direction total, including setup/checks | 45.661 | 37.763 |
| Streamed finite setup outside iteration | 23.377 | 18.113 |
| Fresh joint reconstruction | 6.988 | 4.992 |
| Krylov iteration outside reconstruction | 1.371 | 1.403 |
| Remaining joint owner/setup/checking work | 13.925 | 13.256 |

The last four rows partition the joint total. Preparation includes coupling and
finite products; this instrumentation does not separately attribute every
operation within it. The measured saving comes from setup/reconstruction, not
a faster recurrence. Source-derived ordinary row visits include one norm scan,
two scans per bounded tile and two reconstruction scans. At K6/T4, six calls
therefore imply at least 61.9 MB of decoded scratch values in the former path;
curvature fallback could add work. This is a source-derived access count, not
measured physical disk traffic. The new path borrows the existing raw columns.

The same baseline diagnostic records 76.036 ms native for one FE, versus 92 ms
whole public: 15.964 ms lies outside the plugin span. Its two implicit canonical
refreshes cost7.956 ms, finite moments5.890 ms, six objective evaluations7.733 ms,
and two concentrated direction preparations4.193 ms inclusive. Fast two-FE is
85.861 ms native/99 ms public; five ordinary LSMR calls cost25.195 ms, including
14.929 ms recurrence, 3.872 ms preconditioner and 3.924 ms finish/certification.
Whole-minus-native differences have the public timer's millisecond resolution.
These measurements do not establish that any remaining cost is scientifically
unavoidable. No new frozen-weight stock/native kernel ledger was collected:
the already small low-mode recurrence was not the target of this change.

## Candidates and adopted organization

`jointrows1` first reuses the existing beta bank for a normalized finite row and
widens the ordinary original-input score binding to the existing score/mean
binding policy. It preserves all numerical export bits in 18 pilot/screen native
pairs, but the representative public gains are only 2–5 ms. Do not call that
experiment sufficient closure of the gap.

`jointorig1` additionally replaces the ordinary scratch source with an immutable
original-column view mapped through stored generated-column identities. Originals
and retained controls span the same joint finite/FE model. Weights, FE numerical
geometry, finite products and checks remain current. The response representative,
canonical rank/within design and final inference storage remain unchanged.
It improves development low two-FE128→117 ms and low two-slope150→142 ms.

`jointorig2` removes the unused former scratch joint adapter. Numerical statements
match `jointorig1`, but final headline confirmation is measured on `jointorig2`;
predecessor timings are not transferred to it. All final comparisons use the
existing scientific envelopes. The changed representative adds two to eight
inner iterations in accepted changed-path cases. There are no new fallback events
or strict profile rejections in these blocks. Initial/strict canonical rank,
inverse, fresh FE and original-input finite scores, reconstruction, objective,
two consecutive outer passes and exact final-Hessian conditions remain mandatory.

## Final public distributions and remaining losses

Final confirmation, three rotations at control seed 202609347:

| Case | Baseline ms | Candidate ms | Stock ms | Candidate/stock |
|---|---:|---:|---:|---:|
| Low two FE N30720/K6/T4 | 130 [128,134] | 118 [113,121] | 58 [58,59] | 2.034 |
| Low dense three FE, same N/K/T | 237 [232,237] | 228 [224,231] | 234 [232,236] | 0.974 |
| Low two slope partitions, same N/K/T | 150 [149,150] | 141 [141,141] | 121 [120,121] | 1.165 |
| Fast two slope partitions, same N/K/T | 124 [122,124] | 122 [120,122] | 116 [115,120] | 1.052 |
| Low one FE N61440/K10/T4 | 92 [90,93] | 93 [89,98] | 53 [53,55] | Unranked |

Fresh seed 202609349, five rotations; weak case has three. All 28 pairs improve
and all stock comparisons in this block pass. Complete distributions and paired
differences are in `summary.json` and individual receipts.

| Low-memory fresh case | Baseline ms | Candidate ms | Stock ms | Candidate/stock |
|---|---:|---:|---:|---:|
| Two FE N49200/K8/T8 | 203 [200,210] | 194 [191,196] | 78 [76,80] | 2.487 |
| Unbalanced three FE N24580/K3/T1 | 147 [146,152] | 143 [137,146] | 110 [103,111] | 1.300 |
| Unbalanced three FE, two slopes, N38400/K8/T4 | 238 [237,241] | 230 [227,233] | 206 [201,209] | 1.117 |
| Two FE N16320/K24/T8 | 239 [233,246] | 230 [227,235] | 71 [66,73] | 3.239 |
| Two FE N24480/K10/T1 | 204 [199,208] | 195 [190,198] | 142 [139,143] | 1.373 |
| Weak barbell, two slope partitions, N7680/K4/T4 | 37 [36,49] | 34 [33,34] | 216 [216,217] | 0.157 |

Five fresh medians improve 2.7–4.4%; weak median improves 8.1%. The weak case meets
the 10% lead, while the five other fresh cases still lose. The large K24 loss is
not hidden by the weak-graph win.

Excluded PPML controls, three rotations: low one-FE K32 is 103→105 versus 119 ms
(paired+1,-1,+2); fast two-FE100→96 versus 58 (-2,-4,-4); fast three-FE 182→183
versus 232 (+1,+1,+1). Fast slopes' confirmation pairs are -4,0,-2. The one-FE K10
confirmation pairs include+8,-3,0 ms. All these control exports/means are exact.
OLS controls have exact coefficient/covariance exports: slope67.5→66.5 ms
(0,-2), low three-FE65→66.5 (+3,0). These small timing losses are retained; this
is a focused control result, not a universal non-regression claim.

## Validation, memory and preserved failures

- **155 distinct Rust tests** pass on the exact final captured source in its own
  Cargo target: analytic changing-weight WLS, selected-column permutations,
  rank-deficient slopes, strict/final basis changes, rollback, generated inputs,
  memory refusal and allocation boundaries. Production actual-plugin ABI and
  panic containment pass. This is not a full Rust/platform qualification.
- **Nine passing final licensed runs of seven fixtures** cover explicit weighted
  Poisson/Hessian/sandwich, FW/PW and clustered inference, generated factor terms,
  FE shifts 0/1000, weak weighted categorical fits, rollback and separation.
  Four real allocation windows cover categorical/slopes at pool/T1/1 and 2/4;
  each has zero events/growth, stable capacities and fresh/warm equivalence.
- Across baseline and candidate headline blocks: **311 passing applications**,
  **303 passing independent public-state checks**, **36 passing small explicit
  Poisson/Hessian oracle results**, **95 passing native pairs**. There are 106
  baseline/stock comparisons, 15 with the preserved one-FE mean discrepancy;
 91 direct candidate/stock comparisons contain 9 such failures. They stay unranked.
- All final PPML modeled peaks and scratch counts are unchanged. Tracked peaks
  are unchanged in 54 of 55 pairs and decrease by 32 bytes in one small pilot.
  No new heap or resident N-by-K state is introduced. Scratch still serves
  canonical products/inference. This does not qualify physical memory caps or
  repair the static allocation inventory's documented drift.
- A premature diagnostic launch was blocked by process isolation while its
  build remained active; no measurements started. Two fresh data-generation
  attempts failed before fits: paper N must be divisible by 240, and the panel
  generator rejects a singleton partial worker. Revised declarations use complete
  groups (49200/16320/24480 and 24580); no estimator outputs informed that change.
  Frozen generator/benchmark definitions are unchanged.
- The first added boundary fixture used unsupported continuous-interaction
  syntax and failed closed. Its successor uses supported numeric/factor terms.
  A1e6 FE shift then reaches the 10000-iteration cap on **both baseline and
  candidate**. The diagnostic baseline labels the failing shift; the separately
  named 0/1000 fixture passes on both. The1e6 failure is unresolved, not converted
  into a pass or claimed repaired. All failed fixtures, hashes and controller
  results are preserved; raw licensed logs are omitted with hashes.

## Next bounded work

Close this data-access experiment. The changed path's remaining joint work is
mostly preparation/checking, not recurrence. Extend attribution to the fresh
wide low two-FE case before changing its kernels. Its final artifact reports
112.8 ms in FE projection and 65.0 ms in strict final solving; these counters
overlap and must not be summed. Test parallel formation of independent finite
Hessian rows using the newly borrowed original controls and existing bounded
buffers, then address measured two-FE canonical construction. Preserve the
current serial arithmetic per row or independently validate changed reductions.
Do not tune thresholds to these cases or seed 202609349.

One-FE still needs a separate whole-command effort: the measured16-ms outer
Stata portion, implicit canonical products and family evaluations are concrete
targets, while its ordinary finite solve is already small. The original-control
change is not a reason to claim one-FE improvement. Fast categorical LSMR, wide
inference and remaining slope setup remain open. Protected seeds202609211/212,
frozen 77/16/111 definitions, SCC deferral, constrained-fit and nonretaining-CG
failures, and stopped campaigns retain their existing status.

## Immutable receipts

The verified archive contains **12,730 logical files**, 2,572 payloads and
22,034,739 bytes. SHA-256:

`14e8c5064e27a0182b10074add7fe9de7aa5c9d191e58a38785ec0ab6f353365`.

It contains exact source snapshots, identities/builds, declarations/controllers,
native diagnostic traces, distributions, exports, independent audits and failures.
Input/output binaries and licensed logs are omitted with hashes. Machine paths
are sanitized; the manifest records both original and archived hashes. Restore
and verify every logical receipt with:

```bash
python3 benchmarks/reports/development/ppml-joint-original-controls-local-2026-09-23/restore_receipts.py target/restored-joint-original
```

Verification/restoration completed locally before the handover was updated.
