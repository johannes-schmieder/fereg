# Scientific contracts and phase status

This document prevents architectural progress from being mistaken for
estimator validation.

| Contract | Phase 0–1 status | Earliest enablement gate |
|---|---|---|
| Candidate vs effective sample | Candidate marker represented; effective-sample reasons not implemented | Phase 2 authoritative-sample and drop-mask tests |
| Observation weights | Syntax placeholders only | Phase 2 native validation plus estimator-specific Phase 4/6 parity |
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
