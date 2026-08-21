# Scientific contracts and phase status

This document prevents architectural progress from being mistaken for
estimator validation.

| Contract | Current status | Earliest enablement gate |
|---|---|---|
| Candidate vs effective sample | Numeric availability/policy precedence, compact row map, and numeric drop reasons implemented; FE/cluster/singleton/separation stages not integrated | Phase 2 full source-role ingestion and composed final-sample identity tests |
| Observation weights | Actual kind and raw finite values are preserved under an explicit policy; domains, scaling, and estimator use remain disabled | Reference capture plus estimator-specific Phase 4/6 parity |
| Singleton pruning | Metadata option only | Phase 3 iterative-pruning identity tests |
| Fixed-effect absorption | Not implemented | Phase 3 one-way identity and multi-way residual certificates |
| OLS/WLS coefficients | Not implemented | Phase 4 independent-oracle and reference-parity gates |
| Robust/cluster VCE and DoF | Not implemented | Phase 4 score, meat, inclusion–exclusion, nesting, and rank gates |
| PPML IRLS | Not implemented | Phase 6 deviance, score, step-control, and inner-solve gates |
| PPML separation | Not implemented | Phase 7 method-specific deletion and certificate gates |
| FE recovery/normalization | Not implemented | Phase 4 OLS and Phase 8 PPML reconstruction gates |
| Prediction/margins/estat | Fail-closed stubs | Phase 4 OLS and Phase 8 PPML semantic-parity suites |

A successful `stfixest, selftest` establishes only that the protocol,
preprocessing primitives, and thread machinery passed their native smoke tests.
It is not evidence that an estimator exists or is correct.

## Locked numeric-preparation contract

The numeric preparation primitive has no default scientific policy. Availability
is mechanical: explicit missing-mask entries and nonfinite `f64` values are
unavailable. Candidate exclusion precedes every other check and invokes no
policy. Otherwise the order is outcome availability and policy, regressor
availability, weight availability and policy, then adjustment availability and
policy. The policy is borrowed immutably so decisions cannot depend on mutable
row history. Retained source bits and original row order are exact.

This contract deliberately defers negative PPML outcomes, exposure positivity
and log transformation, every weight-type domain and scaling rule, and invalid
value behavior outside `if`/`in` to captured reference semantics. It also
defers FE/cluster missingness, singleton deletion, and separation reason merging.
Empty and all-dropped preparation outputs are valid; estimator policies must
fail closed when their scientific contract requires observations.
