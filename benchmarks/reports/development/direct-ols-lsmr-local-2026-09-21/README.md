# Direct automatic LSMR cost diagnostic

The captured dl1 transform bypasses the initial Schur/CMG attempt for public
fast two-intercept OLS. It preserves automatic LSMR's stricter recurrence,
memory admission, certified continuation and publication checks. The checkout's
automatic routing rule is unchanged. Both fresh routing seeds remain unused.

Thirty-two training cells complete three sf1/dl1 pairs: 192 successful calls
with no detected competing process. All 64 independent native inference audits,
192 exact within-arm repeat checks and 48 independent candidate saved-output
audits pass. Paired samples match; finite exports and saved vectors can differ
within the existing scientific tolerances because the numerical route differs.
All paired timings and tracked allocation peaks are preserved in the scores.

Cycles favor direct automatic LSMR across unit/AW, K6/K50, T1/T16 and both output
requests. Their paired ratios range from 0.5291 to 0.7673. Ring results are
mixed: direct LSMR helps wide serial and unit threaded fits, but AW threaded
K6 regresses 14.6–16.7% and AW threaded K50 robust regresses 7.4%. A blanket
LSMR switch is rejected. Serial unit narrow ring differences are within 5%.

These screens isolate routing overhead; they do not establish a production
graph guard or cross-platform acceptance. A structural cycle-specific rule is
a possible next experiment. Ordinary explicit LSMR has a different internal
recurrence target and must not be identified with this automatic diagnostic.

| Graph | Weight | K | Threads | Request | sf1 ms | dl1 ms | Paired dl1/sf1 |
|---|---|---:|---:|---|---:|---:|---:|
| bottleneck_ring | none | 6 | 1 | robust_none | 695 | 688 | 0.9899 |
| bottleneck_ring | none | 6 | 1 | cluster2_cross_all | 850 | 813 | 0.9541 |
| bottleneck_ring | none | 50 | 1 | robust_none | 5392 | 4613 | 0.8555 |
| bottleneck_ring | none | 50 | 1 | cluster2_cross_all | 5678 | 4834 | 0.8485 |
| cycle | none | 6 | 1 | robust_none | 338 | 221 | 0.6538 |
| cycle | none | 6 | 1 | cluster2_cross_all | 503 | 310 | 0.6143 |
| cycle | none | 50 | 1 | robust_none | 2370 | 1500 | 0.6276 |
| cycle | none | 50 | 1 | cluster2_cross_all | 2562 | 1580 | 0.6128 |
| bottleneck_ring | none | 6 | 16 | robust_none | 298 | 238 | 0.7881 |
| bottleneck_ring | none | 6 | 16 | cluster2_cross_all | 421 | 364 | 0.8646 |
| bottleneck_ring | none | 50 | 16 | robust_none | 1056 | 865 | 0.8191 |
| bottleneck_ring | none | 50 | 16 | cluster2_cross_all | 1281 | 1098 | 0.8571 |
| cycle | none | 6 | 16 | robust_none | 156 | 121 | 0.7673 |
| cycle | none | 6 | 16 | cluster2_cross_all | 247 | 173 | 0.7004 |
| cycle | none | 50 | 16 | robust_none | 601 | 444 | 0.7354 |
| cycle | none | 50 | 16 | cluster2_cross_all | 803 | 586 | 0.7347 |
| bottleneck_ring | aw | 6 | 1 | robust_none | 678 | 661 | 0.9764 |
| bottleneck_ring | aw | 6 | 1 | cluster2_cross_all | 865 | 806 | 0.9318 |
| bottleneck_ring | aw | 50 | 1 | robust_none | 4894 | 4555 | 0.9336 |
| bottleneck_ring | aw | 50 | 1 | cluster2_cross_all | 5096 | 4704 | 0.9246 |
| cycle | aw | 6 | 1 | robust_none | 448 | 245 | 0.5469 |
| cycle | aw | 6 | 1 | cluster2_cross_all | 648 | 345 | 0.5291 |
| cycle | aw | 50 | 1 | robust_none | 2882 | 1587 | 0.5531 |
| cycle | aw | 50 | 1 | cluster2_cross_all | 3154 | 1697 | 0.5474 |
| bottleneck_ring | aw | 6 | 16 | robust_none | 201 | 237 | 1.1675 |
| bottleneck_ring | aw | 6 | 16 | cluster2_cross_all | 318 | 365 | 1.1465 |
| bottleneck_ring | aw | 50 | 16 | robust_none | 884 | 949 | 1.0735 |
| bottleneck_ring | aw | 50 | 16 | cluster2_cross_all | 1079 | 1085 | 1.0018 |
| cycle | aw | 6 | 16 | robust_none | 207 | 137 | 0.6618 |
| cycle | aw | 6 | 16 | cluster2_cross_all | 312 | 192 | 0.6154 |
| cycle | aw | 50 | 16 | robust_none | 703 | 476 | 0.6771 |
| cycle | aw | 50 | 16 | cluster2_cross_all | 910 | 615 | 0.6768 |

`verify.py` checks archive integrity, independent-auditor binding and complete
sf1/dl1 source reconstruction. Large output vectors remain local with hashes;
raw licensed logs and compiled plugins are excluded. Final independent blocks,
broader routing validation and Linux confirmation remain open. No release,
installation or push is performed.
