# OLS derived-output omissions: adopted correctness repair

Adopt [ADR 0061](../../../../docs/adr/0061-ols-derived-output-omitted-columns.md).
Saved FE and derived predictions now accept generated columns that the finite
fit correctly omitted. Those columns contribute no fitted term; their canonical
slots, status, zero public coefficients/covariance coordinates and rank remain.

## Identity and boundary

Accepted baseline is `834703b798a560e4a52cb8df7ed0719777149f0c`.
[Identities](identities.json) bind 176 source/ado/fixture inputs and the isolated
[one-file patch](implementation.patch):

- Baseline plugin `fdc3e141992fe4449064b688e995b472175bbd4966efdc10d65a9a25de51869d`.
- Candidate plugin `fe066c026275e10f09501db66b3f1903026b6aafed9cbb96b9315fcd88ae4483`.

Both exclude the pending saved-Schur/recovery prototype. The only production
change is a match arm in `build_derived_observation_outputs_in_team`, accepting
an absent value with a generated identity for the existing FE-collinear or
numerically-rank-deficient status. Missing kept values and other inconsistent
identities still fail. No numerical fit, allocation, worker, SPI call, sample,
weight, rank, inference or publication policy changes.

The exact absorbed-slope control stopped before measurement on both prior
artifacts with r(498), `derived OLS output coefficient identity is incomplete`.
The finite fit correctly omitted x1, which lies in the absorbed slope span.
The selector had mistaken that valid omission for a malformed fitted term.
The earlier [saved-Schur screen](../saved-schur-controls-local-2026-09-13/README.md)
preserves the failure and remains unadopted.

## Validation

The closed-form regression replaces a finite regressor by its FE group code.
Before the repair it fails at the same selector. Afterward, fast and low modes
publish residuals, xb, xbd, the combined absorbed component and saved FE, matching
independent group means at 1e-11. Omission status and receipt-last completion
are checked. The first post-fix test asserted the wrong numeric status (4);
it was corrected to the existing FE-collinearity status 3. That fixture mistake
and the genuine pre-fix failure are separately preserved in log identities.

All **95 plugin tests** pass, including output-store failures and transaction
checks. The new [independent Stata oracle](omitted_outputs_oracle.do) covers:

- Unit conventional, fast, a finite column collinear with an intercept FE.
- AW robust, fast, a finite column spanned by absorbed slopes.
- FW clustered, low, duplicate finite columns.
- PW clustered, fast, duplicate finite columns.

Stata explicit dummies and group slopes independently determine coefficients,
identified covariance combinations, residuals and DoF. Both artifacts first
pass ordinary fits. The baseline's four requested-output fits return expected
r(498), preserve prior b/V/status/sample and leave no requested output. The
candidate's four fits succeed, preserve their ordinary b/V/status/sample and
pass the original independent residual tolerances. These expected baseline
failures are not successful output fits or performance denominators.

The existing **48-fit saved-output oracle** passes on the candidate, with its
ordinary accepted solver configuration (`expect_schur=0`). The exact original
30,720-row slope and 50-column rank-deficient saved requests, including their
original 7,680-row warm-ups, now pass with native certificates and ranks 15/49.
Those are functional captures, separate from performance comparisons. They
retain public base-model gaps: 28.87/89.07 MB observed versus 18.93/52.37 MB
reported bases. This selector repair does not close P0 accounting.

Both architectures visibly compile core/plugin; universal static audits and
actual arm64 ABI/panic-containment checks pass. No x86 runtime, SCC, Windows or
qualification was run. [Validation results](validation-results.json),
[original requests](original-results.json), [validation summary](validation.json)
and [log hashes](log-identities.json) preserve the evidence.

## Complete-command cost and unresolved variation

The [protocol](protocol.md) precedes six fixed local comparisons and two required
PW rechecks. Fresh M2 Ultra Stata/MP 19 processes use unchanged original inputs,
small warm-ups and rotated arm order. No builds, tests, profiles or data
generation overlap timing. Native workers are distinct from Stata's license
processor count; affinity is uncontrolled.

| Fixed comparison | Baseline → candidate seconds | Change |
|---|---:|---:|
| Unit, saved both FE | 3.138 → 3.119 | −19 ms / −0.6% |
| PW, partial saved FE | 3.791 → 4.610 | **+819 ms / +21.6%** |
| Weak degree-three | 0.048 → 0.045 | −3 ms |
| Low FW clustered | 0.210 → 0.211 | +1 ms |
| Small AW | 0.005 → 0.005 | 0 ms |
| Complete one-FE IID | 0.617 → 0.608 | −9 ms |
| PW reversed recheck 1 | 3.871 → 3.801 | −70 ms / −1.8% |
| PW reversed recheck 2 | 4.407 → 6.368 | **+1961 ms / +44.5%** |

The flagged excesses lie mostly in unchanged initial CG, before the modified
selector: 3.159→3.959 and 3.788→5.666 seconds. Input remains near 80 ms. System
load rises during the sequence; a subsequent process observation finds another
project's Rust test active. Neither proves the cause of these differences.
No other work was interrupted and no measurement was discarded.

A [fixed four-pair diagnostic](cost-diagnostic-protocol.md), specified after
those flags, separates identical-binary A/A from implementation A/B. A/A changes
are +132 ms/3.7% and −9 ms/0.25%; A/B changes are +48 ms/1.35% and +9 ms/0.25%.
All four preserve exact science and solver fields. They do not explain away
the earlier large increases. Adoption is a correctness decision with remaining
timing uncertainty, not a speed claim or proof of uniformly negligible cost.

All eight initial/recheck pairs have byte-identical full b/V, fit/certificates,
sample/status/DoF/FE/cluster/subset/saved exports and non-time solver decisions.
Native peaks, public bases and scratch are exact. Sampled RSS varies. Complete
phase/solver/retry/fallback telemetry, including the original cost flags, remains
in [summary](summary.csv), [comparisons](cost-fix-results.json),
[diagnostics](cost-diagnostic-results.json) and the [numeric archive](archive.json).
The 26 capture records comprise 16 initial/recheck calls, eight diagnostic calls
and two functional full requests. A/A and functional calls are not pooled into
implementation timing summaries. Existing public-base omissions remain visible.

Installed dist and unrelated Veneto source/evidence remain intact. No agents,
cluster work, dependency change or push. Next rebase the preserved saved-Schur
prototype on this repair and complete its original protected comparisons;
other #27 items remain open. Scripts use [environment roles](environment-roles.json),
with executed/archived hashes and unchanged shared-harness hashes. Expand those
roles when replaying request JSON. Licensed logs, datasets and binaries remain
outside Git; only synthetic numeric captures are archived.
