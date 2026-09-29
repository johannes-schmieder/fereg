# Lighter systematic development benchmarks

Candidate `161b9491b2df535ae88f7a38dbc83c9875a30861`; 568/568 complete groups. Status: complete. Independent smoke accepted: True.

Four warm repetitions per configuration; eight cold anchor groups. Command timers exclude generation, import, initialization and extraction; whole-arm elapsed time is separate. OLS requests IID inference; PPML robust inference. This campaign does not time fitted-mean publication. Each pair runs sequentially on one reserved 28-core E5-2680v4 node; Stata control work remains license-capped. RSS is absolute process-tree peak, sampled every 10 ms. All generated inputs are synthetic.

Comparisons require successful commands, identical input and exact retained sample, coefficient/full covariance/objective checks, native convergence and accepted independent smoke. Eligibility is pair-specific development evidence: a failed or excluded ppmlhdfe arm does not disqualify a passing fixest pair. Default fixest and fixest_strict (glm.tol=1e-10, fixef.tol=1e-11) are separate. No tolerances were changed. gtools arms without exact samples remain diagnostic. This is not qualification.

Summary rows require all four valid warm repetitions. Ratios below one favor fereg; min/max and quartiles describe repetitions, not confidence intervals. All application failures, exclusions, scientific discrepancies and missing groups remain in the CSV files. Historical exclusions are budget decisions, not new observed failures.

Arm status counts: {'pass': 1370, 'command_timeout': 8, 'budget_exclusion': 50}

| Model | Comparator | Complete configurations | Median ratio across configurations | Range of configuration medians | Repetition losses |
|---|---|---:|---:|---:|---:|
| ols | fixest | 44 | 1.133 | 0.401–7.320 | 102 |
| ppml | fixest | 52 | 0.827 | 0.231–2.800 | 75 |
| ppml | fixest_strict | 58 | 0.548 | 0.019–1.555 | 34 |
