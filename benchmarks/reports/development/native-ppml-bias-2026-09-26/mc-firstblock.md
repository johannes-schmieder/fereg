# First fresh-seed Monte Carlo block

This is a partial confirmation receipt: 20 of the planned 300 draws per design,
140 independent datasets and 280 native/legacy attempts. It uses the frozen v04
binary; see [artifact binding and guard-only v05 change](README.md). The remaining
draws and exact/1024-probe subsets are not represented here. These small-sample
summaries are descriptive, not a completed confirmation decision.

All attempts completed with return code zero and both FEVC populations succeeded.
Four fixed_3840 legacy fits used fallback values (two full and two A fits).
No native fit failed or used a fallback. Joint-interior results omit those four
paired draws only from that explicitly named comparison population; the full
attempt denominator and all-available summaries retain them.

Below, bias is the mean estimate-minus-truth error; parentheses contain its
Monte Carlo standard error. All values use paired available draws. Split Branch
moment 1 is undefined, so its ordinary plug-in result is displayed explicitly.
The machine-readable [receipt](mc-firstblock.json) additionally reports raw Split
Branch, FEVC M/MS wage moments 1/2/5, all joint-interior comparisons, RMSE MCSE,
median error, 5th/95th error percentiles and maximum absolute error.

No universal zero-bias or RMSE-improvement requirement is imposed. Differences
in the fixed_3840 all-available pipeline include the legacy fallback behavior
and cannot be attributed solely to coefficient mean correction.

## duration_960

| Moment | Native bias (MCSE) | Legacy bias (MCSE) | Native RMSE | Legacy RMSE | n |
|:---|---:|---:|---:|---:|---:|
| 1: var(aw) (plug-in) | 0.07460 (0.00745) | 0.07460 (0.00745) | 0.08135 | 0.08135 | 20 |
| 2: cov(aw,pw) | -0.00252 (0.00171) | -0.00252 (0.00171) | 0.00788 | 0.00788 | 20 |
| 3: cov(aw,as) | 0.00444 (0.00492) | 0.00446 (0.00490) | 0.02188 | 0.02182 | 20 |
| 4: cov(aw,ps) | -0.00016 (0.00345) | -0.00014 (0.00343) | 0.01502 | 0.01494 | 20 |
| 5: var(pw) | -0.00678 (0.00525) | -0.00678 (0.00525) | 0.02385 | 0.02385 | 20 |
| 6: cov(pw,as) | -0.00243 (0.00313) | -0.00241 (0.00312) | 0.01385 | 0.01379 | 20 |
| 7: cov(pw,ps) | -0.00329 (0.00396) | -0.00329 (0.00394) | 0.01757 | 0.01750 | 20 |
| 8: var(as) | 0.00481 (0.00501) | 0.00486 (0.00496) | 0.02235 | 0.02215 | 20 |
| 9: cov(as,ps) | 0.00105 (0.00175) | 0.00107 (0.00176) | 0.00769 | 0.00773 | 20 |
| 10: var(ps) | -0.00406 (0.00449) | -0.00404 (0.00445) | 0.01997 | 0.01983 | 20 |

## fixed_240

| Moment | Native bias (MCSE) | Legacy bias (MCSE) | Native RMSE | Legacy RMSE | n |
|:---|---:|---:|---:|---:|---:|
| 1: var(aw) (plug-in) | 0.04478 (0.01112) | 0.04478 (0.01112) | 0.06597 | 0.06597 | 20 |
| 2: cov(aw,pw) | 0.00213 (0.00215) | 0.00213 (0.00215) | 0.00961 | 0.00961 | 20 |
| 3: cov(aw,as) | -0.02951 (0.01418) | -0.02913 (0.01420) | 0.06849 | 0.06839 | 20 |
| 4: cov(aw,ps) | 0.00957 (0.01217) | 0.00907 (0.01231) | 0.05390 | 0.05440 | 20 |
| 5: var(pw) | 0.01624 (0.00687) | 0.01624 (0.00687) | 0.03407 | 0.03407 | 20 |
| 6: cov(pw,as) | -0.00509 (0.00849) | -0.00473 (0.00858) | 0.03737 | 0.03769 | 20 |
| 7: cov(pw,ps) | 0.02356 (0.01302) | 0.02299 (0.01310) | 0.06146 | 0.06155 | 20 |
| 8: var(as) | -0.04070 (0.01939) | -0.04019 (0.01948) | 0.09383 | 0.09393 | 20 |
| 9: cov(as,ps) | 0.00405 (0.00604) | 0.00365 (0.00610) | 0.02662 | 0.02684 | 20 |
| 10: var(ps) | 0.02413 (0.02136) | 0.02332 (0.02157) | 0.09617 | 0.09686 | 20 |

