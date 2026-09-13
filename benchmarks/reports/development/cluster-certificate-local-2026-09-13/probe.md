# P3 categorical cluster certificate: initial unadopted probe

Source-bound initial evidence against accepted `257f4ce`; see [receipt](probe.json).
Categorical OLS/PPML cluster construction currently re-factorizes every interacted
cluster during certification. The prototype independently checks each existing
code against one representative source tuple and verifies strictly increasing
lexicographic representatives, proving membership, no splits/merges, contiguity
and canonical code order without constructing a second encoding.

One usize per observed group G fits the old dense factorization's 4*N output
codes plus 4*P map on the supported 64-bit targets (G <= N and G <= P), and the
fallback's 8*N order plus 4*N codes. No additional contiguity bitmap overlaps.
Simple clusters continue to borrow final categorical codes. Original-row
classification, singleton deletion and inference are unchanged.

Three isolated unit tests pass on the recorded prototype: exact-factorizer
comparison over row orders and repeated components; merged/split/relabelled/
missing/out-of-range group rejection; workspace bounds and allocation failure.
The initial build required an explicit Option<usize> annotation; that compiler
failure is preserved separately. No failing numerical test was rerun.

There is no candidate plugin, licensed oracle or command-performance evidence
for this prototype yet. It is not adopted. Existing covariance consumers already
share each subset ordering between production and certification. Low-memory OLS
QR overwrites its scratch matrix, so merely retaining that file cannot supply
an unchanged within design to inference. These observations do not close the
remaining P3 checkboxes.
