# Expanded fereg command comparison

Warm medians. Requested threads are native fereg threads; the recorded Stata processor count is license/machine capped for Stata-side comparators.

## fit

| Scale | Design | K | Requested | Stata | fereg | Greg | reghdfe |
|---|---|---:|---:|---:|---:|---:|---:|
| n1m | akm_lowmob | 30 | 1 | 1 | 109.2900 | 23.3080 | 136.0670 |
| n1m | akm_lowmob_year | 30 | 16 | 4 | 28.7910 | 33.4280 | 111.7080 |
| n1m | akm_standard | 30 | 1 | 1 | 109.2920 | 7.7900 | 52.4730 |
| n1m | akm_standard | 30 | 16 | 4 | 17.3540 | 7.6470 | 27.8760 |
| n1m | akm_standard_year | 30 | 1 | 1 | 132.3130 | 10.6140 | 72.9620 |
| n1m | akm_standard_year | 30 | 16 | 4 | 28.7470 | 10.6820 | 44.9890 |
| n1m | one_fe | 0 | 1 | 1 | 0.9440 | 0.2450 | 0.6310 |
| n1m | one_fe | 0 | 16 | 4 | 0.7400 | 0.2000 | 0.4450 |
| n1m | one_fe | 10 | 1 | 1 | 17.2320 | 0.5000 | 2.7390 |
| n1m | one_fe | 10 | 16 | 4 | 0.8200 | 0.3670 | 1.3840 |
| n1m | one_fe | 30 | 1 | 1 | 104.2780 | 1.3660 | 13.8130 |
| n1m | one_fe | 30 | 16 | 4 | 14.9700 | 1.2220 | 9.7780 |

## save_fe

| Scale | Design | K | Requested | Stata | fereg | Greg | reghdfe |
|---|---|---:|---:|---:|---:|---:|---:|
| n1m | akm_lowmob | 30 | 1 | 1 | 116.0880 | 30.3410 | 145.0600 |
| n1m | akm_lowmob_year | 30 | 16 | 4 | 26.2630 | 52.6820 | 121.6130 |
| n1m | akm_standard | 30 | 1 | 1 | 115.9650 | 8.8060 | 55.5330 |
| n1m | akm_standard | 30 | 16 | 4 | 14.4070 | 10.0620 | 33.4740 |
| n1m | akm_standard_year | 30 | 1 | 1 | 139.8810 | 11.9340 | 76.8310 |
| n1m | akm_standard_year | 30 | 16 | 4 | 26.1550 | 12.0170 | 47.9210 |
| n1m | one_fe | 0 | 1 | 1 | 1.4570 | 0.5700 | 1.4030 |
| n1m | one_fe | 0 | 16 | 4 | 1.0240 | 0.6480 | 1.1920 |
| n1m | one_fe | 10 | 1 | 1 | 19.8890 | 0.8510 | 3.5490 |
| n1m | one_fe | 10 | 16 | 4 | 1.0860 | 0.5720 | 1.8020 |
| n1m | one_fe | 30 | 1 | 1 | 111.2390 | 1.8630 | 14.8240 |
| n1m | one_fe | 30 | 16 | 4 | 11.5710 | 1.6860 | 10.3260 |

## Aggregate fereg/comparator ratios

| Mode | Comparator | Requested | Cells | Geometric mean | Faster | Slower |
|---|---|---:|---:|---:|---:|---:|
| fit | greg | 1 | 6 | 14.233 | 0 | 6 |
| fit | greg | 16 | 6 | 2.847 | 1 | 5 |
| fit | reghdfe | 1 | 6 | 2.449 | 1 | 5 |
| fit | reghdfe | 16 | 6 | 0.733 | 4 | 2 |
| save_fe | greg | 1 | 6 | 11.322 | 0 | 6 |
| save_fe | greg | 16 | 6 | 1.782 | 1 | 5 |
| save_fe | reghdfe | 1 | 6 | 2.259 | 1 | 5 |
| save_fe | reghdfe | 16 | 6 | 0.556 | 5 | 1 |

## fereg saved-FE overhead

| Scale | Design | K | Requested | Save/fit | Increment |
|---|---|---:|---:|---:|---:|
| n1m | akm_lowmob | 30 | 1 | 1.062 | 6.7980s |
| n1m | akm_lowmob_year | 30 | 16 | 0.912 | -2.5280s |
| n1m | akm_standard | 30 | 1 | 1.061 | 6.6730s |
| n1m | akm_standard | 30 | 16 | 0.830 | -2.9470s |
| n1m | akm_standard_year | 30 | 1 | 1.057 | 7.5680s |
| n1m | akm_standard_year | 30 | 16 | 0.910 | -2.5920s |
| n1m | one_fe | 0 | 1 | 1.543 | 0.5130s |
| n1m | one_fe | 0 | 16 | 1.384 | 0.2840s |
| n1m | one_fe | 10 | 1 | 1.154 | 2.6570s |
| n1m | one_fe | 10 | 16 | 1.324 | 0.2660s |
| n1m | one_fe | 30 | 1 | 1.067 | 6.9610s |
| n1m | one_fe | 30 | 16 | 0.773 | -3.3990s |

## Registered limitations

- `platform`: This round qualifies macOS and Linux/SCC only; Windows remains deferred.
- `greg_threads`: Greg exposes no documented per-call thread option. It is repeated on every requested-thread row, but its effective native thread count is uncontrolled and is reported as such.
- `stata_license`: reghdfe and Stata-side work are capped at the licensed/machine processor limit. fereg receives the requested native thread count through 16 independently of the Stata license.
- `saved_fe_cross_command`: Each command is checked against its own residual identity. Cross-command saved-predictor distances remain diagnostic because reghdfe uses a weaker saved-FE stopping rule than fereg.

## Focused ADR-0013 qualification

This is a 12-scope Linux/SCC qualification of exact source `93292b94f84aee7fced05dcd6985037765dfeb3f`, not the exhaustive 116-scope release matrix. It covers one, two, and three fixed-effect dimensions; 0, 10, and 30 controls; standard and low-mobility graphs; fit and saved-FE modes; and requested native threads 1 and 16.
