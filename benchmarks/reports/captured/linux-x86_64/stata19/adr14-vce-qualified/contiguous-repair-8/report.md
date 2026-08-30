# ADR-0014 contiguous VCE reduction repair

Warm medians from eight strict SCC shards.  Comparator ratios are within-shard;
`new/a8` is a cross-run diagnostic and the nodes are preserved in
`evidence.json`.

| N | VCE | Threads | fereg | Greg | reghdfe | f/Greg | f/reghdfe | a8fbad2 | new/a8 |
|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|
| 1m | robust | 1 | 10.982 | 8.933 | 46.182 | 1.229 | 0.238 | 29.642 | 0.370 |
| 1m | robust | 16 | 3.000 | 7.252 | 14.086 | 0.414 | 0.213 | 17.207 | 0.174 |
| 1m | cluster1 | 1 | 6.875 | 5.759 | 25.006 | 1.194 | 0.275 | 29.195 | 0.235 |
| 1m | cluster1 | 16 | 4.437 | 9.512 | 30.308 | 0.466 | 0.146 | 16.955 | 0.262 |
| 10m | robust | 1 | 161.455 | 78.262 | 442.199 | 2.063 | 0.365 | 456.748 | 0.353 |
| 10m | robust | 16 | 87.216 | 104.651 | 369.233 | 0.833 | 0.236 | 397.109 | 0.220 |
| 10m | cluster1 | 1 | 131.854 | 65.545 | 350.998 | 2.012 | 0.376 | 453.018 | 0.291 |
| 10m | cluster1 | 16 | 60.488 | 65.959 | 205.534 | 0.917 | 0.294 | 77.437 | 0.781 |

## Aggregate ratios

| Population | Comparator | Cells | Geometric mean | fereg faster |
|---|---|---:|---:|---:|
| all | Greg | 8 | 0.987 | 4 |
| all | reghdfe | 8 | 0.258 | 8 |
| 1 thread | Greg | 4 | 1.571 | 0 |
| 1 thread | reghdfe | 4 | 0.308 | 4 |
| 16 threads | Greg | 4 | 0.620 | 4 |
| 16 threads | reghdfe | 4 | 0.216 | 4 |
| all | rejected a8fbad2 | 8 | 0.303 | 8 |

## Scientific and operational gates

- Source `490e03a3e2458d8bc463c3f60389062e2f8fae42`; archive SHA-256
  `4f4e44cc539419ae67043cfca824c92aed7c6b1969c1a37825939e2397d02aab`;
  Linux plugin SHA-256
  `9cbe00b6c5a0d5c47af3d5d1fd32166a200d27202d335a602d584a3d5b48a4a2`.
- Setup `7367526`, array tasks `7367527.1-8`, and collector `7367528` all
  have structurally complete SGE accounting with `failed=0` and
  `exit_status=0`.
- Eight strict shard success receipts; nine timing rows and one passing
  scientific preflight per shard; 72 merged timing rows and eight preflights.
- Zero command failures, zero science failures, requested native fereg threads
  honored, and clean `FEREG_VCE_COMPARISON_SUCCESS` Stata endings.
- Arms are exactly fereg, Greg, and reghdfe.  Areg is absent.
- Every final shard ran on a 32-processor SCC node.  Stata IC used one/four
  processors for requested fereg threads one/sixteen; the native request is
  recorded separately and validated.

Cross-run comparisons are intentionally secondary.  The rejected a8fbad2
stage mixed 16- and 32-processor nodes, while this run used 32-processor nodes.
The large 70-percent geometric-mean repair and final same-shard comparator
ratios are the acceptance evidence; small cross-run differences are not.
