# PPML streamed finite products: column access and independent scheduling

Local macOS arm64 development milestone, September 23, 2026. Adopt `jointcolpar2`
under [ADR 0145](../../../../docs/adr/0145-column-oriented-streamed-ppml-products.md).
Direct source-column loading and independent Hessian rows improve wide low-memory
PPML, with a serial accumulation organization when one worker buffer is admitted.
The broad **whole public PPML <=0.90 times comparable stock fixest** objective
remains unfinished. Dense two-FE cases still lose substantially.

## Source and measurement identity

Baseline `jointorig2` matches `47bb7ef5`, plugin SHA-256
`ebf09233b4f56e01cd18accd7c0d6183f46b329025edbc33b077de7c0f9b91de`.
Final `jointcolpar2` overlays `fe/slope_coefficients/joint.rs`,
`ols/linear_session.rs` and the new Stata oracle on that commit. All **542 captured
inputs** match the adopted worktree. Final plugin:

`92e878f6d394242480b3030123d97a23edf67a808bc0f48656d20e3af1b8743f`.

The pinned, unmodified stock comparator is fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched requested threads and serial BLAS.
Public PPML clocks include robust inference and fitted means; stock includes
response prediction. Startup, generation/import, warmup and exports are outside
both clocks. Low memory versus stock is not a physical-memory-matched comparison.
OLS controls use their separately captured surface.

Every rotation alternates baseline/candidate/stock order. Completed headline
runs have no detected competing build/fit, monitor error, timeout or application
failure. Diagnostic builds, compilation, tests and archives run separately.
Generated-term performance controls compare native artifacts only, with the
same public work and alternating fresh processes; they make no stock speed claim.

## Current baseline and attribution

The initial three-rotation screen at development seed 202609347 establishes:

| Low two-FE case | Baseline median [min,max] ms | Stock median [min,max] ms |
|---|---:|---:|
| N16320/K24/T8 | 231 [230,231] | 68 [65,74] |
| N30720/K6/T4 | 116 [115,116] | 58 [57,58] |

Both comparisons pass the existing scientific envelopes. Separate RAII span
builds `jointwideledger1` and `jointcolparledger1` identify the timed fit apart
from warmup. Exclusive spans subtract the union of children and sum to the
native root. These are single-fit diagnostics, not headline timings.

For the wide case, the baseline's six joint directions spend 46.982 ms forming
finite products, 20.377 ms loading finite tiles and 2.665 ms projecting them.
The complete tile scope is 71.532 ms including loop/control overhead. Its
exclusive joint Krylov recurrence is only 0.959 ms. The first column-access
candidate gives the following attribution:

| Diagnostic phase | Baseline ms | Column-only candidate ms |
|---|---:|---:|
| Native plugin | 232.472 | 182.416 |
| Joint directions including setup/checks | 101.889 | 58.768 |
| Load/project/finite-product tiles | 71.532 | 30.381 |
| Stable finite scales | 6.082 | 5.558 |
| Fresh joint reconstruction | 12.944 | 11.570 |
| Joint recurrence, exclusive | 0.959 | 0.905 |
| Canonical projection wrappers | 72.916 | 66.722 |
| Canonical symmetric sweeps, inside projection | 35.239 | 35.312 |
| Categorical certificates, inside projection | 14.763 | 14.736 |
| Canonical finite solves | 17.152 | 17.688 |
| Final inference | 4.365 | 4.664 |

These rows have nesting; do not add them all. The diagnostic column candidate
is **`jointcolpar1`**, not the later final artifact. Its measured attribution
motivates the design; final performance claims below use `jointcolpar2`'s own
uninstrumented runs. The final change adds a distinct single-worker product
organization. No diagnostic timing is transferred to that artifact.

For the narrow case, diagnostic joint work falls 39.083 to 35.418 ms, while
native time is 110.102 versus 109.231 ms. Its recurrence is about 1.3 ms. Neither
case supports spending another cycle tuning Krylov iterations. No new frozen
stock/native inner-solver ledger was needed: this experiment changes finite
preparation, not the recurrence or its convergence policy.

## Candidates, rejected organization and final mechanism

1. `jointrowpar1` forms independent Hessian rows in parallel but repeatedly
   materializes complete raw rows. The wide public median is 232 to 222 ms;
   narrow is 117 to 114 ms. Science is exact, but the organization leaves much
   of the avoidable input work intact. It is not retained as another route.
