# Original three-FE saved control adjudication

The cost comparison stopped at its fifth original pair, before any later pair
or prescribed cost recheck. Both arms return r(0), but raw saved-output parity
fails on four residual and 470 first-FE rows. Original captures are preserved.

An independent explicit-dummy calculation eliminates the first partition and
solves the 324-dimensional reduced system by LAPACK SVD with one residual
correction. The reduced rank is307; the complete FE rank is6451. The first two
partitions have16 connected components, and the reduced system has17 null
directions. The entire numerical nullspace vanishes after centering first-FE
values within those first-pair components. Thus those contrasts are identified;
global three-FE mean normalization alone cannot identify all transfer constants.
The independent comparison uses these identified contrasts at the unchanged
residual/output tolerances, as required for nonunique FE models.

Full sparse LSQR/LSMR solve the original design independently and agree with the
direct residual within4.9e-12/1.6e-11. Full normalized-intercept HC1 covariance,
finite rank16, FE rank6451, sample30720 and residual DoF24253 agree with both
artifacts. Baseline residual error reaches7.972e-9 (four failing rows); identified
first-FE error reaches9.886e-9 (455 failing rows). Candidate errors are4.13e-13
and5.06e-13, with no failing rows. Both global saved-first-FE means remain near
zero. This establishes baseline error and candidate accuracy, not arbitrary
individual-FE identification.

The original0.101→0.108-second pair remains failed-parity cost evidence outside
accuracy-matched performance aggregates. The run resumes remaining original
requests only after this independent adjudication; no input or tolerance changes.
The executed oracle source is retained as initial-three_fe_oracle.py. The later
version removes an unreachable alternative in the centering expression without
changing its arithmetic; it was not rerun.
