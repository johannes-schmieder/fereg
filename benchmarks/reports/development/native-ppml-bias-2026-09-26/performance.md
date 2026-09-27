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
