# One-FE PPML: checked-kernel candidate rejected; embedding overhead isolated

This is bounded development evidence, not qualification. **No production change
is retained.** The accepted computational source remains snapshot **5c0d4685…**
(main 524c8e8b). All 211 inputs and the exact accepted Mac plugin were restored;
see [restoration.json](restoration.json). The broader performance goal remains
open. [ADR0239](../../../../docs/adr/0239-checked-reduction-inlining.md) records the
rejection. Historical receipts are unchanged.

## Attribution and candidate

[Profile 7763535](profile01/README.md) first measured the current public one-FE
path. The portable multiply-add hypothesis did not identify the main avoidable
cost. Serial self samples put 5.43% in the checked compensated accumulator,
3.60% in the scaled norm accumulator, and 4.66% in group means, principally
through fresh centered preparation and independent replay. These are CPU shares,
not a prediction of removable call overhead.

The [seven-line candidate](rejected-candidate.patch) added inline annotations and
comments to those three helpers. It changed no recurrence, arithmetic expression,
finite check, tolerance, rank decision, weight semantics, certificate, ownership,
thread policy or allocation. Its source snapshot is **cda0912f…**, Linux plugin
**570eedc0…**, Mac **1fdf46c0…**. Source and artifact bindings are in
[candidate-source.json](candidate-source.json), [Linux validation](linux-validation02.json)
and [Mac validation](mac-validation02.json). The baseline is **5c0d4685…**, Linux
**94925723…**, Mac **0172ddaf…**. The original report directory name reflects the
initial portable-kernel hypothesis, which was not implemented.

Both platforms pass **74 distinct focused Rust tests** and **six licensed
fixtures**. These cover independent explicit-dummy weighted Poisson/Hessian and
sandwich calculations, robust/FW/PW/cluster semantics, means, saved effects and
predictors, 16-iteration allocation audits in fast/low modes, categorical and
slope PPML and multipartition OLS controls. Linux passes PPML smoke; both pass
actual-plugin ABI/panic checks. No full qualification or static allocation
inventory claim. The initial linear-session test filter matched zero tests; its
log is bound and counted as zero. The corrected seven-test filter passed locally
and in [SCC preflight 7763576](linear-validation03.json), before headline fits.

## Comparable headline measurements

Initial **7763576** and fresh **7763579** complete **144 applications**, **48/48
native scientific pairs** and **48/48 stock scientific pairs**. Native coefficient
and full covariance exports are byte-identical in every pair. Sample, convergence,
rank/DoF, objective, inner/outer work counts and all science gates pass. There
are no application failures, timeouts or scientific NR cells in these blocks.
The separate explicit oracles are the fixtures above; headline cases were not
individually solved with a new dense oracle.

Both jobs reserve the complete 28-core E5-2680v4 node scc-wo4. Accounting is clean:
165s/1.959GiB and 287s/1.953GiB maxvmem respectively. No compilation, profiling,
archiving or competing fit overlaps headline timing. Each fit uses normal Stata
host settings and the registered affinity/native threads. The subsequent host
experiment never changes these comparisons.

The existing synthetic designs, frozen generator, warmup and source-verifying
rotation harness are reused. Initial seed 2026092817 is the tuning/diagnostic seed,
with two alternating baseline/candidate/stock rotations. Fresh seed 2026092819 was
predeclared, with three rotations and an added N122880/K10/T8 case. **Do not tune
to the fresh seed.** PPML uses robust inference; OLS uses IID. Headline calls omit
optional observation outputs. Stock is the pinned, unmodified fixest 0.14.2
installation, fingerprint 041bf479…; the slope control uses glm.tol=1e-10 and
fixef.tol=1e-11. Other stock calls use their existing default reference profile.

