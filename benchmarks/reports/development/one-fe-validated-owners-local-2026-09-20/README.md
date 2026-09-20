# Immutable prepared owners — local Mac development

Checked, immutable owners now preserve numeric-sample and categorical-topology
validation through preparation, fitting, DoF and saved-effect recovery. The
current twenty-cell screen wins all 60 pairs against pinned fixest. Nineteen
cells meet the 0.90 median paired ratio; **wide full outputs remain at 0.927342**.
Neither required final five-pair block has run. This is a development checkpoint,
not release or cross-platform qualification.

## Change and scope

The prepared categorical numeric sample is held behind a module-private field.
Its constructor performs the existing alignment, finite-value, weight and row-map
checks. It provides only shared access. Internal consumers therefore need not
repeat complete numeric scans or recheck the same slope source's values. Both
ordinary categorical preparation and PPML's empty-topology constructor use the
owner, after rebasing at the original validation boundary. Caller-supplied
working responses and weights retain their checks.

The finalized topology has a separate checked owner. Construction validates
the exact source partitions, code ranges, membership and coverage together.
No mutable or consuming accessor can replace its buffers. One-source, one-term
consumers can construct a borrowed descriptor from this proof without scanning
membership again. Observation dimensions, term-map checks, effect metadata,
DoF policy, cluster nesting and numerical certificates remain consumer checks.
Public `PreparedCategoricalOlsData::validate()` still explicitly revalidates
the complete artifact. Generic solver adapters retain their existing checks.

These owners apply to categorical OLS, weighted and clustered saved-FE paths,
interactions, absorbed slopes and PPML preparation. Direct descriptor reuse is
narrower: one compact source and one noninteraction topology term. Ordinary
unsaved numeric one-FE commands use the legacy prepared representation, whose
public mutable fields cannot support the same permanent proof. Their measurements
are controls for this increment. No PPML speed claim is made here.

No observation-sized buffer, unchecked constructor, SPI worker call or new public
command is introduced. Ownership, allocation admission and transactional output
publication remain unchanged. Pointer, retained-byte and object-size tests check
that wrapping the existing owners does not copy or enlarge their storage.

## Incremental measurements

All primary cells use N=983,040, K=10/50, 16 native/fixest workers, Stata MP
eight processors and BLAS one on the local M2 Ultra. Three rotated repetitions
run in fresh processes with a small warm-up. Input import and audit exports are
excluded. Full outputs include saved FE, residual publication and public
`predict` calls for xb/xbd, with matching fixest recovery/expansion. No builds,
tests or data preparation ran alongside timing.

The numeric-owner screen reduces wide full-output medians from **496 to 481 ms**
(median paired ratio 0.965863), and wide robust saved FE from **552 to 540 ms**
(0.978142). Narrow changes and wide clustered savings are small.

The separate topology-owner screen gives these medians:

| Request | K | Numeric owner (ms) | Both owners (ms) | fixest (ms) |
|---|---:|---:|---:|---:|
| Full outputs, IID | 10 | 198 | 196 | 220 |
| Robust, saved FE | 10 | 171 | 164 | 240 |
| Two crossing clusters, saved FE | 10 | 227 | 219 | 336 |
| Full outputs, IID | 50 | 483 | 487 | 524 |
| Robust, saved FE | 50 | 542 | 543 | 942 |
| Two crossing clusters, saved FE | 50 | 622 | 621 | 1,382 |

Narrow robust and clustered saved-FE median paired ratios are 0.959064 and
0.964758. The topology increment has **no clear wide-model gain** in this screen.
The preceding numeric-owner phase probe records about 1.8 ms of narrow recovery
setup validation, alongside further scans at fitting and DoF boundaries. That
probe belongs to the numeric-owner artifact, not the final combined artifact.

## Current breadth screen

| Request | K=10 fereg/fixest (ms) | K=50 fereg/fixest (ms) |
|---|---:|---:|
| IID | 127 / 148 | 410 / 497 |
| Robust | 142 / 165 | 534 / 934 |
| Cluster on FE | 145 / 182 | 501 / 978 |
| Crossing cluster | 154 / 185 | 517 / 867 |
| Two crossing clusters | 184 / 253 | 583 / 1,326 |
| Two nested clusters | 175 / 243 | 595 / 1,267 |
| IID, saved FE | 152 / 212 | 418 / 536 |
| FE cluster, saved FE | 175 / 253 | 536 / 1,015 |
| Two crossing clusters, saved FE | 221 / 335 | 631 / 1,426 |
| Full outputs, IID | 206 / 259 | 485 / 525 |

