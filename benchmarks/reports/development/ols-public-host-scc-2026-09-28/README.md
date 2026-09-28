# OLS public host-worker scope — rejected SCC development candidate

2026-09-28; [ADR0242](../../../../docs/adr/0242-reject-ols-host-scope.md).

## Decision and retained source

Reject the general OLS host-processor scope. Wide and low-memory OLS show useful
repeated gains, but a fresh absorbed-slope design loses 7.0% initially and 15.1%
in a separate six-pair confirmation. Individual losses approach 50%. No further
threshold tuning or categorical/slope guard is adopted without a mechanism that
explains this difference. The accepted PPML public scope remains in production.

All changed production/test files were restored exactly to **4ddf7aaa**. The
[restoration receipt](restoration.json) verifies all 215 retained package inputs.
This is a rejected experiment report, not a claimed OLS improvement. The restored
source already has the accepted PPML report's validation; no restored-package
fixture rerun or new native compilation is claimed.

| Binding | Identity |
|---|---|
| Retained baseline package, 215 inputs | `14c753ede241d94812a25b52f06325cb81d5a08d5465efe0a343ec8b9c4bfad9` |
| Rejected candidate, 217 inputs | `72448897d6e25ccc8616e51c82efa0652386a4163cb44322f4cdafe0376172f8` |
| Same Linux plugin in both arms | `d007491ab5dfb5aa659f6c5639c8ac90c7e2e6c2bcd2e2603216cb91cb812116` |
| Reused Mac universal plugin | `a7ad04933211678788632916184fdda2ff9139321ff70e935c6a9d9d66767db0` |

[Baseline](baseline-source.json) and [candidate](candidate-source.json) enumerate
all inputs. [The rejected patch](rejected-candidate.patch) preserves the three
tracked-file changes; [the added fixture](harness/ols_public_scope.do) belongs at
`tests/stata/development/ols_public_scope.do` when reconstructing the candidate.
The remote immutable candidate also retains all four overlays. Both measured
native binaries are unchanged. Candidate inputs were checked unchanged through
all three timing blocks before restoration.

## Candidate and boundary checks

The candidate saves Stata's processor count and quiet state before the public
OLS syntax adapter, sets host processors to one, and restores the saved count on
success, error and Break. Administrative version/doctor/selftest actions and the
optional final coefficient table remain outside the scope. Native worker requests
are independent. One substantive plugin call, main-thread SPI, e()/sample/output
rollback and all numerical contracts remain unchanged.

[Mac validation](mac-validation01.json) passes five static parser tests, five
licensed fixtures and both original/public-scope genuine PTY Break modes. The
fixtures cover explicit/default native requests, host1/host4, fast/low memory,
weighted clustered saved effects/residuals, quiet/noisy callers, populated/null
e(), public syntax/sample failures and seven private transaction injections.
Existing independent weighted three-FE and multipartition slope oracles and a
PPML public-scope control pass. The public Break fixture substitutes only a test
syntax adapter to expose the existing private injection; real public wrapper,
core and inner transaction execute. No hidden injection becomes public syntax.

[SCC7763733](linux-validation01.json) passes the same five fixtures, OLS smoke,
five parser tests and both Break modes. Six licensed fixtures, 32s, 8.211G maxvmem,
failed=0, exit_status=0, terminal sentinel verified. Sixteen slots and inherited
16-CPU affinity cover the existing oracle's explicit 16 request and bound default
availability. Initial job 7763730 was still queued when cancelled to correct its
four-slot request; it executed no application. Both submission records remain.
Accounting publication lag caused read-only collection retries, never refits.

## Measurement design

The frozen rotation harness compares public `fereg`/stock `fixest::feols` with
IID inference, and unchanged `feppml`/stock `fixest::fepois` robust controls.
These calls omit optional observation outputs; saved outputs and clustering are
checked in licensed fixtures rather than timed. Pinned unmodified fixest 0.14.2
uses its registered strict FE tolerance 1e-11 for slopes and unchanged defaults
otherwise. The request also contains the existing GLM tolerance 1e-10; that has no
Poisson role in these OLS slope fits. No reference diagnostic build is timed.

