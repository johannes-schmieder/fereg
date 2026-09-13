# P5 native entry experiment: dispatch exclusion found

The centered estimator now compiles in the non-test core and supplies all six
ordinary OLS entry points: conventional/robust/clustered inference with simple
or categorical preparation. It remains **unadopted**. The [receipt](receipt.json)
binds 182 inputs and the [complete patch](prototype.patch) against `436fe2a`.
The [previous scoped stage](../scope-saved-outputs/README.md) remains unchanged.

## Core and artifact evidence

All **141 distinct tests / 142 invocations**, non-test checking, the isolated
universal build and actual arm64 ABI checks pass. The candidate plugin is
`b1370e7ace9a8ab02ef716a15c2f5b5abe88beeb5d4ca1635dc1bd8c41c38303`.
All 176 accepted baseline build inputs and plugin `c64ead0763d662a8f7bd1c74f1058426ea03f90c871ff387e76a6fe935b2e3da`
were independently rechecked. [Build identities](build-identities.json) preserve
both input sets; installed dist was not used as the baseline or rebuilt.

Twenty-two full covariance comparisons exercise actual native entry points
against explicit matrix-MAP inference, with unit/FW/AW/PW and independently
known orthogonal coefficients/residuals. Two additional large-mean IID cases
pass. The large-mean robust fixture fails with the identical sandwich-certificate
error on both centered and matrix routes; its strict rejection remains a control.

A separate licensed public reproduction confirms that the accepted baseline
and candidate both pass its independent IID coefficient oracle, return **r(459)**
for robust inference, and preserve the prior posted coefficient matrix. The
initial harness incorrectly expected r(430); source inspection confirms the
existing InvalidData-to-459 mapping. The corrected fixture changes only that
expectation. This pre-existing covariance boundary remains unresolved; no
successful robust result or tolerance change is claimed.

The actual native fallback controller also passes: a failed first centered
certificate declines from untouched inputs, ordinary Auto/MAP certifies in two
iterations, coefficient/residual values remain exact and discarded elapsed time
is added to absorption setup with the fallback flag set. A contradictory
execution/usable budget rejects before the inference consumer. One initial
immutability assertion compared all prepared numeric sources to finite X alone,
omitting the separately loaded FE source; the corrected assertion compares the
complete original prepared source bank. Its full generated-data failure log is
compressed and preserved, not truncated.

## Public screen and disposition

The [prospective protocol](protocol.md) fixed three workloads before timing.
All six commands pass, with exact b/V, samples, status/rank/DoF and FE/cluster
structure. However, all three workloads have approximately **N/5 FE levels**,
so the provisional **N/16** admission guard excluded the centered route.
These are dispatch controls, not centered speed or memory measurements:

| Case | Baseline / candidate command seconds | Native peak bytes, both |
|---|---:|---:|
| Unit IID, N983040/K50/16 workers | 0.578 / 0.582 | 925,865,188 |
| AW robust missing, N245760/K50/4 workers | 0.426 / 0.430 | 233,241,144 |
| PW two-way cluster missing, N245760/K50/16 workers | 0.329 / 0.341 | 239,038,342 |

[Screen results](dispatch-screen.json) retain unchanged models, native peaks,
process RSS and comparisons. [Numeric evidence](dispatch-numeric-evidence.zip)
is checksummed in [its archive manifest](dispatch-archive.json); licensed logs
are excluded. The first launch stopped at the process-memory sampling preflight
before any Stata command; the authorized local run then completed all six calls.
No additional primary seeds or protected campaign ran.

The new frame/finite component allowance uses one displaced `8*N*K` matrix
payload and does not lower public fast/low admission. Full overlapping native
peak validation remains open. The provisional level-count exclusion was too
restrictive for the roadmap's existing one-FE targets. Next replace it with an
actual bounded mean-bank check and rerun **the same workloads**, preserving this
snapshot and every first-screen observation. Extra packed raw passes, serial
score Gram certification, rank-zero scratch and saved-retention admission remain
cost/coverage questions. PPML extension remains separate. No adoption, centered
command gain, cluster work, agents, qualification or push is claimed.

The fifteen core logs also preserve the initial non-test promotion compilation
errors, the first fixture's SourceId type mismatch, and the first large-mean
success assumption. Scientific arithmetic and tolerances were not relaxed.