2. `jointcolpar1` adds direct column access, removes shared serial tile loading
   and reads one source column per Hessian entry. Wide development improves
   231 to 192 ms, and fresh K32 improves 312 to 253 ms. A T1 follow-up, however,
   regresses 194 to 204 ms. Preserve both the original nearly tied fresh block
   and this five-rotation loss. The serial scalar accumulation chain is a
   plausible instruction-throughput explanation, not a hardware-counter proof.
3. `jointcolpar2` retains direct column loading but accumulates independent
   Hessian entries together when only one worker buffer is admitted. Its row
   scratch borrows the unused factor bank. This condition follows available
   parallelism; there is no benchmark, seed, N or K threshold. Fresh seed
   **202609351** validates the final source. Seed 350 remains consumed evidence
   for its predecessor and is not relabeled as final fresh validation.

For normalized V = sqrt(W) X and eliminated-partition projector P0, each worker
forms H_ij = (P0 V_i)' V_j directly. There is no subtraction of large Gram matrices.
Entry-wise row/FMA order, division order and symmetrization remain unchanged.
Column access borrows numeric source slices or evaluates the selected generated
recipe through the same stored-column mapping. The serial path keeps independent
entry accumulators active; the threaded path gives workers disjoint Hessian rows.

Stable norms, current-weight geometry, FE cross-products, the common joint solve,
reconstruction and fresh FE/original-input finite certificates remain. Initial
rank, canonical inverse/bread, objective, two outer convergence passes, strict
witness and exact final-Hessian acceptance are unchanged. No state receives new
cross-weight authority. Errors revoke existing geometry authority and publish no
partial result. Fast PPML, one-FE concentration and OLS keep their algorithms.

Threaded column products repeat some normalization and traverse more individual
columns than serial row tiles. Their source access is more contiguous and an
accumulating matrix entry remains local. This is a measured locality/scheduling
tradeoff, not a claim that source-access counts equal physical memory traffic.

## Final public distributions and remaining losses

Development confirmation at seed 202609347, three rotations:

| Low two-FE case | Baseline ms | Final ms | Stock ms | Final/stock |
|---|---:|---:|---:|---:|
| N16320/K24/T8 | 234 [229,236] | 190 [187,191] | 69 [68,73] | 2.754 |
| N30720/K6/T4 | 116 [114,116] | 115 [113,116] | 56 [55,58] | 2.054 |

Final fresh seed 202609351, five rotations; weak case has three:

| Low case | Baseline ms | Final ms | Stock ms | Final/stock |
|---|---:|---:|---:|---:|
| Two FE N16320/K32/T8 | 319 [310,324] | 254 [253,258] | 89 [88,93] | 2.854 |
| Two FE N38400/K10/T4 | 196 [190,200] | 174 [173,177] | 86 [84,87] | 2.023 |
| Unbalanced three FE, two slopes, N28800/K8/T4 | 151 [151,154] | 142 [141,146] | 168 [166,172] | 0.845 |
| Two FE N24480/K10/T1 | 195 [193,199] | 179 [178,183] | 141 [134,145] | 1.270 |
| Weak barbell, two slopes, N7680/K4/T4 | 34 [34,35] | 33 [33,35] | 266 [264,268] | 0.124 |

The four main fresh medians improve **20.4%, 11.2%, 6.0% and 8.2%**. All twenty
main fresh pairs improve. Weak pairs include a small loss; retain the individual
receipts. Two slope cases clear the 10% stock lead, but three categorical fresh
cases still lose. Wide gains do not imply parity, and narrow development remains
nearly unchanged.

A separate deterministic generated-factor control has N16320, eight numeric
controls plus fifteen nonbase indicators, PW robust inference, saved means,
low storage and pool width two. Three native pairs per thread setting:

| Threads | Baseline ms | Final ms | Native median reduction |
|---|---:|---:|---:|
| T1 | 621 [621,622] | 443 [439,446] | 28.7% |
| T4 | 423 [421,428] | 310 [309,313] | 26.7% |

All six pairs improve and have exact coefficients, covariance and means. These
are native-only performance controls; independent generated-term science is
covered separately, not inferred solely from these matching exports.

Excluded PPML controls, three rotations at N30720/K6/T4 unless noted:

| Control | Baseline ms | Final ms | Stock ms |
|---|---:|---:|---:|
| Low one FE K32 | 104 [103,104] | 101 [100,104] | 120 [120,121] |
| Fast two FE | 97 [96,101] | 100 [96,104] | 58 [55,58] |
| Fast three FE | 181 [181,184] | 183 [182,188] | 229 [228,231] |
| OLS two-slope | 66 [64,67] | 66 [64,69] | — |
| Low three-FE OLS | 65 [64,67] | 65 [65,67] | — |

