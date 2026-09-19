# Stata temporary-variable lifecycle

Eight accepted diagnostic calls preserve the native scientific exports exactly.
The private candidate marker is dropped explicitly before the instrumented
transaction ends, moving that cost inside timer 94. Two target calls per width
give the following medians:

| Operation | K=10 | K=50 |
|---|---:|---:|
| Candidate marker construction | 1 ms | 1 ms |
| Copy candidate to result sample | 2.5 ms | 9.5 ms |
| Hold/unhold prior estimates | <1 ms | <1 ms |
| Remove private candidate marker | 3.5 ms | 20 ms |
| Ereturn post, including FV information | 2 ms | 15.5 ms |

The explicit drop preserves e(sample). Moving it changes the location of the
time, and does not by itself establish an optimization. The result motivates
a guarded same-call ownership transfer of the private marker after metadata
parsing, when candidate_all already tells Rust not to read the candidate slot.
No such change is adopted by this diagnostic receipt.

A separate 18-call screen skipped buildfvinfo only for plain posted names.
Median baseline/candidate times were 189/187 ms for K=10 and 482/480 ms for
K=50; there was no material repeatable gain, and that branch was removed. Ten
full public/hidden transaction cases matched public macros, coefficient names,
scientific matrices and return codes. The initial audit only replaced the public
dispatcher and therefore did not distinguish the two hidden transactions; it
was insufficient for this change. The final audit loads a separately renamed
baseline transaction and compares all public macros as well. Earlier scripts
and logs remain in the ignored work root.

Both variants reuse the exact factor native plugin. These are focused local
development measurements, not qualification. No dedicated licensed fixture
sets were run for these diagnostic/rejected variants.
