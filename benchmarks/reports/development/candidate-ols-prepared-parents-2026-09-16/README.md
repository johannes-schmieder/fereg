# OLS retained-parent accounting and pruning lifetimes

The uncommitted implementation adds capacity-based OLS preparation and inference
parents to internal admission, and releases obsolete pruning buffers earlier.
Scientific and boundary checks pass. **Adoption remains pending:** the fixed
screen flags a slope timing regression, and one bounded identical-artifact
control check leaves attribution indeterminate. Whole-action memory coverage
is still incomplete. This is development evidence, not a production candidate.

## Implemented scope

- Before SPI row reads, admit the numeric value/missing banks, candidate flags,
  maps and reason masks that can coexist during classification/compaction.
  The checked scalar model takes maxima over sequential stages; it allocates
  no reporting arrays. Factorization dictionaries remain outside this bound.
- Observe all retained prepared buffers, including excess capacities, nested
  recipes, labels, category levels, deduplicated codes and membership indexes.
  Add only their excess over the old base's explicit prepared-payload allowance;
  retain all its other numerical reserves. Reevaluate internal fast/low selection
  and pass the reconciled parent to existing solver admission.
- Before saved-output construction/recovery/publication, add the simultaneously
  live prepared and inference owners. Inference includes residual/bread, score
  caches, FE coefficients, warnings, DoF metadata, VCE and cluster subsets.
  Preserve any larger existing execution reserve. Repeated checks do not
  accumulate duplicate charges. Public base fields keep their old definitions.
- Release obsolete singleton lists, pruning codes and availability topology
  before replacement compaction. Classifier, weights, missingness/singleton
  precedence, row order, solver policies and publication transaction are unchanged.

Counters walk container descriptors, never observation values. Independent
allocator-deallocation tests check exact retained totals rather than restating
the formulas. Preparation covers simple/complex terms, duplicate effects,
ordinary/interaction clusters and none/AW/FW/PW weights. Inference covers all six
VCE/topology routes in fast and low modes. Exact/one-byte-short admission checks
verify unchanged public base fields, propagation into solver budgets, repeat
calls and rejection before stores. Existing failure/interruption controls pass.

## Source and tests

Native snapshot `5240e58-ols-prepared-parent2` is based on main
`5240e589400c888690666265263ad96f9b694aa6`. All **163 native source files** still
match the tested snapshot. SCC job **7590173** passes **285 distinct tests**:
172 core, 108 plugin, 3 testkit and 2 constrained-memory checks. It has
failed=0, exit_status=0 and `OLS_PREPARED_PARENT_PASS`; wall time including
build is 145 s and scheduler maxvmem is 4.891G. The earlier narrower snapshot,
job 7590163, is preserved separately and does not stand in for the final patch.

OLS inventory v21, action inventory v4 and the complete repository checker pass.
Of 35 source-contract Python tests, 34 initially passed; one mutation anchor
also matched a new test body. Adding the terminating newline makes the anchor
unique, and that focused recheck passes. All three action-inventory tests pass.
The original failed Python receipt is preserved. F0 and prior SHA-based boundary
pins remain unchanged; the new exact dispatch body has its own preserved
source excerpt and hash checks. Structural inventories do not prove byte coverage.

The isolated comparison baseline is the preceding **unadopted publication
prototype**, not the committed main artifact. The hashes are:

- Baseline `5240e58-ols-publication-admission1`:
  `dd3d00eee8e114a0c325ae14b6d44601eb0d1a8394ec267bbba297067b3bfac4`.
- This snapshot:
  `a1c53228f37a23d02ea1ac06fbb2a7a0c68aedb21a1b072f9b6bf8d2adb7bd19`.

The Linux build, binary audit and actual-plugin ABI/self-test pass. Source,
reference lock, artifact, input and harness hashes accompany the receipts.

## Fixed paired screen

Job **7590187** completes **27 alternating pairs / 54 fresh-process commands**
on nine predeclared synthetic cases. It has failed=0, exit_status=0 and
`OLS_PARENT_PAIRED_COMPLETE`. All scientific matrices, sample/order, saved
outputs and non-time solver fields match exactly. Weight totals, rank/DoF,
public base-memory fields, selected modes and scratch also match.

| Case | Baseline median (s) | New median (s) | Change |
|---|---:|---:|---:|
| small_narrow | 0.039 | 0.038 | -2.56% |
| wide_one_fe | 1.056 | 1.069 | +1.23% |
| weak_wide | 4.539 | 4.672 | +2.93% |
| tiny_weighted_bridge | 0.287 | 0.282 | -1.74% |
| three_fe_fw_generated | 0.555 | 0.579 | +4.32% |
| absorbed_slopes | 0.238 | 0.262 | +10.08% |
| saved_generated_aw | 0.941 | 0.849 | -9.78% |
| singleton_cascade_16384 | 0.387 | 0.373 | -3.62% |
| singleton_cascade_32768 | 0.315 | 0.293 | -6.98% |

The slope result exceeds both 5% and 10 ms. Its native-phase medians are
0.133931/0.147370 s; native allocation is 3,243,531/3,239,679 bytes. The first
pair is slightly faster in the candidate, so the original observations alone
do not identify an accounting-cost mechanism.

The cascade cases remove exactly half their rows (8,192 and 16,384 retained),
with PW, clustered inference, absorbed interactions and low memory. The larger
case's native peak falls from 9,771,567 to 8,360,877 bytes (14.44%). This is a
bounded component-lifetime observation, not a general memory or speed claim.
The complete-command scale timings are nonmonotonic and cannot qualify pruning
complexity; the cases protect a substantial singleton share during this patch.

The three-FE base-reporting discrepancy reproduces: 49,456,530 observed native
bytes versus 22,983,168 base bytes. The baseline reports 49,452,240 against the
same base. Separately admitted solver memory remains absent from that public
base field; these ratios do not establish a cap breach. Preserved German
outputs and their prior reconciliation were not rerun or relabeled.

`paired-results.json` retains total and native phase times, allocation, sampled
RSS, scratch, solver/fallback fields and scientific export hashes. Discarded
attempt cost is included in elapsed time but is not separately observed and
remains null. Gains in this noisy screen are not optimization-adoption claims.

## One bounded attribution check

Job **7590196** interleaves three identical-baseline controls with three new
baseline/candidate pairs for the same slope input and unchanged artifacts.
All 12 commands pass exact science, with failed=0, exit_status=0 and
`OLS_PARENT_CONTROL_CHECK_COMPLETE`. Comparison medians are 0.217/0.236 s
(+19 ms, +8.76%); identical-artifact controls are 0.317/0.215 s (-102 ms).
The controls exceed the same threshold. Attribution remains **indeterminate**;
the original failed gate is preserved and remains open. No further fit retry
or campaign expansion is scheduled.

## Remaining obligations

Request/compiler containers, factorization and construction scratch, complete
finite/inference transient frames, slope recovery, and opaque dependency/lazy
worker-pool storage still require byte-and-lifetime proofs. Retained-owner
checks do not certify those scopes or PPML's complete parent/frame overlap.
The whole-action ratio remains undefined. Versioned public action diagnostics,
final Mac/Linux candidate freeze, Mac prerequisites and clean-package validation
remain later gates. The original publication prototype's cost uncertainty also
remains recorded in its separate report.

The installed Mac plugin is unchanged. No production commit, push, package
replacement, tag, release, empirical rerun or unrelated job intervention was
performed. Portable receipts normalize execution paths and preserve literal
receipt hashes; raw licensed logs and data are not committed.
