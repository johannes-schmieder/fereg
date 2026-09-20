# Rejected RHS validation parallelism and finer setup profile

The experiment parallelized legacy RHS row-map and categorical-code checks on
installed teams of at least eight workers and at least 16,384 rows. It retained
all checks, exact integer counts and the first source/row diagnostic. Eleven
focused RHS tests pass, including partition/threshold boundaries, simultaneous
bad-code/mismatched-value errors, corrupt row maps and partial retained counts.
Three actual-artifact Stata input/oracle/transaction fixtures and ABI checks
also pass. This prototype was removed because command timing did not establish
a useful gain. [The patch](unadopted.patch) preserves it and its tests.

Five rotated three-arm pairs, on the same M2 Ultra and frozen narrow one-FE
inputs as the ordered-factor checkpoint, give IID medians 125 ms for the parent
and 126 ms for the candidate; the candidate wins only one parent pair. Median
native time changes 43.83→43.46 ms and input time 69.46→69.66 ms. Full-output
runs also provide no reliable increment. Late candidate/fixest command spikes
remain in the archive; no repetitions were removed. The post-screen CPU
snapshot has truncated process labels and is not causal evidence about those
spikes. No builds or tests overlapped these measurements.

The unchanged ordered-factor implementation was then profiled using an isolated
instrumented build. Two pairs per width distinguish the aggregate RHS setup
phase into its actual operations. Narrow phase ranges in milliseconds are:

| Operation | Time |
|---|---:|
| Finite response/source checks | 1.11–1.13 |
| Retained-row map and count | 0.94–1.12 |
| Categorical code/value checks | 0.82–0.90 |
| Exact speculative-matrix matching | 1.24–1.49 |
| Borrowed FE descriptor construction | 1.22–1.29 |

For fifty regressors, exact matching takes 7.96–7.98 ms, finite checks
2.44–2.50 ms, row checks 1.03–1.06 ms, code checks 0.77–0.88 ms and descriptor
construction 1.19–1.22 ms. Frame metadata checks are negligible. The diagnostic
helper adds these phase boundaries only to isolated builds; production is
unchanged. These profiles locate costs but do not establish that a prospective
optimization improves total command time.

The archive contains 38 successful application calls, fourteen nonself exact
native scientific comparisons and five byte-identical saved-output bundles.
The rejected plugin is
`f64d2604c6a8e69e4491b4ab3617f3594be9ad453e0e61370d7e6cf78b72da14`;
the accepted-production diagnostic plugin is
`5bb29aeac79b67b0277b58f3c78888fcff9d93d1a9dc3fac96b815e572ff4f96`.
Source and ABI identities remain in the two collector directories, and
[validation](validation.json) binds the test log and rejected overlays.
No regression matrix or final acceptance block was run for this rejected
candidate. No installed package, remote state or qualification changed.
