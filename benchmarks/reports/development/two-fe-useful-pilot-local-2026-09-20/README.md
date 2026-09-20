# Retain useful narrow OLS pilot work

Local development measurements on 2026-09-20 on M2 Ultra, Stata/MP 19.0
(8 licensed processors), and pinned stock fixest. Native/R requests use 16
threads unless specified. The embedded build date is September 19. These
focused results do not close the broad milestone or qualify a release.

Candidate `usefulpilot` is base `558f4c46` plus the three recorded core overlays;
all 516 captured source files match. Plugin SHA-256:
`79141637b5252f27d6f86a721fb04cf5648df6b8d0d82ee26ea7e39b9b94de14`.
Parent is accepted `fusedreconstruction`. The actual-plugin ABI/panic check
passes. No installed package or remote state changed.

Already-admitted 3–8 RHS OLS calls advance every column during the eight-step
Schur pilot. The same-call owner retains graph state and independent original
norms, while the original three selected columns determine admission exactly.
Accepted safe state continues the existing certified solve. Rejected probes
return the original matrix unchanged; unsafe additional-column state preserves
the prior accepted solve. Retained steps count once within the former eight
plus 64 allowance and the same total continuation budget. Existing banks and
preflight bounds are reused. Wider OLS and shared/PPML behavior remain unchanged.
[ADR 0086](../../../../docs/adr/0086-useful-narrow-ols-schur-pilot.md)
records the decision and safeguards.

## Timing

Milliseconds are separate medians of three paired runs. Individual calls,
pair orders, paired ratios and dispersion remain in the receipts.

| Cell | Parent | Candidate | Default fixest |
|---|---:|---:|---:|
| Shuffled unit K=1 IID | 223 | 200 | 227 |
| Shuffled unit K=3 IID | 225 | 210 | 245 |
| Shuffled unit K=5 IID | 255 | 243 | 250 |
| Shuffled unit K=6 IID | 249 | 234 | 256 |
| Shuffled AW K=6 IID | 291 | 276 | 293 |
| Shuffled AW K=6 full outputs | 458 | 440 | 544 |
| Ordered AW K=6 IID | 274 | 256 | 476 |
| Ordered AW K=6 full outputs | 411 | 409 | 703 |
| Ordered AW K=50 IID | 730 | 730 | 1964 |
| Ordered AW K=50 full outputs | 920 | 927 | 2164 |
| Degree-three unit K=6 IID | 73 | 60 | 74 |
| Degree-three AW K=6 IID | 83 | 72 | 86 |
| Degree-six unit K=6 IID | 64 | 53 | 65 |
| Degree-six AW K=6 IID | 77 | 64 | 74 |
| Unbalanced unit K=6 IID | 73 | 61 | 77 |
| Unbalanced AW K=6 IID | 84 | 71 | 82 |

All sixteen screened cells have median paired candidate/fixest ratios below
one; the maximum is 0.948. This is a focused three-pair screen, not acceptance
of the 77-cell milestone. K=50 does not use the changed path; its movements
are not attributed to pilot reuse. Default fixest output accuracy differences
remain explicit below.

Three initial single-pair controls exceed 5%: dispersed AW full outputs
(1.06341), low-memory AW IID (1.05656), and sparse AW IID (1.07500). Their
fresh three-pair confirmation medians are respectively 1.02287, 0.98571 and
1.03333. No confirmation exceeds 5%. All original and repeated measurements
are retained. Dispersed-output timings vary substantially (one repeated pair
is 24.5% slower); these data do not establish a stable speedup there. Sparse AW
retains a modest measured cost. Other controls include PW/FW robust and
clustered inference, saved FE, constant/concentrated weights, weighted tiny
bridges, thread boundaries, one-FE and protected wide-unit calls.

Four separate trace pairs support the mechanism. Initial projection time,
including setup, falls 132.465 -> 112.988 ms for shuffled AW K=6,
124.068 -> 112.209 ms for shuffled unit K=5, and 24.983 -> 13.418 ms for
weighted degree-six K=6. Reported charged iterations fall 43 -> 35 in the
shuffled cases and 18 -> 10 in degree-six cases. The contraction is unchanged.
Candidate trace iteration counts include eight retained pilot steps; its
operator/recurrence timings cover only subsequent steps. Probe timing also
includes the newly shared owner and norms, so raw probe cost alone is not an
equivalent before/after quantity. Diagnostic application timings are separate
from the headline medians.

## Science, memory and evidence

All 232 application calls succeed with explicit result and terminal checks:
214 original calls and 18 confirmation calls. The 92 native pairs include
four diagnostic pairs. All retain exact status/DoF and pass registered
coefficient, covariance, sample, fit and output comparisons; 53 preserve all
six scientific files bitwise. Every original pair retains the exact pilot
contraction. No pair increases the measured native allocation peak.

All seventeen output pairs pass independent weighted normal-equation,
component normalization, RSS and reconstruction audits; eight are bitwise
exact. The independent grounded sparse-direct oracle passes all eighteen
unit/AW combinations over nine other graph families at K=6. Maximum native
coefficient error against these oracles is below 1e-12. Nine default-reference
output discrepancies remain explicit; the finite coefficient comparisons pass.
Frequency controls use the native baseline with the existing frequency
semantics, not ordinary fixest weights as a frequency-weight oracle.

Sixteen focused Schur tests pass in a fresh release target. The new checks
cover connected/disconnected graphs, unit and supplied weights with common
scales 1e-100 through 1e100, RHS counts 3/5/7/8, constant and FE-collinear
columns, 8/16 workers, exact admission, original-input rejection, unsafe extra
columns, insufficient memory and capped continuation. Capped fallback values
and charged iterations match the prior solver exactly. Existing shared/PPML,
operator/cache, weighted-cut and independent MAP checks pass. Three
final-artifact Stata fixtures pass, including weighted/unit edge-cache oracles
and the eclass transaction fixture.

The first test used a requested solve tolerance of 1e-12 and exceeded its
2e-9 absolute oracle bound under extreme weights and forced fallback. A
localization run proved the new and prior capped fallback produced the same
2.157756654875147e-9 value. The test then requested 1e-14 while retaining the
2e-9 bound and production settings; it passes. Both failed test logs, the
initial missing-parameter compile error, corrected check, final tests and raw
hashes are retained. No failed measurement was replaced.

The next step is the full 111 native-control inventory against the milestone
baseline, then the sixteen predeclared fresh-seed holdouts and two independent
five-pair primary blocks. Preserve reference-accuracy adjudications and
investigate any confirmed loss before making a broader speed claim.
