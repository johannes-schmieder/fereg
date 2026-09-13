# Stopped weak saved-output comparison: independent adjudication

The original protocol stopped the control batch after both weak_d3 commands
returned r(0), when saved residual parity failed at the unchanged atol=1e-9,
rtol=1e-7. Both original captures and the failed assertion remain unchanged.
This is not a passing baseline/candidate saved-output comparison and its command
times are excluded from accepted paired performance aggregates.

The same exact 30,720-row, 16-control, seed31415927 input has 10,240 and 3,649
levels in one connected component. Independent explicit-dummy SciPy LSMR and
LSQR, with scaled columns and 2e-15 iterative tolerances, stop successfully in
90/91 iterations. A separate grounded direct FE normal solve with SuperLU,
one residual correction, and LAPACK SVD finite regression agrees with both
residual vectors within 9e-13. The direct normal residual is below 5e-13 and
the fitted-value reconstruction below 8e-16. Its FE normalization explicitly
centers each effect on the observation sample. These diagnostics do not alter
production convergence tolerances or use tighter Stata fitting requests.

Against the direct oracle under the original saved-output tolerances, the
baseline fails one residual (obs_id18650; maximum error over all rows 6.3242e-9,
largest tolerance ratio 1.2891). Candidate residual error is at most 1.1811e-9,
largest ratio 0.7787, and no row fails. Both candidate FE channels pass, with
maximum errors 8.1874e-10 and 1.6398e-9 and maximum ratios 0.4583 and 0.8382.
Baseline FE channels also pass. Independently formed finite HC1 covariance and
coefficients use full absorbed rank and the existing residual DoF. Full paired
b/V and exact sample/status/DoF comparisons already passed before the saved
residual assertion. The direct oracle adds finite b/V, residual and normalized
FE checks; it does not claim an independent normalized-constant covariance test.

Disposition: candidate passes the independent scientific contract on this exact
weak control; the baseline residual mismatch remains an explicit baseline
failure, not a relaxed threshold or candidate regression. Keep the original
failed comparison out of ordinary timing aggregates. Resume only the ten
unexecuted controls, with their original requests, data, seeds and arm rotations.
Do not change solver guards, fit tolerances or sample selection to match the
less accurate baseline residual. No new seed or replacement control is added.
