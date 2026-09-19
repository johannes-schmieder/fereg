# One-FE OLS: pass reduction and loading overlap

**Overlapping input conversion with Stata reads works in the tested prototype.
Together with the earlier guarded fast accessor and a new parallel validation
check, it cuts the wide command from 582 to 522 ms. Fixest takes 456 ms in the
same five comparison blocks.** This is a 10.3% paired command reduction and
closes about 48% of the difference between command medians. Fereg still takes
about 14.5% longer than fixest on this case.

Only the small parallel-validation change is retained in production source.
The loading pipeline is a measured feasibility prototype: its buffers and
worker must be integrated with Rust memory admission and the resolved thread
policy before adoption. Installed plugins remain unchanged.

## Complete-command confirmation

All primary fits use the same frozen 983,040-row dataset, 50 continuous
regressors, one categorical FE with 196,608 levels, unit weights and IID VCE.
Five fresh-process blocks rotate these five arms. Each has an untimed small
warm-up and a byte-level input audit; estimator loading, computation and
publication are timed. Stata requests 16 native workers; fixest requests 16
threads, with external BLAS limited to one.

| Arm | Input, ms | Native through precommit, ms | Command, ms |
|---|---:|---:|---:|
| Stock fereg | 304.3 | 205.5 | 582 |
| Parallel finite-value validation | 302.5 | 184.3 | 562 |
| Validation plus guarded fast accessor | 279.3 | 185.1 | 542 |
| Validation, fast accessor and pipelined conversion | 258.0 | 190.8 | 522 |
| Pinned fixest 0.14.2 | — | — | 456 |

Entries are separate medians, so differences need not equal paired medians.
The validation-only paired command reduction is 3.1%; the combined reduction
is 10.3%. Every combined repetition beats stock, saving 60, 60, 69, 50 and 65 ms.
Pipeline versus fast-accessor-plus-validation saves 24, 26, 22, 11 and 25 ms.
The pipeline's slightly higher native median limits the translation of its
input saving into a command saving; no speedup is inferred from input alone.

The primary plugin hashes begin `0c6d5966` (stock), `275ad05a` (validation),
`a217945e` (validation/accessor) and `48e54eb8` (pipeline). Full identities,
source manifests and the exact fixest archive hash are preserved below.
The comparator was rerun; 456 ms is not a borrowed historical measurement.

## What reduced the native cost

The one-FE initial projector performed a serial finite-value check of the entire
raw RHS immediately before its parallel numerical work. At this width the RHS
contains 50,135,040 doubles, approximately 382.5 MiB. The new check runs on the
already installed Rayon team for fresh one-effect input with at least 16,384
rows and eight RHS columns. Other entry paths and one-thread calls retain the
serial check. No buffer, numerical reduction or certificate is removed.

The check remains before weight validation and workspace reservation. Deleting
it or relying on a later norm pass would change invalid-input error precedence.
A focused test injects NaN and both infinities at the beginning, middle and end,
on both sides of the row/width thresholds, with one and four workers. It also
supplies a bad weight shape and an allocation failure: the same nonfinite-data
error must win without an allocation attempt. No ambient Rayon pool is created
by this check.

Separate final traces put initial absorption at 55.9 ms for stock and 39.3 ms
for the retained change. An earlier screening trace measured 55.4 versus 33.7
ms. The five uninstrumented command comparisons support a repeatable modest
gain; the larger individual trace difference is not substituted for that result.

The preliminary validation-only holdouts have medians of 223 versus 221 ms
(narrow), 606 versus 584 ms (shuffled wide), and 1,674 versus 1,680 ms (one
thread). The latter retains identical serial code and has a median paired
slowdown of 0.12%, with noisy individual differences. These holdouts tested
`parallelcheck`; the final patch additionally guards against an ambient pool.
Final artifact confirmation and licensed fixtures ran after that guard was added.

## What loading overlap changes

Direct batch reads write each returned cell into a different column buffer.
The prototype instead reads consecutive rows into one small row-major tile.
A helper transposes that completed native tile into the final column buffers
while the Stata thread fills the other tile. Mutex/condition-variable ownership
prevents either tile from being overwritten while consumed. Every SDK access
and metadata check remains on the bound Stata caller. The helper never reads
Stata memory and is joined on success or an injected read error.

The standalone probe tests 256-, 1,024- and 4,096-row tiles, serially and with
overlap. These timings exclude destination allocation and page commitment but
include helper startup and final join. One warm-up rotation is discarded;
ten rotations are measured. All 176 kernel calls produce matching output hashes
within their dataset, including the checked-accessor reference.

| Wide input kernel | Median, ms |
|---|---:|
| Cached checked accessor, direct columns | 259.5 |
| Cached fast accessor, direct columns | 238.0 |
| Fast accessor, serial 256-row conversion | 244.3 |
| Fast accessor, overlapping 256-row conversion | 233.7 |
| Fast accessor, serial 1,024-row conversion | 245.6 |
| Fast accessor, overlapping 1,024-row conversion | 219.5 |
| Fast accessor, serial 4,096-row conversion | 250.3 |
| Fast accessor, overlapping 4,096-row conversion | 217.8 |

The complete-command adapter uses two 4,096-row tiles within each existing
eligible numeric batch. Unlike the kernel, its command timing includes tile
allocation, page commitment, worker startup, conversion and joining for every
batch. Rust's subsequent missing-value classification and sample preparation
are unchanged. This distinguishes a demonstrated command gain from a faster
kernel that merely moves cost elsewhere.

Three additional rotated blocks test two controls:

