# Cross-platform Stata command comparison

Warm medians; ratios below 1 mean fereg is faster.

## fit

| Scale | Design | Threads | fereg | Greg | reghdfe |
|---|---|---:|---:|---:|---:|
| n100k | akm_highmob | 1 | 0.1970 | 0.0470 | 0.4790 |
| n100k | akm_highmob | 4 | 0.1460 | 0.0470 | 0.3900 |
| n100k | akm_lowmob | 1 | 0.1430 | 0.1220 | 0.9490 |
| n100k | akm_lowmob | 4 | 0.1320 | 0.1230 | 0.4990 |
| n100k | akm_standard | 1 | 0.1430 | 0.0610 | 0.5070 |
| n100k | akm_standard | 4 | 0.1240 | 0.0620 | 0.3390 |
| n100k | iid_two_fe | 1 | 0.2140 | 0.0440 | 0.5040 |
| n100k | iid_two_fe | 4 | 0.1530 | 0.0430 | 0.4100 |
| n100k | one_fe | 1 | 0.0750 | 0.0190 | 0.0750 |
| n100k | one_fe | 4 | 0.0690 | 0.0180 | 0.0550 |
| n10m | akm_lowmob | 1 | 14.2190 | 31.6560 | 164.2340 |
| n10m | akm_lowmob | 4 | 9.6550 | 31.4310 | 82.3900 |
| n10m | akm_standard | 1 | 19.8630 | 11.0660 | 64.7200 |
| n10m | akm_standard | 4 | 12.2760 | 10.7610 | 40.1950 |
| n10m | iid_two_fe | 1 | 44.7840 | 8.5600 | 55.9210 |
| n10m | iid_two_fe | 4 | 24.9600 | 8.2800 | 43.5920 |
| n10m | one_fe | 1 | 6.9300 | 1.3710 | 6.3310 |
| n10m | one_fe | 4 | 5.3470 | 1.1190 | 4.0330 |
| n1m | akm_lowmob | 1 | 1.2300 | 2.0000 | 12.7130 |
| n1m | akm_lowmob | 4 | 0.8770 | 1.9720 | 6.2090 |
| n1m | akm_standard | 1 | 1.2560 | 0.7400 | 5.6760 |
| n1m | akm_standard | 4 | 0.8880 | 0.7380 | 3.5260 |
| n1m | iid_two_fe | 1 | 2.5290 | 0.4680 | 5.0590 |
| n1m | iid_two_fe | 4 | 1.5060 | 0.4390 | 3.9440 |
| n1m | one_fe | 1 | 0.6710 | 0.1370 | 0.5790 |
| n1m | one_fe | 4 | 0.5010 | 0.1130 | 0.3720 |

## save_fe

| Scale | Design | Threads | fereg | Greg | reghdfe |
|---|---|---:|---:|---:|---:|
| n100k | akm_highmob | 1 | 0.2200 | 0.0740 | 0.5840 |
| n100k | akm_highmob | 4 | 0.1740 | 0.0810 | 0.4700 |
| n100k | akm_lowmob | 1 | 0.1670 | 0.3000 | 1.4900 |
| n100k | akm_lowmob | 4 | 0.1580 | 0.3040 | 0.8350 |
| n100k | akm_standard | 1 | 0.1700 | 0.1050 | 0.6900 |
| n100k | akm_standard | 4 | 0.1530 | 0.1070 | 0.4700 |
| n100k | iid_two_fe | 1 | 0.2380 | 0.0700 | 0.5900 |
| n100k | iid_two_fe | 4 | 0.1830 | 0.0740 | 0.4710 |
| n100k | one_fe | 1 | 0.0960 | 0.0310 | 0.1110 |
| n100k | one_fe | 4 | 0.0870 | 0.0330 | 0.0880 |
| n10m | akm_lowmob | 1 | 17.6030 | 157.5780 | 245.0350 |
| n10m | akm_lowmob | 4 | 12.8120 | 156.4110 | 135.9480 |
| n10m | akm_standard | 1 | 23.4400 | 18.3950 | 88.3190 |
| n10m | akm_standard | 4 | 15.3140 | 18.0820 | 55.0930 |
| n10m | iid_two_fe | 1 | 49.3440 | 12.5330 | 68.2680 |
| n10m | iid_two_fe | 4 | 30.0180 | 12.1850 | 52.0660 |
| n10m | one_fe | 1 | 9.5790 | 2.4670 | 10.4150 |
| n10m | one_fe | 4 | 7.5940 | 2.2070 | 6.7750 |
| n1m | akm_lowmob | 1 | 1.5350 | 12.1700 | 18.7570 |
| n1m | akm_lowmob | 4 | 1.1780 | 12.1380 | 9.6710 |
| n1m | akm_standard | 1 | 1.5570 | 1.2890 | 7.5660 |
| n1m | akm_standard | 4 | 1.1910 | 1.2680 | 4.6630 |
| n1m | iid_two_fe | 1 | 2.8590 | 0.7480 | 5.9900 |
| n1m | iid_two_fe | 4 | 1.8370 | 0.7200 | 4.5840 |
| n1m | one_fe | 1 | 0.9040 | 0.2380 | 0.9340 |
| n1m | one_fe | 4 | 0.7290 | 0.2210 | 0.6210 |

