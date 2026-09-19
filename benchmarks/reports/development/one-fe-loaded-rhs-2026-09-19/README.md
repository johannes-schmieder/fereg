# Construct the numerical RHS while loading

The bounded Mac input worker now builds the row-major RHS beside its existing
source-column/missing-mask conversion for eligible ordinary numeric, unit-weight,
one-FE IID calls. It appends into one fallibly reserved, admitted owner without
an initial full-matrix zero fill. The original FE source reaches its column
before its tile position becomes the generated constant. Preparation and RHS
validation remain authoritative: every candidate matrix value is checked bit
for bit against the final prepared columns before ownership moves to the fit.
Missing-value or singleton deletion, changed recipes, unsupported shapes, one
thread and insufficient optional budget retain the ordinary path.

The matrix is charged beside input scratch and throughout preparation. On entry
to fitting it occupies the existing RHS allowance; it is not counted as a second
fit matrix. The independently observed wide native peak remains 925,859,855
bytes in the primary cases. Existing opaque runtime/dependency coverage gaps
remain explicit. No public option, data cache, extra plugin call or installed
plugin is introduced.

A second change parallelizes dense integer FE range scanning and code filling
on the installed team. The first fractional/out-of-range/nonfinite observation
still determines specialization/fallback behavior. Canonical coding, signed
zero and level order are unchanged. No new reduction buffer is allocated.

Five fresh rotated primary blocks, after two three-block screens, give:

| Regressors | Committed scheduling baseline | Loaded RHS | Plus FE encoding | Pinned fixest |
|---:|---:|---:|---:|---:|
| 10 | 204 ms | 188 ms | 190 ms | 140 ms |
| 50 | 502 ms | 485 ms | 479 ms | 462 ms |

All primary cases have 983,040 rows and 16 native threads. The separate traced
three-block comparison gives 213/195/195/150 ms and 506/491/485/494 ms. Preserve
these blocks separately: fixest and native times vary between blocks, so a
near-win in one block is not evidence of the requested repeatable 5% win. That
target remains unmet on both cases in the confirmation block.

The initial phase screen reduces RHS construction from 20.65 to 5.60 ms for
10 regressors and 31.27 to 15.34 ms for 50. Narrow input falls 86.58 to 77.69 ms;
wide input remains about 257 ms. The measurements include allocation and every
new check. Eleven protected controls have three rotated blocks: the combined
candidate's maximum median ratio is 1.0094, with geometric mean 0.9709.

The main receipts contain 163 accepted calls; the separate initial screen adds
18. Every native scientific export remains bit-identical. Both candidate builds
pass actual-plugin ABI checks and three licensed native-input, independent-oracle
and transaction fixtures. Focused tests exercise RHS bit identity, stale values
and shapes, tail/missing conversion, caller-thread SPI, shutdown errors/panics,
exact/one-byte-short optional RHS admission, one-thread/reordered-recipe/missing
fallback, and FE code/error-order parity across parallel thresholds.

Release identities bind the exact production overlays to `054605b1`. The final
loader-boundary unit fixture was added afterward under `cfg(test)` only;
production source is identical. Failed test-development attempts are described
in [validation.json](validation.json), and their original local logs remain
untouched. This is focused local development evidence, not qualification.
