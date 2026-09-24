# Streamed canonical PPML group coefficients — local development, 2026-09-24

Adopt the complete-budget coefficient-lifetime policy in
[ADR 0172](../../../../docs/adr/0172-streamed-canonical-group-coefficients.md).
It opens the existing direct canonical product engine to two wide low-memory
families: fresh medians improve **694→557 ms** and **1128→759 ms**, respectively
**36% and 25% faster** than comparable pinned stock. This closes the current
storage experiment, not the broad multi-FE PPML performance goal.

## Source, comparator and timing scope

- Baseline **canonicalwork1**, plugin SHA-256
  ae8859fd082e6aa6481ab814c893d5ff119a5c0ee2ff15350577002c893b5579,
  accepted at 91fa4c7600cfc39002dff573f40ea30682039305.
- Accepted **canonicalstream3**, plugin SHA-256
  49df141053ad0e99603fb33147012a2e1cd2cd8157ca8d2c1470b3f7590b5399,
  captured from that revision plus only ols/canonical_products.rs. All **548
  captured inputs match** final source. No diagnostic instrumentation is in
  the measured production artifact.
- Stock is the pinned, unmodified fixest 0.14.2 source archive, SHA-256
  17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9.
  Public Poisson calls use glm.tol=1e-10 and fixef.tol=1e-11, matched robust
  inference and fitted means. Default-reference failures remain separate.
- Local Mac T1/T4/T8 development, low mode unless specified, positive unweighted
  numeric PPML data. FW/PW and generated terms receive separate independent
  licensed checks; these timings are not frequency-weight inference evidence.
  Data generation and process startup are outside the public-command timer;
  preparation, estimation, inference and requested output are included.
- Development seed 202609347 is reused as declared. Final seeds
  **202609387/202609388** were declared before generation and are consumed/frozen.
  First-policy seeds **202609385/202609386** also remain consumed/frozen.
  The 77 primary, 16 holdout and 111 native-control definitions are unchanged.

All headline calls use the separately captured CPU-aware monitor, SHA-256
1c7fe37c714ba2516869b3e42b6d33d639792865152d42b348a75b1959e52686.
All application records are complete with no detected overlap or monitor error.
No compilation, profiling, oracle audit, archiving or competing fit ran alongside
headline measurements. The unrelated Stata GUI was left untouched and monitored.
This remains sampled local development evidence, not platform qualification or
proof of absence of ambient OS variation.

## Diagnosis and adopted organization

The previous canonical product engine could already beat iterative projection
plus separate products. Its overlapping P-by-W eliminated-group coefficient bank
and private RHS/certificate reductions nevertheless refused useful wide calls.
On the development connected N21120/K100/T8 design, the old complete peak is
4,456,416 bytes versus the 3,379,200-byte allowance; the streaming plan requires
3,202,400. At N42240 the corresponding old/new/allowed counts are
8,138,336 / 6,315,488 / 6,758,400. These are structural phase calculations from
actual data dimensions, not observed allocations.

RHS workers now own complete eliminated groups. They load an affordable retained
center or compute one group in bounded scratch, assemble its original-input RHS,
and merge compensated weighted means. All RHSs still share one freshly weighted,
component-grounded Schur factor. Original-input residual replay still forms the
direct Gram and moments against both original FE partitions before acceptance.

The complete phase plan retains coefficients whenever affordable. Otherwise it
streams group coefficients through RHS assembly and the full certificate, drops
all reductions, and rebuilds the bank for retained-row writing using the same
pure recurrence and immutable inputs. Streaming adds two original-column center
passes and one right-effect averaging pass. Cached calls avoid those passes.
There is one solver and one acceptance implementation, with no seed/width-specific
threshold, larger memory budget, or cross-weight certificate authority.

Samples, separation, weight semantics, rank/DoF, finite/span/inverse checks, score,
objective, reconstruction, final Hessian and failure atomicity are unchanged.
The current file changes only admitted low two-categorical-FE canonical PPML
construction. Ordinary joint IRLS, fast mode, slopes, three-plus effects and OLS
retain their engines. Every fresh weight frame is independently certified.

## Experiment sequence and rejected policy

The initial current-baseline screen has 18 passing native/stock calls. Baseline
medians are 510/872/315 ms versus stock 437/773/521 on the three development cases.
Each block has its own process/timing receipts; do not transfer those medians to
another block.

The always-streaming **canonicalstream1** artifact, plugin
053f4740af986b703a0f631b4bfb2d47b404b039a29c087bccd427eb579d9769,
passes science and improves newly admitted development cases 504→353 and 860→574
ms. Its fresh block has 180 passing states and 60 passing direct stock pairs, but
already-admitted serial/wide/unbalanced medians worsen 811→823, 320→323 and
104→109. That avoidable storage policy is rejected; all its receipts remain.

