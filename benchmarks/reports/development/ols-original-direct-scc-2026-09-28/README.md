# Original large OLS discrepancy: independent direct-graph adjudication

The original **strong_d2 N983040/K10/seed104729** result and the current fereg
artifact both agree with an independent direct weighted-least-squares oracle.
The original default fixest, areg and pyfixest results fail coefficient, RSS and
standard-error checks against that oracle. This diagnoses this captured input;
it does not reclassify other seeds, the old campaign, or any historical timing.
This report is diagnostic development evidence, not qualification.

**The overall SCC probe is partial, not a successful complete run.** Its direct
oracle, current native fit and default-reference fit completed; the strict
reference process timed out. [receipt.json](receipt.json) keeps the failed
accounting and missing parent sentinel separate from completed numerical checks.

## Exact input and independent calculation

Regeneration in the original SCC Python environment matches the original DTA
SHA-256 `0e4b437326bfe01a0e6058b0487f75b66036ae9f4f65d1ea94bfcea21c758ff0`
and every column's literal IEEE-754 hash. Those checks precede the large solve
and all new estimator fits. The earlier local regeneration rejection remains
unchanged. This job does not substitute locally regenerated doubles.

The new [diagnostic oracle](../../../systematic/two_fe_direct_oracle.py)
eliminates the first FE partition analytically. For weighted group incidence B
and group weight sums s and t, the remaining graph matrix is
`H = diag(t) - B' diag(1/s) B`. It grounds one second-partition coordinate per
connected component, factors the reduced matrix with SciPy SuperLU, and solves
the response and controls together. One direct refinement step is followed by
original-row reconstruction and independently recomputed FE/finite scores.
The finite coefficient/bread solve uses the resulting projected controls.
Neither package's iterative absorption, stopping rule or reported rank is used.

There are 491,520 first-partition levels, 12,288 second-partition levels and one
component. With ten identified controls the model rank is **503,817** and residual
DoF **479,223**. The grounded graph has 12,287 coordinates and 995,167 nonzeros;
the two sparse factors together have 1,961,240 nonzeros. No million-by-half-million
dummy matrix is materialized. The finite Gram condition number is **1.01830**.

The oracle RSS is **119761.04313891233**. Its largest normalized finite score is
**4.39e-15**, either FE score at most **1.47e-16**, all-column projection score
**2.52e-16**, sparse factor backward error **8.47e-16**, and independent
reconstruction discrepancy **6.67e-16**.

Six small full-dummy SVD cross-checks pass on Mac and SCC: connected/disconnected
graphs and three weight generations, including weights spanning exp(-6) to
exp(6). Coefficients, full residuals, bread and rank agree. Rank-deficient finite
controls fail closed; the zero-control boundary passes. These are mathematical
WLS checks, not a claim about Stata frequency-weight inference.

## Results against the oracle

All tolerances are unchanged from manifest-light-v2. All four original samples
and model ranks agree with the oracle.

| Captured/current output | Max coefficient error | RSS excess | Coefficients / RSS / SE |
|---|---:|---:|---|
| Original fereg | 3.18e-15 | 1.08e-8 | pass / pass / pass |
| Current fereg | 3.18e-15 | 1.08e-8 | pass / pass / pass |
| Original default fixest | 2.12e-6 | 0.870469 | fail / fail / fail |
| Original areg | 4.90e-6 | 1.664993 | fail / fail / fail |
| Original pyfixest | 4.30e-6 | 1.632167 | fail / fail / fail |

The current native coefficient and full finite covariance files are byte-identical
to the original native exports. Maximum native covariance error against the
oracle is **4.82e-20**. This finding does not require changing native rank,
inference, certificates or tolerances.

The separate default stock feols rerun reproduces the original numerical errors.
Its maximum original-row finite residual score is **2.12e-6**, second-FE score
**2.83e-7**, and residual distance to the oracle **0.00271235**. Its response/control
demeaning counts range from 120 to 240. These are diagnostic iterations, not
comparable native work counts.

