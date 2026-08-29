# Expanded fereg command comparison

Warm medians. Requested threads are native fereg threads; the recorded Stata processor count is license/machine capped for Stata-side comparators.

## fit

| Scale | Design | K | Requested | Stata | fereg | Greg | reghdfe |
|---|---|---:|---:|---:|---:|---:|---:|
| n1m | akm_lowmob | 30 | 1 | 1 | 109.9310 | 22.9830 | 135.6160 |
| n1m | akm_lowmob_year | 30 | 16 | 4 | 29.5290 | 33.2920 | 113.4660 |
| n1m | akm_standard | 30 | 1 | 1 | 110.0100 | 7.8370 | 52.5490 |
| n1m | akm_standard | 30 | 16 | 4 | 17.9320 | 7.5190 | 27.7060 |
| n1m | akm_standard_year | 30 | 1 | 1 | 133.1100 | 10.7190 | 73.1590 |
| n1m | akm_standard_year | 30 | 16 | 4 | 29.2970 | 10.4950 | 45.0380 |
| n1m | one_fe | 0 | 1 | 1 | 1.3230 | 0.3570 | 0.8210 |
| n1m | one_fe | 0 | 16 | 4 | 0.2820 | 0.0810 | 0.2060 |
| n1m | one_fe | 10 | 1 | 1 | 17.3780 | 0.4940 | 2.7240 |
| n1m | one_fe | 10 | 16 | 4 | 4.3140 | 0.3980 | 1.7520 |
| n1m | one_fe | 30 | 1 | 1 | 7.3810 | 1.8750 | 15.3880 |
| n1m | one_fe | 30 | 16 | 4 | 15.6450 | 1.1180 | 9.7790 |

## save_fe

| Scale | Design | K | Requested | Stata | fereg | Greg | reghdfe |
|---|---|---:|---:|---:|---:|---:|---:|
| n1m | akm_lowmob | 30 | 1 | 1 | 116.8840 | 29.8910 | 144.0300 |
| n1m | akm_lowmob_year | 30 | 16 | 4 | 31.8970 | 52.5320 | 120.5200 |
| n1m | akm_standard | 30 | 1 | 1 | 116.9050 | 8.8310 | 55.4090 |
| n1m | akm_standard | 30 | 16 | 4 | 19.5760 | 8.5910 | 29.3680 |
| n1m | akm_standard_year | 30 | 1 | 1 | 140.9880 | 12.0050 | 76.8070 |
| n1m | akm_standard_year | 30 | 16 | 4 | 31.6710 | 11.6460 | 47.5370 |
| n1m | one_fe | 0 | 1 | 1 | 1.8470 | 0.8250 | 1.7370 |
| n1m | one_fe | 0 | 16 | 4 | 0.3930 | 0.2100 | 0.4560 |
| n1m | one_fe | 10 | 1 | 1 | 20.0240 | 0.8480 | 3.5230 |
| n1m | one_fe | 10 | 16 | 4 | 5.0500 | 0.8060 | 2.4420 |
| n1m | one_fe | 30 | 1 | 1 | 8.7030 | 2.4440 | 16.3500 |
| n1m | one_fe | 30 | 16 | 4 | 17.1870 | 1.5910 | 10.3300 |

## Aggregate fereg/comparator ratios

| Mode | Comparator | Requested | Cells | Geometric mean | Faster | Slower |
|---|---|---:|---:|---:|---:|---:|
| fit | greg | 1 | 6 | 8.681 | 0 | 6 |
| fit | greg | 16 | 6 | 3.822 | 1 | 5 |
| fit | reghdfe | 1 | 6 | 1.574 | 2 | 4 |
| fit | reghdfe | 16 | 6 | 0.916 | 3 | 3 |
| save_fe | greg | 1 | 6 | 6.968 | 0 | 6 |
| save_fe | greg | 16 | 6 | 2.795 | 1 | 5 |
| save_fe | reghdfe | 1 | 6 | 1.471 | 2 | 4 |
| save_fe | reghdfe | 16 | 6 | 0.839 | 4 | 2 |

## fereg saved-FE overhead

| Scale | Design | K | Requested | Save/fit | Increment |
|---|---|---:|---:|---:|---:|
| n1m | akm_lowmob | 30 | 1 | 1.063 | 6.9530s |
| n1m | akm_lowmob_year | 30 | 16 | 1.080 | 2.3680s |
| n1m | akm_standard | 30 | 1 | 1.063 | 6.8950s |
| n1m | akm_standard | 30 | 16 | 1.092 | 1.6440s |
| n1m | akm_standard_year | 30 | 1 | 1.059 | 7.8780s |
| n1m | akm_standard_year | 30 | 16 | 1.081 | 2.3740s |
| n1m | one_fe | 0 | 1 | 1.396 | 0.5240s |
| n1m | one_fe | 0 | 16 | 1.394 | 0.1110s |
| n1m | one_fe | 10 | 1 | 1.152 | 2.6460s |
| n1m | one_fe | 10 | 16 | 1.171 | 0.7360s |
| n1m | one_fe | 30 | 1 | 1.179 | 1.3220s |
| n1m | one_fe | 30 | 16 | 1.099 | 1.5420s |

## Registered limitations

- `platform`: This round qualifies macOS and Linux/SCC only; Windows remains deferred.
- `greg_threads`: Greg exposes no documented per-call thread option. It is repeated on every requested-thread row, but its effective native thread count is uncontrolled and is reported as such.
- `stata_license`: reghdfe and Stata-side work are capped at the licensed/machine processor limit. fereg receives the requested native thread count through 16 independently of the Stata license.
- `saved_fe_cross_command`: Each command is checked against its own residual identity. Cross-command saved-predictor distances remain diagnostic because reghdfe uses a weaker saved-FE stopping rule than fereg.

## Focused ADR-0012 qualification

This is a 12-scope Linux/SCC qualification of exact source `6be8f067f5c54e46e71e470b55beda7eb30a0ea1`, not the exhaustive 116-scope release matrix. It covers one, two, and three fixed-effect dimensions; 0, 10, and 30 controls; standard and low-mobility graphs; fit and saved-FE modes; and requested native threads 1 and 16.
