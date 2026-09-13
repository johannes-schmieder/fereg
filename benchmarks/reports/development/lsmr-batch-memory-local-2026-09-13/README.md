# P0/R1/P7 categorical LSMR memory and batch investigation

**Disposition: implemented and tested prototype; not adopted.** The candidate
reduces wide OLS and PPML allocation, but ordinary-width PPML timing review is
unresolved. Accepted main remains `ac47b1907c7bc7bc8ee78de85621b0cc96546a4c`.
Do not count these savings among adopted optimizations or use the diagnostic
model-only artifact for wide-input admission.

The investigation began September 12 and continued September 13. Its frozen
plugins retain their September 12 build labels. All runs are local synthetic
development comparisons; no SCC, external-comparator, qualification, installed
plugin replacement or push occurred. Historical receipts remain unchanged.

## Implemented candidate

The [patch](implementation.patch), [new modules](candidate-inputs/crates/fereg-core/src/)
and [152 native input identities](variants/wide-guard/identities.json) bind the
latest full candidate, plugin SHA-256
`c9db0dceea82dc7b7b87a51979d0fd7ca1f83f1e71f04f034be32312527990f6`.
The accepted baseline artifact is
`6c364b062dc505f478ad9079a8106db4e89c0aed4079735ad89d939f7d1a0497`.

* Correct the categorical solver estimate's observation/coefficient recurrence
  axes, local history, primary dense cross tabs, sorted frames, scatter scratch,
  batch-output overlap and pooled Schwarz scratch. Share the estimate with PPML
  checkpoints. See the [memory audit](memory-audit.md) and its explicit limits.
* Cap an upstream batch to the installed worker team only above 64 RHS and above
  four times that team's width. Smaller batches retain upstream transport. This
  is a conservative performance guard, not a rank policy or an optimal crossover.
* Reuse one immutable weighted solver across windows; reserve the final output
  banks fallibly, concatenate in original RHS order, and drop all partial output
  on any later error before the existing whole-batch retry. Preserve positivity
  fallback classification and fatal allocation errors.
* Derive the window from the actual installed Rayon pool. PPML checkpoints use
  a default `LsmrConfig`, whose resolved machine thread count can differ from the
  admitted team. The earlier candidate therefore did not enforce the model's
  intended PPML batch limit. Existing optional traces now identify rows, RHS,
  window and installed threads.

No recurrence arithmetic, reference scale, tolerance, certificate, rank policy,
weight/sample semantics or publication rule is changed. Main-thread SPI and
the safe-core boundary remain intact. The ordinary joint PPML solver is unchanged.

## Complete-command results and unresolved regression

The [experiment inventory](experiments.json) contains **72 pairs / 144 measured
commands** across successive prototypes and diagnostic decompositions. These are
not 72 independent tests of the final candidate. The latest full candidate has
18 pairs: three wide OLS targets, six OLS controls, and nine PPML width/recheck
pairs. Each comparison uses a fresh Stata process after a small warm-up, rotated
arm order and the specified synthetic seed. Affinity is uncontrolled.

| Latest full candidate | Command medians, seconds | Median paired change | Native peak medians |
|---|---:|---:|---:|
| Weak AW robust OLS, N122880/K65/4 threads | 0.995 → 1.006 | +1.5% | 403.31 → 351.32 MB |
| Weak PW offset/cluster PPML, N30720/K65/4 threads | 1.332 → 1.344 | +0.5% | 182.18 → 169.43 MB |
| Same PPML, K17, original three pairs | 0.360 → 0.367 | +1.9% | 93.07 → 90.72 MB |
| K17 triggered rechecks, three pairs | 0.345 → 0.381 | +10.4% | 90.56 → 92.93 MB |

The [OLS receipt](variants/wide-guard/ols/comparisons.json) shows exactly
51,985,712 fewer peak bytes in each wide pair, a 12.9% reduction. Input/outer
medians remain about 48/28 ms; native time is 0.918→0.928 seconds. This is a
memory result, not a command-speed gain. The six protected routes retain science;
largest command increase is complete IID +7 ms/1.2%. Strong weighted Schur,
small/narrow weak, low FW clustering and explicit generic LSMR are included.

The [PPML receipt](variants/wide-guard/ppml/comparisons.json) shows a 7.0% wide
peak reduction. Preparation is 0.02750→0.02764 seconds, projection
0.29296→0.30064, strict/final work 0.65322→0.65463 and inference
0.05010→0.04936. These independently computed, overlapping phase medians must
not be summed. Full science and non-time solver/retry decisions remain exact.

Ordinary-width PPML remains the adoption concern. Across all six K17 pairs,
the median paired command increase is 4.0%; three individual increases are
21/36/38 ms (6.1/10.4/11.0%), exceeding the prospectively specified
`max(10 ms, 5%)` review envelope. Reversing order does not remove the largest
same-seed loss. The increase is in native work, including strict/final work,
while outer time stays about 14 ms. Preserve these results rather than declaring
them harmless noise.

