# Monitored local correction performance

These development measurements use the v05 artifact and frozen plan linked in
[the evidence record](README.md). All 48 processes passed the established
process monitor, and all coefficient/covariance comparisons passed. Each engine
has two cold and ten warm calls per case. No Linux or million-row inference
is supported by this local matrix.

Correction times below are warm medians in seconds. Current Mata solves from
the information matrix each time; cached Mata retains its Cholesky factor.
Both references include the same probe-group numerical diagnostics. Native
correction time includes FE materialization, while Mata publication is separate.

| Rows | Levels per FE | Units | B | Threads / memory | Native | Current Mata | Cached Mata | Cached / native |
|---:|---:|---:|---:|:---|---:|---:|---:|---:|
| 2,880 | 8 | 240 | 64 | 1 / auto | 0.00872 | 0.08100 | 0.08100 | 9.29 |
| 2,880 | 8 | 240 | 256 | 1 / auto | 0.02943 | 0.27300 | 0.27200 | 9.24 |
| 2,880 | 8 | 240 | 1024 | 1 / auto | 0.11389 | 1.04600 | 1.04600 | 9.18 |
| 11,520 | 32 | 960 | 256 | 1 / auto | 0.11733 | 1.13350 | 1.13000 | 9.63 |
| 46,080 | 64 | 3,840 | 256 | 1 / auto | 0.46966 | 4.57600 | 4.53700 | 9.66 |
| 46,080 | 64 | 3,840 | 256 | 4 / auto | 0.48290 | 4.53950 | 4.49150 | 9.30 |
| 11,520 | 32 | 960 | 256 | 2 / low | 0.32470 | 1.13300 | 1.12500 | 3.46 |
| 120,000 | 256 | 10,000 | 64 | 1 / auto | 1.06890 | 4.72250 | 3.62150 | 3.39 |

The dense correction is serial. Four threads help parts of ordinary estimation
but did not improve correction time in the paired 46,080-row cases. The explicit
low-memory case and 256-level case use the sparse route. These are measured
routing tradeoffs, not evidence that every design has the same relative cost.

End-to-end warm calls include a fresh PPML fit and correction publication for
every engine. Native totals below show the observed minimum–maximum; the JSON
also retains cold calls and distributions for every named phase and engine.

| Rows / B / threads / memory | Native median [min, max] | Current Mata median | Cached Mata median | Peak Stata RSS MB: native / current / cached |
|:---|---:|---:|---:|:---|
| 2,880 / 64 / 1 / auto | 0.0220 [0.0210, 0.0220] | 0.0930 | 0.0930 | 47.9 / 59.3 / 60.5 |
| 2,880 / 256 / 1 / auto | 0.0430 [0.0420, 0.0460] | 0.2860 | 0.2850 | 47.3 / 61.3 / 60.0 |
| 2,880 / 1024 / 1 / auto | 0.1270 [0.1260, 0.1280] | 1.0595 | 1.0600 | 47.5 / 61.8 / 62.2 |
| 11,520 / 256 / 1 / auto | 0.1530 [0.1520, 0.1550] | 1.1670 | 1.1655 | 49.4 / 61.3 / 62.3 |
| 46,080 / 256 / 1 / auto | 0.5980 [0.5860, 0.6190] | 4.7010 | 4.6600 | 82.8 / 123.5 / 117.9 |
| 46,080 / 256 / 4 / auto | 0.5585 [0.5540, 0.5650] | 4.6250 | 4.5775 | 87.7 / 112.7 / 112.7 |
| 11,520 / 256 / 2 / low | 0.3555 [0.3530, 0.3580] | 1.1655 | 1.1570 | 51.9 / 62.9 / 64.7 |
| 120,000 / 64 / 1 / auto | 1.4025 [1.3840, 1.4840] | 5.0490 | 3.9465 | 236.8 / 232.7 / 236.8 |

RSS is the Stata child-process maximum, including its loaded banks and saved
outputs; it is not whole-machine memory, Python preparation memory, or native
workspace admission. The largest local case shows no consistent RSS advantage.
The native additional workspace bounds range from 1.11 to 47.23 MB.

Input and sign-file hashes match within every pair. Mata receives precomputed
Philox signs, whose Python preparation is timed outside repeated calls; native
generates signs inside its correction. This favors Mata in the correction-time
comparison. Startup and Mata compilation are excluded from repeated calls;
process elapsed includes those costs plus every call and export and does not
isolate startup duration. The sampled monitor cannot rule out all ambient OS load.