## Aggregate fereg/comparator ratios

| Mode | Comparator | Threads | Cells | Geometric mean | Faster cells | Slower cells |
|---|---|---:|---:|---:|---:|---:|
| fit | greg | 1 | 13 | 2.474 | 2 | 11 |
| fit | greg | 4 | 13 | 1.883 | 2 | 11 |
| fit | reghdfe | 1 | 13 | 0.366 | 10 | 2 |
| fit | reghdfe | 4 | 13 | 0.407 | 10 | 3 |
| save_fe | greg | 1 | 13 | 1.459 | 3 | 10 |
| save_fe | greg | 4 | 13 | 1.126 | 5 | 8 |
| save_fe | reghdfe | 1 | 13 | 0.319 | 13 | 0 |
| save_fe | reghdfe | 4 | 13 | 0.367 | 11 | 2 |

## fereg saved-FE overhead

| Scale | Design | Threads | Save/fit | Increment |
|---|---|---:|---:|---:|
| n100k | akm_highmob | 1 | 1.117 | 0.0230s |
| n100k | akm_highmob | 4 | 1.192 | 0.0280s |
| n100k | akm_lowmob | 1 | 1.168 | 0.0240s |
| n100k | akm_lowmob | 4 | 1.197 | 0.0260s |
| n100k | akm_standard | 1 | 1.189 | 0.0270s |
| n100k | akm_standard | 4 | 1.234 | 0.0290s |
| n100k | iid_two_fe | 1 | 1.112 | 0.0240s |
| n100k | iid_two_fe | 4 | 1.196 | 0.0300s |
| n100k | one_fe | 1 | 1.280 | 0.0210s |
| n100k | one_fe | 4 | 1.261 | 0.0180s |
| n10m | akm_lowmob | 1 | 1.238 | 3.3840s |
| n10m | akm_lowmob | 4 | 1.327 | 3.1570s |
| n10m | akm_standard | 1 | 1.180 | 3.5770s |
| n10m | akm_standard | 4 | 1.247 | 3.0380s |
| n10m | iid_two_fe | 1 | 1.102 | 4.5600s |
| n10m | iid_two_fe | 4 | 1.203 | 5.0580s |
| n10m | one_fe | 1 | 1.382 | 2.6490s |
| n10m | one_fe | 4 | 1.420 | 2.2470s |
| n1m | akm_lowmob | 1 | 1.248 | 0.3050s |
| n1m | akm_lowmob | 4 | 1.343 | 0.3010s |
| n1m | akm_standard | 1 | 1.240 | 0.3010s |
| n1m | akm_standard | 4 | 1.341 | 0.3030s |
| n1m | iid_two_fe | 1 | 1.130 | 0.3300s |
| n1m | iid_two_fe | 4 | 1.220 | 0.3310s |
| n1m | one_fe | 1 | 1.347 | 0.2330s |
| n1m | one_fe | 4 | 1.455 | 0.2280s |

## Independent science checks

All preflights passed. The table reports the largest observed distance.

| Check | Maximum |
|---|---:|
| fereg slope max | 2.576e-14 |
| fereg VCE max | 1.118e-19 |
| fereg relative RSS max | 2.090e-13 |
| fereg reconstruction max | 9.948e-11 |
| reghdfe reconstruction max | 6.622e-04 |
| fereg/reghdfe predictor max | 6.622e-04 |

## Registered limitations

- `platform_threading`: Both registered platforms use Stata 19. The macOS matrix uses 1 and 8 application threads; the SCC Linux license permits 1 and 4 application threads.
- `greg_science`: greg is checked for sample and slope parity, but remains diagnostic because its beta Mata interface does not expose the full e-class fit contract.
- `saved_fe_cross_command`: Each command's saved predictor is checked against its own residual identity. Cross-command saved-predictor distances remain diagnostic because reghdfe uses a weaker saved-FE stopping rule than fereg.
