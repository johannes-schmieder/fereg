# ADR 0122: Preserve the disconnected PPML projection failure boundary

Status: accepted diagnostic closeout, 2026-09-23; no production change.

## Evidence and decision

The [bounded local diagnosis](../../benchmarks/reports/development/ppml-disconnected-map-local-2026-09-23/README.md)
reduces the disconnected three-FE/two-slope PPML failure to 935 retained rows.
On production `190946a1`, an isolated frame capture locates exhaustion in the
first MAP call, with all weights exactly one. Its infinity certificate is
`8.62e-5` after 16000 sweeps, against `1e-6`, and remains slowly contracting.
Independent dense SVD/pivoted QR agree on rank and the correct projection;
independent local QR reproduces the native failed certificate. Unresolved FE
components reach 1.73% of an original RHS norm. This is a solver convergence
gap, not evidence that the accepted per-call certificate cache is invalid.

Preserve the existing failed return. Do not accept these residuals by loosening
certificates, raising limits for this benchmark, changing rank/sample policy,
or silently substituting the unresolved generic nonretaining-CG path. Retain
the bounded captured frame as a reproducible diagnostic, outside production.
The capture allocates and writes files and must not become a public option or
support a production memory/performance claim.

A subsequent solver experiment requires a separate declaration and independent
certification on this frame plus fresh topology, weight-generation and excluded
path controls before adoption. The prior large failed timing block stays
stopped. ADR 0120's rejected extrapolation is not restored by this diagnosis.
No convergence remedy is selected here; work pauses at the user's request.

## Comparator boundary

The smaller stock fit passes the independent explicit Poisson, current-Hessian,
sample and public-state checks. The second reduced case (1901 retained rows)
passes coefficient, mean, objective and public-state checks but fails three
covariance coordinates at the unchanged tolerance. Dense SVD and pivoted QR
agree with the existing LSMR oracle. The discrepancy persists using stock's
own published means, so mean disagreement alone does not explain it.

Keep this second comparison scientifically unranked. Diagnosing stock's exact
inference mechanism remains separate work; no relaxed reference tolerance or
inference policy is adopted. Single stock clocks and native warmup failures
in this reduction are not speed comparisons. All accepted production inputs,
memory behavior, scientific contracts and previous deferrals are unchanged.