## fixed_3840

| Moment | Native bias (MCSE) | Legacy bias (MCSE) | Native RMSE | Legacy RMSE | n |
|:---|---:|---:|---:|---:|---:|
| 1: var(aw) (plug-in) | 0.06583 (0.00252) | 0.06583 (0.00252) | 0.06674 | 0.06674 | 20 |
| 2: cov(aw,pw) | 0.00122 (0.00066) | 0.00122 (0.00066) | 0.00314 | 0.00314 | 20 |
| 3: cov(aw,as) | -0.00227 (0.00420) | -0.01655 (0.01147) | 0.01847 | 0.05267 | 20 |
| 4: cov(aw,ps) | 0.00388 (0.00308) | -0.00763 (0.00836) | 0.01396 | 0.03722 | 20 |
| 5: var(pw) | 0.00144 (0.00185) | 0.00144 (0.00185) | 0.00818 | 0.00818 | 20 |
| 6: cov(pw,as) | -0.00053 (0.00244) | -0.00901 (0.00658) | 0.01063 | 0.03008 | 20 |
| 7: cov(pw,ps) | 0.00323 (0.00290) | -0.00618 (0.00663) | 0.01303 | 0.02954 | 20 |
| 8: var(as) | -0.00449 (0.00561) | -0.02834 (0.01829) | 0.02488 | 0.08461 | 20 |
| 9: cov(as,ps) | 0.00157 (0.00114) | -0.01564 (0.01196) | 0.00522 | 0.05445 | 20 |
| 10: var(ps) | 0.00457 (0.00404) | -0.01287 (0.01181) | 0.01820 | 0.05308 | 20 |

## fixed_960

| Moment | Native bias (MCSE) | Legacy bias (MCSE) | Native RMSE | Legacy RMSE | n |
|:---|---:|---:|---:|---:|---:|
| 1: var(aw) (plug-in) | 0.06527 (0.00605) | 0.06527 (0.00605) | 0.07041 | 0.07041 | 20 |
| 2: cov(aw,pw) | 0.00098 (0.00122) | 0.00098 (0.00122) | 0.00541 | 0.00541 | 20 |
| 3: cov(aw,as) | -0.00715 (0.00941) | -0.00730 (0.00940) | 0.04165 | 0.04160 | 20 |
| 4: cov(aw,ps) | 0.00564 (0.00638) | 0.00563 (0.00637) | 0.02839 | 0.02835 | 20 |
| 5: var(pw) | 0.00235 (0.00442) | 0.00235 (0.00442) | 0.01942 | 0.01942 | 20 |
| 6: cov(pw,as) | -0.00199 (0.00350) | -0.00205 (0.00350) | 0.01539 | 0.01539 | 20 |
| 7: cov(pw,ps) | 0.00676 (0.00745) | 0.00672 (0.00744) | 0.03319 | 0.03313 | 20 |
| 8: var(as) | -0.00888 (0.01216) | -0.00906 (0.01214) | 0.05375 | 0.05367 | 20 |
| 9: cov(as,ps) | 0.00080 (0.00312) | 0.00079 (0.00313) | 0.01361 | 0.01365 | 20 |
| 10: var(ps) | 0.01157 (0.01074) | 0.01146 (0.01072) | 0.04822 | 0.04811 | 20 |

## groups_3840

