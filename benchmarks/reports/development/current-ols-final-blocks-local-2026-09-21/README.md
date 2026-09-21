# Current OLS final Mac timing blocks

The frozen owf1 candidate completes two independent five-pair blocks on the
unchanged 77 primary and sixteen holdout cells: 1,860 application calls.
The full timing and scientific records, including reference discrepancies,
are preserved. These blocks concern the captured Mac artifact, not completion
of the broader slope, coverage, routing and Linux milestone.

| Block | Cells | Wins | Ties | Losses | Clean cells | Worst paired ratio | Reference inference pass/discrepancy |
|---|---:|---:|---:|---:|---:|---:|---:|
| final1-primary | 77 | 77 | 0 | 0 | 77 | 0.92369 | 61/16 |
| final1-holdout | 16 | 16 | 0 | 0 | 16 | 0.91304 | 14/2 |
| final2-primary | 77 | 77 | 0 | 0 | 77 | 0.91016 | 61/16 |
| final2-holdout | 16 | 16 | 0 | 0 | 16 | 0.91200 | 14/2 |

All 186 independent native inference checks, 930 exact within-arm repeat
checks and 100 independent saved-output audits pass. The optional cache
reuses only source-bound independent weighted dummy equations for byte-identical
input and weight/VCE. New holdout inputs receive fresh solves; every new native
export is checked. The runtime residual certificates are unchanged. Default
fixest discrepancies remain separate from timing results.

The candidate includes the scoped serial OLS memory window and the preceding
residual traversal. The rejected narrow residual experiment and unaccepted cycle
routing experiment are absent. Fresh routing seeds remain untouched. These
blocks do not resolve serial weighted slopes, constrained weighted fits,
cross-platform routing or the remaining current-candidate control/coverage work.

