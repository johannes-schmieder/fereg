# Strict-target ordinary PPML solves: accepted bounded candidate

Development evidence, 2026-09-28. Not qualification.

Retain the shared scheduling change in [ADR0238](../../../../docs/adr/0238-ppml-publication-target-ordinary-solve.md).
Once accepted objective and score diagnostics both meet their existing outer
bounds, the next ordinary weighted-linear solve uses the existing strict inner
floor. Two outer convergence passes and every independent final check remain.
This avoids an inaccurate ordinary direction causing a strict correction and
repeated finalization. There is no new threshold, solver layer, buffer or
sample/width/thread guard. OLS does not call this schedule.

Fresh small-fast, wide, weak and strong-d2 PPML improve by about **24–25%** versus
the corrected baseline; serial small PPML improves **36%**. Gains do not extend
to every case: three-FE loses 7% in the fresh block, and low-mode/slope results
are variable. Retain the small, broadly useful change with these losses visible.
The universal 1.20 ceiling, 0.90 aspiration and monotonic core scaling remain unmet.

## Source and work

- Baseline computational snapshot `f2e37c75932a30d857efe3192394bac305decca68845412db186a6b24044f231`,
  Linux plugin `7c01f4514454c1fb7f9804abb08ea3917ac98a67ef65d69ed9c3ea4083e792ec`.
  This already includes the ADR0236 accuracy repair.
- Candidate based on commit **18ff2dbf**, snapshot
  `5c0d4685f7e10f126f140d8bfe9a6177ffc8ce73dc00f51f1845cd319800b7e0`,
  Linux plugin `94925723e066bdbacf067a66524ff08918d6f3612fcbf557a1c4f1be3091a00e`,
  Mac plugin `0172ddaf57d2a6037ead11135dc9ae2a5267c35fdc619a2b37f3a4a6381e6a0d`.
  Of 211 bound computational inputs, only `ppml/mod.rs` changes.
- Stock is pinned, unmodified fixest 0.14.2, runtime fingerprint
  `041bf47963205620c13f2137c17a25a47afd70d73c5d03c42a936bbb39ce29e6`.
  Compare public `feppml` with `fepois`, robust inference; `fereg` with `feols`,
  IID inference. Slopes and strong-d2 use declared strict stock
  `glm.tol=1e-10`, `fixef.tol=1e-11`; other stock calls use pinned defaults.
  No optional saved observation outputs in headline calls. Licensed fixtures
  separately exercise reconstruction and scientific/output boundaries.

Initial seed2026092811 repeats the preceding cost-screen definitions, with two
alternating B/C/stock then stock/C/B rotations. Fresh seed2026092813 and its 11
cases were registered before generation, with three rotations; no source, guard,
tolerance or case is tuned between blocks. All arms consume identical generated
DTA bytes with matched public thread requests and verified native effective teams.
The stock library remains uninstrumented. Generation/warm-up and extraction are
outside command clocks; complete public estimation, inference and publication
remain inside. Process elapsed time and sampled RSS are separate fields.

Initial job **7763505**: isolated E5-2680v4, 28 slots, 191s, maxvmem1.998G.
Fresh **7763512**: same CPU class/node, 28 slots, 335s, maxvmem1.998G.
Both have clean accounting and both terminal sentinels. No build, profile,
archive or competing fit overlaps their headline calls. All **153 applications**
complete; **51/51 native pairs** and **49/51 stock pairs** pass science. The two
initial weak-d3 stock SE mismatches remain NR and independently unadjudicated;
all 33 fresh stock pairs pass. A different seed passing does not resolve an old
failure. No historical receipt is reclassified.

## Whole-command distributions

Ratios below are paired medians; below1 means candidate faster. Parentheses give
the fresh paired min–max versus baseline. N/K/T records rows, controls and public
thread request. Full command/process/RSS distributions and failure reasons are
in [initial07](initial07/README.md) and [fresh08](fresh08/README.md), with source-
and raw-receipt bindings in their JSON summaries.

| Family | N/K/T | Initial C/B | Fresh C/B (range) | Fresh C/stock |
|---|---:|---:|---:|---:|
| Small fast PPML | 30,720/6/4 | 0.848 | 0.756 (0.740–0.840) | 1.629 |
| Small low PPML | 30,720/6/4 | 1.027 | 0.988 (0.895–1.003) | 1.467 |
| Small serial PPML | 30,720/6/1 | 0.570 | 0.641 (0.523–0.655) | 1.688 |
| Wide PPML | 61,440/24/4 | 0.820 | 0.754 (0.739–0.809) | 0.618 |
| Sparse three-FE PPML | 61,440/10/8 | 0.973 | 1.071 (1.054–1.111) | 0.460 |
| Two-slope PPML | 30,720/6/4 | 0.799 | 0.956 (0.936–0.989) | 0.375 |
| OLS control | 61,440/10/8 | 0.979 | 1.011 (1.000–1.049) | 1.638 |
| Weak-d3 PPML | 30,720/6/4 | 0.771 | 0.763 (0.732–0.801) | 1.295 |
| Strong-d2 PPML | 61,440/10/4 | 0.714 | 0.753 (0.743–0.827) | 0.325 |
| One-FE PPML control | 30,720/6/4 | — | 0.991 (0.918–1.033) | 3.517 |
| Low-memory slopes | 30,720/6/4 | — | 0.903 (0.729–1.207) | 0.349 |