| Moment | Native bias (MCSE) | Legacy bias (MCSE) | Native RMSE | Legacy RMSE | n |
|:---|---:|---:|---:|---:|---:|
| 1: var(aw) (plug-in) | 0.06992 (0.00493) | 0.06992 (0.00493) | 0.07314 | 0.07314 | 20 |
| 2: cov(aw,pw) | -0.00127 (0.00108) | -0.00127 (0.00108) | 0.00487 | 0.00487 | 20 |
| 3: cov(aw,as) | 0.01248 (0.01022) | 0.01221 (0.01029) | 0.04628 | 0.04649 | 20 |
| 4: cov(aw,ps) | -0.00869 (0.00828) | -0.00838 (0.00835) | 0.03711 | 0.03735 | 20 |
| 5: var(pw) | -0.00511 (0.00359) | -0.00511 (0.00359) | 0.01647 | 0.01647 | 20 |
| 6: cov(pw,as) | 0.00301 (0.00597) | 0.00283 (0.00601) | 0.02621 | 0.02634 | 20 |
| 7: cov(pw,ps) | -0.00867 (0.00629) | -0.00846 (0.00633) | 0.02877 | 0.02887 | 20 |
| 8: var(as) | 0.01780 (0.01588) | 0.01764 (0.01600) | 0.07146 | 0.07193 | 20 |
| 9: cov(as,ps) | -0.00671 (0.00485) | -0.00681 (0.00487) | 0.02216 | 0.02230 | 20 |
| 10: var(ps) | -0.00521 (0.00986) | -0.00466 (0.00995) | 0.04330 | 0.04361 | 20 |

## groups_960

| Moment | Native bias (MCSE) | Legacy bias (MCSE) | Native RMSE | Legacy RMSE | n |
|:---|---:|---:|---:|---:|---:|
| 1: var(aw) (plug-in) | 0.04746 (0.00746) | 0.04746 (0.00746) | 0.05753 | 0.05753 | 20 |
| 2: cov(aw,pw) | 0.00130 (0.00208) | 0.00130 (0.00208) | 0.00914 | 0.00914 | 20 |
| 3: cov(aw,as) | -0.03915 (0.01076) | -0.03910 (0.01074) | 0.06109 | 0.06098 | 20 |
| 4: cov(aw,ps) | 0.02122 (0.01047) | 0.02128 (0.01044) | 0.05034 | 0.05025 | 20 |
| 5: var(pw) | 0.01334 (0.00607) | 0.01334 (0.00607) | 0.02962 | 0.02962 | 20 |
| 6: cov(pw,as) | -0.01440 (0.00898) | -0.01439 (0.00902) | 0.04172 | 0.04186 | 20 |
| 7: cov(pw,ps) | 0.02790 (0.00946) | 0.02801 (0.00952) | 0.04980 | 0.05008 | 20 |
| 8: var(as) | -0.04366 (0.01365) | -0.04385 (0.01368) | 0.07380 | 0.07402 | 20 |
| 9: cov(as,ps) | 0.00223 (0.00772) | 0.00231 (0.00776) | 0.03374 | 0.03390 | 20 |
| 10: var(ps) | 0.03480 (0.02036) | 0.03497 (0.02039) | 0.09534 | 0.09552 | 20 |

## negative_960

| Moment | Native bias (MCSE) | Legacy bias (MCSE) | Native RMSE | Legacy RMSE | n |
|:---|---:|---:|---:|---:|---:|
| 1: var(aw) (plug-in) | 0.08402 (0.00964) | 0.08402 (0.00964) | 0.09395 | 0.09395 | 20 |
| 2: cov(aw,pw) | -0.00304 (0.00169) | -0.00304 (0.00169) | 0.00796 | 0.00796 | 20 |
| 3: cov(aw,as) | -0.00971 (0.00905) | -0.00963 (0.00914) | 0.04061 | 0.04097 | 20 |
| 4: cov(aw,ps) | -0.00441 (0.00913) | -0.00444 (0.00916) | 0.04003 | 0.04016 | 20 |
| 5: var(pw) | -0.00964 (0.00633) | -0.00964 (0.00633) | 0.02922 | 0.02922 | 20 |
| 6: cov(pw,as) | 0.00565 (0.00854) | 0.00573 (0.00853) | 0.03765 | 0.03761 | 20 |
| 7: cov(pw,ps) | 0.00471 (0.00505) | 0.00466 (0.00503) | 0.02250 | 0.02241 | 20 |
| 8: var(as) | -0.00526 (0.01524) | -0.00551 (0.01522) | 0.06666 | 0.06656 | 20 |
| 9: cov(as,ps) | 0.01481 (0.00819) | 0.01491 (0.00821) | 0.03863 | 0.03877 | 20 |
| 10: var(ps) | -0.02021 (0.01267) | -0.02026 (0.01265) | 0.05881 | 0.05873 | 20 |

Exact dataset/branch signatures match for every pair. Unchanged wage/FEVC
outputs differ by at most 1.93e-13 (prespecified tolerance 1e-10), consistent
with floating-point summation after FE spreading. The pending full confirmation
must be analyzed with its original settings, including failed attempts.
