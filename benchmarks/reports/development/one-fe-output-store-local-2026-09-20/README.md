# Checked batch publication for OLS observation outputs

The retained change removes per-value Rust/C crossings and repeated thread
checks when publishing full-sample OLS observation outputs. It keeps the same
checked Stata SDK accessor, row/column store order and first-error behavior.
All thirty-one original-baseline regression cells pass. These are local Mac
development measurements, not release or cross-platform qualification.

## Incremental results and scope

Three rotated repetitions, 983,040 observations, 196,608 FE levels and sixteen
native workers on the M2 Ultra. Stata MP uses eight processors and external
BLAS one. Imports and audit exports are outside command timing. Builds,
tests and archive hashing did not overlap timing. The comparator is stock
fixest 0.14.2, pinned archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Command medians in milliseconds:

| Request | K | Prior checkpoint | Checked batch stores | fixest |
|---|---:|---:|---:|---:|
| Saved FE | 10 | 150 | 148 | 216 |
| FE, residuals, xb and xbd | 10 | 196 | 192 | 223 |
| Saved FE | 50 | 408 | 403 | 520 |
| FE, residuals, xb and xbd | 50 | 470 | 459 | 520 |

The candidate wins all twelve paired comparisons against its immediate
parent. Median paired candidate/parent ratios are 0.973856, 0.979592,
0.987745 and 0.976596, respectively. All twelve native allocation peaks are
identical to the parent; [the memory record](batch-store-memory.json)
retains exact byte counts. Full repetitions, dispersion and sampled RSS are
in [the timing archive](primary/timings.json). This screen is not either of
the two required final acceptance blocks.

Eligibility is a complete retained observation axis, at least 4,096 rows,
no requested drop-reason variable and at most 64 requested numeric output
columns. Final preflight has already validated all results and the identity
row map. Rust collects borrowed residual, prediction, absorbed and saved-FE
columns in fixed stack arrays. The C adapter checks all coordinates, column
pointers and callback availability before the first store, then uses exactly
the SDK callback selected by `SF_vstore`. It does not switch to an unchecked
accessor. SPI calls stay on Stata's main thread. Matrix publication is
unchanged, and receipt COMPLETE remains the last store.

The implementation applies to OLS publication across covariance types,
weights and FE representations when those eligibility conditions hold.
Partial samples, small samples, drop-reason requests, excessive output width
and unsupported SPI adapters retain their prior path. The real adapter enables
the batch path on macOS only. PPML does not call this helper. There is no new
heap owner, public option, protocol revision or installed-package change.

## Rejected experiments retained in the archive

Fusing legacy RHS finite checks with exact speculative-row matching showed
an initial narrow IID median of 125 versus 127 ms. A separate five-pair
confirmation returned 127 ms for both arms, with candidate/parent paired
median 1.007874 and only two wins. The prototype was removed completely;
[its patch](rhs-fusion-unadopted.patch), ten passing focused tests, three
passing Stata fixtures, all initial and confirmation runs remain recorded.
The mixed wide screen is not evidence of a universal absence of gain.
A sanitized [background CPU snapshot](rhs-finite-screen-background.json)
records Finder, UI and backup activity during that screen; no runs were
excluded or processes stopped.

The earlier pure-ado deferred-initialization experiment was separately
revisited without the previously rejected native excluded-row stores.
Complete-output medians improved from 201 to 198 ms (K=10) and 480 to 469 ms
(K=50), but the 95%-missing control worsened from 146 to 150 ms. The
half-missing wide control worsened from 434 to 450 ms and lost all three
pairs. Those cells do not breach the formal 5% paired-ratio limit, but the
tradeoff is unattractive for the small narrow gain. The ado change remains
unadopted; [its patch](ado-nofill-unadopted.patch), three passing fixtures and
all full, half-missing and sparse runs are preserved. Production ado is
unchanged.

## Regression and scientific evidence

Twenty breadth cells, three alternating pairs per cell, pass against the
original `normonly` baseline: maximum median paired ratio 0.969231 and
geometric mean 0.669356. Eleven size/thread/group-layout controls also pass:
maximum 1.030303 and geometric mean 0.989931. No confirmation is required.
These are cumulative campaign gains, not gains attributable solely to batch
stores. [The assessment](regression-assessment.json) lists every cell.

