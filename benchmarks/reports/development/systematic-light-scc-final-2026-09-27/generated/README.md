# Lighter systematic development benchmarks

Candidate `58284f76a65b226e6df09a48cafa5caa68461af2`; 568/568 complete groups. Status: complete. Independent smoke accepted: True.

Four warm repetitions per configuration; eight cold anchor groups. Command timers exclude generation, import, initialization and extraction; whole-arm elapsed time is separate. OLS requests IID inference; PPML robust inference. This campaign does not time fitted-mean publication. Each pair runs sequentially on one reserved 28-core E5-2680v4 node; Stata control work remains license-capped. RSS is absolute process-tree peak, sampled every 10 ms. All generated inputs are synthetic.

Comparisons require successful commands, identical input and exact retained sample, coefficient/full covariance/objective checks, native convergence and accepted independent smoke. Eligibility is pair-specific development evidence: a failed or excluded ppmlhdfe arm does not disqualify a passing fixest pair. Default fixest and fixest_strict (glm.tol=1e-10, fixef.tol=1e-11) are separate. No tolerances were changed. gtools arms without exact samples remain diagnostic. This is not qualification.

Summary rows require all four valid warm repetitions. Ratios below one favor fereg; min/max and quartiles describe repetitions, not confidence intervals. All application failures, exclusions, scientific discrepancies and missing groups remain in the CSV files. Historical exclusions are budget decisions, not new observed failures.

Arm status counts: {'pass': 2936, 'worker_failure': 227, 'budget_exclusion': 12, 'command_timeout': 151, 'unsupported_surface': 80, 'startup_timeout': 2}

| Model | Comparator | Complete configurations | Median ratio across configurations | Range of configuration medians | Repetition losses |
|---|---|---:|---:|---:|---:|
| ols | reghdfe | 64 | 0.088 | 0.013–0.766 | 0 |
| ols | areg | 42 | 0.361 | 0.070–1.181 | 13 |
| ols | greg | 0 | — | — | — |
| ols | fixest | 44 | 1.159 | 0.402–4.495 | 98 |
| ols | pyfixest | 39 | 0.576 | 0.082–2.858 | 52 |
| ppml | ppmlhdfe | 58 | 0.117 | 0.024–0.375 | 0 |
| ppml | gglm | 0 | — | — | — |
| ppml | fixest | 51 | 1.157 | 0.267–5.751 | 118 |
| ppml | fixest_strict | 55 | 0.806 | 0.035–3.712 | 69 |
| ppml | pyfixest | 21 | 1.423 | 0.109–2.685 | 50 |
