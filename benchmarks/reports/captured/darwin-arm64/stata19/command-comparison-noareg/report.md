# Cross-platform Stata command comparison

Warm medians; ratios below 1 mean fereg is faster.

## fit

| Scale | Design | Threads | fereg | Greg | reghdfe |
|---|---|---:|---:|---:|---:|
| n100k | akm_highmob | 1 | 0.1630 | 0.0390 | 0.2630 |
| n100k | akm_highmob | 8 | 0.0860 | 0.0380 | 0.1900 |
| n100k | akm_lowmob | 1 | 0.0800 | 0.1690 | 0.4630 |
| n100k | akm_lowmob | 8 | 0.0630 | 0.1770 | 0.2260 |
| n100k | akm_standard | 1 | 0.0840 | 0.0700 | 0.2690 |
| n100k | akm_standard | 8 | 0.0650 | 0.0680 | 0.1640 |
| n100k | iid_two_fe | 1 | 0.1770 | 0.0350 | 0.2710 |
| n100k | iid_two_fe | 8 | 0.0910 | 0.0340 | 0.2040 |
| n100k | one_fe | 1 | 0.0460 | 0.0170 | 0.0500 |
| n100k | one_fe | 8 | 0.0410 | 0.0120 | 0.0290 |
| n10m | akm_lowmob | 1 | 8.0040 | 24.9590 | 66.2380 |
| n10m | akm_lowmob | 8 | 4.0760 | 24.9330 | 27.6730 |
| n10m | akm_standard | 1 | 8.1870 | 6.7040 | 28.7620 |
| n10m | akm_standard | 8 | 4.2890 | 6.4910 | 16.6100 |
| n10m | iid_two_fe | 1 | 15.8550 | 3.3080 | 27.0970 |
| n10m | iid_two_fe | 8 | 7.4090 | 3.0980 | 20.6960 |
| n10m | one_fe | 1 | 2.8730 | 0.9620 | 2.8950 |
| n10m | one_fe | 8 | 1.7150 | 0.7570 | 1.5500 |
| n1m | akm_lowmob | 1 | 0.6660 | 2.3200 | 5.9940 |
| n1m | akm_lowmob | 8 | 0.3960 | 2.3200 | 2.6540 |
| n1m | akm_standard | 1 | 0.7770 | 0.7130 | 2.8650 |
| n1m | akm_standard | 8 | 0.4330 | 0.7120 | 1.6160 |
| n1m | iid_two_fe | 1 | 1.5700 | 0.3060 | 2.5930 |
| n1m | iid_two_fe | 8 | 0.7240 | 0.3210 | 2.1190 |
| n1m | one_fe | 1 | 0.2930 | 0.0980 | 0.2970 |
| n1m | one_fe | 8 | 0.1820 | 0.0790 | 0.1660 |

## save_fe

| Scale | Design | Threads | fereg | Greg | reghdfe |
|---|---|---:|---:|---:|---:|
| n100k | akm_highmob | 1 | 0.1780 | 0.0540 | 0.3090 |
| n100k | akm_highmob | 8 | 0.0970 | 0.0540 | 0.2170 |
| n100k | akm_lowmob | 1 | 0.0930 | 0.6630 | 0.7200 |
| n100k | akm_lowmob | 8 | 0.0750 | 0.7080 | 0.3410 |
| n100k | akm_standard | 1 | 0.0950 | 0.1010 | 0.3480 |
| n100k | akm_standard | 8 | 0.0760 | 0.0990 | 0.2040 |
| n100k | iid_two_fe | 1 | 0.1830 | 0.0460 | 0.3080 |
| n100k | iid_two_fe | 8 | 0.1020 | 0.0450 | 0.2270 |
| n100k | one_fe | 1 | 0.0610 | 0.0230 | 0.0700 |
| n100k | one_fe | 8 | 0.0420 | 0.0170 | 0.0380 |
| n10m | akm_lowmob | 1 | 9.4740 | 102.3830 | 97.7010 |
| n10m | akm_lowmob | 8 | 5.5420 | 101.3780 | 39.9690 |
| n10m | akm_standard | 1 | 9.8040 | 10.0690 | 38.2050 |
| n10m | akm_standard | 8 | 5.7810 | 9.8280 | 21.0880 |
| n10m | iid_two_fe | 1 | 17.7590 | 4.6170 | 32.0450 |
| n10m | iid_two_fe | 8 | 8.9040 | 4.3630 | 23.4740 |
| n10m | one_fe | 1 | 4.0800 | 1.3010 | 4.5810 |
| n10m | one_fe | 8 | 2.7970 | 1.0920 | 2.4490 |
| n1m | akm_lowmob | 1 | 0.8150 | 9.8550 | 8.8900 |
| n1m | akm_lowmob | 8 | 0.5360 | 9.9510 | 3.8950 |
| n1m | akm_standard | 1 | 0.9230 | 1.0290 | 3.6880 |
| n1m | akm_standard | 8 | 0.5620 | 1.0070 | 2.0150 |
| n1m | iid_two_fe | 1 | 1.6850 | 0.4120 | 2.9970 |
| n1m | iid_two_fe | 8 | 0.8490 | 0.4080 | 2.3060 |
| n1m | one_fe | 1 | 0.4110 | 0.1300 | 0.4590 |
| n1m | one_fe | 8 | 0.2870 | 0.1120 | 0.2530 |