Against the original `normonly` fereg baseline, all twenty breadth regression
cells pass: maximum cell median paired ratio **1.019851**, geometric mean
**0.689816**. These are cumulative campaign gains, not owner-only gains.

All eleven size/thread/group-layout controls also pass against `normonly`:
maximum cell median paired ratio **1.046178**, geometric mean **1.002524**.
Their 66 calls include 33 nonself exact native scientific comparisons. The
largest ratio is the K=50, 1,000-group, four-thread cell. See the
[control summary](controls/regression.json) and [science](controls/science.json).

The narrow full-output/wide IID transition showed a timing spike. A separate
process snapshot found Time Machine backupd, Finder and SystemUIServer consuming
CPU. This is possible contention, not a measured causal attribution. Every call
is preserved; the narrow full-output median above should not replace the cleaner
incremental 196/220 ms comparison. See the
[background observation](owners-background-observation.json). The full breadth
screen is not a final acceptance block.

## Scientific evidence

All application statuses, process return codes and terminal sentinels pass.
The 18 numeric-owner comparisons, eight phase-probe comparisons and 18 topology-
owner comparisons are exact native scientific matches; their 44 saved-output
bundles are byte-identical. The current full breadth screen also matches the
previous `recoverycore` checkpoint exactly in all 60 native comparisons and
all 24 saved-output bundles.

A stricter full-array comparison with the much earlier `normonly` baseline
initially failed. Thirty-six comparisons are fully exact; the other 24 differ
only in certificate residuals (rows 7, 8, 23) and fit statistics (rows 3, 7, 8,
12). Those differences already occur in the earlier accepted checkpoint, as
the 60 exact `recoverycore` comparisons establish. Coefficients, covariance,
sample, statuses, DoF, certificate pass flags and every saved-output byte match
the original baseline exactly. The unchanged registered numerical checks pass.
The [full baseline audit](owners-breadth-regression-native-audit.json) preserves
every difference; no tolerance or certificate requirement was changed.

The final combined artifact passes **24 focused Rust tests**, **nine licensed
Stata fixtures** and the actual-plugin ABI/panic-containment harness. Tests cover
corrupted code/membership rejection, wrong-owner receipts, empty and generic
topologies, numeric/weight/row-map validation, retained versus fresh projection,
explicit weighted recovery and PPML generations. Fixtures include independent
input/dummy oracles, unit/AW/FW/PW OLS, categorical interactions, absorbed slopes,
prediction after edits, saved outputs, rollback and PPML saved slopes.
See [validation](validation.json) for source-bound records. An initial test-only
compilation mistake was corrected before the final build; its log hash remains
recorded. Raw licensed logs remain local.

## Identity and next boundary

The current `topologyowner` plugin is
`4339b23d7ff3249544ccd600751231139f21412675c43bee0535ae1a6d3f6366`,
built from `e032b1a4e617b1f39a5656659076c6c48b8a1e71` plus the five exact
overlays in [its identity](identity-topologyowner.json). The numeric-only and
instrumented artifacts have separate identities. Build metadata retains the
harness's September 19 stamp; these executions occurred September 20.
fixest remains the stock 0.14.2 archive with SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.

The [primary archive](primary/receipts.json) retains 296 calls, paired
[timings](primary/timings.json), sampled RSS, source/input/artifact hashes and
registered comparisons. The 66 controls bring this checkpoint to **362 distinct
timed calls**. No installed package or remote state changed.

The remaining measured gap is wide full outputs. The next bounded experiment
is extending independent outcome, residual/bread and score/RSS overlap to wider one-FE fits while
preserving the existing reduction order, deferred-error order and memory
allowance. The current width guard and 16-slot anticipated-status array must be
handled together. No such widening is part of this checkpoint. Directly reusing
the raw residual for output construction was not implemented: its negative-FMA
accumulation differs from the output predictor's positive-FMA accumulation and
subtraction, so algebraic equivalence alone is insufficient.
