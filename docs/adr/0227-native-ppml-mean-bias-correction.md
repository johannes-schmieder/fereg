# ADR 0227: optional native PPML coefficient mean correction

**Status:** Accepted design; implementation and evidence gates tracked in STATUS.md
**Date:** 2026-09-26
**Decision owner:** Johannes Schmieder

Johannes requested implementation of the native PPML mean-bias proposal and
approved the six-milestone plan. The feature evaluates the derivative-score and
curvature contractions at the accepted ordinary fit. It does not refit PPML,
change ordinary inference, clip coefficients, or correct quadratic moments.

The scientific and public contract is [ppml_bias_correction.md](../ppml_bias_correction.md).
Implementation must reuse the final identified weighted-linear state before
the session is consumed, with separate auxiliary workspaces. No new standalone
estimator, public solver handle, design matrix in ado/Mata, or extra substantive
plugin call is introduced. Both exact and fixed-probe methods require
independent mathematical checks; agreement with Separations alone is insufficient.

Ordinary calls retain their behavior and incur no correction allocation or
solve. Requested corrections participate in receipt-last publication and full
rollback. The request and result wire revisions move together with validators,
compiled Stata 18 transport helpers, and packaging tests.

The implementation request authorizes synthetic local/SCC validation, paired
Monte Carlo and performance measurements. It does not authorize changes to
FEVC or Separations production files, remote pushes, a release, cross-platform
qualification, or real IAB data work. Historical evidence remains immutable.