All sixty breadth candidate results match the prior `widecore` scientific
arrays exactly, including samples, coefficients, covariance, statuses, DoF,
fit and certificates. Twenty-four saved-output bundles are byte-identical.
All thirty-three nonself size/thread/layout comparisons are exact against
original `normonly`. The inherited original-baseline saved-FE fit/certificate
norm differences remain as adjudicated in the
[validated-owner checkpoint](../one-fe-validated-owners-local-2026-09-20/README.md);
this change does not reinterpret them or weaken comparison tolerances.

The native publication unit test compares complete scalar and batch store
sequences at 4,095, 4,096 and 4,097 rows, with complete and missing samples.
Injected failures cover early and late numeric stores, the first matrix
store and COMPLETE. The initial test failed because its fixture removed a
drop-reason variable without renumbering the remaining output bindings;
production correctly rejected the invalid varlist. The corrected test passes
on the final production source. An earlier corrected run also had the RHS
prototype present; its source hashes are preserved separately and it is not
substituted for the final-source run.

The C guard test passes under UBSan, covering worker-thread refusal, invalid
shape/coordinates/string destinations, null buffers/callbacks, exact value
bits and each of twelve possible store failures. Three final-artifact Stata
fixtures pass with explicit terminal sentinels: final FE recovery with an
independent dummy oracle, private-output rollback, and e-class transaction.
The first two use 16,400 rows and exercise the new threshold with full and
filtered samples, numeric/string/interaction FE and AW/FW/PW requests.
The final artifact also passes actual-plugin ABI and panic checks.

Final `batchstorechecked` plugin:
`76f8ec6018fa047fc1277880120502dc9499a9f4deba72ee900279a359d1161e`,
built from `03906246` plus the six recorded publication/test overlays.
The first `batchstore` build preceded the fixture correction and was never
timed or used for licensed fixtures. The build helper embeds September 19;
these executions and collection occurred September 20. No remote state,
release qualification or installed package changed.

## Preparation profile and next experiment

Eight diagnostic calls compare unchanged publication production with an
isolated instrumented build. All four nonself scientific comparisons are
exact. Narrow loading spends 68.8–69.0 ms inside main-thread SPI reads;
consumer copy/missing/RHS work totals 39.1–40.2 ms, while the producer waits
only about 7 microseconds for free tiles. Wide equivalents are 211.6–212.8 ms
SPI, 172.4–174.7 ms consumer work and 0.07–0.29 ms producer waiting.
These times overlap and must not be added.

After loading, narrow preparation takes 9.6–11.5 ms while the concurrent raw
moment job takes 7.1–7.6 ms. Its norm pass takes 4.7–5.0 ms and means
2.1–2.7 ms. The preparation classification/factorization join takes 5.0–7.4 ms,
followed by about 1.9 ms for ordered membership and 1.4 ms for topology.
For wide models, preparation takes 23.8–27.8 ms and moments 22.4–23.9 ms.
The observations support reducing FE/sample preparation before attempting to
hide only the raw norm pass under loading: doing the latter alone would leave
the current preparation critical path. The consumer has some headroom, but
its serial headroom is not evidence that a parallel reduction's work will fit.

The diagnostic helper now recognizes the shared preparation helper and reports
individual jobs, norm/mean passes and producer/consumer work. Its first build
stopped on an outdated nonunique insertion marker before compilation; a more
specific marker fixed it. The second diagnostic artifact passed ABI checks
and all eight application calls. No production timer or algorithm changed.

This archive contains 317 successful application calls: 105 primary probe
calls, six sparse and twelve half-missing controls, 120 breadth regressions,
66 size/thread/layout controls and eight phase calls. There are 141 nonself
exact native comparisons and 51 byte-identical saved-output comparisons.
[Validation](validation.json) binds source hashes, logs, fixtures and counts.
The comfortable-win goal remains active; no final acceptance block was run
for this candidate. The prior failed block is preserved in the wide-overlap
report. The next bounded experiment is ordered numeric factorization with
identical canonical levels/codes and unchanged fallback/error behavior.
