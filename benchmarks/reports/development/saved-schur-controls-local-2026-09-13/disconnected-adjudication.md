# Second stopped control: disconnected saved outputs

The resumed batch stopped on saved residual parity for the original disconnected
control, before any later request ran. Both r(0) captures and the failed comparison
remain preserved and are excluded from passing paired timing aggregates.

The same independent direct dummy solver now grounds one FE coordinate in each
of the 16 connected components and transfers each second-effect component mean
into its first-effect anchor. It compares against independently scaled explicit
LSMR and LSQR fits; residual agreement is below 2.5e-14, the direct normal residual
below 5.1e-13 and fitted reconstruction below 8.1e-16. Finite b/V and exact
residual DoF 20224 pass their original oracle thresholds in both artifacts.

Baseline again misses the original residual tolerance on one observation:
maximum residual error 1.7393e-9, maximum tolerance ratio 1.0621. Candidate
residual error is at most 1.5212e-11, maximum ratio 0.00943. All candidate FE
outputs pass, with maximum errors 1.0007e-11 and 1.2187e-11. Both baseline FE
outputs also pass. This is a baseline residual discrepancy, with independently
more accurate candidate saved values. No threshold, runtime configuration or
input changed. The generic oracle source and all resulting arrays are retained.

Resume only original controls at indices 3 through 11, preserving seeds, arm
rotation and input hashes. Keep both independently adjudicated baseline-failing
controls separate from ordinary paired performance evidence. Native scientific
policy remains unchanged; these are bounded oracle adjudications, not a general
exception for mismatches or a claim that native FE moments alone bound pointwise
residual errors on all graphs.
