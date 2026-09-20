# Numeric SPI loop and completed one-FE comparison

The frozen candidate passes the local comfortable-win target in all twenty
one-FE OLS cells, in both independent five-pair blocks. Every cell has a median
paired fereg/fixest runtime ratio at most 0.844278; all cells win at least
nine of ten pairs. Across the blocks, fereg wins 199 of 200 pairs.
The full thirty-one-cell regression refresh passes. These are local Mac
development results under the [unchanged acceptance plan](final-plan.json),
not release or cross-platform qualification.

## Final command timings

M2 Ultra, 983,040 observations, 196,608 FE levels, K=10/50, sixteen native
workers, Stata MP eight processors, external BLAS one, and pinned stock fixest
0.14.2. Each arm starts in a fresh process, runs a small warm-up and uses the
same audited numeric input. Arm order alternates by pair. No builds, tests,
kernel probes or archive hashing overlap timed runs. The implementation and
binary remain fixed for block A, the regression refresh and block B.

Times include Stata-to-native input, estimation, inference and requested
publication. Full outputs include public prediction of xb/xbd plus residuals
and absorbed/saved effects. R includes feols, requested fixef recovery through
fitted group codes, normalization and requested observation vectors. File
imports and audit exports are excluded from both timers. The explicit
small-sample corrections and numeric/output audits are unchanged from the
[breadth protocol](../../../one_fe_breadth/README.md).

Both time columns show **fereg / fixest**, in milliseconds. The ratio column
contains median paired ratios, which need not equal the ratio of medians.

| Request | K | Block A ms | Block B ms | Paired ratios A / B | Wins |
|---|---:|---:|---:|---:|---:|
| IID | 10 | 104 / 141 | 103 / 142 | 0.730 / 0.732 | 10/10 |
| IID | 50 | 374 / 455 | 368 / 458 | 0.820 / 0.802 | 10/10 |
| Robust | 10 | 119 / 170 | 116 / 167 | 0.702 / 0.695 | 10/10 |
| Robust | 50 | 491 / 863 | 490 / 853 | 0.565 / 0.574 | 10/10 |
| Cluster at FE level | 10 | 118 / 180 | 118 / 179 | 0.661 / 0.659 | 10/10 |
| Cluster at FE level | 50 | 473 / 927 | 470 / 944 | 0.502 / 0.504 | 10/10 |
| One crossing cluster | 10 | 129 / 180 | 128 / 184 | 0.694 / 0.690 | 10/10 |
| One crossing cluster | 50 | 488 / 821 | 488 / 807 | 0.593 / 0.607 | 10/10 |
| Two crossing clusters | 10 | 160 / 258 | 159 / 259 | 0.625 / 0.614 | 10/10 |
| Two crossing clusters | 50 | 551 / 1333 | 552 / 1288 | 0.413 / 0.427 | 10/10 |
| Two clusters, nested FE | 10 | 148 / 236 | 148 / 241 | 0.624 / 0.618 | 10/10 |
| Two clusters, nested FE | 50 | 568 / 1246 | 567 / 1250 | 0.457 / 0.450 | 10/10 |
| IID + saved FE | 10 | 124 / 217 | 125 / 218 | 0.560 / 0.574 | 10/10 |
| IID + saved FE | 50 | 394 / 523 | 398 / 527 | 0.752 / 0.755 | 9/10 |
| FE-level cluster + saved FE | 10 | 146 / 254 | 148 / 259 | 0.575 / 0.571 | 10/10 |
| FE-level cluster + saved FE | 50 | 504 / 998 | 509 / 1001 | 0.504 / 0.513 | 10/10 |
| Two clusters + saved FE | 10 | 194 / 325 | 192 / 336 | 0.591 / 0.562 | 10/10 |
| Two clusters + saved FE | 50 | 599 / 1363 | 593 / 1372 | 0.440 / 0.430 | 10/10 |
| Full outputs | 10 | 166 / 223 | 165 / 221 | 0.744 / 0.755 | 10/10 |
| Full outputs | 50 | 450 / 530 | 451 / 537 | 0.844 / 0.834 | 10/10 |

