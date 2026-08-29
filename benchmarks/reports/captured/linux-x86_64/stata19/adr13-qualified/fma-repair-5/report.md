# ADR-0013 portable-x86 FMA repair

Warm medians from five strict SCC shards. Ratios against Greg and reghdfe are within-shard; old/new fereg times are cross-run diagnostics.

| Design | K | Threads | Mode | fereg | Greg | reghdfe | f/Greg | f/reghdfe | old fereg | new/old | Native |
|---|---:|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|
| akm_standard | 30 | 1 | fit | 5.080 | 5.637 | 25.424 | 0.901 | 0.200 | 109.292 | 0.046 | 4.547 |
| akm_standard | 30 | 1 | save_fe | 5.939 | 7.160 | 29.473 | 0.829 | 0.202 | 115.965 | 0.051 | 5.323 |
| akm_standard | 30 | 16 | fit | 2.129 | 5.791 | 11.958 | 0.368 | 0.178 | 17.354 | 0.123 | 1.753 |
| akm_standard | 30 | 16 | save_fe | 2.448 | 5.941 | 12.474 | 0.412 | 0.196 | 14.407 | 0.170 | 2.061 |
| one_fe | 10 | 1 | fit | 1.438 | 0.413 | 2.204 | 3.482 | 0.652 | 17.232 | 0.083 | 0.860 |
| one_fe | 10 | 1 | save_fe | 1.910 | 0.607 | 2.749 | 3.147 | 0.695 | 19.889 | 0.096 | 1.252 |
| one_fe | 30 | 1 | fit | 3.210 | 1.257 | 8.245 | 2.554 | 0.389 | 104.278 | 0.031 | 2.385 |
| one_fe | 30 | 1 | save_fe | 4.026 | 1.529 | 8.697 | 2.633 | 0.463 | 111.239 | 0.036 | 3.074 |
| one_fe | 30 | 16 | fit | 1.424 | 1.135 | 4.967 | 1.255 | 0.287 | 14.970 | 0.095 | 0.814 |
| one_fe | 30 | 16 | save_fe | 2.002 | 1.417 | 5.438 | 1.413 | 0.368 | 11.571 | 0.173 | 1.301 |

## Aggregate within-shard ratios

| Mode | Comparator | Cells | Geometric mean | fereg faster |
|---|---|---:|---:|---:|
| fit | greg | 5 | 1.299 | 2 |
| fit | reghdfe | 5 | 0.304 | 5 |
| save_fe | greg | 5 | 1.320 | 2 |
| save_fe | reghdfe | 5 | 0.342 | 5 |

## Scientific and operational gates

- Five strict shard SUCCESS receipts; 180 timing rows and five passing preflights.
- Zero command failures, zero science failures, and exact requested fereg thread counts.
- Only fereg, Greg, and reghdfe arms; areg is absent.
- Setup and every task have complete SGE accounting with `failed=0` and `exit_status=0`.
- Exact source, archive, and Linux plugin identities are recorded in `evidence.json`.
- Every shard ran on a 32-processor SCC node; original shard environments are preserved.
