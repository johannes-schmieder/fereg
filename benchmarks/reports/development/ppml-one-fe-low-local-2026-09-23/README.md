# Concentrated one-FE PPML in low memory — local development

Accept `onelow1` under ADR 0137. Ordinary low-memory one-FE Poisson steps now
borrow the original prepared columns and analytically optimize group intercepts.
This removes repeated scratch projection from ordinary steps and permits the
existing exact-frame strict/final product sharing. The original-input score,
canonical initial rank, strict WLS and independent final-Hessian/output gates
remain. No numerical threshold changes. This is a useful production extension;
**every measured one-FE case still loses to comparable stock fixest**.

## Source and measurement boundary

Baseline is production `e6f63a1b`, artifact `catjoint2`, plugin SHA-256
`2697e671ce99185f173ad5be28ee0ca32b98e6eb5744103c386046139a8d8736`.
Candidate is its captured four-Rust-file plus fixture overlay `onelow1`, plugin
`c69cc27f61955232ee73f3183ec60f947203c0d5d9e91164ef97f8d58aac78c0`.
All **539 captured computational inputs** match the accepted source. Documentation
and this report do not alter those inputs. Full identities and source payloads
are in the archive. Diagnostic instruments are separate artifacts and never
supply headline times.

Pinned unmodified fixest 0.14.2 uses `glm.tol=1e-10`, `fixef.tol=1e-11`, matched
threads, serial BLAS, robust inference and fitted means. Its source archive SHA
is `17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
No default-tolerance claim follows. Cases use the existing positive-outcome
numeric PPML harness; weights/generated terms receive separate scientific
oracles. Low mode against ordinary stock is **not physically memory-matched**.
No full qualification or frozen OLS campaign was run.

Each application warms once, then measures the whole public command. Rotations
alternate baseline/candidate/stock; process creation and data generation/loading
by the harness precede this command timer. Plugin SPI loading, native preparation,
solving, inference and public output are included. Declaration and cell hashes
precede candidate results. Seed 202609335 is development; 202609337 was predeclared
fresh. Follow-ups reuse identical input/sample hashes without source tuning.
No compilation, instrumentation run or archiving overlaps headline fits. Monitor
records contain no overlap or monitor errors. Report all distributions below;
individual millisecond differences on small fits are not precise speed claims.

## Current-source baseline and decision screen

A separate three-rotation current-source baseline first found fast/low N30720
at 53/79 ms versus stock 27/27, and fast/low N61440 at 104/178 versus 54/54.
All baseline public states and stock comparisons passed. This reestablished the
opportunity rather than transferring historical timings to a new artifact.

The five-rotation decision screen, robust plus fitted means, reports median
[min,max] milliseconds:

| One FE, T4 | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Low N30720 K6 | 80 [78,84] | 59 [58,60] | 27 [26,28] | 2.185 |
| Low N61440 K10 | 180 [172,186] | 124 [123,128] | 54 [51,56] | 2.296 |
| Fast N61440 K10 control | 103 [102,104] | 105 [103,107] | 54 [49,55] | 1.944 |

Low-mode reductions are **26.3% and 31.1%**, with all ten paired fits improving.
The fast control initially loses all five pairs by 1–4 ms. A predeclared five-pair
identical-input follow-up gives baseline `[102,105,104,106,103]`, candidate
`[105,105,102,103,101]`, medians **104/103**: one loss, one tie, three gains. Both
blocks remain evidence; do not claim a universal timing non-regression.

The small pilot gives baseline/candidate/stock 14/12/10, 19/17/11 and 36/26/15 ms.
These single rotations establish science and basic behavior, not precise margins.

## Fresh confirmation

Three rotations each; topology names identify the existing data generator. For
one FE they describe group occupancy, not connectivity between FE partitions.
All means, coefficients, covariance, objective, sample, rank and public-state
checks pass under existing envelopes.

| Low one FE | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| N46080 K8 T8 | 141 [140,144] | 91 [91,95] | 37 [33,38] | 2.459 |
| N61440 K10 T1 | 223 [214,224] | 167 [161,170] | 98 [96,100] | 1.704 |
| Unbalanced N30720 K16 T4 | 151 [150,163] | 98 [94,100] | 46 [45,51] | 2.130 |
| Disconnected generator N15360 K3 T1 | 47 [47,48] | 38 [38,39] | 15 [14,16] | 2.533 |
| Small N3840 K3 T1 | 20 [20,21] | 19 [17,22] | 11 [10,11] | 1.727 |

Fresh median reductions range **5.0–35.5%**; 14/15 native pairs improve. The small
case's 22 versus 21 ms loss triggers five identical-input follow-up rotations:
baseline `[26,26,19,25,32]`, candidate `[22,19,18,23,17]`, stock `[13,12,12,10,10]`.
All five native pairs improve; medians are 26/19/12. Retain the first block's
small median margin and both distributions; the follow-up's larger baseline
spread is visible, not evidence for a universal 27% small-fit gain.

The target remains unmet: N61440/K10/T4 low needs **124 -> <=48.6 ms** at this
stock median, and fast needs **105 -> <=48.6 ms**. Faster ordinary steps alone
cannot close that budget. The next design must reduce canonical construction,
preparation and accepted-state scans coherently while retaining independent
rank, original-input score, objective and Hessian witnesses.

## Scientific and regression validation

- **348 distinct Rust tests**: all 343 PPML tests plus five linear-session tests.
  New tests cover original column bindings at N80/N4096, K3/K65, noncontiguous
  active columns, group shifts, changing prior-weight frames and T1/T4; cold
  explicit matrices independently recompute means, gradient, Hessian and direction.
  Final-product tests now include low pools 1/2, T1/T4, unit/PW, clustered inference,
  saved effects, canonical fallback and deliberately stale/corrupt authority.
- Production-plugin ABI/panic checks pass. **Ten licensed runs of eight fixtures**
  pass with explicit return codes and terminal sentinels. Low full-dummy Poisson
  and independently assembled Hessian/sandwich checks cover unit/FW/PW, offset,
  robust/clustered inference and generated factor terms. Zero identified controls
  match independently computed weighted group totals.
- Real production-plugin allocation audits at pool1/T1, pool1/T4 and pool2/T4,
  each in a separate process, complete sixteen iterations with zero allocation
  events/growth, stable capacities and warm/cold equivalence. Saved outputs obey
  reconstruction. These do not repair the older mixed-thread pool-retirement
  audit failure or static allocation-inventory source drift.
- Separation, fast one-FE, joint categorical and low-slope fixtures pass. Eight
  OLS and eight excluded multi-FE/slope PPML pairs have exact scientific exports.
  Their timing pairs include individual losses and remain in the receipts.

The original pilot's independent audit refused its third cell because 1,229 FE
columns exceed the existing 1,200-column explicit-oracle bound. The initial empty
output and traceback are preserved. A bounded audit checks all public states
and comparisons, runs independent Poisson/Hessian oracles for the first two
cells (six passing application checks), and explicitly marks the third as
skipped. No bound or tolerance was relaxed. Separate licensed explicit-design
oracles validate the larger/weighted/generated low route.

Public receipts total **180 passing applications**, **164 passing PPML states**,
**51 native PPML pairs**, **50 baseline/stock pairs**, **38 additional direct
candidate/stock pairs**, six small independent oracle checks and eight exact OLS
pairs. The eight exact excluded PPML pairs are part of the native count. The
separate diagnostics add eight passing applications/states and two native pairs.
No application failure, timeout or scientific discrepancy occurred in these
blocks; tooling failures and the bounded-oracle skip are separate evidence.

## Memory

No extra N-by-K design is materialized. Low mode retains admitted profile state:
three N-sized vectors plus group and finite-system/moment buffers. Original
columns already belong to the prepared model; numeric binding is stack-only and
generated terms use the existing evaluator. Storage remains charged through
strict solving, then is released before inference. Optional-memory refusal
preserves the canonical route. Requested allocation audits separately reserve
cold raw-input storage outside the measured iteration window.

Fresh tracked peak increases range **0.108–1.891 MB**; selected modeled peak
increases **0.109–1.929 MB**. N61440/K10/T4 tracked peak changes
**32,712,369 -> 34,603,553 bytes**, within the candidate model of 43,061,399 bytes.
Scratch storage remains **4,915,200 bytes**. Fast control tracked peak is unchanged.
These are modeled/allocator observations, not physical-cap qualification.

## Retained limitations

Ordinary iteration counts are not comparable computational units across profiled
Newton, canonical MAP or fixest. Use the isolated operation/phase ledger below.
No sample, weight semantics, rank, DoF, separation behavior, numerical envelope,
certificate or failure-atomicity contract changed. Frozen 77 primary, 16 holdout
and 111 native-control definitions are untouched. SCC/platform, constrained-fit,
nonretaining-CG and historical comparator issues remain open. No push or release.

## Whole-command attribution and next design

Isolated instruments use one monotonic epoch across plugin/core and report every
span's boundaries. The summary subtracts the union of nested intervals; exclusive
costs sum to the plugin span, so no phase is double-counted. The first baseline
instrument omitted the `solve_frozen_ppml` wrapper; its ledger keeps that work
unassigned. The refined matched instruments correct that coverage and split SPI
loading, core preparation/inference/publication and plugin publication. Remaining
unassigned core/IRLS spans are explicitly visible. Stata/ado work outside the
plugin is the difference from the measured whole command, not an invented native
phase. Instruments incur logging overhead and are not stock-comparator rankings.

Diagnostic artifacts:

| Artifact | Purpose | Plugin SHA-256 |
| --- | --- | --- |
| oneledg2 | Initial baseline core ledger | 41ae0d6f6a475248e7c40d0e26e09a4df7f078b907d85081be63c038d34e73b5 |
| oneledg3 | Refined baseline plugin/core ledger | c09fe630c2f05aeccc8c7e3458013ce2407a63e627ee97809fa291070ef8f1ce |
| oneledg4 | Same refined candidate instrument | f11e8b4be60278a3cb06765605fddfb600fd8d237b436185670dd060aff085ce |

At N61440/K10/T4 low, baseline/candidate diagnostic plugin spans are
**166.058/110.242 ms**. The measured organizational change is:

| Work | Baseline calls / exclusive ms | Candidate calls / exclusive ms |
| --- | ---: | ---: |
| Full retained-rank construction | 3 / 29.108 | 2 / 20.868 |
| Retained-control reprojection | 4 / 27.123 | 1 / 7.288 |
| Ordinary frozen PPML solve (excluding children) | 2 / 25.112 | 0 / 0 |
| Concentrated direction (excluding profile) | 0 / 0 | 2 / 4.255 |
| Profile predictor/mean evaluation | 0 / 0 | 4 / 2.684 |
| Strict solve (excluding children) | 2 / 11.787 | 1 / 5.957 |
| Full retained-design setup | 1 / 13.459 | 1 / 13.690 |
| Poisson objective evaluation | 7 / 12.836 | 6 / 10.972 |
| Original-input score certification | 5 / 3.029 | 5 / 3.106 |

Fresh final-product reuse costs 0.407 ms in the candidate. Initial fresh WLS is
24.192 ms inclusive and strict WLS 24.046 ms inclusive; do not add these to the
exclusive rows above. Candidate SPI loading is 4.932 ms, initial core preparation
4.397, inference 1.915, core publication 10.680 inclusive (6.591 exclusive), and
plugin publication 1.791. The 128 ms diagnostic public command leaves about
17.8 ms outside its 110.242 ms plugin span. This difference includes control-plane
work and timer rounding, not data setup by the harness.

The fast candidate still spends 13.275 ms on two full retained-rank constructions,
9.469 ms building the retained design, 5.140 ms reprojecting it, 10.905 ms in six
objective calls, 4.587 ms on working-state construction and 3.052 ms on fresh outer
scores. Its two profiled directions total only 3.395 ms inclusive. Therefore
another small profiled-kernel optimization cannot plausibly close the approximately
56 ms whole-command deficit to the 0.90 stock target.

**Next coherent design:** a one-intercept canonical moment owner should perform
initial rank and accepted final Hessian construction with bounded current-weight
group-centered finite products, avoiding general scratch/retained-design passes
where algebra permits. Keep independently replayed original-input projection,
rank, finite solve and predictor reconstruction; do not promote speculative profile
moments into authority. Measure representation and rank policy against explicit
WLS/Poisson oracles before routing it. In parallel conceptual work, identify which
accepted predictor/mean/working-state scans can share a frame owner with exact
sample/prior/current-weight generations. Prior state is only a guess.

The repeated objective ledger also justifies a bounded immutable-outcome cache
experiment, but only as part of this larger cost budget: all six objective calls
cost about 11 ms, far less than the whole gap. Cache only truly invariant outcome/
prior-weight contributions and retain independent final objective replay. Do not
label all remaining rank/check time irreducible verification overhead, or remove
checks solely because they are expensive. Existing multi-FE/slope setup and
canonical-boundary work remains the next broader subsystem after this bounded
one-FE design, not reopened serial weighted-slope micro-tuning.

## Tooling attempts and receipts

Preserve the first build CLI invocation failure (revision passed positionally),
the initial isolated-instrument failure (wrong score function name, before
compilation), the pilot oracle-bound refusal, and a zero-test linear-session
filter. The corrected filter executes all five tests. The initial verifier
correctly refused to count that empty run; final verification counts only passing
executed tests. No failed production candidate or numerical discrepancy is hidden
by these corrections. Recipes, logs, identities, comparisons, declarations and
source captures are archived; raw licensed logs, binaries and data are omitted,
with hashes and regeneration recipes retained.

The content-addressed archive contains **8,304 logical receipts**, **1,899 unique
payloads**, **18,877,401 bytes**. SHA-256:
`cedb9532dd73a72fa91a1b8914e92884068557ed6cd862592924d14951633ba6`.
`restore_receipts.py` verified and restored all 8,304 logical files into a new
local directory. It verifies both the container and every logical archived hash;
original hashes remain in the manifest where paths were sanitized. No licensed
raw logs, license data, data matrices or compiled binaries are committed.
