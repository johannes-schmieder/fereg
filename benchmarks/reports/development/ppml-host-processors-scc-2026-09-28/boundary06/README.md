# Host-setting boundary diagnostic

Job 7763667, after headline 7763641, uses the same rejected candidate package d2120024 and plugin d007491a for every configuration. Four native workers are required throughout. Seed 2026092817, N30,720, K6, robust inference; 12 repeated calls per process. No stock comparison and no independent per-fit Poisson oracle are part of this diagnostic. Native acceptance, within-process b/V equality, exact cross-mode final b/V equality, host-setting assertions and both sentinels pass for all 96 calls.

| Design / host policy | Command ms min / median / max | Process seconds | Partial Stata self samples |
|---|---:|---:|---:|
| one_fe-scoped-native | 149.0 / 213.0 / 259.0 | 2.974 | 67.39% |
| one_fe-host1-persistent | 130.0 / 135.5 / 187.0 | 2.021 | 8.63% |
| one_fe-scoped-public | 121.0 / 134.5 / 188.0 | 2.021 | 8.60% |
| one_fe-passive-scoped | 116.0 / 127.0 / 169.0 | 1.921 | 8.47% |
| two_fe_dense-scoped-native | 218.0 / 274.5 / 329.0 | 3.625 | 62.42% |
| two_fe_dense-host1-persistent | 164.0 / 172.0 / 234.0 | 2.522 | 3.80% |
| two_fe_dense-scoped-public | 174.0 / 185.5 / 239.0 | 2.623 | 4.12% |
| two_fe_dense-passive-scoped | 162.0 / 179.0 / 237.0 | 2.572 | 5.38% |

`scoped-native` changes Stata processors immediately around the plugin, as in the rejected candidate. `host1-persistent` keeps Stata at one processor outside all public calls. `scoped-public` sets one processor before each entire public command and restores four afterward, with both setting calls inside its diagnostic command clock. `passive-scoped` runs the candidate with OMP_WAIT_POLICY=PASSIVE and KMP_BLOCKTIME=0 supplied before launching Stata. No environment policy is adopted.

Moving the scope to encompass the whole public command nearly reproduces persistent-host1 performance. The measured Stata CPU share also falls. This supports the inference that earlier Stata control-plane work activates competing workers before the native-only scope begins. It does not isolate the exact activation command or prove a public wrapper safe. The next candidate should cover public parsing/temporary-output preparation and restore the caller setting across syntax errors, native errors, Break and final publication. Keep the native thread request unchanged.

These are fixed-order CPU profiles on a shared node, not uncontended alternating headline timings. Startup/loading and instrumentation are included in process elapsed time. Partial self percentages count only symbols above 0.5%; they are not complete causal decompositions. Fresh full-command confirmation and direct scaling remain required. Raw licensed logs and perf binaries remain remote; collected synthetic exports and traces match their remote hashes.
