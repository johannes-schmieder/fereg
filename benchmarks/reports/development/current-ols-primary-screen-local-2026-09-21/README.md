# Current OLS primary training screen

The in1 kernel wins all 77 unchanged primary cells against pinned default
fixest in this three-pair screen. All 462 applications complete with no
detected timing overlap. Seventy-five cells meet the ten-percent target.
The smallest margins are unbalanced K6/AW (paired ratio 0.92208) and shuffled
K5/unit (0.91892). These are development screens, not the two final five-pair
blocks or a claim about every estimator/domain.

All 77 independent native inference checks, 231 exact within-arm repeat
checks and 24 independent saved-output audits pass. The reference inference
audits give 61 passes and 16 discrepancies. All reference
science/output disagreements remain recorded; timing does not adjudicate them.
The grounded sparse-direct oracle uses the original input and its own weighted
dummy equations, independently of native FE state.

The auditor supports both one-native primary and two-native paired inventories.
Its two focused fixtures cover separate reference discrepancies and missing,
extra, failed or changed native runs. Each native arm must contain exactly
the planned passing repeats; an extra run cannot offset a missing other arm.
Earlier v1 auditors and their historical receipts remain unchanged.

The captured in1 source predates the scoped serial-window and narrow-residual
experiments. No new routing guard, constrained-memory result or Linux
acceptance follows from this screen. Candidate allocation peaks are preserved
per pair; this comparison provides no native memory-neutrality claim.