Setting/restoration costs are inside public clocks. Data generation/import/audit
and warm-up are outside; total process clocks and sampled RSS are retained too.
Each job exclusively reserves a 28-core E5-2680v4 node. No compilation, profiling,
archiving or competing fit overlaps headline timing. Alternate B/C/stock and
stock/C/B orders, exact imports and scientific gates are unchanged.

| Block | Job | Inputs | Applications | Wall seconds | maxvmem |
|---|---|---|---:|---:|---:|
| Initial | 7763739 | seed2026092817, 10cases ×2rotations ×3arms | 60 | 176 | 1.998G |
| Fresh | 7763749 | seed2026092827, 10cases ×3rotations ×3arms | 90 | 265 | 1.973G |
| Confirmation | 7763758 | both existing slope seeds, wide OLS and unchanged PPML; 4cases ×6rotations ×3arms | 72 | 186 | 1.272G |

All three jobs have failed=0, exit_status=0 and terminal application sentinels.
Every one of **222 applications and 74 native scientific comparisons passes**.
**69/74 stock comparisons are eligible**; all five three-FE OLS stock comparisons
remain unranked for covariance/SE policy differences below. There are no missing
applications or application failures. All74 native b/V export pairs are byte-
identical and recorded iteration counts agree. Separate licensed independent
oracles establish science; `oracle: false` rotation cells are not themselves
independent-oracle evidence.

The fresh seed is consumed validation, not future tuning. Confirmation repeated
fixed validation designs with unchanged source after a material contradiction;
it did not select a new favorable seed. All registrations precede their runs.

## Distributions and remaining losses

Ratios are paired command-time ratios, lower is better. Medians of ratios need
not equal ratios of time medians. These bounded samples are not statistical or
universal speed guarantees. Full source-bound distributions, RSS, application
receipts and comparisons are in [initial](initial03/README.md),
[fresh](fresh04/README.md) and [confirmation](confirmation06/README.md) summaries.

| Configuration | Initial C/B median | Fresh C/B min / median / max | Fresh rejected C/stock | Fresh retained B/stock |
|---|---:|---:|---:|---:|
| ols-one-t1 | 0.931 | 0.703 / 0.893 / 1.169 | 1.971 | 2.286 |
| ols-one-t4 | 0.826 | 0.837 / 0.949 / 0.974 | 1.762 | 1.857 |
| ols-one-wide-t4 | 0.726 | 0.716 / 0.744 / 0.829 | 1.872 | 2.379 |
| ols-one-low-t4 | 0.815 | 0.765 / 0.800 / 0.807 | 2.844 | 3.719 |
| ols-two-t1 | 1.050 | 0.969 / 0.993 / 1.043 | 2.682 | 2.691 |
| ols-two-t8 | 0.883 | 0.865 / 0.911 / 0.923 | 1.362 | 1.496 |
| ols-three-t8 | 0.879 | 0.903 / 0.915 / 0.952 | NR | NR |
| ols-slopes-t4 | 0.616 | 0.986 / 1.070 / 1.496 | 1.423 | 1.274 |
| control-ppml-one | 1.032 | 0.925 / 1.009 / 1.024 | 2.726 | 2.663 |
| control-ppml-three | 0.938 | 0.903 / 0.947 / 0.971 | 0.457 | 0.483 |

One-FE cases use N30,720/K6 (wideK24), at T1/T4; low requests native low memory.
Two-FE cases use N61,440/K10 at T1/T8; three-FE sparse uses N61,440/K10/T8.
Slopes use two intercept-plus-slope partitions at N30,720/K6/T4. PPML controls
use the corresponding one-FE T4 and three-FE T8 designs. Full designs are frozen
in each registration. These are not the earlier OLS closeout's 93 cases, larger
systematic campaign or saved-output comparisons.

Fresh categorical one-FE wide/low and two-/three-FE medians improve 25.6%,20.0%,
8.9%,8.5%. One-FE T4 improves 5.1%. T1 is variable; initial two-FE T1 loses 5%.
Fresh slopes lose 7%, with one 49.6% loss, reversing their initial 38.4% gain.
This triggered one bounded confirmation, not candidate tuning:

| Confirmation case | C/B min / median / max | C/stock median |
|---|---:|---:|
| slopes-known-confirm | 0.633 / 0.899 / 1.198 | 1.099 |
| slopes-fresh-confirm | 0.604 / 1.151 / 1.500 | 1.425 |
| ols-wide-confirm | 0.628 / 0.681 / 0.770 | 1.891 |
| ppml-unchanged-confirm | 0.980 / 1.032 / 1.103 | 2.087 |

