# Native preparation attribution

Eight accepted calls compare a diagnostic preparation build with its exact
uninstrumented `outcomefit` counterpart. All native scientific exports match
bit for bit. Two target measurements per width give these phase medians:

| Phase | K=10 | K=50 |
|---|---:|---:|
| Input metadata validation | 0.011 ms | 0.026 ms |
| Complete-sample checks | 2.141 ms | 4.365 ms |
| Numeric sample ownership | 0.679 ms | 0.645 ms |
| FE factorization/recipes | 3.372 ms | 2.521 ms |
| Singleton pruning | 4.218 ms | 4.099 ms |
| Final row compaction/validation | 0.789 ms | 0.794 ms |
| FE topology construction/validation | 5.675 ms | 5.645 ms |

The input and sample phases end before FE preparation begins; the final
topology phase also includes final categorical recipe resolution. These are
diagnostic boundaries, not separate command benchmarks. They motivate reuse
of same-call group membership/counts across one-FE singleton checks and final
topology construction. Such reuse is not implemented by this diagnostic build.

`phases.json` binds target phase values to source log hashes; `identities.json`
binds the instrumented sources and artifact. Both actual-plugin ABI checks
pass. No dedicated licensed fixtures were run for the timer-only artifact;
its uninstrumented counterpart has the focused fixture receipts. This is
local arm64 development evidence, not qualification.
