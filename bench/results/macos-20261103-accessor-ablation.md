# Mac numeric-loading ablation, November 3, 2026

One predeclared comparison of the packaged beta.15 Mac binary with itself.
`inject(serial_input)` changes only the reader; both arms retain 20 estimation
workers. There are no arithmetic, routing or scientific specification changes.
The complete declaration, source/binary hashes, input hash and numerical checks
are in the companion summary JSON; all twelve timed calls are in the CSV.

The host is an Apple M2 Ultra (24 cores: 16 performance and 8 efficiency), 192 GiB,
macOS 26.6.2. Stata/MP 19 uses its licensed eight processors; the plugin uses 20
workers. No competing builds, tests or estimation were run during timing.
Ordinary desktop activity is retained. No observations were excluded or retried.

The fixed seed 739391 generates 20,000,000 observations, 16 independent standard
normal double controls, a long categorical identifier with 5,003 levels, and
y = x1 + 0.5*x2 - 0.25*x3 + 0.1*x4 + sin(g/11) + standard normal noise.
The model is OLS with that single fixed effect and conventional covariance.
Data start in Stata memory; disk input and generation are outside the timers.
`bench/accessor_ablation.do` is the retained reproducer. Dispatch verification,
timing and reporting with `bench/qualify.py input {verify,run,report} CONFIG`;
CONFIG follows the sanitized declaration in the summary, with local Stata,
output, ado and plugin paths supplied. No SCC environment is reused.

After independent areg checks and input/transport tracing, a fresh Stata process
warms each reader once and runs six alternating pairs (serial first in odd pairs,
parallel first in even pairs). Both arms use the same hidden estimation adapter;
the science preflight separately requires exact hidden/public agreement. Timing
traces are disabled. Every timed fit also retains exact coefficient, covariance,
sample-size, rank and degrees-of-freedom agreement with the preflight.

| Median, seconds | Serial reader | Parallel reader |
|---|---:|---:|
| Complete loading phase | 1.7312757915 | 0.2134546665 |
| Native runtime, including loading | 3.6386609785 | 2.1237389375 |
| Whole Stata command | 4.5055 | 2.9825 |

Loading is 8.11 times faster (87.67% less time); whole-command time falls 33.80%.
The scope is this fixed same-Mac ablation. It does not estimate the accessor's
contribution to the separate SCC campaign or a general 20-core speedup.

All six science fits load the same input digest, 6a5a9c3d4127ddab. Traces verify
20-worker parallel windows in both repeated hidden fits and the public fit;
forced serial and one-worker fits have no parallel window. The largest absolute
coefficient difference from areg is 3.081e-14 and covariance difference is
1.835e-20. Fixed-worker repeats and reader arms agree exactly; the one-worker
fit passes the unchanged 1e-9 coefficient / 1e-8 covariance tolerances. All
12 timed calls pass exact same-worker science checks. The maintained reporter
accepts complete coverage and finds no whole-command regression under its
max(5%, 1 ms) rule. Five existing input-report/diagnostic tests also passed.
