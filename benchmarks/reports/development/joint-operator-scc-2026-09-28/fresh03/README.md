# Repeated joint PPML products: fresh confirmation

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 37/37 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| fresh-profiled-small | 0.742 / 0.998 / 1.177 | 1.545 / 1.894 / 1.991 |
| fresh-small-low | 0.769 / 1.019 / 1.040 | 1.430 / 1.591 / 1.642 |
| fresh-small-serial | 0.568 / 0.678 / 0.975 | 0.972 / 1.327 / 1.682 |
| fresh-larger-wide | 0.921 / 0.958 / 0.993 | 0.949 / 1.027 / 1.113 |
| fresh-sparse-two | 0.967 / 1.015 / 1.060 | 0.434 / 0.466 / 0.473 |
| fresh-wide-serial-adjoint | 0.777 / 0.783 / 0.826 | 0.548 / 0.549 / 0.626 |
| fresh-three-fe-core-pair | 0.970 / 1.009 / 1.053 | 0.469 / 0.474 / 0.499 |
| fresh-excluded-slope | 0.897 / 1.049 / 1.197 | 0.354 / 0.365 / 0.408 |
| fresh-excluded-ols | 0.974 / 1.000 / 1.016 | 1.504 / 1.570 / 1.700 |
| fresh-larger | 0.831 / 1.004 / 1.073 | 0.889 / 1.062 / 1.098 |
| fresh-weak | 0.793 / 0.934 / 1.082 | 1.029 / 1.090 / 1.153 |

fresh-profiled-small: selected native baseline / candidate teams = 4 / 4.

fresh-small-low: selected native baseline / candidate teams = 4 / 4.

fresh-small-serial: selected native baseline / candidate teams = 1 / 1.

fresh-larger-wide: selected native baseline / candidate teams = 8 / 8.

fresh-sparse-two: selected native baseline / candidate teams = 4 / 4.

fresh-wide-serial-adjoint: selected native baseline / candidate teams = 4 / 4.

fresh-three-fe-core-pair: selected native baseline / candidate teams = 8 / 8.

fresh-excluded-slope: selected native baseline / candidate teams = 4 / 4.

fresh-excluded-ols: selected native baseline / candidate teams = 8 / 8.

fresh-larger: selected native baseline / candidate teams = 8 / 8.

fresh-weak: selected native baseline / candidate teams = 4 / 4.

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

The recorded T1/T4 comparison is reported below and in thread-scaling.json.
The large timing cases do not include a new independent-oracle fit; focused
independent validation is described below.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.

## Decision

Accept the small joint-operator kernel change for bounded development under
ADR 0235. All **111 applications, 37 native science pairs and 37 stock science
pairs** pass on unchanged snapshot `4c3d7c44…` / plugin `a7e95e67…`, versus
baseline `d1156e6d…`. Across initial and fresh blocks, there are 192 headline
applications and 64/64 native plus 64/64 stock science passes. Stock is pinned,
unmodified fixest 0.14.2: 58 default pairs and 6 explicitly strict slope pairs.
Earlier default-stock slope failures from other source-bound blocks stay NR.

Fresh seed 202609288 was declared before generation. Robust PPML and IID OLS
inference are requested, without optional saved observation outputs. Independent
weighted-design, Poisson/Hessian/sandwich, strict-retry/rank and excluded-path
fixtures passed separately; there is no new independent large-design oracle in
this timing block. No source, tolerance, guard, sample or rank-policy changes
were made between initial and fresh measurements.

The reliable fresh gains are **32.2% for small serial PPML** and **21.7% for
K24 PPML**, with all five/three respective pairs faster. K12 improves 4.2%, all
three faster, and weak connectivity 6.6% with one loss. Small T4 is neutral at
0.998; its initial 12.5% median gain does not replicate. Sparse, three-FE and
larger cases are also approximately neutral (1.015/1.009/1.004). Retain this
small implementation for meaningful serial/wide gains; do not add selective
routing to manufacture a gain in the neutral cases.

