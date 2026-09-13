# Third stopped control: unbalanced saved outputs

The unbalanced control's two original commands return r(0), but saved residual
parity again fails. Both captures and the failed assertion remain preserved.
The same direct explicit-dummy/SVD oracle and separate LSQR/LSMR calculations
agree on residuals within 4.1e-14, with reconstruction below 8e-16. The larger
unscaled group-normal residual is 1.52e-11 on this skewed design; independent
solution agreement is reported separately. Both arms pass finite b/V and exact
residual DoF 20337 under their original thresholds.

Baseline misses saved residual tolerance on 155 rows (maximum error 7.621e-9,
largest tolerance ratio 5.068) and first-effect tolerance on 216 rows (maximum
error 6.482e-9, ratio 6.358). Its second effect passes. Candidate passes all
rows for residual and both effects: maximum errors 1.909e-9, 1.569e-9 and
1.715e-9, maximum ratios 0.8671, 0.7844 and 0.3185. The tolerances remain
atol1e-9/rtol1e-7 and the original input, seed and production request remain.

As with the weak and disconnected controls, the candidate passes independent
scientific checks while this baseline/candidate parity comparison remains
failed and excluded from passing paired performance aggregates. Resume only
indices 4 through 11. These three specific adjudications provide no automatic
exception for a future mismatch; a candidate oracle failure must still stop.
