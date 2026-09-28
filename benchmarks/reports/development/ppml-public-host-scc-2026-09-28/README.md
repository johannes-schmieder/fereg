# PPML public host-worker scope — SCC development evidence

Accepted bounded development candidate, 2026-09-28; [ADR 0241](../../../../docs/adr/0241-ppml-public-host-scope.md).

## Change and identity

`feppml` temporarily sets Stata's processor count to one before its syntax
adapter and restores the caller's count after the complete public call. Native
worker selection remains independent. The protected scope covers parsing,
private outputs, one native call and publication; the inner transaction remains
the rollback authority. Caller quiet/noisy behavior and error codes are retained.
No Rust/C numerical code, solver, certificate, tolerance, output, memory plan or
persistent process setting changes.

| Binding | Identity |
|---|---|
| Baseline parent | `05690632` |
| Baseline package (213 inputs) | `7a24596908c18db21b4aaca6fa30ac146afb653b5664352b0e751edd1432f5e0` |
| Candidate package (215 inputs) | `14c753ede241d94812a25b52f06325cb81d5a08d5465efe0a343ec8b9c4bfad9` |
| Identical Linux plugin in both arms | `d007491ab5dfb5aa659f6c5639c8ac90c7e2e6c2bcd2e2603216cb91cb812116` |
| Reused Mac universal plugin | `a7ad04933211678788632916184fdda2ff9139321ff70e935c6a9d9d66767db0` |

[Baseline](baseline-source.json) and [candidate](candidate-source.json) bind every
input. Candidate overlays are the public wrapper, Break controller/fixture and
new public-scope fixture. Both artifacts are reused without rebuilding. Current
source hashes were verified unchanged after all timings.

## Validation and failures

[Mac validation](mac-validation01.json): 24 Python static tests, five distinct
licensed fixtures and both genuine PTY Break modes pass. The fixtures cover
public syntax/domain errors; populated/null e(); quiet/noisy calls; native panic,
allocation, postplugin and precommit rollback; explicit/default native requests;
fast/low memory; saved effects/means; independent weighted Poisson/Hessian and
sandwich, FW/PW/cluster, multipartition slopes and OLS controls.

The public Break mode substitutes only a test syntax adapter to expose private
injections. It executes the real `feppml` wrapper, native core and transaction,
including interruptions inside native work and after public metadata/output
posting. It also checks real public prediction rollback. The original native
Break mode remains checked separately. No developer injection becomes public.

The new Mac fixture initially referenced a loop-local macro outside its scope in
its final assertion, returning r(198) after its rollback assertions passed.
[That failed summary](mac-initial-fixture-failure.json) remains intact. Correcting
only the fixture's expected host count and rerunning it passed. The final Mac
receipt explicitly combines the four initial passing fixtures and corrected
fixture; it does not reinterpret the failed run as success.

[SCC validation 7763686](linux-validation01.json) passes those five fixtures,
PPML smoke, 24 static tests and both genuine Break modes. Four slots, 23 seconds,
2.909G maxvmem, failed=0, exit_status=0; terminal application sentinel verified.
There is no new native build or claim that native unit tests were rerun.

## Headline design

Two isolated 28-core E5-2680v4 jobs run baseline/candidate/pinned-stock rotations,
with alternate order, identical per-pair data and audited imports. No overlapping
build, profile, archive or competing fit. Stata processor changes and restoration
are inside the public command clock. Data generation, import and audit are
outside; total process clocks and sampled RSS remain available separately.

PPML is public `feppml` versus stock `fixest::fepois` with robust inference; OLS
is public `fereg` versus stock `fixest::feols` with IID inference. No optional
observation outputs are requested in these clocks. Saved outputs and clustering
are validated separately, not timed here. Stock is pinned unmodified fixest
0.14.2; slopes use `glm.tol=1e-10`, `fixef.tol=1e-11`, other cells unchanged
reference defaults. These ratios cannot be transferred to previous saved-output,
other-reference, larger or weak-connectivity campaigns.

