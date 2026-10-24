# Comparing fereg with fixest

Use the maintained [SCC package comparison](../bench/README.md#scc-package-comparison)
for OLS/PPML comparisons with fixest, reghdfe and ppmlhdfe. The campaign freezes
package versions, datasets, model options, worker counts and a fixed repetition
budget. Compare packages within the same isolated allocation on a general
eligible node.

Timing includes estimation, inference and requested fixed-effect recovery;
input generation, import and export are outside the measured command. Report
paired summaries by workload and worker count, including failed, timed-out and
scientifically ineligible comparisons. A fast result is eligible only if sample,
coefficient, covariance, objective and requested-output checks pass. Default
and strict comparator tolerances are separate profiles.

Benchmark results apply to the source revision and binaries measured. They do
not establish a speed ranking for a newer release or for another platform.
The accepted changes to the current OLS Auto policy and their qualification
limits are summarized in [VALIDATION](VALIDATION.md).

For a quick local development check, use [the small benchmark](../bench/README.md#local-benchmark).
For a change to fereg itself, use the [fixed-candidate qualification](../bench/README.md#fixed-candidate-qualification).