| Cell | Graph | Weight | K | Shuffled | Request | Block 1 fereg/fixest ms | Paired ratio | Block 2 fereg/fixest ms | Paired ratio |
|---|---|---|---:|---|---|---:|---:|---:|---:|
| primary 0 | two_fe_dense | none | 1 | False | iid | 185/344 | 0.54203 | 180/334 | 0.53892 |
| primary 1 | two_fe_dense | aw | 1 | False | iid | 238/435 | 0.54713 | 233/425 | 0.54824 |
| primary 2 | two_fe_dense | none | 3 | False | iid | 200/358 | 0.55402 | 191/353 | 0.54108 |
| primary 3 | two_fe_dense | aw | 3 | False | iid | 259/474 | 0.54184 | 254/464 | 0.54390 |
| primary 4 | two_fe_dense | none | 5 | False | iid | 217/373 | 0.57560 | 208/367 | 0.56676 |
| primary 5 | two_fe_dense | aw | 5 | False | iid | 277/484 | 0.57231 | 265/474 | 0.55907 |
| primary 6 | two_fe_dense | none | 6 | False | iid | 207/373 | 0.55795 | 202/369 | 0.54743 |
| primary 7 | two_fe_dense | aw | 6 | False | iid | 244/482 | 0.50617 | 243/488 | 0.50410 |
| primary 8 | two_fe_dense | none | 13 | False | iid | 321/453 | 0.70861 | 318/447 | 0.71141 |
| primary 9 | two_fe_dense | aw | 13 | False | iid | 367/583 | 0.63167 | 361/572 | 0.63112 |
| primary 10 | two_fe_dense | none | 50 | False | iid | 671/1506 | 0.43958 | 658/1504 | 0.43750 |
| primary 11 | two_fe_dense | aw | 50 | False | iid | 736/1960 | 0.37506 | 731/1966 | 0.37824 |
| primary 12 | two_fe_dense | aw | 6 | False | robust | 254/494 | 0.51324 | 251/489 | 0.51329 |
| primary 13 | two_fe_dense | aw | 6 | False | cluster_fe | 250/505 | 0.49102 | 250/508 | 0.48544 |
| primary 14 | two_fe_dense | aw | 6 | False | cluster_cross | 263/505 | 0.52929 | 264/512 | 0.51562 |
| primary 15 | two_fe_dense | aw | 6 | False | cluster2_cross | 290/548 | 0.52899 | 294/554 | 0.52527 |
| primary 16 | two_fe_dense | aw | 6 | False | cluster2_nested | 274/551 | 0.49910 | 276/551 | 0.50182 |
| primary 17 | two_fe_dense | aw | 6 | False | iid_saved | 354/705 | 0.50571 | 350/707 | 0.49649 |
| primary 18 | two_fe_dense | aw | 6 | False | iid_outputs | 394/715 | 0.55430 | 390/710 | 0.54852 |
| primary 19 | two_fe_dense | aw | 50 | False | robust | 850/2355 | 0.36093 | 852/2346 | 0.36189 |
| primary 20 | two_fe_dense | aw | 50 | False | cluster_fe | 846/2448 | 0.34395 | 833/2441 | 0.33880 |
| primary 21 | two_fe_dense | aw | 50 | False | cluster_cross | 854/2314 | 0.36906 | 857/2312 | 0.36870 |
| primary 22 | two_fe_dense | aw | 50 | False | cluster2_cross | 925/2806 | 0.33106 | 924/2820 | 0.32263 |
| primary 23 | two_fe_dense | aw | 50 | False | cluster2_nested | 935/2763 | 0.33978 | 929/2758 | 0.33668 |
| primary 24 | two_fe_dense | aw | 50 | False | iid_saved | 866/2164 | 0.40411 | 866/2169 | 0.39620 |
| primary 25 | two_fe_dense | aw | 50 | False | iid_outputs | 914/2166 | 0.42329 | 924/2170 | 0.42473 |
| primary 26 | two_fe_dense | pw | 6 | False | robust | 253/491 | 0.51527 | 253/494 | 0.51008 |
| primary 27 | two_fe_dense | pw | 6 | False | cluster_fe | 251/506 | 0.49312 | 248/507 | 0.48697 |
| primary 28 | two_fe_dense | pw | 6 | False | cluster2_cross | 292/546 | 0.53493 | 297/551 | 0.53591 |
| primary 29 | two_fe_dense | pw | 6 | False | cluster2_saved | 410/772 | 0.53203 | 427/784 | 0.54715 |
| primary 30 | two_fe_dense | pw | 50 | False | robust | 857/2340 | 0.36655 | 898/2496 | 0.35861 |
| primary 31 | two_fe_dense | pw | 50 | False | cluster_fe | 837/2447 | 0.33831 | 864/2513 | 0.34228 |
| primary 32 | two_fe_dense | pw | 50 | False | cluster2_cross | 920/2820 | 0.32467 | 950/2912 | 0.32590 |
| primary 33 | two_fe_dense | pw | 50 | False | cluster2_saved | 1073/3036 | 0.35343 | 1105/3141 | 0.35233 |
| primary 34 | two_fe_dense | none | 1 | True | iid | 199/234 | 0.86076 | 201/233 | 0.87234 |
| primary 35 | two_fe_dense | none | 5 | True | iid | 233/256 | 0.92369 | 233/254 | 0.91016 |
| primary 36 | two_fe_dense | none | 50 | True | iid | 695/1125 | 0.62311 | 713/1157 | 0.61625 |
| primary 37 | two_fe_dense | aw | 6 | True | iid | 264/296 | 0.88742 | 284/319 | 0.89342 |
| primary 38 | two_fe_dense | aw | 6 | True | iid_outputs | 426/559 | 0.75801 | 444/575 | 0.76684 |
| primary 39 | two_fe_dense | aw | 50 | True | iid | 782/1321 | 0.58970 | 816/1401 | 0.58173 |
| primary 40 | two_fe_dense | aw | 50 | True | iid_outputs | 981/1528 | 0.64005 | 981/1576 | 0.61270 |
| primary 41 | strong_d2 | none | 6 | False | iid | 205/624 | 0.33119 | 209/619 | 0.33495 |
| primary 42 | strong_d2 | aw | 6 | False | iid | 221/563 | 0.39362 | 232/565 | 0.40845 |
| primary 43 | strong_d2 | none | 50 | False | iid | 633/2196 | 0.28825 | 680/2180 | 0.31193 |
| primary 44 | strong_d2 | aw | 50 | False | iid | 685/1878 | 0.36475 | 677/1895 | 0.36261 |
| primary 45 | strong_d3 | none | 6 | False | iid | 59/74 | 0.78571 | 55/74 | 0.76316 |
| primary 46 | strong_d3 | aw | 6 | False | iid | 70/85 | 0.82558 | 70/82 | 0.86420 |
| primary 47 | strong_d3 | none | 50 | False | iid | 183/278 | 0.65827 | 185/282 | 0.65233 |
| primary 48 | strong_d3 | aw | 50 | False | iid | 211/308 | 0.68506 | 209/305 | 0.68977 |
| primary 49 | strong_d6 | none | 6 | False | iid | 53/65 | 0.80597 | 52/65 | 0.78462 |
| primary 50 | strong_d6 | aw | 6 | False | iid | 63/72 | 0.85915 | 61/72 | 0.86957 |
| primary 51 | strong_d6 | none | 50 | False | iid | 171/248 | 0.68254 | 170/259 | 0.66926 |
| primary 52 | strong_d6 | aw | 50 | False | iid | 193/277 | 0.69455 | 193/275 | 0.69091 |
| primary 53 | two_fe_sparse | none | 6 | False | iid | 106/790 | 0.13418 | 107/795 | 0.13459 |
| primary 54 | two_fe_sparse | aw | 6 | False | iid | 124/729 | 0.17127 | 123/725 | 0.16871 |
| primary 55 | two_fe_sparse | none | 50 | False | iid | 435/2307 | 0.18915 | 438/2310 | 0.18920 |
| primary 56 | two_fe_sparse | aw | 50 | False | iid | 472/3645 | 0.12974 | 470/3631 | 0.12944 |
| primary 57 | unbalanced | none | 6 | False | iid | 58/72 | 0.83333 | 59/73 | 0.77632 |
| primary 58 | unbalanced | aw | 6 | False | iid | 66/78 | 0.84615 | 67/80 | 0.83750 |
| primary 59 | unbalanced | none | 50 | False | iid | 180/264 | 0.67045 | 181/274 | 0.66544 |
| primary 60 | unbalanced | aw | 50 | False | iid | 208/298 | 0.70333 | 205/309 | 0.68333 |
| primary 61 | disconnected_blocks | none | 6 | False | iid | 53/166 | 0.31928 | 50/170 | 0.29878 |
| primary 62 | disconnected_blocks | aw | 6 | False | iid | 62/199 | 0.31527 | 61/196 | 0.30198 |
| primary 63 | disconnected_blocks | none | 50 | False | iid | 169/348 | 0.48563 | 169/353 | 0.48246 |
| primary 64 | disconnected_blocks | aw | 50 | False | iid | 193/374 | 0.51482 | 193/377 | 0.50806 |
| primary 65 | weak_d3 | none | 6 | False | iid | 113/152 | 0.73377 | 111/154 | 0.70779 |
| primary 66 | weak_d3 | aw | 6 | False | iid | 164/217 | 0.76147 | 166/212 | 0.78571 |
| primary 67 | weak_d3 | none | 50 | False | iid | 408/595 | 0.68439 | 407/598 | 0.68896 |
| primary 68 | weak_d3 | aw | 50 | False | iid | 619/709 | 0.87358 | 623/717 | 0.86926 |
| primary 69 | bottleneck_ring | none | 6 | False | iid | 297/2600 | 0.11397 | 292/2593 | 0.11257 |
| primary 70 | bottleneck_ring | aw | 6 | False | iid | 196/1999 | 0.09774 | 195/1997 | 0.09711 |
| primary 71 | bottleneck_ring | none | 50 | False | iid | 1001/8029 | 0.12467 | 1007/8022 | 0.12542 |
| primary 72 | bottleneck_ring | aw | 50 | False | iid | 845/7876 | 0.10703 | 851/7905 | 0.10795 |
| primary 73 | cycle | none | 6 | False | iid | 147/3941 | 0.03741 | 147/3933 | 0.03705 |
| primary 74 | cycle | aw | 6 | False | iid | 201/3242 | 0.06204 | 200/3240 | 0.06183 |
| primary 75 | cycle | none | 50 | False | iid | 566/11809 | 0.04788 | 560/11859 | 0.04752 |
| primary 76 | cycle | aw | 50 | False | iid | 664/11940 | 0.05561 | 668/11933 | 0.05621 |
| holdout 0 | two_fe_dense | none | 1 | True | iid | 201/226 | 0.88938 | 199/221 | 0.90045 |
| holdout 1 | two_fe_dense | none | 5 | True | iid | 229/250 | 0.91304 | 225/250 | 0.91200 |
| holdout 2 | two_fe_dense | none | 50 | True | iid | 688/1107 | 0.62489 | 690/1127 | 0.62289 |
| holdout 3 | two_fe_dense | aw | 6 | True | iid | 268/297 | 0.89037 | 266/295 | 0.90068 |
| holdout 4 | two_fe_dense | aw | 6 | True | iid_outputs | 431/558 | 0.77362 | 425/550 | 0.77289 |
| holdout 5 | two_fe_dense | aw | 50 | True | iid | 786/1325 | 0.58657 | 787/1311 | 0.59848 |
| holdout 6 | two_fe_dense | aw | 50 | True | iid_outputs | 976/1556 | 0.63124 | 985/1547 | 0.63736 |
| holdout 7 | strong_d3 | none | 6 | False | iid | 56/70 | 0.77143 | 57/72 | 0.80263 |
| holdout 8 | strong_d3 | aw | 6 | False | iid | 70/80 | 0.86250 | 69/80 | 0.85882 |
| holdout 9 | strong_d6 | none | 6 | False | iid | 52/66 | 0.77941 | 54/66 | 0.83333 |
| holdout 10 | strong_d6 | aw | 6 | False | iid | 64/71 | 0.86486 | 63/74 | 0.85135 |
| holdout 11 | unbalanced | none | 6 | False | iid | 59/74 | 0.79730 | 58/75 | 0.78205 |
| holdout 12 | unbalanced | aw | 6 | False | iid | 69/78 | 0.90000 | 68/81 | 0.83951 |
| holdout 13 | weak_d3 | aw | 6 | False | iid | 166/201 | 0.81095 | 167/203 | 0.83085 |
| holdout 14 | bottleneck_ring | aw | 6 | False | iid | 195/3489 | 0.05592 | 196/3487 | 0.05621 |
| holdout 15 | cycle | aw | 6 | False | iid | 197/3402 | 0.05791 | 195/3383 | 0.05764 |

`verify.py` checks portable hashes, reconstructs the complete captured source,
binds independent auditor sources and verifies cached-oracle provenance. Large
output arrays stay local with explicit hashes. Raw licensed logs and compiled
binaries are excluded. No release, push or installation is performed.
