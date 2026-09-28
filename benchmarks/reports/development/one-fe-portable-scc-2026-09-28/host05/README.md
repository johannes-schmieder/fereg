# Embedding-worker diagnostic — separate from headline timing

Job 7763580 runs only after fresh timing 7763579 completes and the wrapper validates its complete summary. Source is the rejected inlining candidate cda0912f, Linux plugin 570eedc0; these are diagnostic results on that artifact, not production timings. Four slots, E5-2680v4, 12 repeated N30720/K6 robust fits per process. The diagnostic/tuning seed remains 2026092817. Every fit asserts convergence, four effective native workers and the registered Stata processor count.

Normal: four Stata processors, four native workers, unchanged wait environment. Passive: same processor counts, with OMP_WAIT_POLICY=PASSIVE and KMP_BLOCKTIME=0 set before launching Stata. Host1: one Stata processor, normal wait environment, four native workers. All use exactly the same source, plugin and per-design generated input. All 72 applications and both sentinels pass. Accounting and file hashes are in receipt.json.

| Configuration | Whole diagnostic process seconds |
|---|---:|
| one_fe-default | 4.328 |
| one_fe-passive | 1.922 |
| one_fe-host1 | 2.021 |
| two_fe_dense-default | 3.927 |
| two_fe_dense-passive | 2.785 |
| two_fe_dense-host1 | 2.823 |

The reported Stata symbols above the 0.5% self-sample cutoff sum to 54.38% for ordinary one-FE, 7.56% passive, 9.79% host1; two-FE sums are 55.89%, 4.87%, 6.43%. These partial CPU shares omit symbols below the cutoff. The evidence supports embedding workers competing with native workers as a substantial cause worth testing. It does not establish a production speedup or an exact attribution of all remaining time.

Do not rank these elapsed clocks against stock or previous headlines. This is one fixed-order CPU-profiling block on a shared node, with twelve same-input calls amortizing load/startup. It changes the host configuration before the repetition loop and omits per-call change/restoration cost. No per-fit exported coefficient/VCE comparison was performed here; native acceptance and thread assertions are diagnostic checks. Environment changes affect the whole Stata process; no such change is adopted.

Next: test normal baseline versus a scoped, reversible host-processor policy on accepted source 5c0d4685. Include host change/restoration in the public clock, prove restoration on error/Break/panic and preserve explicit/default native requests. First check Stata processor changes can be used safely with one guarded native call and main-thread SPI. Then measure directly alternating T1/T2/T4 requests, wider/large/low/OLS controls and stock under normal settings. If the reversible policy is too costly, preserve that rejection and examine a safe wait-policy interface instead of changing global process state silently.