All ordinary coefficient and covariance comparisons are exact within each
process. The largest matched D/C or numerical-covariance error is 8.14e-15
against the prespecified 2e-8 accuracy threshold. Numerical work, sample, probe
count and published correction channels are matched. The initial unmonitored
matrix remains diagnostic and is not used for these timing claims.

## Completed v06 local comparison

The separate [v06 comparison](benchmark-v06.json) uses the precision repair
preceding the million-row v07 change. Its [acceptance record](benchmark-v06-acceptance.json)
binds the frozen plan, source and Mac binary. Both cases pass all twelve
process-monitor receipts and matched D/C/numerical-covariance checks; the
maximum discrepancy is 5.96e-15. There are 48 timed calls, with two cold and
six warm calls per engine per case. The earlier v05 measurements above remain
unchanged; these are separate within-run comparisons, not a v05/v06 speedup.

| Rows | Levels per FE | Units | B | Threads / memory | Native correction | Current Mata correction | Cached Mata correction | Cached / native |
|---:|---:|---:|---:|:---|---:|---:|---:|---:|
| 2,880 | 8 | 240 | 256 | 1 / auto | 0.02954 | 0.27650 | 0.27600 | 9.34 |
| 120,000 | 256 | 10,000 | 64 | 1 / auto | 1.11548 | 4.72800 | 3.62000 | 3.25 |

All times are warm median seconds, with the same phase definitions above.
End-to-end native/current-Mata/cached-Mata medians are 0.0430/0.2890/0.2885s
for the smaller case and 1.4500/5.0555/3.9460s for the larger case. Cached Mata
takes 6.71 and 2.72 times as long end to end. This completes the queued v06
local measurement; it is not v07 performance evidence.

## Completed v07 SCC single-thread subset

Job 7757609 completed all six single-thread processes before the first
eight-thread process stopped at Stata's four-processor license limit. The
full job failed. A [separately identified analysis](benchmark-scc-v07-t1-verification.json)
of the completed subset retains both engine orders and all 24 calls; it does
not replace or complete the missing eight-thread case. The collected archive,
inputs and probes are hash-verified. Every selected process and monitor passes,
and the original accuracy check passes with maximum discrepancy 3.01e-17.

The case has 1,000,000 rows, 256 levels per FE, 100,000 units, B=64, one thread
and automatic memory routing. Each engine has two cold and six warm calls.
The [full distributions and phase measurements](benchmark-scc-v07-t1.json)
retain the frozen v07 source/binary bindings and original parent plan.

| Engine | Warm correction median [min, max], s | Warm total median [min, max], s | Peak Stata RSS, MB |
|:---|:---|:---|---:|
| Native | 32.922 [32.174, 33.087] | 51.711 [51.271, 52.907] | 948.4 |
| Current Mata | 72.147 [71.934, 72.462] | 93.885 [92.157, 94.241] | 954.5 |
| Cached Mata | 71.327 [71.134, 71.434] | 92.956 [91.444, 93.201] | 953.1 |

Cached Mata takes 2.17 times as long for correction and 1.80 times as long end
to end. This is a within-host, within-subset comparison, not a Mac/Linux or
v06/v07 speedup. Native additional correction workspace is 385.15 MB; process
RSS includes loaded data and saved outputs. No parallel SCC scaling result or
ten-million-row result is available from this job.

## Final v07 local comparison

The [final Mac validation](licensed-mac-v07.json) includes the [frozen two-case v07 plan](benchmark-plan-v07.json)
and [all matched results](benchmark-v07.json). All twelve process monitors and
48 timed calls pass; the maximum matched-output discrepancy is 1.94e-15.
Case definitions and phase scope are the same as the v06 table. Each engine
has two cold and six warm calls per case. Medians below are seconds.

| Rows | Native correction | Current Mata correction | Cached Mata correction | Cached / native | Native total | Current Mata total | Cached Mata total |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 2,880 | 0.02916 | 0.27250 | 0.27200 | 9.33 | 0.0460 | 0.2880 | 0.2885 |
| 120,000 | 1.09151 | 4.71550 | 3.76150 | 3.45 | 1.5630 | 5.1750 | 4.2470 |

Cached Mata takes 6.27 and 2.72 times as long end to end. These are within-run
engine comparisons. The separate v05/v06 measurements above are retained;
they do not form a controlled before/after timing experiment for the precision
repair. The installed local development plugin now matches this v07 binary.
