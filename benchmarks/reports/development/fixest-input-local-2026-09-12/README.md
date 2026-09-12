# Moving OLS input preparation into Rust

The retained implementation reduces complete-command time by 11–16% on the
large, 50-control comparisons, without changing the estimated results or
solver behavior. One FE now narrowly beats fixest at four native threads.
At sixteen threads, fereg beats fixest on the well-connected degree-three
graph and is within 2.4% on degree six. One FE still takes 28% longer than
fixest at sixteen threads. This is a measured improvement, not universal
parity or evidence of a hard performance limit.

These are local development results on the M2 Ultra, with no SCC execution.
The baseline is the retained implementation from the
[preceding scheduling investigation](../fixest-scheduling-local-2026-09-12/README.md),
not the original committed source. No solver routing or tolerance changes
were made in this follow-up.

## Complete-command results

Median seconds across three seeds; 983,040 observations, 50 controls, IID
inference. Each arm runs in a fresh process after a small untimed warm-up.
Baseline, candidate, and fixest order rotates across seeds. Dataset import,
process startup, and result extraction are excluded; all estimator preparation,
loading, computation, certification, and publication are included.

| Graph | Native threads | Previous fereg | Current fereg | fixest | Reduction vs previous | Current vs fixest |
|---|---:|---:|---:|---:|---:|---:|
| One FE | 4 | 0.916 | 0.776 | 0.806 | 15.3% | 3.7% faster |
| Well-connected degree 3 | 4 | 1.271 | 1.130 | 1.900 | 11.1% | 40.5% faster |
| Well-connected degree 6 | 4 | 1.195 | 1.037 | 1.542 | 13.2% | 32.7% faster |
| One FE | 16 | 0.690 | 0.582 | 0.454 | 15.7% | 28.2% slower |
| Well-connected degree 3 | 16 | 0.878 | 0.763 | 0.841 | 13.1% | 9.3% faster |
| Well-connected degree 6 | 16 | 0.836 | 0.712 | 0.695 | 14.8% | 2.4% slower |

The small four-thread one-FE lead should be interpreted cautiously: the three
candidate times were 0.775–0.777 seconds and fixest's were 0.787–0.813, but
these are three local repeats, not a broad hardware qualification. Degree-three
candidate timing at four threads ranged from 1.113 to 1.199 seconds. See
[all ranges and measured RSS](primary-summary.csv).

The machine has 16 performance and 8 efficiency cores. Stata is limited by
its license to eight application processors; the native fereg/fixest teams
request 4 or 16 workers. macOS controls placement. The references and runtime
versions are preserved in [environment.json](environment.json).

## What changed, and what the experiments establish