| Index | Graph | Weight | K | Shuffled | Request | in1 ms | fixest ms | Paired ratio |
|---:|---|---|---:|---|---|---:|---:|---:|
| 0 | two_fe_dense | none | 1 | False | iid | 182 | 340 | 0.5401 |
| 1 | two_fe_dense | aw | 1 | False | iid | 235 | 435 | 0.5402 |
| 2 | two_fe_dense | none | 3 | False | iid | 195 | 361 | 0.5402 |
| 3 | two_fe_dense | aw | 3 | False | iid | 251 | 461 | 0.5445 |
| 4 | two_fe_dense | none | 5 | False | iid | 209 | 367 | 0.5679 |
| 5 | two_fe_dense | aw | 5 | False | iid | 264 | 485 | 0.5485 |
| 6 | two_fe_dense | none | 6 | False | iid | 206 | 377 | 0.5491 |
| 7 | two_fe_dense | aw | 6 | False | iid | 238 | 485 | 0.4897 |
| 8 | two_fe_dense | none | 13 | False | iid | 317 | 453 | 0.6954 |
| 9 | two_fe_dense | aw | 13 | False | iid | 366 | 579 | 0.6321 |
| 10 | two_fe_dense | none | 50 | False | iid | 657 | 1511 | 0.4325 |
| 11 | two_fe_dense | aw | 50 | False | iid | 731 | 1943 | 0.3721 |
| 12 | two_fe_dense | aw | 6 | False | robust | 255 | 493 | 0.5120 |
| 13 | two_fe_dense | aw | 6 | False | cluster_fe | 246 | 507 | 0.4900 |
| 14 | two_fe_dense | aw | 6 | False | cluster_cross | 263 | 502 | 0.5229 |
| 15 | two_fe_dense | aw | 6 | False | cluster2_cross | 291 | 549 | 0.5301 |
| 16 | two_fe_dense | aw | 6 | False | cluster2_nested | 276 | 555 | 0.4991 |
| 17 | two_fe_dense | aw | 6 | False | iid_saved | 362 | 704 | 0.5142 |
| 18 | two_fe_dense | aw | 6 | False | iid_outputs | 390 | 704 | 0.5516 |
| 19 | two_fe_dense | aw | 50 | False | robust | 861 | 2388 | 0.3606 |
| 20 | two_fe_dense | aw | 50 | False | cluster_fe | 839 | 2469 | 0.3406 |
| 21 | two_fe_dense | aw | 50 | False | cluster_cross | 864 | 2386 | 0.3597 |
| 22 | two_fe_dense | aw | 50 | False | cluster2_cross | 932 | 2883 | 0.3296 |
| 23 | two_fe_dense | aw | 50 | False | cluster2_nested | 940 | 2814 | 0.3355 |
| 24 | two_fe_dense | aw | 50 | False | iid_saved | 903 | 2202 | 0.4088 |
| 25 | two_fe_dense | aw | 50 | False | iid_outputs | 953 | 2191 | 0.4286 |
| 26 | two_fe_dense | pw | 6 | False | robust | 256 | 503 | 0.5090 |
| 27 | two_fe_dense | pw | 6 | False | cluster_fe | 255 | 517 | 0.5020 |
| 28 | two_fe_dense | pw | 6 | False | cluster2_cross | 300 | 560 | 0.5329 |
| 29 | two_fe_dense | pw | 6 | False | cluster2_saved | 437 | 799 | 0.5469 |
| 30 | two_fe_dense | pw | 50 | False | robust | 884 | 2401 | 0.3644 |
| 31 | two_fe_dense | pw | 50 | False | cluster_fe | 856 | 2489 | 0.3454 |
| 32 | two_fe_dense | pw | 50 | False | cluster2_cross | 954 | 2887 | 0.3284 |
| 33 | two_fe_dense | pw | 50 | False | cluster2_saved | 1096 | 3142 | 0.3514 |
| 34 | two_fe_dense | none | 1 | True | iid | 203 | 235 | 0.8675 |
| 35 | two_fe_dense | none | 5 | True | iid | 238 | 259 | 0.9189 |
| 36 | two_fe_dense | none | 50 | True | iid | 720 | 1180 | 0.6025 |
| 37 | two_fe_dense | aw | 6 | True | iid | 266 | 306 | 0.8693 |
| 38 | two_fe_dense | aw | 6 | True | iid_outputs | 438 | 584 | 0.7500 |
| 39 | two_fe_dense | aw | 50 | True | iid | 802 | 1361 | 0.5856 |
| 40 | two_fe_dense | aw | 50 | True | iid_outputs | 1050 | 1652 | 0.6306 |
| 41 | strong_d2 | none | 6 | False | iid | 219 | 638 | 0.3433 |
| 42 | strong_d2 | aw | 6 | False | iid | 227 | 588 | 0.3864 |
| 43 | strong_d2 | none | 50 | False | iid | 707 | 2224 | 0.3179 |
| 44 | strong_d2 | aw | 50 | False | iid | 739 | 1895 | 0.3786 |
| 45 | strong_d3 | none | 6 | False | iid | 58 | 73 | 0.7895 |
| 46 | strong_d3 | aw | 6 | False | iid | 69 | 83 | 0.8434 |
| 47 | strong_d3 | none | 50 | False | iid | 180 | 285 | 0.6316 |
| 48 | strong_d3 | aw | 50 | False | iid | 208 | 317 | 0.6530 |
| 49 | strong_d6 | none | 6 | False | iid | 51 | 65 | 0.7846 |
| 50 | strong_d6 | aw | 6 | False | iid | 63 | 71 | 0.8714 |
| 51 | strong_d6 | none | 50 | False | iid | 173 | 256 | 0.6914 |
| 52 | strong_d6 | aw | 50 | False | iid | 196 | 279 | 0.7025 |
| 53 | two_fe_sparse | none | 6 | False | iid | 107 | 791 | 0.1336 |
| 54 | two_fe_sparse | aw | 6 | False | iid | 123 | 730 | 0.1685 |
| 55 | two_fe_sparse | none | 50 | False | iid | 439 | 2317 | 0.1895 |
| 56 | two_fe_sparse | aw | 50 | False | iid | 477 | 3668 | 0.1300 |
| 57 | unbalanced | none | 6 | False | iid | 59 | 76 | 0.7500 |
| 58 | unbalanced | aw | 6 | False | iid | 70 | 77 | 0.9221 |
| 59 | unbalanced | none | 50 | False | iid | 185 | 274 | 0.6767 |
| 60 | unbalanced | aw | 50 | False | iid | 211 | 297 | 0.7081 |
| 61 | disconnected_blocks | none | 6 | False | iid | 52 | 165 | 0.3152 |
| 62 | disconnected_blocks | aw | 6 | False | iid | 63 | 199 | 0.3119 |
| 63 | disconnected_blocks | none | 50 | False | iid | 168 | 351 | 0.4843 |
| 64 | disconnected_blocks | aw | 50 | False | iid | 193 | 380 | 0.5026 |
| 65 | weak_d3 | none | 6 | False | iid | 113 | 155 | 0.7290 |
| 66 | weak_d3 | aw | 6 | False | iid | 165 | 211 | 0.7847 |
| 67 | weak_d3 | none | 50 | False | iid | 415 | 606 | 0.6981 |
| 68 | weak_d3 | aw | 50 | False | iid | 617 | 706 | 0.8690 |
| 69 | bottleneck_ring | none | 6 | False | iid | 295 | 2609 | 0.1129 |
| 70 | bottleneck_ring | aw | 6 | False | iid | 194 | 2002 | 0.0972 |
| 71 | bottleneck_ring | none | 50 | False | iid | 979 | 8044 | 0.1223 |
| 72 | bottleneck_ring | aw | 50 | False | iid | 853 | 7934 | 0.1065 |
| 73 | cycle | none | 6 | False | iid | 149 | 3952 | 0.0377 |
| 74 | cycle | aw | 6 | False | iid | 201 | 3250 | 0.0618 |
| 75 | cycle | none | 50 | False | iid | 566 | 12097 | 0.0470 |
| 76 | cycle | aw | 50 | False | iid | 682 | 12143 | 0.0558 |

`verify.py` checks archived hashes, the complete captured native source and
independent auditor bindings. Original output arrays remain local with hashes.
Raw licensed logs and compiled binaries are excluded. No push or installation
is part of this checkpoint.
