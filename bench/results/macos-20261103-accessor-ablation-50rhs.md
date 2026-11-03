# Mac numeric-loading ablation with 50 RHS variables, November 3, 2026

Johannes requested this specification after the retained 16-control experiment.
It is one fixed new comparison, not a retry or replacement of adverse results.
The original 16-control observations and summary remain unchanged.

The packaged beta.15 Mac binary, maintained accessor_ablation harness, seed
739391, 20,000,000 observations, one categorical fixed effect with 5,003 levels,
20 estimation workers and six alternating pairs are unchanged. This run has
50 independent standard-normal double controls (52 input columns including
outcome and FE identifier). The outcome is x1 + 0.5*x2 - 0.25*x3 + 0.1*x4 +
sin(g/11) + standard-normal noise. Because the generator draws the noise after
the controls, its realization differs from the 16-control dataset; both readers
in this experiment use exactly the same frozen dataset. No runtime, numerical
policy or route changes were made. The existing inject(serial_input) control
is the only difference between the two reader arms.

Host: Apple M2 Ultra, 24 physical cores (16 performance, eight efficiency),
192 GiB, macOS 26.6.2. Stata/MP 19 uses eight licensed processors; the plugin
uses 20 workers. No competing builds, tests, estimation or agent work were run
during timing. Ordinary desktop activity was retained, with no exclusions,
resampling or additional repetitions. Data generation and disk input are outside
the timers. After science checks, one fresh Stata process warms each reader
once, then measures six alternating pairs. Serial runs first in odd pairs and
parallel first in even pairs. Trace instrumentation is disabled during timing.

| Median, seconds | Serial reader | Parallel reader |
|---|---:|---:|
| Loading | 4.5974070005 | 0.9584306870 |
| Native runtime, including loading | 8.9359179160 | 5.2652062915 |
| Whole Stata command | 10.6555000000 | 6.9805000000 |

Loading is 4.797 times faster (79.15% less time);
whole-command time falls 34.49%. The native runtime includes
loading, so these phase times must not be added together. This is a same-Mac
reader ablation, not the accessor's contribution to the separate SCC campaign
or a general 20-core speedup.

All six preflight fits load input digest 8f028cc6ded3cb03.
Traces verify 52-column, 20-worker parallel windows for repeated hidden and
public fits; forced serial and one-worker fits have no parallel window.
Maximum absolute differences from areg are 2.373e-13 for coefficients and
1.216e-20 for covariance entries. Reader arms and fixed-worker repeats agree
exactly. Cross-worker checks pass the unchanged 1e-9 coefficient and 1e-8
covariance tolerances. All 12 timed calls pass exact coefficient, covariance,
sample-size, rank and degrees-of-freedom checks. Complete coverage and the
maintained max(5%, 1 ms) whole-command regression gate pass.

The companion summary retains the full frozen declaration, source/plugin hashes,
input hash, science results, transport traces and observation hash; the CSV
retains every timing. Reproduce with bench/qualify.py input verify/run/report,
using the summary's sanitized configuration plus local Stata, ado, plugin and
output paths. The harness and plugin are unchanged from the 16-control run;
its five input-report/diagnostic tests remain applicable.
