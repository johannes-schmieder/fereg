# P4 retained numeric predictor tiles: adopted local checkpoint

Adopt [ADR 0066](../../../../docs/adr/0066-retained-numeric-predictor-tiles.md)
against `e65d6f9983f4bfb7f317f00ea6bf469269e19443`. Fresh and strict retained
PPML solves bind eligible numeric columns once and accumulate finite predictors
in 128-row tiles using the existing worker team. Actual selected generated
identities remain in original coefficient-slot order, preserving each row's FMA
sequence. Ordinary frozen PPML already uses an O(N) combined predictor.

## Source and coverage

[Identities](built-identities.json) bind 178 inputs and isolated universal plugins:

- Baseline: `365bee73f4b08ceacd1f6feef35b8fbb8ddeb97462f573c6a5e41bc731b24e0c`.
- Candidate: `2689faf0418896ac66f0cb7fcaa9552b339a1928ab38409b43710dab6f63faeb`.

The [patch](implementation.patch) changes two core files; the
[source archive](source-archive.json) and executed/archived script hashes bind
the experiment. Both architectures forcibly rebuild core/plugin in isolated
sources. Universal static and actual arm64 ABI checks pass. Main and installed
dist were not built; the installed plugin and unrelated work are preserved.
These are local M2 Ultra/Stata-MP development findings, not SCC, Windows,
external-comparator or qualification claims.

| Consumer | Behavior and cost |
|---|---|
| Fresh/strict numeric design, N>=4096, generated width 8–64 | Existing stack-only column bindings plus a fixed 64-entry selected bank; row tiles update the already-owned finite predictor |
| Small/narrow/wider/generated recipes | Existing per-cell evaluation, including factor controls |
| Absorbed slope with numeric finite controls | Same numeric binding where eligible; absorbed solver remains unchanged |
| Low-memory retained sessions | Same numeric predictor helper; retained scratch design and I/O algorithms unchanged |
| Ordinary frozen PPML | Existing combined response-minus-residual path; no K-column finite predictor is rebuilt |
| One-shot OLS | Unchanged; does not enter retained-session reconstruction |

The selected bank is 1024 stack bytes on these targets; the existing bounded
bindings add fixed metadata. There is no new N-vector, N-by-K matrix or heap
workspace. Numeric source visits remain N times selected width. The layout
removes per-cell recipe dispatch and repeated full predictor-vector sweeps in
favor of cache-local tiles; this is source-derived work attribution, not measured
hardware traffic. All workers operate on native storage.

## Complete-command measurements

The original [protocol](protocol.md), [follow-up](followup-protocol.md),
[cost review](cost-review-protocol.md) and [variability review](low-variability-protocol.md)
precede their respective calls. Original full/warm inputs and hashes are retained.
No build, test, numerical oracle, data generation or compression overlaps timing.

There are 25 untraced A/B pairs, six A/A pairs and three separate diagnostic
pairs: 68 complete commands. Primary comparisons use three original seeds and
rotated order; the low-memory variability review repeats its identical original
input. Every comparison passes exact scientific exports and discrete solver
decisions. Diagnostics additionally match the original noninstrumented arms.

| Primary | N / finite K / workers | Command medians, seconds | Median paired reduction |
|---|---|---:|---:|
| Unit one FE, robust | 983040 / 50 / 16 | 7.785 → 7.467 | 5.72% |
| FW exposure, sparse missing, robust | 245760 / 50 / 4 | 2.585 → 2.436 | 5.76% |
| PW offset, sparse missing, two-way cluster | 245760 / 50 / 16 | 2.376 → 2.202 | 6.33% |

Independent medians need not have the same ratio as median paired changes.
The first complete pair costs 60 ms/0.80%, and one FW repeat costs 14 ms/0.55%;
both remain included. Strict/final phase medians fall 2.633→2.381,
0.708→0.644 and 0.666→0.582 seconds. Some unchanged finite work also varies;
the whole command difference is not assigned exclusively to predictor tiles.
Full phases, iteration/line-search/retry/fallback fields and peaks are retained
in [summary.csv](summary.csv), [summary](adoption-summary.json) and
[captures](captures.json).

