# Exact-input slope adjudication passed

Job 7747280 completed on SCC with failed=0/exit_status=0, 28 slots, 1,934s and
2.973G maxvmem. `PILOT_ADJUDICATION_COMPLETE True` is present. Source 925c4b33,
parent estimator/controller 58284f76 and the original frozen runtime are bound by
the launch/result receipts. Every regenerated DTA and column hash matched the
captured pilot input. No native estimation was repeated.

All three independent oracles and both comparisons for each case pass:

| Captured case, N61,440 | Native max coefficient error | New reference max coefficient error | Independent model rank |
|---|---:|---:|---:|
| OLS three-FE slopes,K10/T28/fast |8.89e-16|4.67e-12 (reghdfe)|38,711|
| PPML three-FE slopes,K10/T28/fast |5.20e-15|4.23e-12 (strict fixest)|38,711|
| PPML three-FE slopes,K6/T4/low |2.81e-14|5.13e-12 (strict fixest)|38,707|

All full covariance, sample, objective and applicable model-rank/residual-DoF
checks pass. Independent FE rank is 38,701 in all cases. Maximum scaled Poisson
scores are 6.73e-13/7.05e-13, projection defects below 1.43e-13 and inverse-Hessian
defects below 2.23e-15. The oracle independently reconstructs model coordinates;
its final Hessian uses newly factored final weights. Raw full numeric oracles
and reference outputs, their hashes and the unchanged acceptance envelopes are
retained in the adjacent receipts. This does not relax any scientific threshold.

The extended command budgets resolved the missing references: reghdfe took
734.694s; strict stock PPML fixest took 322.253s/517.104s. The original 120s timeouts
remain true statements about those attempts. New diagnostic clocks are not
paired with the original native clocks, and the default-reference discrepancies
are not reclassified as passing. These elapsed times inform resource planning
only. The existing ppmlhdfe initialization failure was not retried.

ADR0226 permits a separately bound production-readiness gate without altering
the rejected pilot, frozen controller or per-pair eligibility. Eight focused
local tests pass. A read-only SCC integration check on the actual receipts
reports `ADJUDICATION_GATE_CHECK_PASS 12 3`. It publishes no production approval
or plan by itself. Production launch will be captured separately; STATUS.md is
the current resume point. This is development evidence, not release qualification.