## Aggregate fereg/comparator ratios

| Mode | Comparator | Threads | Cells | Geometric mean | Faster cells | Slower cells |
|---|---|---:|---:|---:|---:|---:|
| fit | greg | 1 | 13 | 1.684 | 3 | 10 |
| fit | greg | 8 | 13 | 1.063 | 6 | 7 |
| fit | reghdfe | 1 | 13 | 0.402 | 13 | 0 |
| fit | reghdfe | 8 | 13 | 0.402 | 10 | 3 |
| save_fe | greg | 1 | 13 | 1.128 | 6 | 7 |
| save_fe | greg | 8 | 13 | 0.746 | 6 | 7 |
| save_fe | reghdfe | 1 | 13 | 0.356 | 13 | 0 |
| save_fe | reghdfe | 8 | 13 | 0.390 | 10 | 3 |

## fereg saved-FE overhead

| Scale | Design | Threads | Save/fit | Increment |
|---|---|---:|---:|---:|
| n100k | akm_highmob | 1 | 1.092 | 0.0150s |
| n100k | akm_highmob | 8 | 1.128 | 0.0110s |
| n100k | akm_lowmob | 1 | 1.162 | 0.0130s |
| n100k | akm_lowmob | 8 | 1.190 | 0.0120s |
| n100k | akm_standard | 1 | 1.131 | 0.0110s |
| n100k | akm_standard | 8 | 1.169 | 0.0110s |
| n100k | iid_two_fe | 1 | 1.034 | 0.0060s |
| n100k | iid_two_fe | 8 | 1.121 | 0.0110s |
| n100k | one_fe | 1 | 1.326 | 0.0150s |
| n100k | one_fe | 8 | 1.024 | 0.0010s |
| n10m | akm_lowmob | 1 | 1.184 | 1.4700s |
| n10m | akm_lowmob | 8 | 1.360 | 1.4660s |
| n10m | akm_standard | 1 | 1.198 | 1.6170s |
| n10m | akm_standard | 8 | 1.348 | 1.4920s |
| n10m | iid_two_fe | 1 | 1.120 | 1.9040s |
| n10m | iid_two_fe | 8 | 1.202 | 1.4950s |
| n10m | one_fe | 1 | 1.420 | 1.2070s |
| n10m | one_fe | 8 | 1.631 | 1.0820s |
| n1m | akm_lowmob | 1 | 1.224 | 0.1490s |
| n1m | akm_lowmob | 8 | 1.354 | 0.1400s |
| n1m | akm_standard | 1 | 1.188 | 0.1460s |
| n1m | akm_standard | 8 | 1.298 | 0.1290s |
| n1m | iid_two_fe | 1 | 1.073 | 0.1150s |
| n1m | iid_two_fe | 8 | 1.173 | 0.1250s |
| n1m | one_fe | 1 | 1.403 | 0.1180s |
| n1m | one_fe | 8 | 1.577 | 0.1050s |

## Independent science checks

All preflights passed. The table reports the largest observed distance.

| Check | Maximum |
|---|---:|
| fereg slope max | 7.266e-14 |
| fereg VCE max | 2.609e-19 |
| fereg relative RSS max | 1.231e-13 |
| fereg reconstruction max | 1.764e-10 |
| reghdfe reconstruction max | 6.920e-04 |
| fereg/reghdfe predictor max | 6.915e-04 |

## Registered limitations

- `platform_threading`: Both registered platforms use Stata 19. The macOS matrix uses 1 and 8 application threads; the SCC Linux license permits 1 and 4 application threads.
- `greg_science`: greg is checked for sample and slope parity, but remains diagnostic because its beta Mata interface does not expose the full e-class fit contract.
- `saved_fe_cross_command`: Each command's saved predictor is checked against its own residual identity. Cross-command saved-predictor distances remain diagnostic because reghdfe uses a weaker saved-FE stopping rule than fereg.