| Block | Job | Seed | Cases × repetitions × arms | Applications |
|---|---|---:|---:|---:|
| Initial | 7763691 | 2026092817 | 10 × 2 × 3 | 60 |
| Fresh confirmation | 7763701 | 2026092821 | 10 × 3 × 3 | 90 |
| Direct scaling | 7763701 | 2026092823 | 8 × 3 × 3 | 72 |

Fresh and scaling cases were registered before execution; those seeds are now
validation inputs, not future tuning seeds. The latter blocks run sequentially
in one allocation. Job 7763691 used 200 seconds/1.966G maxvmem, job 7763701
512 seconds/1.998G; both have failed=0, exit_status=0 and terminal sentinels.
All **222 applications, 74 native comparisons and 74 stock comparisons pass**.
There are no missing or unranked cells in these bounded blocks. This does not
close discrepancies in other campaigns. Independent oracles are the separate
licensed fixtures; the rotation's `oracle: false` cells are reference comparisons.

## Performance and controls

Ratios below are paired command-time ratios; lower is better. The median of
ratios need not equal the ratio of separate time medians. Three repetitions do
not establish a statistical guarantee. Full distributions and per-fit failures,
RSS, clocks, bindings and science results are in [initial](initial03/README.md),
[fresh](fresh04/README.md) and [scaling](scaling05/README.md) summaries.

| Case | Initial candidate/baseline median | Fresh candidate/baseline min / median / max | Fresh candidate/stock median | Fresh candidate ms |
|---|---:|---:|---:|---:|
| one-small-t1 | 0.805 | 0.702 / 0.985 / 1.003 | 2.583 | 266 |
| one-small-t2 | 0.820 | 0.908 / 0.941 / 0.971 | 2.032 | 288 |
| one-small-t4 | 0.811 | 0.755 / 0.762 / 0.797 | 2.049 | 252 |
| one-low-t4 | 0.853 | 0.819 / 0.824 / 0.846 | 2.450 | 276 |
| one-wide-t4 | 1.011 | 0.938 / 0.966 / 1.012 | 0.992 | 547 |
| control-profiled-small | 1.010 | 0.868 / 0.889 / 0.914 | 1.766 | 369 |
| control-three-fe-core-pair | 0.975 | 1.009 / 1.024 / 1.042 | 0.547 | 1927 |
| control-excluded-slope | 0.906 | 0.925 / 1.002 / 1.056 | 0.407 | 617 |
| control-excluded-ols | 1.011 | 0.963 / 0.995 / 1.038 | 1.533 | 187 |
| one-larger-t8 | 1.059 | 0.926 / 0.984 / 1.074 | 1.172 | 928 |

Small/low one-FE cases use N30,720/K6, wide uses K24, larger uses N122,880/K10.
The two-FE control is N30,720/K6; three-FE sparse N61,440/K10; slopes are two
intercept-plus-slope partitions at N30,720/K6; OLS is two FE at N61,440/K10.
Threads and all design fields are retained in each registered plan and summary.

The one-FE T4 and low gains repeat in both blocks (fresh 23.8% and 17.6%). The
small two-FE gain is 11.1% fresh but its initial block is flat. Retain this small,
coherent host scope without case-specific guards. Wider/larger and excluded
controls do not establish a general speedup. Explicit losses remain: fresh
three-FE is 2.4% slower in the median and slower in all three pairs; the initial
larger case loses 5.9%, with a 9.5% worst pair, although that median loss does
not repeat fresh. Initial T1's apparent gain is highly variable and does not
repeat. OLS code is unchanged; its fluctuations are control measurements.

The universal 1.20 ceiling and 0.90 aspiration remain unmet: fresh small one-FE
medians are 2.03–2.58× stock, low 2.45×, small two-FE 1.77×, OLS 1.53×.
Wide is near parity, larger 1.17× with a worst pair 1.207×. Three-FE and slopes
are substantially faster on these designs (.547× and .407×).

