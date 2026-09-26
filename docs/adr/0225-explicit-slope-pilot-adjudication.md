# ADR 0225: Explicit slope-design adjudication of the SCC pilot

Date: 2026-09-26. Status: accepted diagnostic design; no gate exception.

Johannes requested continuation after the lighter pilot's comparability gate
rejected three sparse three-FE absorbed-slope groups. All native commands
completed, but default references disagreed, strict PPML fixest timed out at
120 seconds, fast OLS reghdfe timed out at 120 seconds, and ppmlhdfe failed
initialization. These original receipts remain unchanged and unranked.

Use one isolated diagnostic attempt against those exact captured inputs and
outputs. Regeneration must match both the DTA and every column hash under the
original Linux generator/runtime. Verify the original frozen runtime before
starting. Bind the independent diagnostic source, its fixed case selection,
timeouts, job, input identities, captured native results and new reference
results. No estimator or measurement-controller byte is replaced.

Run only the previously timed-out strict stock PPML references (same glm.tol
1e-10/fixef.tol1e-11 options) and the unchanged OLS reghdfe command, allowing
1,800 seconds per timed call. Retain 600-second startup/extraction bounds.
Do not retry ppmlhdfe's known initialization failure. Longer elapsed time is
new diagnostic evidence, never a correction to the original 120-second bound.
Do not compare these clocks to native fits from the earlier job.

For each case, independently solve the explicit mathematical model. Group
Householder QR eliminates the first partition, after a local SVD verifies its
full column rank. Direct SVD solves the remaining explicit finite/FE design;
no native or reference absorption, geometry, rank, coefficients or weights
enter this calculation. This instrument is deliberately bounded to 61,440
rows, 2,200 remaining columns, equal first-partition group sizes and strictly
positive Poisson outcomes. It fails on unsupported inputs rather than changing
a sample or generalizing a separation policy.

Rank uses the standard dimension-scaled machine-epsilon SVD threshold and a
singular-value gap check across wider cutoffs, with a floating-noise floor for
small controls. Initial development's lower sensitivity cutoff was below that
floor and incorrectly treated numerical gauge noise as ambiguity; analytic
dense-design controls identified this before any cluster run. Thresholds are
captured in every factor trace. Finite coordinates must be absent from the
numerical null space. OLS inference uses independently established residual
DoF. Poisson uses Newton increments, objective backtracking, explicit model
reconstruction and independently computed full-model scaled scores. It must
reach score1e-11; a fresh final-weight factorization supplies the Hessian and
HC0 sandwich multiplied by N/(N-1). Independent nuisance orthogonality and
inverse-Hessian checks guard the extracted bread and projected controls.

The existing campaign coefficient, covariance, objective and sample envelopes
are unchanged. The diagnostic records agreement with the original native
output and the new reference independently. A reference agreement alone is
not acceptance. Its result does not manufacture a passed pilot or waive the
existing production gate. Any subsequent evidence-backed gate continuation
must explicitly bind this adjudication and preserve the rejected pilot.

Local validation covers five tests: weighted projection/bread against a full
dense design, OLS/Poisson against the existing independent sparse full-design
oracle, row permutation, invalid local rank/group geometry, and rejection of
zero outcomes. Additional N960/K10 controls exercise the actual three-FE,
two-slopes-per-partition design. These are diagnostic tests, not qualification.
