# Ordered numeric factorization

The retained shortcut encodes already ordered numeric values with two linear
passes, avoiding the dense-range lookup or hash/sort/remap construction.
Canonical levels, signed-zero normalization and codes are unchanged. A first
ordering violation returns to the existing factorization without allocation.
Inputs below 16,384 rows keep the previous path. This shared core applies to
numeric factorization in OLS and PPML; command timing here covers one-FE OLS
on the local Mac only.

## Command measurements

The same M2 Ultra, 983,040 observations, 196,608 FE levels, sixteen native
workers, Stata MP eight processors, external BLAS one and pinned stock fixest
0.14.2 as the preceding output-store checkpoint. No builds, tests, kernel
probes or archive hashing overlapped command timing. Imports and audit exports
are outside command timing. Rotating arm order and all individual runs are
retained. Command medians in milliseconds:

| Request | K | Pairs | Prior checkpoint | Ordered factorization | fixest |
|---|---:|---:|---:|---:|---:|
| IID | 10 | 5 | 128 | 125 | 139 |
| Full outputs | 10 | 5 | 190 | 191 | 220 |
| IID | 50 | 3 | 371 | 367 | 460 |
| Robust | 50 | 3 | 488 | 493 | 908 |
| Two clusters, saved FE | 50 | 3 | 590 | 598 | 1,412 |
| Full outputs | 50 | 3 | 453 | 455 | 521 |

Narrow IID wins four of five parent comparisons, with median paired ratio
0.976563. Its paired ratio against fixest is 0.899281; that is a screen, not
final acceptance. There is no incremental narrow full-output gain. Wide IID
wins all three parent comparisons. The initial wide robust and clustered
slowdowns remain in the archive rather than being discarded.

A separate three-pair phase run on unchanged artifacts gives robust medians
492/492 ms and two-cluster saved-FE medians 602/599 ms before/after. Preparation
medians fall 23.70→22.66 ms and 50.98→48.13 ms, respectively. The earlier
whole-command slowdown does not recur; the phases support a preparation gain,
not a reliable whole-command gain for every wide covariance/output case.
[Detailed phase records](phase-detail.json) preserve all runs.

All eleven size/thread/group-layout controls pass against original `normonly`:
maximum median paired ratio 1.025825 and geometric mean 0.992193. There are
thirty-three exact nonself native comparisons in that control series.
The full twenty-cell breadth regression and both final acceptance blocks
remain pending for the next final candidate. This intermediate checkpoint
makes no all-thirty-one-cell or final-goal claim.

## Kernel benefit and fallback cost

An isolated same-process diagnostic compiles the parent and candidate
factorization source with the same rustc 1.85.1 optimization flags. Six data
layouts, one and sixteen workers, eleven alternating pairs per layout, and
exact canonical-level-bit/code checks distinguish the local algorithm change
from whole-command noise. Construction is timed; output destruction is outside
the timer. These are kernel diagnostics, not frozen command acceptance data.
At sixteen workers, medians in milliseconds are:

| Numeric layout | Prior | Candidate |
|---|---:|---:|
| Ordered dense groups | 2.195 | 1.175 |
| Ordered fractional groups | 14.417 | 1.199 |
| Ordered unique levels | 4.841 | 1.438 |
| Constant | 1.537 | 1.088 |
| Early ordering violation | 3.061 | 3.234 |
| Last-row ordering violation | 2.220 | 2.737 |

The late violation pays an extra unsuccessful scan, about 0.5 ms at this N;
its median paired kernel ratio is 1.233. This is an explicit tradeoff, not a
universal speedup. The command controls above include ordered and shuffled
inputs but do not constitute a command-level late-violation measurement.
All raw [kernel times](factor-probe-times.tsv), [paired summaries](factor-probe-summary.json)
and [source/compiler identity](factor-probe-identity.json) are retained.
To rebuild [the diagnostic driver](factor-probe.rs), place the identified
parent/candidate `factorize.rs` snapshots beside it as `baseline_factorize.rs`
and `candidate_factorize.rs`; link the recorded build's fereg_core/Rayon rlibs
with both target and host dependency search directories. The first standalone
compile omitted host dependencies and produced no measurements; the corrected
compile and run pass.

## Validation and provenance

Eight focused factorization tests pass. An independently sorted/deduplicated
level oracle checks empty, singleton, threshold-adjacent and large inputs,
negative/signed zero, fractional and extreme finite values, late inversions,
nonfinite values and existing parallel dense fallback behavior. Three Stata
fixtures pass with explicit sentinels: final one-FE recovery with independent
dummy comparisons, candidate-input oracle and e-class transaction rollback.
The initial fixture attempt preceded build completion and returned plugin-not-
found; its log remains local and hashed. The subsequent `-built` fixtures use
the completed artifact and pass. Actual-plugin ABI/panic checks pass.

There are 144 successful application calls: thirty narrow, thirty-six wide,
twelve phase and sixty-six controls. Sixty-one nonself native scientific
comparisons are exact, and fourteen saved-output bundles are byte-identical.
All twenty-eight incremental/phase native allocation peaks match the parent.
The kernel diagnostic adds 264 timed factorizations and twelve exact layout/
thread checks, counted separately from application calls.

Candidate plugin:
`2538f103891fea59cb3af258149390794e013e90b4e47b831e2b2b6c72f1d58b`,
built from `81505d0d` plus `crates/fereg-core/src/factorize.rs`.
[Validation](validation.json) binds the source, test log and fixture receipts.
The build helper embeds September 19; execution and collection occurred
September 20. No installed package, remote state or qualification changed.

The comfortable-win goal remains active. The next prototype parallelizes the
legacy RHS row-map and categorical-code validation scans on existing large
worker teams, preserving the first error and all checks. It is separate from
the previously rejected finite-check/RHS-match fusion. Its evidence is not
part of this checkpoint.
