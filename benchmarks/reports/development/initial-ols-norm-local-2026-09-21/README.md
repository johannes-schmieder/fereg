# Initial automatic OLS norm pass

Candidate in1 replaces the duplicate initial norm calculation inside borrowed
automatic absorption with the existing initial-certificate helper. The input
is unchanged since its reference norms were computed. All FE moments are still
freshly checked, and later residuals retain their independent norm scans.
The change adds no allocation and changes no solver selection or tolerance.

The focused fixture covers serial/parallel execution, 257/16,385 rows,
1/7/52 RHS columns, zero and extreme values, and unit/supplied weights across
extreme common scales. Both certificate calculations and their resulting
workspace values match. The captured plugin passes the actual ABI fixture.

Sixteen three-pair whole-command cells complete 96 successful calls with clean
isolation. All 32 independent native inference audits, 96 within-arm repeats
and 48 candidate output audits pass. All 48 native pairs have exact finite
exports, samples, status, FE metadata, fresh certificates and saved vectors.
Every solver-report field except setup/solve time is also exact.

The paired runtime ratios range from 0.9653 to 1.0189; these are small changes,
not a broad speed breakthrough. All measured cells remain within 5% of sf1.
The original times, pair ratios and tracked allocation peaks remain in the
scores. Forty paired peaks match; eight differ by -2,536,968 to +51,081,104
bytes in threaded fits. No owner or modeled bound changes, but the observed
peak increase remains explicit; this is not memory-neutral evidence.
This is development evidence, not final-block or Linux acceptance.

The first archive attempt stopped on a nonexistent second screen controller.
Its error and original script remain preserved; the partial archive was moved
to an ignored local directory before this corrected attempt.

| Graph | Weight | K | Threads | Request | sf1 ms | in1 ms | Paired in1/sf1 |
|---|---|---:|---:|---|---:|---:|---:|
| strong_d2 | none | 6 | 1 | cluster2_cross_all | 698 | 688 | 0.9857 |
| strong_d2 | aw | 6 | 1 | cluster2_cross_all | 749 | 733 | 0.9653 |
| strong_d2 | none | 50 | 1 | cluster2_cross_all | 3674 | 3669 | 1.0068 |
| strong_d2 | aw | 50 | 1 | cluster2_cross_all | 3911 | 3899 | 0.9921 |
| bottleneck_ring | none | 6 | 1 | cluster2_cross_all | 867 | 857 | 0.9827 |
| bottleneck_ring | none | 50 | 1 | cluster2_cross_all | 5666 | 5715 | 1.0078 |
| bottleneck_ring | aw | 50 | 1 | cluster2_cross_all | 5100 | 5111 | 1.0037 |
| cycle | none | 6 | 1 | cluster2_cross_all | 509 | 510 | 1.0020 |
| cycle | none | 50 | 1 | cluster2_cross_all | 2558 | 2582 | 1.0106 |
| cycle | aw | 50 | 1 | cluster2_cross_all | 3155 | 3159 | 0.9981 |
| strong_d2 | none | 6 | 16 | cluster2_cross_all | 352 | 343 | 0.9776 |
| strong_d2 | aw | 6 | 16 | cluster2_cross_all | 369 | 362 | 0.9810 |
| bottleneck_ring | none | 50 | 16 | cluster2_cross_all | 1256 | 1244 | 0.9897 |
| bottleneck_ring | aw | 50 | 16 | cluster2_cross_all | 1067 | 1065 | 0.9991 |
| cycle | none | 50 | 16 | cluster2_cross_all | 797 | 790 | 0.9962 |
| cycle | aw | 50 | 16 | cluster2_cross_all | 902 | 918 | 1.0189 |

`verify.py` verifies the portable archive, source reconstruction and independent
auditor bindings. Output arrays remain local with hashes. Raw licensed logs
and compiled binaries are excluded. No installation, release or push occurs.
