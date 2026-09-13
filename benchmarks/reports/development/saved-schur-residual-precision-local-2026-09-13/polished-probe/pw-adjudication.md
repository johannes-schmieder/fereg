# Original PW primary: polished candidate passes, retained baseline fails

The fresh comparison stops on the original PW seed104729 primary. Unit and the
rejecting AW primary pass; PW returns r(0) on both arms but fails saved_e/saved_g
parity. No later primary or protected comparison is run. The original 3.474→2.642
seconds and 841.84→581.41 MB are failed-parity diagnostics, outside passing timing
aggregates. Earlier failed and completed screens are preserved.

Independent availability classification includes y, all X, both FE, raw PW and
both clusters before the singleton fixed point. It reproduces all 244274 active
observations and missing output masks. Weighted grounded two-FE direct projection
with a residual correction, followed by LAPACK SVD, gives rank50. Full sparse
weighted LSQR and LSMR each take235 iterations and agree with the direct residual
within7e-14. Component-normalized FE and the public normalized constant reconstruct
y within1.5e-15. The independent two-way CGM calculation uses97/3/291 subset groups,
shared finite-sample correction1.51941306727707, explicitly detected nesting,
full normalized-intercept influence and NumPy PSD clipping. Full b/V, both DoF
frames and final sample pass the original tolerances for both artifacts; no
cross-eigensolver negative-count equality is claimed.

The accepted retained baseline fails121138 residual rows (max2.618e-7, tolerance
ratio226.72) and242668 saved-first-FE rows (max2.694e-6, ratio2660.17). The polished
candidate passes every row: residual max2.695e-12, saved FE max5.177e-10 with
maximum tolerance ratios0.00105/0.47651. Coefficients are accurate to9.54e-15 and
full covariance to2.09e-11. Original scientific tolerances remain unchanged.

This independently establishes candidate accuracy and baseline error. It also
shows the final-residual precision issue extends to the existing retained saved-FE
route. Before continuing the optimization adoption comparison, repair that
accepted baseline separately, preserving actual solver families, memory admission,
requested stricter precision, certificates and transactional publication. The
polished five-file optimization remains unadopted. Do not treat the failed baseline
as an accepted accuracy-matched timing denominator or replace this input/seed.