## Direct core scaling

Both native arms below use the candidate source and identical plugin. “Baseline”
in the scaling harness means the lower thread request, not older code. Stock
uses the higher requested setting. Every corresponding sample/scientific pair
passes; 15 of 24 coefficient/covariance export pairs are also byte-identical.
Bitwise identity across different teams is not a requirement.

| Native request | Higher/lower min / median / max |
|---|---:|
| one_fe-t1-to-t2 | 0.871 / 1.080 / 1.097 |
| one_fe-t2-to-t4 | 0.851 / 0.867 / 0.879 |
| one_fe-t1-to-t4 | 0.660 / 0.933 / 1.007 |
| two_fe_dense-t1-to-t2 | 0.800 / 0.878 / 0.949 |
| two_fe_dense-t2-to-t4 | 0.791 / 0.832 / 0.949 |
| two_fe_dense-t1-to-t4 | 0.675 / 0.799 / 0.850 |
| one-wide-t4-t1-to-t4 | 0.669 / 0.680 / 0.820 |
| control-three-fe-core-pair-t8-to-t16 | 0.930 / 0.931 / 0.947 |

Seven of eight medians improve, but one-FE T1→T2 loses 8.0% and is slower in
two of three repetitions. T1→T4 has one 0.7% loss. Requested three-FE T8→T16
uses **effective 8→8** under the existing ADR0233 ceiling; it is not evidence
that 16 actual workers scale. No new native cap is fitted to these observations.

## Work, memory and attribution

All 50 same-thread native pairs have byte-identical b/V exports. Recorded outer
and inner iteration counts, strict-profile rejections and PPML selected memory
bytes agree. Tracked allocation peaks agree except the existing three-FE overlap
variation (both directions, maximum absolute difference about 7.0 MiB). See each
block's `native-equality.json`; missing PPML fields on OLS are not equality
claims. The scope adds scalar Stata locals and no native numerical buffer or
memory-admission credit. Different thread counts retain their existing different
worker memory charges. Sampled whole-process RSS is preserved separately and is
not a proof of a fixed physical-memory saving. Static allocation inventory drift
and constrained-fit failures remain open.

[Fresh phase telemetry](fresh04/phases.json) locates remaining costs. In one-FE
T4, candidate medians are preparation 27.5ms, FE projection 13.7ms, finite solve
46.4ms, strict final solve 57.0ms, inference 6.4ms and native publication 16.6ms,
against 252ms public time. Small two-FE projection remains 146.8ms of a 369ms
public median. These clocks can nest and must not be summed. They are measured
native phases, not a causal decomposition or comparable stock iteration counts.
Output transport is included; optional saved-observation output costs are absent.

The preceding host-boundary profile motivated this change; these headline
measurements independently establish its more modest practical benefit. The
full harness performs a columnwise Mata import audit before each fit. That may
reactivate host workers outside the new scope and help explain why the repeated
fit diagnostic was faster, but this is an untested inference. The frozen setup
and input audit were not altered, and diagnostic timings do not replace these
clocks. Worker existence also does not establish worker CPU activity.

## Next bounded work

Test whether the same public host scope helps OLS, with its own transaction and
Break tests and headline comparisons; the unchanged OLS control is still 1.53×
stock. Separately locate one-FE T1→T2 scheduling/finite/strict work before changing
native team policy. Small two-FE's measured projection cost is another concrete
remaining solver target. A fresh result on these seeds cannot adjudicate the
old weak-design SE discrepancies or large timeouts. No stopped reference campaign
was restarted. No Windows/full qualification, push or release is implied.

Receipts preserve application sentinels, accounting, source/artifact/reference
identities and collected-file hashes. Synthetic result summaries were recomputed
locally and compared semantically; original remote JSON bytes were retained.
Raw licensed logs stay outside Git. `harness/` captures the submitted job scripts;
registration and deployment records bind the remaining harness closure.
