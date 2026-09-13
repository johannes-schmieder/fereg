# Exact PPML Jacobi pivot reuse: prospective local evaluation

Baseline accepted d9aa8c0, isolated plugin
 d1e8efc290851ace10f917ac3cb630415bfaa975a2da588dead4a85b87fa2abc.
The original PSD producer/validator, rotations, threshold, iteration cap, output
reports and certificates must remain exact. Above 64 identified dimensions,
reuse the already-required eigenvalue buffer as upper-row maximum magnitudes.
Before each rotation, invalidate rows whose existing maximum could change;
afterward rescan those rows and update the two changed entries in other rows.
Global and within-row ties must retain first row/column order. Up-to-64 uses the
original full pivot search. No new heap buffer or retained state: eigenvalues
are allocated within the already modeled work/eigenvector/eigenvalue lifetime.

First validate every cached pivot against the independent exhaustive upper-
triangle loop over dense/tied/sparse/diagonal/signed-zero/extreme-scale inputs,
including dimensions 1/2/7/63/64/65/129/257 and transitions across many rotations.
Run focused PSD/result/VCE and memory-bound tests, independent Poisson/Hessian/
sandwich fixtures, existing wide PSD oracle and isolated universal/arm64 checks.
No tolerance, clipping count, PSD/rank policy, sample or transaction change.

Use the unchanged original one-FE public targets: unit K65/N122880/t4 robust;
FW K129/N61440/t4 exposure/missing robust; PW K257/N30720/t16 offset/missing
two-way cluster. Screen seed104729 with rotated order. If any saves >=5% AND
20ms with changed inference/publication savings and exact science, repeat all
three at 8675309 and 20260819. Adopt only if a repeated target meets that gate.

Protected seed31415927: complete unit K50; FW K50; low FW K10; small K2; saved/
separated K3; weak two-FE K65 PW one-way cluster; explicit low-mobility three-FE
K65; one-FE robust K64; no-FE robust K2. Use actual manifest FE counts (weak_d3
means degree-three/two-FE). Record full command and phase time, native/model/
scratch/RSS, iterations/retries/fallbacks. Investigate increases above
max(10ms,5%). Preserve all observations. A separate wide PW allocation audit
runs only if adoption remains plausible, checking zero events/growth, stable
capacities and fresh/warm equivalence, outside public aggregates.

Expected total: 9 primary +9 control +1 private audit =19 pairs. No builds/tests
alongside timing. Exact b/V and discrete scientific fields plus unchanged
independent certificates are required. Do not use standalone timing as a command
speed claim. Dist and unrelated work stay untouched. No agents, cluster, push
or qualification. Other #27 inference/ownership items remain separately open.
