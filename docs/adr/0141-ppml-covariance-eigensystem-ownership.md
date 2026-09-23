# ADR 0141: Own one certified eigensystem per PPML covariance state

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-23

PPML production inference keeps the certified eigensystem of its symmetrized
covariance when PSD policy leaves that exact matrix unchanged. Repeating the
same deterministic eigendecomposition does not establish an additional
independent invariant. The owned matrix, spectrum, reconstruction and
orthogonality diagnostics remain together within `apply_ppml_psd_policy`;
there is no cache across matrices, fits or changing-weight frames.

If multiway inclusion--exclusion needs clipping, production still independently
certifies the clipped reconstruction, releases the raw eigensystem, symmetrizes
the repaired matrix and computes a fresh certified eigensystem. Final rank,
minimum eigenvalue and maximum raw/final diagnostic errors describe the actual
output. Robust and one-way covariance retain their strict policy, including
acceptance of immaterial negative roundoff without clipping.

The separate result boundary always inspects the actual published covariance
under ADR 0056. Its rank, minimum-eigenvalue, report consistency, diagnostic
underreport and further-repair-distance checks remain. Production and independent
sandwich reductions, score, objective, current-weight projection, rank,
reconstruction and final-Hessian acceptance are unchanged. No tolerance, DoF,
weight, sample, separation or failure-atomicity rule changes.

The exact maximum-pivot cache of ADR 0058 now serves every matrix width. This
supersedes only that ADR's dimension guard. Exhaustive pivot search remains a
test oracle; production uses one implementation. Strict tie ordering, rotations,
stopping threshold and iteration cap are unchanged. The required eigenvalue
vector temporarily holds upper-row maxima and then the final diagonal. No
additional heap payload, retained state, worker team or SPI access is introduced.
The conservative phase memory model is unchanged. An unrepaired production
covariance avoids one work matrix, eigenvector matrix and eigenvalue-vector
allocation over its lifetime; their peak overlap was already sequential.

The [source-bound local report](../../benchmarks/reports/development/ppml-wide-inference-local-2026-09-23/README.md)
contains isolated attribution, unchanged-source baseline/candidate/stock rotations,
fresh widths, independent Poisson/Hessian/sandwich and closed-form Householder
oracles, exhaustive pivot comparisons, repaired/forged-report rejection controls,
and narrow PPML/OLS controls. This is bounded local development evidence, not
cross-platform or constrained-memory qualification.