## Remaining losses and controls

Marginal baseline/candidate/stock medians (ms), in table order, are
420/428/226, 364/366/226, 521/370/288, 717/709/637, 517/525/1116,
1304/1013/1830, 1764/1787/3774, 577/605/1609, 190/188/121,
1183/1181/1112 and 518/484/425. Paired ratios and ratios of marginal medians can
differ substantially; use the paired distributions above for the registered
comparison, and keep both visible.

Fresh slope control C/B is **1.049**, range 0.897–1.197, with two losses; its
marginal 577→605ms also loses 4.9%. OLS is **1.000**, range 0.974–1.016, one loss
and one tie. Low is 1.019 with two losses. These excluded-path variations do not
prove regression freedom or an algorithmic benefit outside the changed route.
They also do not justify another control-specific tuning cycle. Numbers of
C/B losing pairs by table row are 2, 2, 0, 0, 2, 0, 2, 2, 1, 2, 1. All individual data remain
visible. Scientific or application failures would remain NR; none occurs here.

Stock gaps remain: small T4 **1.894**, serial **1.327**, low **1.591**, OLS
**1.570**. K12/larger/weak are near parity at 1.027/1.062/1.090, while sparse,
K24, three-FE and strict slopes are 0.466/0.549/0.474/0.365 on these designs.
Do not transfer these ratios to the historical absorbed-slope designs, other
source revisions, or default slope accuracy failures. Neither a universal 1.20
ceiling nor 0.90-times-stock objective has been achieved.

## Iterations, scaling and memory

Every paired native outer and reported inner count matches; every modeled
memory field matches. Serial retains 4 outer/103 reported inner iterations and
median absorption falls 229→135ms. K24 retains 4/109 and absorption falls 590→390ms.
Small T4 absorption falls 207→183ms without a whole-command gain. Native phase
clocks may nest and must not be summed; these are observations, not a complete
causal decomposition. Native and stock iteration definitions are not equated.

Recorded candidate T4/T1 comparisons pass the frozen scientific comparator,
exact input/sample, native rank/DoF and actual-team checks on all five pairs.
The paired time ratio is **1.005**, range **0.843–1.543**, with three T4 losses;
marginal medians 428/370ms imply a 15.7% T4 loss. Requests were separately grouped,
not directly alternated. This blocks any monotonic-scaling claim and identifies
a remaining question for matched-thread work. The source still switches finite
adjoint traversal according to width and team size; this is a possible mechanism,
not a demonstrated attribution or permission to fit a thread cap to this seed.

No heap bank or modeled budget is added. Tracked PPML peaks match except the
three-FE candidate, which is 585,288 bytes lower. Process-peak deltas (MiB) are
-3.12,+4.82,-11.35,+6.33,-3.82,-8.28,-0.72,+6.25,-0.71,+2.01,+4.80.
Sampling variation and changed execution can alter physical peaks; this does
not repair allocation-inventory drift or qualify a physical memory cap.

Job **7763189** has clean accounting: 385s, 28 isolated slots, maxvmem 1.998G,
and both rotation/wrapper sentinels. No compilation, profiling, archiving or
competing fit overlaps timing. Both Mac and Linux pass 32 focused Rust tests and
four licensed oracle/control fixtures; Linux also passes PPML smoke and Mac
universal ABI/panic checks pass. ARM performance was not benchmarked here.

## Next boundary

Close this kernel experiment. The next bounded milestone is captured-input
adjudication of the original large degree-two OLS and three strict PPML
coefficient discrepancies, using the pinned SCC generator and independent
mathematical checks. Smaller explicit designs have not answered those questions.
For later performance work, retain the small-command gap and mixed T1/T4 evidence;
measure matched requests before revising the adjoint traversal or team policy.
Do not restart the rejected coefficient-engine swaps or threshold tuning.
