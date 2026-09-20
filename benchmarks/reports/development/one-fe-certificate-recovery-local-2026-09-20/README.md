# One-FE certificate and recovery checkpoint

The retained core change extends the fixed-group OLS certificate reduction to
eligible narrow one-FE tables, carries exact single-source topology validation
through the same immutable recovery borrow, and avoids allocating unused
iterative scratch on direct recovery. Stata output initialization is unchanged.
The native missing-output experiment was rejected after a sparse-sample slowdown.

## Complete breadth comparison

Three rotated repetitions per cell, 983,040 observations, 196,608 FE levels,
16 native workers, Stata MP at eight processors and BLAS at one, on the local
M2 Ultra. Inputs are audited columnwise. The comparator is stock pinned
fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Imports and audit exports are outside command timing. Requested outputs and
inference are matched. No build or test overlapped timed runs.

Command medians in milliseconds:

| Request | K=10 fereg / fixest | K=50 fereg / fixest |
|---|---:|---:|
| IID | 126 / 141 | 387 / 457 |
| Robust | 144 / 172 | 509 / 884 |
| Cluster on FE | 143 / 179 | 497 / 931 |
| Crossing cluster | 151 / 187 | 509 / 812 |
| Two crossing clusters | 183 / 253 | 574 / 1337 |
| Two clusters, FE nesting | 171 / 236 | 584 / 1273 |
| IID, saved FE | 166 / 217 | 437 / 521 |
| FE cluster, saved FE | 189 / 249 | 554 / 1021 |
| Two clusters, saved FE | 234 / 332 | 643 / 1386 |
| FE, residuals, xb and xbd | 209 / 225 | 491 / 541 |

The candidate wins all 60 paired comparisons. Eighteen of twenty cells have
median paired candidate/fixest ratios at most 0.90. Full outputs remain above
that threshold: 0.946667 at K=10 and 0.907579 at K=50. Ratios are computed within
repetition, so they need not equal ratios of the displayed medians.

Against the original `normonly` fereg baseline, the twenty breadth regression
cells have maximum median paired ratio 0.992288 and geometric mean 0.697002.
These are cumulative gains, including earlier committed improvements. They
must not be attributed solely to this increment. The current eleven size,
thread-count and group-layout controls and both final five-pair acceptance
blocks remain pending. This is development evidence, not release qualification;
the comfortable-win goal remains active.

## Incremental experiments

All observations, dispersions, sampled RSS, paired ratios and identities are in
[timings.json](timings.json), [receipts.json](receipts.json) and
[identities.json](identities.json).

- `narrow-certificate`: the native-only narrow certificate arm records IID at
  126 versus 130 ms, robust at 142 versus 145 ms and full outputs at 214 versus
  217 ms for its immediate native predecessor. The parallel reduction retains
  each moment, maximum and acceptance comparison; only the diagnostic two-norm
  merges fixed group partials, as on the previously accepted wide route.
- `recovery-receipt`: the first same-borrow receipt records narrow saved IID at
  162 versus 169 ms. Wider and full-output differences are mixed. It replaces
  repeated exact membership validation with an immutable source-bound receipt.
- `recovery-stream`: extending that receipt from prepared validation through
  recovery and deferring unused direction/increment buffers gives narrow full
  outputs at 204 versus 206 ms and wide full outputs at 480 versus 496 ms in
  that experiment. Narrow saved-only medians are 165 versus 162 ms and wide
  saved-only medians 430 versus 427 ms. Those mixed results are preserved; no
  universal incremental speedup is claimed. This experiment still included the
  subsequently rejected no-fill adapter.
- `recoverycore` is the final measured source: the retained core with the
  original missing-initialized Stata adapter. Its complete breadth results are
  the table above, not the experimental no-fill results.

The last direct-recovery change removes one observation-length direction and
one level-coefficient increment owner on the direct route, about 9.4 MB of
numeric payload for this design. The existing conservative recovery admission
is retained. There is no new observation-sized owner or persistent cache.

## Rejected output initialization

The experiment used no-fill private Stata tempvars and an explicit request
requiring native canonical-missing stores for excluded rows. Its Rust store
failure test, C main-thread guard, five Stata fixtures and old-plugin rejection
check passed. The failure was performance, not scientific or transactional.

On complete full-output data, `nativefill` records 210/489 ms at K=10/50 versus
214/507 ms for the same binary with initialized outputs. On half-missing data,
three pairs give a wide median paired slowdown of 1.76%; a five-pair follow-up
reduces that to 0.65%. On a further narrow control retaining outcomes only in
every twentieth complete FE group, 49,150 of 983,040 rows remain. All three
native-fill runs are slower: baseline 148/150/147 ms versus 157/163/159 ms.
The median paired ratio is 1.081633. This violates the no-regression objective.

The adapter, request field, FFI operation and C wrapper were removed. Their
source patch is preserved in [native-output-fill-rejected.patch](native-output-fill-rejected.patch).
The [half-missing controls](partial-control/) and
[sparse controls](sparse-control/) retain every run and audited dataset identity.
No later result is substituted for the rejected measurements.

## Science, source identity and remaining work

The archive contains 310 primary/profiling application calls, 22 half-missing
calls and six sparse calls. All application results, process codes and terminal
sentinels pass. There are 74 exact native scientific comparisons, including
samples, coefficients, VCE, statuses, DoF, fit and acceptance-certificate arrays;
62 saved-output comparisons are byte-identical. The original-baseline and
fixest coefficient/VCE/output comparisons use the controller's registered
numerical tolerances and exact samples. Native-versus-native comparisons also
check statuses and DoF exactly, as recorded in science.json.

The final core passes 22 focused Rust tests (20 recovery-module tests, one
certificate reduction test and one receipt/corruption test) and five Stata
fixtures. The fixtures cover independent oracles, saved recovery, prediction
on edited/sorted data, missing/restricted samples, all supported weight modes,
and restoration of prior results and variables on transaction failures.
Receipt tests reject a different owner and preserve weighted results and
malformed-target errors. Independent reconstruction and FE certificates remain.

The final `recoverycore` plugin is
`b2cbf8bd9b1ae9fdd130ee1300e3c9ee853e13366985df3f78c93c29d111fe6a`,
built from `6aabd2d2` with five source-hashed core overlays. The isolated build
passes the real-plugin ABI and panic-containment harness. Each experimental
snapshot retains its own identity; these are not exact-release-SHA receipts.
The harness's embedded build-date string remains 2026-09-19; executions and
this collection occurred on 2026-09-20.

Collection exposed a label-prefix collision between a series and its
confirmation. The collector now matches the complete label/cell boundary and
rejects duplicate run identities. The erroneous draft collection is retained
locally; the archived control collection contains exactly 22 distinct runs.
Raw measured receipts were unchanged.

The profile still identifies repeated categorical DoF validation as a concrete
next target. The source-bound receipt can be tested there before considering
further overlap with fitting. The final acceptance blocks and remaining
regression controls must use the eventual frozen candidate. No installed
package, remote state or cross-platform qualification changed.
