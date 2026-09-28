# ADR 0236: scale strict PPML coefficient movement near zero

Status: accepted for bounded development, 2026-09-28. Not qualification.

## Problem and independent evidence

The [original-input adjudication](../../benchmarks/reports/development/ppml-original-positive-scc-2026-09-28/README.md)
proves that original and current fereg stop outside the registered coefficient
accuracy envelope on x8 of positive-outcome strong_d2 N61440/K10. Strict stock
fixest is correct on this input. A separately implemented grounded full-design
Poisson solve, checked against scalar dummy-design oracles, supplies the anchor;
the existing outward-rounded Decimal certificate establishes an optimum
coefficient radius below 8.66e-14. The discrepancy is not sample or separation
ambiguity and cannot be excused by reference inaccuracy.

Strict publication previously normalized coefficient Newton movement by
`max(1, abs(beta), abs(beta_strict))`. At default strict tolerance 1e-8, a small
coefficient could move by more than its registered accuracy envelope
`1e-10 + 1e-7*abs(beta)` and still pass. Objective, normalized score and eta
checks do not by themselves prevent this finite-coordinate accuracy miss.

## Decision

In the shared strict-profile predicate, normalize each checked coefficient by

```
min(max(1, magnitude), 1e-3 + magnitude)
magnitude = max(abs(beta), abs(beta_strict))
```

The 1e-3 scale is the existing coefficient envelope's absolute/relative tolerance
ratio, not a threshold chosen from the failing coefficient. The unchanged strict
tolerance remains `min(outer_tolerance, score_tolerance, 1e-8)`: at most one tenth
of that envelope's movement scale, retaining the old requirement if stronger.
Nonfinite checked coordinates, mismatched lengths and invalid slots fail closed.
Existing identified-coordinate slot selection remains authoritative; no new
comparison of absorbed gauge coordinates is introduced.

This is a stricter stopping diagnostic, not a general forward-error proof. It
uses the already required current-weight strict Newton solution. Independent
mathematical oracles and the numerical envelopes still establish development
accuracy. It adds no solve, observation pass or heap bank to a passing check.
If it rejects, existing bounded canonical refinement and IRLS recovery apply;
final products must be recertified at the accepted means. No stale Hessian,
score, reconstruction, model-space or rank certificate becomes publishable.
The same scaled discrepancy drives the existing recovery-progress test, so
acceptance and progress use consistent units.

No change to public tolerances, samples, weights, separation, DoF, inference,
solver routing, ownership or transactional publication. User tolerances tighter
than the defaults remain binding. OLS does not call this predicate.

## Validation scope

The [repair report](../../benchmarks/reports/development/ppml-coefficient-stop-scc-2026-09-28/README.md)
binds original-input and fresh scientific checks, platform tests and cost.
The focused guard covers zero/small/ordinary/large coefficients, invalid active
coordinates/slots and retained normalization exclusions. Existing IRLS tests
exercise tiny user tolerances, current-Hessian synchronization, rank changes,
forced retry, slopes, low memory and atomic failure.

An older dynamic-range regression expected exactly one natural rejection. Both
unchanged baseline and candidate now reach that optimum without rejecting, so
the test explicitly injects the strict movement it is intended to exercise.
Its analytical mean check and six-iteration recovery bound remain unchanged.
This is a test repair, not permission to remove the bounded recovery requirement.

The original input now passes the certified envelope in fast T1/T4 and low T4.
Both platforms pass 61 focused Rust tests and four licensed fixtures; Linux
passes PPML smoke. Fresh rotations expose an explicit cost: roughly 25–29%
slower on small fast, wide and weak inputs because rejected checkpoints resume
two further ordinary iterations. Retain the correctness repair and identify
strict correction/finalization as the next performance target. It is not
a speed improvement. Three fresh weak-connectivity stock SE discrepancies
remain unranked and unadjudicated. No historical timing is reclassified.
