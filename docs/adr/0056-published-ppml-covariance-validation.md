# ADR 0056: Validate the published PPML covariance eigensystem

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-13

The final result validator independently inspects the covariance that will be
published. It compares the existing PSD report with that matrix's own minimum
eigenvalue, rank, reconstruction and orthogonality diagnostics. It must not
substitute diagnostics from a newly clipped comparison matrix: clipping tiny
roundoff eigenvalues can change the latter eigensystem without making the
published matrix invalid.

Production CGM inclusion--exclusion and clipping remain unchanged under ADR 0016.
The existing tolerance, report-state/count/rank/minimum checks, diagnostic
underreport checks, and finite/symmetry/standard-error/certificate guards remain.
The validator also retains the old bound on any further PSD repair: when a
multiway published matrix has a tiny negative eigenvalue, it reconstructs and
certifies the clipped comparison and bounds its distance from the published
matrix. It does not eigendecompose that hypothetical comparison a second time
or replace the published covariance/report. Material indefiniteness fails closed.

Inspection uses existing fallible K-by-K scratch and no observation-sized state,
new retained cache, worker team or SPI call. Production and independent covariance
certificates remain distinct; all validation finishes before Stata stores.

The [local source-bound report](../../benchmarks/reports/development/ppml-psd-validation-local-2026-09-13/README.md)
contains a closed-form Householder spectral reproducer, the original wide PW
request, an independent profiled-Poisson/full-sandwich oracle, forged-report and
matrix rejection tests, transaction fixtures and paired cost controls. This is
an implementation repair to the existing scientific contract, not a new PSD
policy or adoption of the separate wider-moment optimization.
