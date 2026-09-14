# OLS residual-derived output precision: local adoption

Adopt [ADR 0063](../../../../docs/adr/0063-ols-derived-output-final-precision.md)
on accepted **58bcf3a**, which repairs generic owned-FE memory admission. This is
the three-file precision repair, excluding the separate saved-Schur/recovery
optimization. [Identities](built-identities.json) bind 178 source/fixture inputs,
the isolated universal plugin **14b396c7f0d1829e24a8f4f92222a46e675d6aac2d17eec1d30b3af1bb73d0d2**,
its baseline **ad725932**, and the [implementation patch](implementation.patch).
The installed plugin remains unchanged. These are local development results.

## Change and scientific evidence

Requested residual, absorbed, combined-prediction or saved-FE outputs now select
a separate final MAP/LSMR tolerance of at most 1e-14. Fast, low and projection-only
fits use it irrespective of FE coefficient retention. Initial projection, raw
reference scales, rank and public accuracy thresholds remain unchanged. Terminal
CMG first certifies at that final precision; when correction is required it
consumes the session, releases its hierarchy, admits CG and reports the certified
continuation. Generic routes use the newly accepted owned-admission guard.

The [preceding investigation](../ols-saved-residual-precision-local-2026-09-13/README.md)
preserves the red regression, superseded 1e-12 targets, low-memory retention bug,
expensive graph retry, oracle derivations and prior plugin-allocation failure.
Its 1e-14 candidate already passed accuracy but could not be adopted before
generic admission was repaired. No historical capture is relabeled for this
new baseline/artifact.

The rebased candidate passes:

- **109 core checks:** 67 prepared-fit, 31 CMG and 11 generic-auto tests, including
  final precision across solver/weight/retention combinations and terminal CG
  exact/one-byte-short admission.
- **95/95 plugin tests**, run as a complete serial suite on the isolated source.
  No additional concurrent allocation-audit campaign was run; its historical
  intermittent failure is not declared fixed.
- **48 independent saved-output dummy fits**, four weighted omission/output cases,
  and both public exact/one-byte-short direct-CG and generic boundaries. Native
  peaks are 296259 within 311496 usable bytes and 730668 within 806328 respectively.
- Both architecture builds, static binary audits and the actual native arm64
  ABI/self-test. No x86 runtime or release qualification was run.

The original tiny weighted cut passes all 30,720 rows against the preserved
80-digit grounded oracle and independent LSQR/LSMR. Maximum residual error is
**3.05e-13**; full b/V and both saved effects pass. Coefficient/covariance maximum
errors are 4.93e-16/8.14e-18. All comparisons use the original inputs and tolerances.

## Original slope memory boundary

The unchanged N30720/K16/AW/robust saved-slope input first rejects a diagnostic
32 MB usable limit: initial CG requires 13275656 bytes beside 19993104 execution
bytes. The resulting exact admission is **33268760 usable bytes** (total test
cap 570139672, including existing headroom). One byte below returns r(909),
preserving prior b/V, sample/order and requested-output absence.

At exact admission the command succeeds with **28906404 observed native bytes**.
Final LSMR cannot fit, so the existing separately admitted CG continuation closes
the tighter certificate, with fallback and warning fields set. Its residual and
saved-FE values agree with the earlier unrestricted strict candidate within
9.22e-15/5.96e-15. This is a regression comparison, complemented by the separate
independent dummy and closed-form checks; it is not a new independent oracle.
[Cap receipts](slope-cap-receipts.json), [output review](slope-cap-review.json) and
the checksummed output archive retain the full evidence. The public 18925832-byte
scalar still reports an execution base, not the joint whole-action requirement.

## Complete-command costs

The [prospective protocol](cost-protocol.md) retains all thirteen original
requests/seeds, rotating artifact order, then two reversed-order follow-ups for
each increase exceeding both 5% and 10 ms. **19 pairs / 38 measured commands**
complete; a separate functional weighted-cut replay is excluded from cost totals.
No build, tests, oracle solving, generation or compression overlaps timings.

| Original case | Baseline → candidate, seconds | Change |
|---|---:|---:|
| Unit saved | 3.003 → 3.094 | +91 ms |
| PW partial saved | 3.599 → 3.715 | +116 ms |
| Rank-deficient saved | 0.480 → 0.484 | +4 ms |
| Slope saved | 0.342 → 0.380 | +38 ms |
| Three-FE saved | 0.102 → 0.108 | +6 ms |
| Explicit CG saved | 0.071 → 0.075 | +4 ms |
| Weak saved | 0.098 → 0.106 | +8 ms |
| Low FW saved | 0.225 → 0.233 | +8 ms |
| Complete IID, no saved outputs | 0.585 → 0.592 | +7 ms |
| Tiny weighted cut saved | 0.055 → 0.108 | +53 ms |
| Small AW saved | 0.005 → 0.005 | unchanged |
| Weak PW PPML | 0.034 → 0.033 | -1 ms |
| Automatic CMG saved | 0.068 → 0.085 | +17 ms |

The prescribed follow-ups confirm median paired costs of **28 ms/8.2% for slope**,
**53 ms/96.4% for weighted cut**, and **18 ms/26.1% for automatic CMG**, including
the original observations. These are correctness costs. Slope final MAP changes
to LSMR; weighted-cut and automatic-CMG terminal corrections use 32 and six CG
iterations. Their initial routes and iteration counts are unchanged. Unit/PW
initial solves also stay close; their new final LSMR setup/solve costs account
for most added native time. Input timing stays close. Do not sum overlapping or
independently measured phases.

Every candidate passes full b/V, structural/sample/DoF and runtime-certificate
checks. Independent saved-output oracles pass in all applicable cases. Six
original baseline cases have independently proven output failures: PW, three FE,
weak, low FW, weighted cut and automatic CMG. Those comparisons remain outside
accuracy-matched speed claims; their inaccurate baseline is not a valid speed
competitor. Three-FE saved first-pair component contrasts are independently
identified; weak finite output uses the existing runtime/parity checks rather
than claiming a separately supplied full normalized-intercept b/V oracle.

Unrestricted slope native peak rises **28.871 → 38.537 MB**. In the other original
pairs, native peaks are unchanged except three FE +78933 bytes, low FW +149 bytes
and weak -1072016 bytes. The smaller changes are not causally attributed. Public
base/scratch models remain unchanged and sampled RSS moves both ways. Earlier
whole-action reporting/accounting gaps persist. There is no general memory
saving or universal bound claim. [Full cost review](cost-review.json),
[phase/peak summary](summary.csv), and checksummed scientific/solver archives
include all follow-ups, fallback flags and warning counts. Public solver column
12 is warning count, not checkpoint retries.

## Disposition

Adopt the output correctness repair with the documented command/memory costs.
The generic admission prerequisite is now satisfied for the original slope route
and the bounded public/closed-form checks. This does not complete whole-action
reporting, specialized slope MAP accounting, recovery admission or opaque factor
bounds. Next rebase the separate saved-Schur/final-target recovery optimization
on this accepted precision baseline and repeat its original comparison. No P6
optimization checkbox is closed by this correctness repair alone. No agents,
SCC, upstream update, qualification or push occurred.
