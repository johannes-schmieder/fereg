# ADR 0050: fresh categorical one-FE OLS projection

**Status:** Accepted for local development
**Date:** 2026-09-13
**Decision owner:** Johannes Schmieder

## Decision

Fresh automatic fast OLS with exactly one intercept-only categorical effect
and no FE coefficient retention uses the existing simple one-FE initial
projection entry point. An interaction such as `g#h` describes one partition;
its identifier syntax does not require an extra initial projection certificate.
Validate the finalized topology and borrow its existing codes and membership.
Do not copy observation-sized topology or introduce another projector.

Project the raw RHS once, independently certify the result against its original
weighted reference norms, and return the current residual norms. Reuse the
existing fixed-partition OLS norm scheduling at its existing row/width bounds.
The stable scaled accumulation, finite rank policy and QR fallback remain.

Already-projected residuals retain the ordinary initial certificate and may
return with zero iterations. Explicit solver requests, retained saved-FE
products, absorbed slopes and multiple effects retain their existing entry
points. Low-memory outcome and column blocks still call the ordinary path;
their fresh-input pass reduction is a separate experiment. PPML's canonical
linear calls select explicit MAP and do not enter this automatic OLS shortcut.
Weight kind, raw/reference scales, sample/singletons, DoF, covariance and public
transaction rules are unchanged.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/categorical-one-fe-local-2026-09-13/README.md)
records seventeen pairs against accepted native `7663a87`, four focused Rust
tests and the same independent interaction-dummy Stata oracle on both isolated
artifacts. Primary command gains are 18.0% for unit IID, 11.3% for AW robust and
8.6% for PW clustering. All paired full b/V values are exact. The trace locates
the gain in initial absorption. Largest protected slowdown is 2 ms.

Native primary peaks are effectively unchanged and fit their models. Sampled
process RSS rises from 0.392 to 0.502 GiB for AW and 0.394 to 0.491 GiB for PW;
no process-memory improvement is claimed or attribution established. Existing
low-memory/weak control model underreports remain open. This is local speed
adoption, not universal memory qualification, a new solver family, or release
evidence. The preceding reporting prototype remains archived and unadopted.