The refinement chooses retention from the complete memory budget. Its eight
canonical and six strict tests pass. An initial test compile missed the new
storage-plan boolean argument; E0061 and the intermediate source are preserved
in test-cache-1.log and cache-missing-test-arg.rs. Correcting that test call changes
no scientific tolerance. The former one-byte test now uses the minimum complete
streaming bound: a smaller budget can legitimately switch an otherwise cached
call to streaming before it must refuse.

**canonicalstream2** contains the final executable logic. The final capture
**canonicalstream3** only clarifies two comments to describe both lifetimes;
public fixtures, oracles and final timings use canonicalstream3. The refined
three-case development block gives 496→352, 832→565 and 309→310 ms, versus stock
411/757/503. All 27 states, 18 baseline pairs and nine direct stock pairs pass.
No frozen validation seed was reused to tune the refinement.

## Fresh public distributions

Five rotating baseline/candidate/stock calls per case, milliseconds, median
[min,max]. Last column is the paired candidate/baseline ratio, median [min,max].
Candidate/stock is the ratio of their medians. Full values and paired stock ratios
are in summary-all.json. Scientific discrepancies would be unranked; none occurs
in this final screen.

| Case | Baseline | Candidate | Stock | C/stock | Paired C/B |
|---|---:|---:|---:|---:|---:|
| Connected wide N28800/K102/T4 | 694 [692, 697] | 557 [552, 559] | 870 [862, 877] | 0.640 | 0.803 [0.796, 0.805] |
| Connected larger wide N53760/K106/T8 | 1128 [1117, 1153] | 759 [756, 768] | 1016 [1009, 1036] | 0.747 | 0.670 [0.666, 0.684] |
| Connected serial, already admitted N18480/K98/T1 | 776 [775, 777] | 770 [770, 775] | 1717 [1717, 1721] | 0.448 | 0.992 [0.991, 1.000] |
| Disconnected, memory refused N55200/K100/T8 | 1082 [1053, 1085] | 1087 [1073, 1098] | 944 [931, 951] | 1.151 | 1.012 [1.002, 1.025] |
| Very wide, memory refused N21600/K140/T8 | 859 [847, 862] | 858 [853, 865] | 668 [654, 673] | 1.284 | 1.002 [0.995, 1.007] |
| Wide, already admitted N27360/K72/T4 | 318 [315, 320] | 316 [315, 319] | 519 [515, 521] | 0.609 | 0.997 [0.991, 1.003] |
| Narrow N22560/K2/T4 | 50 [49, 50] | 50 [49, 53] | 32 [31, 32] | 1.562 | 1.000 [0.980, 1.060] |
| Unbalanced N24480/K26/T4 | 109 [108, 115] | 109 [108, 114] | 104 [99, 105] | 1.048 | 0.991 [0.982, 1.009] |
| Fast mode N22560/K10/T4 | 66 [66, 69] | 67 [66, 68] | 58 [56, 58] | 1.155 | 1.015 [0.957, 1.030] |
| Two slope partitions N22560/K10/T4 | 106 [105, 107] | 105 [105, 109] | 113 [110, 114] | 0.929 | 1.000 [0.981, 1.028] |
| Three FE N24480/K10/T4 | 116 [116, 117] | 116 [116, 116] | 351 [350, 353] | 0.330 | 1.000 [0.991, 1.000] |
| Weak graph N12480/K8/T1 | 53 [50, 54] | 51 [50, 52] | 1524 [1519, 1527] | 0.033 | 0.962 [0.926, 1.020] |

Every paired native call improves on the two new wide targets. Six selected
medians meet the <=0.90× stock target; slopes are faster but miss that margin;
five cases still lose. This is not a coverage-wide win rate. The disconnected
control's +5 ms and fast control's +1 ms median losses, narrow 53 ms outlier and
other small paired losses remain visible. There is no universal improvement claim.

## Phase attribution and memory

Public native phase medians, ms; these are phase measurements, not a new isolated
fixest profile. Independent medians need not add to the median command time.

| Target / artifact | Preparation | FE projection | Finite solve | Strict final | Inference | Publication |
|---|---:|---:|---:|---:|---:|---:|
| 0 / canonicalwork1 | 46.06 | 187.75 | 71.71 | 195.67 | 49.50 | 29.82 |
| 0 / canonicalstream3 | 45.84 | 142.98 | 49.33 | 121.25 | 49.77 | 29.92 |
| 1 / canonicalwork1 | 89.16 | 276.37 | 123.76 | 363.26 | 56.79 | 39.39 |
| 1 / canonicalstream3 | 88.65 | 177.57 | 62.18 | 154.98 | 56.40 | 39.26 |

Projection, canonical finite work and strict final solving supply the gains;
preparation, inference and publication remain similar. Ordinary joint directions
are unchanged. Whole-action tracked peaks rise **1,322,960** and **2,768,112 bytes**
on the newly admitted targets, inside the unchanged model. Already-admitted
serial/wide controls add 792/2,336 bytes; the other fresh tracked peaks are
unchanged. Maximum fresh tracked/model ratio is **0.854394**. Scratch-file sizes
are unchanged. Total RSS and physical constrained-memory behavior were not tested.

