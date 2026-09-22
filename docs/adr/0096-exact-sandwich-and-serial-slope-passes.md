# ADR 0096: contiguous sandwich rows and serial slope reference passes

**Status:** Accepted for local development; milestone validation remains open
**Date:** 2026-09-22
**Decision owner:** Johannes Schmieder

## Decision

Dense sandwich production and certificate meats accumulate contiguous upper
triangle slices. Each coordinate retains its multiplication order, observation
order and fixed-block reduction tree. Production and certificate multipliers,
separate accumulators, reversed certificate block reduction and compensated
certificate sandwich remain independent. The shared OLS/PPML reducer allocates
the same workspaces. OLS's dense score-Gram certificate uses the same contiguous
traversal while retaining its fresh weighted products and original reductions.

An admitted cached single-slope OLS fit with one actual worker, a packed score
cache and at most 64 generated columns computes reference norms and raw maxima
in one row traversal. Each column retains the scalar scaled-norm recurrence,
maximum and final square root. A fixed 512-byte stack bank replaces repeated
column traversals; there is no added heap owner or raw/normalized norm handoff.
Other solver routes, shared weighted-linear callers, threaded fits and strided
score caches retain their reference-pass path.

## Evidence and limits

The [local data-pass report](../../benchmarks/reports/development/serial-slope-data-passes-local-2026-09-22/README.md)
binds four captured sources, 356 application calls, focused release fixtures,
two three-pair screens and two frozen five-pair confirmation blocks. The latter
show a 13–15% paired gain for the weighted K50 robust saved-output fit and more
modest gains in several K10 cases. All 118 screen/confirmation native pairs
have exact numerical exports, samples and saved vectors, with unchanged tracked
peak allocation. Seven additional diagnostic pairs are exact.

The arithmetic fixtures cover distinct production/certificate multipliers,
vector tails, multiple teams, weight kinds, extreme scales and fail-closed
nonfinite inputs. Focused OLS inference/mutation and PPML team/FW fixtures pass.
These local results do not qualify a release or transfer earlier final-block
claims to the new source. Serial PW losses, broader milestone validation and
Linux confirmation remain open. Automatic solver selection is unchanged.