1. **Missing-value detection moved into Rust.** The old ado generated a
   completeness marker, ran `markout` over every source, and counted exclusions
   before calling the plugin. Rust now fills numeric columns in the same
   efficient observation order and constructs their missing masks from the
   SDK's runtime missing boundary. This retains ordinary and extended missing
   codes for every numeric storage type. The C bridge transports values and
   SDK metadata; it contains no sample or estimation logic. Stata documents
   [the missing-value ordering](https://www.stata.com/manuals/dmissingvalues.pdf)
   and [the SDK representation](https://www.stata.com/plugins/).
2. **The redundant sort-preservation wrapper was removed.** This transaction
   does not sort: metadata parsing and TS evaluation leave row order intact,
   and native row maps control stores. The isolated three-seed experiment
   reduced one-FE sixteen-thread medians from 0.693 to 0.644 seconds. Direct
   transaction failure tests check row order and stored sort metadata, so an
   outer test wrapper cannot conceal an order change.
3. **Wide native preparation uses the existing worker team.** The initial
   complete-sample predicate was still scanning controls serially. Running
   wide IID preparation on the requested team allows independent source checks
   to run in parallel and keeps ownership transfer free of redundant full-data
   copies. In the isolated traced comparison, one-FE preparation fell from
   52.3 to 19.7 ms and degree-three preparation from 61.2 to 28.4 ms. The
   one-FE command median fell from 0.615 to 0.572 seconds in that experiment.

The native input change alone reduced the isolated median from 0.679 to 0.650
seconds. Combining it with wrapper removal produced 0.608 seconds before the
parallel-preparation change. These are separate screening batches; differences
between their medians should not be added as an exact decomposition.

Reducing the loading chunk from 65,536 to 8,192 rows produced no consistent
benefit: paired one-FE times were 0.605/0.610, 0.602/0.604, and 0.613/0.600
seconds for the original/smaller chunk. The original size is retained.
The [source patches](identities.json) preserve all measured alternatives.

The new design extends the loading procedure in ADR 0013 under the user's
explicit authorization to investigate native preparation; its scope is
recorded in [ADR 0036](../../../../docs/adr/0036-native-ols-input-preparation.md).
It preserves the protocol-1.5 completeness field, checked when set, and sends
zero from the current ado. Restrictions and string inputs keep candidate-gated
scalar SPI loading. All SPI calls finish on Stata's main thread before the
preparation worker closure runs. Missing/weighted inputs retain the general
native sample-classification path. Factor syntax and TS semantics remain in
Stata.

## Where the remaining one-FE gap lives

The sixteen-thread one-FE primary medians separate as follows. Component
medians are independent and need not add exactly to the command median.

| Component | Previous | Current |
|---|---:|---:|
| SPI loading, allocation, native missing masks | 0.292 s | 0.301 s |
| Native preparation, estimation, validation and stores | 0.236 s | 0.207 s |
| Outer Stata work | 0.160 s | 0.073 s |
| Complete command | 0.690 s | 0.582 s |

The extra native missing-mask pass costs roughly 9 ms here; it replaces more
expensive Stata scanning. This illustrates why timing only the Rust loader
would have rejected a useful change. The final
[component medians](command-parts.csv) and [diagnostic traces](phase-timings.json)
separate these effects.

About half the remaining command time is loading roughly 51 million numeric
cells through main-thread SPI calls. The original suspicion of a hard one-FE
limit was premature: native preparation and outer Stata work both contained
avoidable costs. However, the remaining 0.128-second gap at sixteen threads
cannot be closed just by improving the now-small completeness check.
Eliminating all current outer Stata work would still leave about 0.51 seconds
of loading and native work, compared with fixest's 0.454-second command.

Overlapping loading with missing-mask construction alone has limited upside:
that mask pass adds only about 9 ms. A broader overlap design would need useful
work on partially loaded data, with correct final-sample handling. It was not
implemented or timed here. A more substantial one-FE experiment would keep
numeric data column-oriented longer, reduce RHS conversion/demeaning/normalization
passes, and retain all independent rank and publication checks. The current
weak-graph path provides a natural boundary for such an isolated one-FE kernel.

## Sparse missing values reveal another preparation bottleneck

A separate Stata-only probe sets the last control to system missing on every
997th zero-based row of the same one-FE data, approximately 0.1% of observations.
All other values are unchanged. Three seeds use rotated baseline/candidate
order and exact imported-value hashes with Stata's missing representation.

| Median component, 16 threads | Previous | Current |
|---|---:|---:|
| Input | 1.839 s | 0.299 s |
| Native | 0.862 s | 0.799 s |
| Outer Stata | 0.156 s | 0.075 s |
| Complete command | 2.846 s | 1.174 s |

The previous input loader lost its efficient observation-order traversal as
soon as the completeness precheck found a missing value. The new loader removes
that cliff: the command is 59% faster, with identical sample, full b/V,
certificates, and solver diagnostics. This probe does not compare fixest;
its binary input convention is explicitly Stata-only.

Missing data still more than doubles current command time relative to the
complete sample. Source inspection identifies the remaining general preparation
path's row-wise classification and copies into new columns. A next experiment
should retain the authoritative sample decision while compacting already-owned
columns in their natural order. This is a separate opportunity; no such
compaction change is included in the retained implementation.

## Preserving existing strengths and exposing smaller-model gaps

These holdouts use one fixed seed each, so small millisecond differences are
diagnostic. Solver choice, probes, iterations, stopping criteria, and every
non-timing solver field are exactly unchanged.

| Holdout | Native threads | Previous | Current | fixest, when measured |
|---|---:|---:|---:|---:|
| Weak degree 3 | 4 | 0.142 | 0.137 | 0.402 |
| Weak degree 3 | 16 | 0.154 | 0.146 | 0.167 |
| Dense two-FE | 4 | 0.383 | 0.354 | 1.070 |
| Dense two-FE | 16 | 0.291 | 0.266 | 0.458 |
| Bottleneck ring | 4 | 0.122 | 0.111 | — |
| Disconnected blocks | 4 | 0.089 | 0.085 | — |
| Unbalanced | 4 | 0.094 | 0.087 | — |
| Cycle | 4 | 0.223 | 0.217 | — |

Four additional narrow strong/weak graph comparisons also preserve results and
show no slowdown. One-FE checks at sixteen threads remain less competitive:

| Observations | Controls | Previous | Current | fixest |
|---:|---:|---:|---:|---:|
| 983,040 | 1 | 0.136 | 0.116 | 0.081 |
| 983,040 | 10 | 0.240 | 0.218 | 0.148 |
| 7,680 | 50 | 0.034 | 0.034 | 0.020 |

These are single-seed diagnostics. They prevent extending the large/wide result
to all one-FE problems. The small-data gap is 14 ms; the 1- and 10-control large
cases still have material native/data-access costs as well as wrapper overhead.

## Validation and reproducibility

There are **134 completed measured calls**, including 31 screening calls and
103 on the final comparison setup. All passed. The primary set contains 54
calls in 18 rotated three-arm comparisons. All 25 final comparisons with fixest
pass. All 39 final current/previous pairs have byte-identical full coefficient
and covariance exports, exact samples/statuses/DoF/fit and certificate tables,
and identical solver fields except timings. Six traced calls also reproduce
their untraced scientific results.

All modeled memory budgets agree. Native peak allocation is identical in
38 of 39 pairs, including every primary case. A narrow weak-graph holdout has
an unexplained 819,240-byte lower reported peak; it is not claimed as a memory
improvement. RSS is sampled and remains diagnostic.

Validation passed 35 focused native preparation tests, 40 plugin action tests
(the filter also includes PPML actions), eight licensed Stata fixtures,
universal macOS binary audit, and the actual arm64 ABI/self-test harness.
The fixtures cover all 27 missing codes in five storage types, candidate
exclusions, weights, strings, clustered inference, singleton deletion, TS and
factor behavior, direct failure/order restoration, memory modes, saved outputs,
PPML smoke, and independent wide dummy-regression checks at 1/4/16 threads.
An intermediate ABI-test change incorrectly broadened the mock's NaN-only
missing convention and failed an artificial fweight-domain case. The test was
corrected without changing the old callback contract; the final harness passes.

[Validation and fixture hashes](validation.json), [source/artifact identities](identities.json),
[replayed patch checks](patch-checks.json), [all measurements](measurements.csv),
[exact scientific audit](extra-science.json), and [compact evidence archive](evidence.zip)
preserve the evidence without licensed logs or generated datasets. The retained
universal plugin SHA-256 is
`93412e7c7654deac201f5f8c1bdc5af62aece7ebf4347180df0acebe3d227470`.
No SCC, Linux/Windows scientific run, release qualification, commit, or push
was performed.