The final topology-based diagnosis is explicit about remaining refusals:

- Disconnected N55200/K100/T8 has P9200/Q920. The minimum solve phase requires
  **8,968,720 bytes**, exceeding its **8,832,000-byte** allowance. Worker Q-by-W
  RHS reductions and factors, rather than the removed coefficient overlap, now
  limit admission. Blocking RHS reductions is a concrete future candidate.
- N21600/K140/T8 needs **4,533,632 bytes** at full coefficient reconstruction,
  versus **3,456,000** allowed. Further admission needs bounded retained-row
  reconstruction that avoids the full P-by-W bank, not a larger guard.

These are source/formula inferences from actual generated topology, not measured
route counters; unchanged tracked peaks and public phases corroborate refusal.
Their final public losses are 1087 versus 944 ms and 858 versus 668 ms.

## Independent science and controls

- **14 distinct focused core tests** pass: eight canonical-product tests and six
  strict PPML tests. The new N8640/K100 nonzero-residual WLS oracle pairs rows with
  identical full explicit design and reciprocal-weight errors. It proves known
  finite coefficients and residuals, direct Gram reconstruction, exact numerical
  equality of streamed/cached products, changing weights, T1/T8 and minimum-budget
  refusal. Existing tests retain extreme scale, near-collinearity, component,
  generation, rank, invalid-frame and rollback checks.
- **Five licensed fixtures** pass with explicit fixture status and terminal
  sentinels: canonical products, categorical joint PPML, multi-partition slopes,
  independent generated-term/FW/PW clustered Poisson and three-FE OLS. The final
  artifact's plugin ABI/self-test passes. No broad Stata profile is claimed.
- All **180 final fresh public states**, **120 baseline pairs** (60 native and
  60 stock), and **60 direct candidate/stock pairs** pass. Each state receives
  sample, score, fresh model-space and objective replay checks.
- Independent explicit-design Poisson/Hessian/sandwich/means checks pass all
  **six** native/stock fits on fresh N6960/K96 and disconnected N6000/K88. FE ranks
  are 1188/1096, residual DoF 5676/4816, oracle Newton iterations 6/5. These wide
  checks exercise the new boundary, not only tiny-regressor surrogates.
- All **24 OLS calls and 12 output/science pairs** pass. Three rotations of
  unchanged one-FE/all-output, two-FE, low slope and low three-FE controls give
  16→15 / 28→28 / 105→105 / 60→61 ms. These are focused controls, not OLS
  requalification. Changed-file rustfmt and git diff --check pass.

The pre-existing initial_full_width_lsmr_preserves_canonical_multiway_rank_policy
reporting assertion remains open as documented by the prior source-bound report;
it was not repaired or rerun here. Static allocation-inventory source drift,
weighted constrained-fit/nonretaining-CG span failures, seed378 comparator
adjudication, Linux T1/8/16 and constrained-memory confirmation also remain open.
No SCC authentication retry, stopped campaign, push or qualification occurred.

## Next subsystem

Prioritize **ordinary/cold two-FE PPML plus full-command cost accounting**.
Narrow low mode still costs 50 versus 32 ms: its <=0.90× target is 28.8 ms, a
42.4% reduction from this candidate, not another 10% native improvement. Reported
FE projection / finite / strict phases are 8.18 / 3.85 / 8.01 ms; inference is only
0.30 ms. Fast K10 costs 67 versus 58 ms, requiring about 22% reduction to reach
52.2 ms. Establish cold/warm wrapper, loading, native and output boundaries before
attributing the remaining public time to a particular subsystem.

Compare a fused current-weight component Schur joint direction against the
existing ordinary coefficient/Krylov direction on identical IRLS frames. Share
immutable topology and within-frame factors across response/control work, retain
independent original-input acceptance, and evaluate dense and scratch consumers
under explicit ownership. Any cold routing change must follow this computational
comparison rather than inflate the recently closed work hint. Target a coherent
20–40% public gain on losing two-FE families; stop a candidate when the measured
phase ceiling cannot deliver it. The two concrete wide storage limits above are
second priority. Preserve the already strong three-FE/weak-graph results with
focused controls and extend successful changes to fresh larger-N designs before
making broader claims.

## Immutable receipts

The content-addressed bundle includes all eight screens, declarations, execution
order, public results, audits, distributions, actual data hashes, source captures
and build identities for baseline and all three candidates, original failed test,
policy rejection, final fixtures' sanitized protocols and command/monitor recipes.
Raw data, raw licensed logs, binaries and credentials are omitted. Original and
sanitized hashes are retained. Restoring verifies every logical payload; no later
source revision inherits these timings merely by reference.

The bundle contains **14,972 logical files / 3,413 payloads**. Its SHA-256 is
`8ae56185ff56c1e0b3cc89888983295fbc61c028893338f6eadb565b2f66f952`.
All logical receipts were verified and restored using the included restore helper.