Two bounded decompositions narrow, but do not settle, attribution:

* [Model only](variants/model-only/ppml/comparisons.json), with the accepted batch
  adapter restored: six pairs have +1.0% median change. The first three add
  11/19/9 ms; their reversed counterparts save 2/10/12 ms. One original 19 ms/5.6%
  review trigger remains. This diagnostic is restricted to unwindowed widths.
* [Adapter versus that identical model](variants/adapter-decomposition/ppml/comparisons.json):
  six pairs have +1.3% median change, with differences from −12 to +9 ms.
  This does not reproduce the large loss, but is not a universal no-regression
  proof or a substitute for comparison with accepted main.

No precise external cause or compiler-layout explanation has been established.
Do not adopt the full candidate until the ordinary-width review has a defensible
resolution. The two decompositions should guide the next investigation instead
of repeating increasingly favorable seeds.

## Rejected intermediate policies

Each [variant](variants/) retains its source inputs in a hash-verified ZIP,
its protocol, complete numeric
exports and comparison results.

| Variant | Evidence and disposition |
|---|---|
| Unbounded revised model | Correctly charges all possible RHS frames, but the wide example models 3.956 GB against 272.69 MB observed. Too loose as the final routing model; motivated explicit batching. |
| Always window to team | Saves wide memory, but narrow weak/generic OLS peaks increase about 2 MB. Reject universal early final-bank reservation. |
| Four-times-team guard using configured threads | Narrow OLS allocation improves; six PPML controls have +5.6% median command cost. PPML's default configuration is not the installed team. Not adopted. |
| Installed team, four-times-team guard | Six narrow PPML pairs stay within the envelope, but 18–19-RHS PPML rises 6.4% in median time and about 2.47 MB in peak allocation. Reject this ordinary-width crossover. |
| Wide-only guard | Wide memory gains survive; ordinary-width PPML review remains open as above. |

The independent [window replay](allocation/window-results.json) uses one fixed
N122880/67-RHS/four-worker operator. Across three rotated repeats, full/4/8/16
windows have median solver times 1.603/1.579/1.666/1.567 seconds. The four-wide
window's complete lifetime peak falls 272.69→222.13 MB. All 11,955,212 returned
coefficient and demeaned values are exactly equal in each of nine comparisons.
These are isolated library timings, not complete-command gains.

## Validation and provenance

[Validation](validation.json) records 67 LSMR-filter and 77 memory-filter Rust
tests, **134 distinct tests**, with no failures. Coverage includes one/three/four/
nine-worker windowed output equality, identity/diagonal/Schwarz preconditioners,
late-window failure, overflow/allocation classification, retained sessions,
independent certificates and constrained routing. The weighted 327680-byte
routing test fails on the old model and selects certified CG on the candidate;
the independent construction witness needs 425777 bytes. See
[routing proof](allocation/routing-proof.json).

Both final artifacts pass three licensed fixtures with explicit r(0) and terminal
sentinels: retained OLS LSMR reuse, weighted explicit-dummy OLS, and the expanded
65-control Poisson oracle. The latter checks all slopes, predictions and the
full covariance against explicit FE dummies and independently formed Hessian/
meat at one/four workers, with unit/FW/PW and robust/clustered inference. It reuses
the registered small-sample multiplier; it does not independently prove that
policy. Universal static and actual native arm64 ABI/self-tests pass.

The [first 65-control oracle expansion](oracle-expansion/) fails the same bread
certificate on both artifacts. Its closely spaced smooth columns have an
independently measured within-design condition number 2.72e15. A separate
well-conditioned Fourier design (condition 1.09) supplies the passing batching
oracle without changing a tolerance or assertion. The first failure is retained
and is not repaired by this work. The earlier low-FW and saved-AW weak certificate
failures from the ac47b19 report also remain open; they were not rerun here.

The [nine final allocation probes](allocation/final-results.json) all fit the
revised model. Three stop unconverged at their bounded 80-iteration limit and
are allocation evidence only. The first eight exploratory probe results bind
a binary but lack a contemporaneously frozen source copy; final source-bound
replays supply the source-bound diagnostic observations. The archived
replay's local dependency and output paths are made portable with explicitly
recorded transformations. No licensed logs, binaries or raw datasets are included.

## Remaining work

Resolve the ordinary-width PPML timing concern, then decide the candidate.
P0 still needs actual attempted/fallback memory reporting: the public OLS model
omits solver increments, so the latest wide case reports 268.55 MB despite
351.32 MB observed. Small weak OLS still reports about 1.045 MB against 55 MB.
Opaque Schwarz factor-allocation constants and nested retained-pool bounds
are not fully qualified. This investigation does not close P0, R1 or P7, nor
the remaining issue #27 packages. Keep the later agent, upstream CMG and new
SCC phases behind completion of that issue.