| Case, 16 threads | Stock | Validation + accessor | Also pipeline |
|---|---:|---:|---:|
| 10 regressors | 222 ms | 217 ms | 215 ms |
| 50 regressors, shuffled rows | 603 ms | 558 ms | 537 ms |

The wide gain survives shuffled rows. The narrow pipeline gain is too small to
call material: its input median actually increases from 79.9 to 81.4 ms, while
native-time variation offsets it. The standalone narrow kernel similarly saves
only about 2 ms with 4,096-row tiles; 256-row synchronization is slower.
No one-thread pipeline speedup is claimed: this prototype creates a helper
independently of the requested Rust team count.

## Pass-reduction attempts that did not pay

These separate screening rounds each include their own rotated stock controls.
They are not pooled with final confirmation.

| Prototype | Its stock median | Prototype median |
|---|---:|---:|
| Initialize the RHS directly through indexed Rayon collection | 578 ms | 587 ms |
| Concurrent RHS construction and raw moments from source columns | 578 ms | 594 ms |
| Both changes together | 578 ms | 590 ms |
| Concurrent moments with contiguous column scans | 574 ms | 590 ms |
| Direct initialization with per-worker traversal state | 572 ms | 580 ms |

Direct initialization removes the zero-fill pass safely, but loses the existing
128-row tiled copy's efficient access pattern; eliminating per-cell division
in a second variant did not recover a repeatable gain. The source-moment
variants preserve fixed row partitions, scaled norm arithmetic, compensated
sums and canonical merges, while evaluating matrix construction and moments
concurrently. They allocate no additional observation-sized matrix.

In the original overlap trace, the joint RHS/moment phase costs 58.1 ms versus
30.4 + 19.0 ms sequentially. The columnwise revision costs 55.1 ms versus
29.9 + 20.8 ms for its control. A near-zero later raw-moment timer simply means
the work was charged to the joint phase. It is not free computation. These
measurements are consistent with memory-access and scheduling costs erasing
the intended overlap benefit; no hardware-counter attribution is claimed.
All five variants are rejected and remain isolated from production source.

## Correctness, memory and adoption boundary

All **104 regression calls** pass process, application and terminal-sentinel
checks. Candidate native sample hashes, full coefficient/covariance matrices,
status, DoF, fit and certificate exports agree exactly with their paired stock
fit. The five fixest fits pass the existing comparator tolerances and exact
sample checks. Four small explicit-dummy smoke checks are retained; their
360-row size does not exercise the wide optimizations.

The final validation artifact and the pipeline artifact each separately pass:

- `ols_native_input.do`, including wide independent Stata regress/dummy oracles
  at one, four and sixteen native threads;
- `candidate_input_oracle.do`, covering restricted runs, mixed sources, weights
  and shared input controls;
- `estimate_ols_eclass_transaction.do`, preserving prior public results on errors.

Three focused Rust tests pass: the new rejection/precedence test, the existing
one-way weighted projection/error oracle and fixed-partition scaled-norm
stability across worker counts. Every newly built Rust variant passes the
actual-plugin ABI/panic-containment test. Two UBSan mock programs check tile
boundaries, tail batches, exact converted values, main-thread-only callbacks,
reordered variables, nonzero observation offsets and injected read failures.

The retained Rust change adds no allocation. Reported native peaks are identical
across primary native arms at 925,859,855 bytes. **That does not establish equal
memory for the pipeline:** the C diagnostic allocates 3,407,872 bytes (3.25 MiB)
for the primary's 52 input columns, plus a helper stack, outside the Rust ledger.
Sampled process RSS is archived without a comparative memory claim.

The pipeline must therefore remain experimental. A production version needs
fallible Rust-owned tile admission, accounted worker lifetime, a bounded worker
under the resolved team policy, and a true one-thread path. The full-command
prototype establishes that overlap can pay; the cost of that production
integration remains unmeasured. The earlier fast-accessor experiment's SDK
limitations also remain. No global SDK fast-mode switch is proposed.

The evidence supports roughly 20–25 ms of additional wide-command improvement
from loading overlap after the accessor/validation changes. It does not support
winning this multithreaded case yet. Further gains need a different boundary,
such as consuming a native row representation more directly or reducing Stata
preparation/publication overhead; neither is implemented or measured here.

## Reproduction and limits

Use the [harness](../../../one_fe_passes/README.md). [measurements.json](measurements.json),
[summary.json](summary.json), [phases.json](phases.json), [science.json](science.json),
[kernel-summary.json](kernel-summary.json) and [identities.json](identities.json)
retain numeric receipts, comparisons and source/artifact identity. Patches apply
to a formatted copy of baseline source `55c19cb679d675194c6baed74b09279e1f026b16`.
`final.patch` is the retained Rust change; `fastfinal.patch` and `pipelinefit.patch`
are diagnostic C additions on that Rust archive. Source manifests distinguish
all earlier rejected variants and the preliminary validation artifact.

Raw data, licensed logs, binaries and source snapshots remain in the ignored
local `target/one-fe-passes-20260919` directory. The original source's formatting
is preserved in the repository; isolated builds run rustfmt, with the same
scientific source content. No historical receipt was rewritten.

This is local development evidence on the Apple M2 Ultra, macOS 26.6.2,
Stata/MP 19, Rust 1.85.1 and Apple Clang 17, using the prior pinned fixest 0.14.2
archive. Stata's license permits eight Stata processors; these experiments
request sixteen native workers. macOS core placement is uncontrolled. Builds
are arm64 diagnostics, not universal/cross-platform qualification. No remote
job, branch, commit, push or installed-package update occurred.