| Case | Initial candidate/baseline median | Fresh candidate/baseline min / median / max | Fresh rejected candidate/stock median |
|---|---:|---:|---:|
| One FE, N30720/K6/T1 | .838 | .546 / .847 / .860 | 2.654 |
| Same, T2 | 1.002 | .852 / .986 / 1.015 | 2.189 |
| Same, T4 | .957 | .793 / .916 / .932 | 3.000 |
| Same, low T4 | .873 | .941 / .967 / 1.153 | 3.473 |
| One FE, N30720/K24/T4 | .932 | .821 / .887 / .990 | .930 |
| Two-FE dense PPML, N30720/K6/T4 | 1.065 | .956 / 1.024 / 1.097 | 2.088 |
| Three-FE sparse PPML, N61440/K10/T8 | 1.060 | .913 / .923 / .991 | .533 |
| Two slope partitions, N30720/K6/T4 | .967 | .970 / 1.039 / 1.123 | .396 strict |
| Two-FE OLS, N61440/K10/T8 | 1.044 | .974 / .979 / .984 | 1.415 |
| One FE, N122880/K10/T8 | Not run | .993 / 1.087 / 1.110 | 1.228 |

Full distributions, process versus command clocks, RSS, input/source bindings,
failures and phase telemetry are in [initial03](initial03/README.md) and
[fresh04](fresh04/README.md). Nested phase clocks must not be summed. Model memory
is identical in every native pair. Tracked peaks are identical outside three-FE,
where the existing workspace-overlap variation goes both directions. Linux
binary size rises by 8192 bytes; no numerical workspace is added.

The candidate provides real useful small/serial and wide gains, but its new
larger holdout loses 8.7% and crosses the 1.20 stock target in the median; the
accepted baseline is 1.140 there. Low-mode benefit shrinks to 3.3% with one 15.3%
paired loss. Two-FE PPML loses in both blocks; slopes lose 3.9% fresh. Those costs
do not justify adopting a shared helper change or fitting exceptions to the
unfavorable cases. **Reject the whole candidate; do not tune this annotation set
or add benchmark-specific guards.** No blanket regression/noise claim.

The accepted source's fresh stock ratios remain **3.165 T1, 2.220 T2, 3.219 T4,
3.011 low, 1.062 wide and 1.140 larger** for one-FE PPML. Other controls are
**2.039 small two-FE PPML, .538 three-FE PPML, .353 strict slopes and 1.454 OLS**.
These come from corresponding baseline/stock clocks, not divided medians; see
[current-stock-ratios.json](fresh04/current-stock-ratios.json). They apply only to
these source-bound workloads. The prior 3.517 one-FE ratio belongs to its prior
seed/block, not to every one-FE fit.

Post-hoc same-input comparisons pass science. Accepted-source T4/T1 medians are
1.126 initial and 1.037 fresh, slower in every corresponding repetition. Rejected
candidate ratios are1.312 and 1.197. The thread requests were grouped, not directly
alternating; these are descriptive scaling warnings, not a monotonicity test.

## More valuable next mechanism

[Post-timing diagnostic 7763580](host05/README.md) executes 72 calls on the isolated
candidate artifact, after both headline blocks. Four native workers are verified
for every fit. Changing only pre-launch OpenMP waiting or Stata's host processor
count removes most of the large Stata self-sample share. Whole diagnostic process
elapsed times for normal/passive/host1 are **4.328/1.922/2.021s one-FE** and
**3.927/2.785/2.823s two-FE**. Both sentinels and accounting pass: 33s, 1.114GiB.

This is a strong diagnostic lead, not a stock-relative win: fixed order, shared
node, twelve repeated same-input calls, profiler overhead, no per-call processor
change/restoration cost, and no exported coefficient/VCE comparison. Full caveats
and identities are retained. No process-wide environment change is adopted.

Next implement and test scoped host/native coordination on accepted source:
include switching/restoration inside the public clock, preserve thread requests,
main-thread SPI and the single native call, and prove restoration on failures,
Break and panic. Directly alternate T1/T2/T4, with wide/large/low/OLS controls and
normal stock settings. If switching consumes the gain, reject it and investigate
an appropriate safe wait interface. This addresses a measured cost much larger
than further reduction-helper tuning.

All five jobs are terminal. Collection caught missing scalar metadata and a JSON
serialization-order hash difference; missing files were fetched, semantic equality
checked, and original remote summary bytes preserved. No fit was rerun. Raw
licensed logs and perf binaries remain remote; synthetic traces and immutable
receipts are retained here. No push, release or stopped-campaign restart.