The full covariance envelope alone passes for the inaccurate references because
its absolute floor is 1e-10, while these variances are around 5.2e-7 and covariance
errors are around 3e-12–6e-12. The separately required standard-error checks fail.
Retaining both checks is consequential. The original areg receipt explicitly
reports `converged=0` and `map_iterate=50`, despite application rc=0. Pyfixest's
OLS receipt has no convergence field. The conservative oracle diagnostic marks
both convergence-evidence checks false; only areg explicitly reports failure.

This reveals a pair-checker defect: its convergence requirement previously
enumerated only some comparator names, allowing an explicitly nonconverged areg
result through if numerical differences happened to pass. The current checker
now requires `converged == 1` whenever any comparator supplies that field. Existing
required-field surfaces remain required; absent OLS pyfixest metadata is not
invented or reinterpreted. Ten focused evidence tests pass. The new negative tests cover identical numerical
outputs with false/null flags and passing/absent-field controls. Frozen campaign
controllers, outputs and reported classifications are unchanged.
An offline replay of the captured areg pair now explicitly records its failed
convergence check. This pure checker change was tested after the SCC probe;
the deployed SCC harness remains bound to its captured parent-controller hashes.

## Failed preflight and bounded strict-reference attempt

Job **7763263** passes the six small SVD checks but rejects the native receipt
binding before generating the large input or running any estimator. Its plan
mistakenly reused hash `8743e065…` from a later paired-candidate result for the
original campaign path. The original file is intact: hash `35aefc50…` agrees with
the already committed expanded campaign extract. The initial registration and
deployment remain preserved. One corrected binding is declared separately in
[registration02.json](registration02.json); no input or scientific guard changes.
Accounting: four slots, 10 seconds, maxvmem 354.781M, scheduler failed=0,
application exit_status=1.

Job **7763270** then passes the exact original DTA/column guards, independent
large solve and current native checks. Stock defaults complete and reproduce the
discrepancy. The separately registered `fixef.tol=1e-11` R process exceeds its
**300-second wall limit**, leaving no coefficient, covariance or fit result.
The limit covers the whole R process, including input/output: it is not a
300-second command-time lower bound. The job exits 1 with no parent completion
sentinel. Accounting: four slots, 354 seconds, maxvmem **2.273G**, failed=0.
The latter is the job's virtual-memory peak, not an isolated oracle RSS or a
native-memory qualification result.

The registered `1e-13` third reference arm is **not executed** after this timeout.
Source review also reveals that this requested value is unsupported: the pinned
package requires `fixef.tol > 10000*.Machine$double.eps` (about 2.22e-12). This is
a harness configuration defect, not a reference fit failure observed in this job.
Do not reuse that setting. No further operational or scientific retry is made.
The strict comparison remains unavailable; none of these clocks are ranked.

The completed default rerun and original-row normal equations establish reference
inaccuracy on this input. No instrumented stopping-branch attribution is claimed,
and the unfinished strict fit cannot establish that a chosen tighter tolerance
resolves the discrepancy. The original areg/pyfixest outputs are independently
assessed, but their internal stopping paths were not instrumented or rerun.

## Identities and limits

Current estimator source is main **2c63aa74**, with all 211 computational inputs
matching the [accepted manifest](../joint-operator-scc-2026-09-28/candidate01-source.json).
Linux plugin SHA-256 is
`a7e95e6712c6b05b0217a9ddffa5b8f14c0f8d95443c2b983ce459f16a0e4596`.
Original campaign source/artifact remain 58284f76 / 1f596718…; no old receipt is
rebound to the current source. New reference calls use the pinned unmodified
fixest 0.14.2 library; every installed fixest file is checked against the original
frozen runtime inventory, including shared-library hash `4a376fa7…`.

The native public call is `fereg y x1-x10, absorb(g1 g2) threads(4)`, unit weights,
IID inference, no optional observation outputs. The original worker supplies one
untimed small warm-up. New stock accuracy calls use one thread, IID inference and
the same full sample. These checks run on a shared SCC node; no new clock is
ranked and no performance ratio is inferred from unlike thread settings.

The full campaign's other seeds, larger timeouts, PPML discrepancies, disconnected
inference policies, saved outputs, clustering and scaling remain separate. This
milestone adds an independent diagnostic, not a native speed or memory change.
