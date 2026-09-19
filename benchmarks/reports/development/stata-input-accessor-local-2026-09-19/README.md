# Stata input loading: a measured, modest accessor gain

**The guarded fast accessor works on the tested Mac and reduces wide-model
loading by about 9%. It does not remove the scalar SPI bottleneck or close the
fixest gap by itself.** Five rotated fresh-process comparisons reduce median
input time from 305.7 to 279.2 ms and complete-command time from 582 to 559 ms.
Every fast-arm command is faster than its paired baseline, with identical
scientific results. Median paired reductions are 8.8% for input and 4.5% for
the command; these need not equal reductions calculated from separate medians.

This is an isolated development experiment. Production source, installed
packages and remote jobs were not changed. The [harness](../../../stata_input_probe/README.md)
and [candidate patch](fast.patch) preserve the tested implementation.

## What was isolated

The current SDK header selects `safevdata` for `SF_vdata` by default. Its
`SD_FASTMODE` alternative selects `vdata`; it also changes variable stores and
matrix accessors. The experiment changes only the numeric batch reader, leaving
every other accessor and publication store checked. Both callbacks are exposed
in the [official SDK header](https://www.stata.com/plugins/stplugin.h).

Three plugins share the same optimized Rust static archive and identical ado
source, from source `55c19cb679d675194c6baed74b09279e1f026b16`:

- **Baseline:** the original bridge loop and checked accessor.
- **Cached checked:** validate the numeric batch's observation bounds, variable
  bounds/types and destination pointers once, cache `safevdata`, and remove the
  repeated destination-pointer check from the cell loop.
- **Guarded fast:** identical to cached checked except that the cached callback
  is `vdata`.

The C bridge still transports data only. Rust owns missing masks, sample rules,
weights and all scientific computation. Calls stay on Stata's bound main thread.
The range check avoids signed overflow, errors still stop the reader, and
the normal chunk interruption checks remain unchanged. The existing action
preflight already validates variable bindings; the experimental bridge checks
its own batch preconditions as well. No uninitialized storage is introduced.

The header establishes the callback and ABI, but does not fully document the
checks omitted by its fast mode. We therefore do not infer equivalence for
invalid SPI accesses. Invalid requests are rejected before entering that
callback; tests check this explicitly. The result is evidence for a guarded
numeric reader on this platform, not for enabling fast mode globally.

## Command results

All large cases have 983,040 observations and one categorical FE. These are
complete-data, unweighted numeric IID OLS fits on the same frozen generated
datasets as the preceding [profile](../one-fe-profile-local-2026-09-19/README.md).
The wide primary uses five rotated blocks; holdouts use three alternating pairs.
Each command runs in a fresh Stata process after a small untimed warm-up.
Input values are hash-verified before timing, and all estimator work is timed.

| Case | Variant | Input, ms | Complete command, ms |
|---|---|---:|---:|
| 50 regressors, 16 threads | Baseline | 305.7 | 582 |
| | Cached checked | 298.8 | 574 |
| | Guarded fast | 279.2 | 559 |
| 10 regressors, 16 threads | Baseline | 84.4 | 221 |
| | Guarded fast | 79.4 | 218 |
| 50 regressors, 1 thread | Baseline | 295.7 | 1,692 |
| | Guarded fast | 269.8 | 1,654 |
| 50 regressors, shuffled rows, 16 threads | Baseline | 303.0 | 600 |
| | Guarded fast | 271.9 | 573 |

The five wide primary command pairs save 20, 32, 26, 27 and 17 ms. Baseline
commands range from 581 to 586 ms; fast commands range from 550 to 568 ms.
Input saves approximately 25–31 ms per pair. The native-computation median
is slightly higher in the fast arm, so the input gain is not amplified into
an implausibly large command claim. Cached checked alone has a smaller,
less consistent complete-command gain, including one slower repetition.

The wide single-thread and shuffled controls retain the input improvement.
The narrow command benefits only by a few milliseconds; this does not resolve
its competitiveness problem. The small 360-row smoke is a correctness check,
not a meaningful millisecond performance comparison.

Raw numeric command data and paired ratios are in [measurements.json](measurements.json)
and [summary.json](summary.json). Native allocation peaks agree across variants
for every configuration; the candidate adds no buffer. Process RSS was sampled
by the existing harness but no comparative RSS claim is made.

## Where the saving comes from

A separate plugin reads the same requested numeric values into preinitialized
buffers. Its timing excludes allocation, page initialization, missing masks
and output hashing. Ten rotated repetitions per mode follow one discarded
warm-up repetition. Every mode produces the same output hash. These kernel
times are not substituted for full-command results.

| Read kernel | 10 regressors, ms | 50 regressors, ms |
|---|---:|---:|
| Current-style checked column writes | 69.4 | 254.7 |
| Cached checked column writes | 67.5 | 247.5 |
| Cached fast column writes | 66.7 | 225.8 |
| Fast contiguous row writes only | 64.8 | 201.3 |
| Fast 256-row tiles plus conversion into columns | 69.4 | 231.5 |

Wide callback caching and loop cleanup account for roughly 7 ms here; switching
the callback with the same surrounding loop saves a further 22 ms. The combined
kernel saving is approximately 29 ms, consistent with the full loader's 27 ms
median reduction. At 51,118,080 cells, the full-loader saving is about half a
nanosecond per cell. This supports a modest repeated-call optimization, not the
earlier hypothetical one- or two-nanosecond saving.

Contiguous row output looks another 25 ms faster, but it is a different data
representation. Paying for the tested 256-row transpose into the existing
column buffers removes that advantage and is slightly slower than direct
fast column writes. We reject that particular transport-only tile design.
This does not rule out a future estimator that consumes row tiles directly,
or every possible tile size. Neither was implemented here.

[kernel-summary.json](kernel-summary.json) and the two kernel TSV files retain
every accepted repetition, including ranges and hash agreement.

## Correctness and failure checks

All 36 full-command calls pass process, application and terminal-sentinel
checks. Every candidate is compared with its matching baseline: the exact
sample hash, full coefficient/covariance tables, rank/status, DoF, fit and
certificate exports agree. Three small explicit-dummy least-squares checks
provide an independent oracle. These checks are repeated during archival.

Separate live accessor probes compare individual doubles byte for byte over
all five numeric storage types, all 27 missing codes, very large finite
positive/negative doubles, reordered plugin varlists, and a plugin call with
`if`/`in`. Each of two calls compares 1,620 values. The accessor reads requested
indices; candidate selection remains the caller's responsibility.

Three existing licensed fixtures pass against the fast candidate:

- `ols_native_input`: storage/missing codes, AW/FW/PW, independent dummy
  regressions, wide and incomplete inputs, restrictions, strings/clusters,
  factor metadata and failure/order restoration.
- `candidate_input_oracle`: retained runs at the scalar/batch boundary, chunk
  crossings, fragmented samples, mixed sources, weights, OLS/PPML oracles and
  adjustment/domain errors.
- `estimate_ols_eclass_transaction`: exact restoration of prior public state
  at injected failure stages and normal eclass behavior.

All three actual-plugin ABI/self-test runs pass, including panic containment.
The mocks initialize both SDK callbacks. Separate bridge tests with UBSan
verify uninitialized sessions, worker-thread refusal, invalid/null shapes and
pointers, numeric-variable bounds/types, observation bounds and integer
overflow, absent callbacks, and immediate propagation of an injected callback
error. A counting callback asserts that invalid batches perform no reads.

[science.json](science.json) contains accepted-call checks, scientific-output
hashes, oracle results and fixture identities. [identities.json](identities.json)
binds the plugins, unchanged Rust archive, probe and controller sources.

## Interpretation and next step

The guarded fast callback is worth retaining as an optimization candidate:
approximately 9% less wide input time and a repeatable 20–30 ms command saving
help close the gap. It does not deliver a loading redesign. The earlier fixest
wide median was 461 ms; the new candidate's 559 ms remains substantially above
that historical measurement. Fixest was not rerun in this experiment, so this
is context rather than a fresh paired competitor comparison.

The remaining input stage is still about 279 ms. The prior diagnostic assigned
27 ms to allocation/initialization and roughly 9 ms to missing masks in an
earlier build. Those figures bound the scale of smaller opportunities; the
whole allocation cost cannot be assumed removable because memory must still
be acquired and written. The numeric SPI remains one call per cell; its
[documented interface](https://www.stata.com/plugins/#sec8a) offers no bulk
numeric-buffer reader.

The next substantial gain likely requires reducing or overlapping work around
those reads: consume a compatible native layout directly and remove later
materializations, or process completed Rust-owned chunks while the main thread
continues reading. That requires a separate sample-aware design and measured
evidence. Missing-mask overlap alone is too small to close the gap.

No production optimization was adopted in this investigation. A production
change should be this small, locally validated numeric-batch path with its
explicit preconditions, rather than a build-wide `SD_FASTMODE` switch. The
current evidence does not establish Windows/Linux behavior, a release claim,
or universal improvement over fixest.

## Environment and excluded attempts

Apple M2 Ultra, 24 cores, 192 GiB RAM, Stata/MP 19 with eight licensed Stata
processors; explicit native thread counts are shown above. The relinks use
Apple clang 17.0.0, the separate Command Line Tools, and the preceding profile's
unchanged release Rust archive. Artifacts are arm64 diagnostic plugins, not
new universal release builds. macOS core placement is uncontrolled. One seed
and bounded repetitions support local development conclusions only.

The first measurement invocation could not enumerate processes inside the
sandbox; no fit started before its permitted retry. An AddressSanitizer guard
test deadlocked during sanitizer/runtime initialization before `main`, as
confirmed by a stack sample. It was terminated; no ASan pass is claimed.
A kernel run overlapping that stalled process was excluded and preserved.
Guard tests subsequently passed with UBSan. The final isolated kernel run also
includes a diagnostic bounds fix for inputs shorter than the 256-row tile;
the measured million-row data are unchanged. Preliminary kernel runs and raw
licensed logs remain local and are not substituted for final receipts.