The fresh-slope loss repeats at 15.1%, with losses in four of six pairs and a
50% worst pair. Known slopes are mixed (three wins/three losses, 10.1% median
gain). Wide OLS wins all six pairs, median 31.9%. Unchanged PPML's median 3.2%
loss illustrates remaining variation but does not explain away the repeated
larger slope loss. Reject the general scope; do not pool blocks to hide it or
ship a categorical-only string/seed guard.

The retained package still misses the 1.20 ceiling on many small OLS cases.
Fresh retained paired stock ratios are 1.857 narrow T4,2.379 wide,3.719 low,
2.691 two-FE T1,1.496 two-FE T8 and1.274 slopes. Three-FE is unranked. These
ratios belong to the measured retained package, seed and outputs. The rejected
candidate's improvements cannot be transferred to production. This experiment
does not directly alternate core settings or change native thread selection.

## Why the five three-FE stock cells are NR

Candidate and baseline coefficients/covariance are exactly equal. Against stock,
samples, coefficient names/values, convergence and RSS pass, while covariance and
SE checks fail. The declared residual DoF differ: native 48,519 initial and 48,528
fresh versus stock 48,498 in both designs (`N - df_k`, with N61,440 and df_k12,942).
Native initial absorbed levels 12934 minus 23 redundancies give df_a12911; stock's
reported total parameter count leaves a different correction.

The readonly [telemetry helper](harness/summarize_ols.py) tests the factor implied
by those reported DoF, without fitting or choosing a factor from the covariance
entries. Dividing stock covariance by native_df_r/stock_df_r reduces the maximum
finite covariance gap to 7.324e-15 initial and 7.965e-15 fresh. RSS-adjusted bread
also agrees closely. Results and input hashes are in each `ols-telemetry.json`.
This localizes these cells to inference correction policy, rather than a new
native numerical regression. It is not an independent full-rank calculation.

The diagnostic never changes exported covariance, scientific eligibility,
tolerances or timings. These five comparisons stay NR. A future separately
registered comparison must establish matched DoF policies before timing. This
does not adjudicate older weak-graph PPML SE failures or every historical NR.

## Phase and memory evidence

The added telemetry files separately retain public clocks, native preflight,
SPI loading, native computation and native total before SPI stores. Native
computation includes validation and science; public-minus-native is not solely
parsing cost. Optional observation output costs are absent from these clocks.

Initial slope native-computation medians fall 277.3→152.9ms under the candidate;
fresh medians instead rise 239.7→264.7ms (one 371.8ms). Loading is only about 12–15ms
and generally improves. Thus the regression is within the measured native
interval, not just setting restoration or a coefficient display. Exact phase
attribution to scheduling, cache state or host-worker behavior is unresolved.
No algorithmic work reduction is claimed: binaries, b/V and recorded iterations
are identical within pairs.

OLS selected phase-two memory bytes and scratch agree within every pair.
Tracked peaks vary only for existing three-FE overlap; this is not a new buffer
or changed memory-admission policy. PPML control telemetry and sampled process
RSS remain separate. No physical-cap qualification, static allocation inventory
repair, constrained-fit fix or general peak-memory saving is claimed.

## Next boundary and receipts

Preserve the accepted PPML scope. The direct PPML T1→T2 screen in the preceding
report measures extra finite/strict/inference time despite faster projection;
those installed-team submissions and reductions are the next bounded native
scheduling target. For OLS, any successor should explain slope native-compute
variation and count caller/team handoffs before adopting another host scope.
OLS preparation, fitting and reconstruction enter the team separately; PPML's
existing scoped adapter already keeps post-load work under one worker scope.
This source distinction is a hypothesis to measure, not attribution proved by
this experiment. Do not assume a second generic scope is sufficient or reopen
closed serial weighted-slope micro-optimizations.

Jobs and raw logs remain at the receipt-bound SCC roots; raw licensed logs are
not committed. Every collected synthetic byte matches its remote hash, summaries
were independently recomputed locally, and original remote summary bytes remain
intact. All jobs are terminal; no unrelated job was changed. No push, release,
full qualification or resumed stopped campaign occurred.