Per-cell median paired runtimes are 15.6–58.7% lower
than fixest across the two blocks. Individual runs, ranges, sampled RSS,
input/output hashes and identities are retained in [block A](final-a/timings.json)
and [block B](final-b/timings.json). The 605 ms block-A wide saved-FE run lost
to fixest and remains included. No outliers are dropped. The earlier failed
widecore final block remains in its original report; it is not reclassified.

## What the latest change contributes

The C numeric row loop makes four explicit SDK calls per main-loop iteration,
then handles the remaining columns in a scalar tail. It makes the same number
of SDK calls, in the same row/column order, with an immediate return-code check
after every call. All bounds, numeric-type, main-thread, pointer and shape
guards remain. The checked Rust/C boundary, missing-value handling and native
arithmetic are unchanged; no heap allocation is added.

Five rotated narrow triples isolate this change against the preceding
ordered-factor checkpoint. IID medians fall 126→103 ms and full outputs
186→166 ms; all ten parent comparisons win. Input phase medians fall
69.594→48.635 ms for IID and 69.368→47.545 ms for full outputs, while native
times remain approximately 43–44 and 56–57 ms. The measured gain is in loading,
not a weaker solve or a changed numerical result. This screen does not establish
the same incremental gain at every width. The final table includes cumulative
improvements from the earlier prepared-owner, overlap and output-store work.

The change benefits every caller of the Mac numeric row loader, independent of
VCE or output request. Its current OLS pipeline retains existing admission,
sample, source-width, numeric-source and thread guards. Single-thread, partial-
sample, string-source and column/scalar-loading paths do not automatically use
this loop. The row helper remains unavailable on other platforms. Shared core
changes such as ordered factorization apply to OLS and PPML, but this report
does not establish a PPML or other-platform command speedup.

## Regression and scientific evidence

Against the original normonly baseline, the twenty breadth cells have maximum
and geometric-mean paired ratios 0.966234 and
0.616879. The eleven size/thread/group-layout
controls give 1.010582 and
0.947205. All meet the predeclared 1.05
cell / 1.02 aggregate limits; no confirmation run is needed. This permits small
timing variation, not a claim that every individual control got faster.

The archive contains 616 successful measured application calls: thirty in the
incremental screen, 200 in each final block, 120 breadth regression and 66
controls. There are 303 exact nonself native comparisons of b, V, status, DoF,
fit and certificate arrays plus exact samples, and 109 byte-identical saved-
output bundles. All 270 allocation-peak comparisons with the nearest archived
scientific parent are identical; the eleven controls retain their individual
[allocation comparisons](control-memory.json). Cross-estimator comparisons
retain the registered numerical tolerances. Historical baseline fit/certificate
rounding differences inherited before this change are not called exact:
the final/breadth native comparisons use the archived batchstorechecked parent.

The final artifact passes actual-plugin ABI/panic checks and three licensed
Stata fixtures, including independent dummy/input oracles, wide/narrow and
thread-boundary cases, weights, filtered/missing inputs, string clusters and
transaction rollback. Existing scientific checks are not weakened. Expanded
UBSan C mocks cover every width 1–64, three rows at first observations 1 and
65,538, complete callback/value order, buffer sentinels, and failure at every
possible read position with no later destination touched. The same suite
retains output-store guards. [Validation](validation.json) and
[fixture receipts](fixtures.json) bind these checks to source and artifact.

The preliminary compiler-pragma variant built and passed its fixtures but was
never timed. Assembly inspection showed a width check after each call, so it
was replaced by the explicit main loop before the screen. Its
[patch](untimed-pragma.patch), [identity and fixtures](untimed-pragma.json) remain;
there is no measured speed claim for it. An initial direct test-script launch
failed because the script was not executable; invoking it with bash passed.
Those logs remain hashed in validation.

Candidate plugin: `4437ffac11ed4018ced4cadf934d82cbe9bc09eaaa44505d446d50de5e86abb7`.
Source: `6f3fd059d6244acbfd7efb70b695a004254459f7` plus the C bridge and mock-test overlays
listed in validation; all 512 source entries and the final
working production files were verified. The helper embeds September 19;
execution and collection occurred September 20. No installed package, remote
state, release or qualification was changed.