Separate diagnostic artifacts time only initialization, finite accumulation,
and the unchanged reconstruction/certificate loop. The three full-data calls
save about 82 ms (FW) and 90 ms (PW) in accumulation: approximately 90.8→8.4 ms
and 93.3→3.5 ms. Reconstruction/certification remains about 3.3–3.4 ms per fit.
Warm-up calls are counted separately and excluded from these full-data sums.
The diagnostic FW complete command barely changes; the PW diagnostic saves
180 ms. These diagnostics explain the changed work without replacing the
untraced command evidence. Hooks and clocks are absent from adopted source.

## Protected costs and memory

Small/narrow, no FE, weak two FE, explicit MAP, saved/separated outputs,
generated factors, slope and width-65 fallback checks preserve their scientific
results. Apart from the low-FW review below, the largest untraced protected
increase is the slope control's 1 ms. Generated-factor validation uses an
independent licensed oracle rather than a speed claim.

Low-FW original costs are +142/+171/+3 ms, the first two exceeding both 5% and
10 ms. Its changed accumulation saves only about 2 ms across three calls, so
that kernel cannot explain the earlier 140–170 ms added cost. The fixed
interleaved review retains all outcomes:

| Review | Pairs | Command medians, seconds | Median paired change | Individual time differences |
|---|---:|---:|---:|---|
| Same accepted binary A/A | 6 | 0.949 → 0.958 | +1.59% | -132, +185, -101, +62, +23, +7 ms |
| Accepted/candidate A/B | 6 | 0.9355 → 0.9285 | -2.62% | +151, -17, +155, -61, -31, -60 ms |
| All original and new A/B | 9 | See complete summary | +0.29% | Median +3 ms; no observations discarded |

The predeclared new-six and all-nine median cost gates pass. Identical-binary
variability demonstrates that large swings can occur without a code change;
it does not identify the cause of each candidate flag. Adopt with those flags
disclosed, without a low-FW gain or zero-cost claim and without a mode/seed guard.

All untraced native peaks are exactly equal within pairs and below the selected
models. Primary peaks are 2.522 GB / 633.45 MB / 645.21 MB, versus modeled
2.963 GB / 745.43 MB / 935.51 MB. Low FW is 47.08 MB versus 53.63 MB.
Primary sampled RSS medians are 1.961→1.962 / 0.526→0.543 / 0.542→0.544 GiB;
RSS is sampled process memory and is distinct from live native allocation.
No general memory reduction or allocation-free iteration claim is made.

## Validation and remaining work

[Validation](validation-summary.json) records 20 distinct focused core tests,
95 serial plugin tests and four independent positive licensed fits per arm.
The tests cover exact scalar FMA ordering at 1/4/16 workers, partial row tiles,
reordered/repeated identities, invalid/nonfinite/overflow input, exact-rational
reconstruction and strict rollback/team/frame checks. The public N65536/K8
oracle covers unit robust, FW robust and PW cluster against explicit Poisson,
Hessian and sandwich calculations. A separate N4099 generated-factor case
checks full finite b/V and means against explicit Poisson. All return codes
and terminal sentinels pass; original fixture and log hashes are preserved.

The first compile used a nonexistent test import and failed before executing
tests. The corrected source passes. An initial reconstruction selector matched
zero tests and is not counted; the corrected selector supplies five checks.
The initial archive helper tried to parse PPML's missing OLS-alias cells; the
collector was corrected to named PPML runtime fields and given the original
baseline environment, preserving raw CSVs and both collection failures.

This adopts the predictor portion of P4. Broader construction/weight-response/
certificate passes, weighted Schur comparison and input-dependent state refresh
remain open. Eight active P0–P7 checkboxes remain. The user excludes R1–R5 from
this run; after P0–P7 come two independent change reviews, upstream CMG assessment
and the bounded SCC campaign. No agents, cluster work, upstream change or push
occurred in this checkpoint.