The initial low-mode loss is **2.7%**; fresh low is near a tie, with one loss in
three pairs. Initial slopes improve 20%, but fresh improves only 4.4%; do not claim
a stable 20% slope gain. Fresh low slopes range 0.729–1.207, including one 20.7%
loss versus baseline. Three-FE is mixed initially (0.892–1.054), then slower in
all three fresh pairs (1.054–1.111) despite unchanged reported outer/inner counts.
That cost is retained as an unresolved regression signal, not dismissed as noise.
OLS has unchanged computation and counts; its fresh 1.011 ratio is not an OLS
algorithm change or proof of performance regression freedom.

## Mechanism, accuracy and memory

Fresh small/serial, wide, weak and strong-d2 go from six to four outer iterations,
with strict rejections1→0. Reported inner counts become 155→108,169→117,
243→158 and139→102. Their strict-final phase medians roughly halve: small
69.1→33.7ms, wide410.8→197.1ms, weak198.4→89.1ms, strong-d2 297.1→139.2ms.
These nested clocks locate reduced work and must not be added to other phases.
Low remains four outer iterations and does more inner work 135→140; ordinary
accuracy was not its limiting checkpoint cost. Three-FE remains 7/132, and the
one-FE control remains 4/5. Those observations limit the optimization's reach.

Both Mac and Linux pass **65 distinct focused Rust tests**, four licensed
independent-oracle/control fixtures and actual-plugin ABI/panic containment.
Linux validation **7763463** also passes PPML smoke (227s, maxvmem11.516G including
compilation). Fixtures cover iterative/canonical PPML, current final Hessian,
three-partition slopes and OLS; Rust covers strict retry/rebuild, rank,
reconstruction, tight tolerances, memory routes and rollback. These are bounded
development checks, not a full qualification campaign.

Independent original-input confirmation uses the previously checked full sparse
Poisson anchor and outward coefficient enclosure. Positive strong-d2 has radius
8.66e-14; candidate fast T1/T4 error3.80e-13, low3.66e-15, all within the unchanged
envelope. Fast goes6/142→4/105. Zero-outcome dense has radius8.20e-15; the independent
530-row recession witness and retained-row hash match. Candidate fast error1.87e-13,
low1.08e-13; fast6/186→4/134. Both corrected baseline and candidate pass every
coefficient, point VCE/SE, sample/count, rank, objective and convergence check.
This is stronger work scheduling, not relaxation of the preceding accuracy fix.
The full proof/job failure history is below.

No new storage or admission allowance. Modeled memory agrees on every native
pair. Tracked peaks agree outside three-FE, whose overlapping existing workspaces
vary in both directions across repetitions; no new memory saving is claimed.
Sampled process peaks and SGE maxvmem remain diagnostic. Static allocation drift
and constrained/nonretaining solver issues remain open.

## Failures and operational adjudication

- Submission7763462 had an erroneous display snapshot label. It was canceled
  while queued, with no source directory or execution, after the cluster refused
  an environment edit. Correctly labeled7763463 supplies actual validation.
- **7763485:** wrapper import failure before fits: relocated tests lacked the
  explicit-dummy module search path. Accounting failed0/exit1. Dependent timing
  **7763488** was canceled before fits. New wrappers explicitly bind that path;
  separate local import checks pass.
- **7763492:** every intended positive-input numerical check and four native fits
  passed. A stale final assertion then expected the already-corrected baseline
  to fail. Accounting failed0/exit1,60s,maxvmem1.606G. Preserve it as a **partial,
  failed probe**, despite its written numerical completion object. Zero did not
  run. Dependent timing **7763493** was canceled before fits.
- **7763502:** independently binds the completed positive proof/comparison files,
  verifies every numerical check, and runs only the missing zero-input probe.
  It passes clean accounting and explicit composite/zero terminal sentinels.
  No positive fit is repeated and no failed process is relabeled successful.
  [Positive adjudication](positive02b/adjudication.json),
  [zero/composite receipt](zero06/receipt.json),
  [composite completion](zero06/composite-complete.json).
- Initial timing's two weak-d3 stock SE comparisons fail, although all applications
  return success. Their maximum SE discrepancy is about 5.59e-9. Keep them NR;
  matching samples/rank and other coefficients/objectives do not settle inference.

The frozen wrappers, registrations, deployments and submissions preserve every
attempt. Collected files match remotely computed hashes. Raw licensed logs,
scratch-path requests and binary anchors stay remote. None of these results
repairs the static-inventory drift or authorizes a release/push.

## Remaining work

The added one-FE control is the largest current loss: 0.313s candidate versus
about 0.089s stock, paired ratio 3.517, while its C/B ratio is 0.991. It is an existing
public-path bottleneck, not a new gain or a diagnosed scientific bug. Native
telemetry identifies finite solving (~57.6ms), strict finalization (~53.9ms),
preparation (~35.9ms) and FE projection (~30.4ms) as useful profiling targets;
these phases overlap. Profile that public path and the small two-FE path next,
including which portable finite-product improvements actually reach them.

Core scaling also remains unresolved. Same-input grouped T4/T1 candidate ratios
are 1.345 initially (both slower), and 1.075 fresh (two of three slower; range
0.846–1.108), with passing numerical comparisons. Calls were not directly
alternating by thread count, so this is descriptive evidence, not a monotonic
scaling experiment. The current team ceiling covers narrow three-FE PPML only;
small one-/two-FE work needs direct T1/T2/T4 measurements and phase attribution
before another concurrency policy is chosen. Preserve the fresh validation seeds.

Keep the three-FE slowdown and initial weak SE mismatch open. Small OLS (fresh
1.638 of stock) is unchanged and still needs its own whole-command comparison.
Large-timeout families, remaining reference/sample failures, wider scaling,
weighted constrained fits, nonretaining-CG instability and allocation-inventory
repair remain separate milestones. This closes one useful candidate, not the
broader repair/performance objective.