All control science is exact. Fast categorical medians retain +3/+2 ms losses;
OLS medians tie with mixed paired differences. The small final pilot has a
15-to-17-ms two-slope loss. Do not claim universal timing non-regression. Earlier
one-FE and OLS losses, follow-ups and all predecessor distributions remain in
`summary.json`; later blocks do not erase them.

## Validation, memory and preserved failures

- **155 distinct Rust tests** pass on the final source in its own Cargo target:
  independent analytic weighted WLS, changed frames, reordered/scaled controls,
  rank-deficient slopes, bad inputs, strict/final transactions, rollback,
  generated terms and memory refusal. Actual-plugin ABI and panic containment pass.
- **Ten passing final licensed runs of eight fixtures** include a new N4800/K8
  explicit Poisson/Hessian/sandwich oracle at T1/T4 with unit/FW/PW weights and
  robust/clustered VCE. Existing multipartition-slope, categorical, factor-term,
  separation and rollback fixtures pass. Stata frequency-weight oracles are
  explicit-design checks; ordinary fixest weights are not used as that oracle.
- Four final actual-plugin allocation windows, categorical/slopes at pool/T1/1
  and 2/4, have exactly zero allocation events and current growth, stable
  capacities and fresh/warm equivalence.
- Across standard headline blocks: **345 passing applications**, **315 passing
  public-state checks**, **45 small independent Poisson/Hessian oracle results**,
  **116 exact native pairs**, **107 passing baseline/stock pairs** and **101
  passing direct candidate/stock pairs**. These counts include prototypes.
  Final standard blocks alone have 135 applications and 47 exact native pairs.
  Generated controls add 24 applications/12 exact pairs across both column
  candidates; the final artifact contributes 12 applications/six pairs.
- All 41 final standard PPML pairs have identical outer/inner counts, step
  halvings, fallbacks, strict rejections, rank, DoF, modeled peak and scratch size.
  Tracked peaks match in 40 pairs and decrease 6,400 bytes in the small three-FE
  pilot. No new heap bank or N-by-K design is admitted. This is not physical-cap
  or cross-platform qualification.
- The first build request collided with an existing artifact tag and stopped
  before compilation. That artifact was left intact and a new unique tag used;
  the failed receipt is preserved. There are no estimator failures or scientific
  exclusions in these new blocks. Known large one-FE stock-mean discrepancies,
  the shared 1e6-FE-shift convergence failure, static allocation drift, weighted
  constrained-fit failures and nonretaining-CG instability are not repaired or
  reclassified here. See the preceding source-bound report for their evidence.

## Next bounded work

Close this finite-product experiment. The diagnostic identifies canonical
projection as the next substantial two-FE cost: about 35 ms of repeated symmetric
sweeps and 15 ms of categorical certification, with roughly 67 ms in projection
wrappers and 18 ms in canonical finite solves. Those are implementation costs,
not established lower bounds imposed by scientific accuracy. Initial and final
canonical construction remain necessary authorities; their current scheduling
and repeated group/weight work are revisable.

Next, attribute same-weight group statistics, scratch transport and repeated RHS
passes within that canonical boundary. Test reusable group/Schur geometry across
compatible RHS tiles and direct level-space solving where the reduced system
permits it, with independent original-weight certificates and disconnected-mass
controls. Preserve the existing failed all-coefficient canonical evidence;
removing its guard alone is not a solution. Compare complete commands and protect
fast categorical PPML and OLS. Do not continue tuning this product loop or assign
the remaining gap generically to mandatory verification.

Whole-command one-FE setup/objective work and wide inference remain later targets.
The frozen 77/16/111 definitions, protected cycle seeds 211/212, deferred SCC and
physical-memory checks remain unchanged. Seeds 350 and 351 are consumed. No push,
release, stopped-campaign restart or SCC authentication retry occurred.

## Immutable receipts

`receipts-content-addressed.zip` contains **13,267 logical receipts** in 2,635
payloads, 18,736,553 bytes. Container SHA-256:

`dd717c7c5ebafa81f608afcdae6dc7d67da2a46f913b16f09aaceb23a7cb6306`.

The archive includes exact artifact sources/identities, controllers, declarations,
all numerical exports, timing distributions, phase spans, tests, allocation checks,
failed build receipt and predecessor follow-ups. Raw licensed logs and generated
binary data are omitted with hashes. Paths are sanitized; binaries/licenses are
not distributed. `restore_receipts.py DESTINATION` checks the container, index
and every logical-file hash before restoring them. The complete archive was
verified and restored locally before handover. `summary.json`, `details-final.json`,
`final-blocks.json`, `final-post-results.json` and `jointcolpar2-licensed-results.json`
are the compact entry points after restoration.
