# Canonical slope products on idle workers — local development evidence

Decision: adopt **canonicalhelpers2** under
[ADR 0221](../../../../docs/adr/0221-idle-workers-in-canonical-slope-products.md).
Baseline production is **5a0f576dea44e928add5b5ff39d42a0f8d4fae48**.
All 567 computational inputs of directionalscore2 match that commit; all 567
current inputs match the measured canonicalhelpers2 snapshot. No computational
input changed after its build. This is a bounded Mac development milestone, not
frozen-suite or release qualification.

## Identities and reproducible evidence

|Artifact|Plugin SHA-256|Role|
|---|---|---|
|directionalscore2|aec92cd86fc9152bf06de471eafb1f8aedc3e976e1391d0901310c7bb70b5a73|Committed baseline|
|canonicalhelpers1|e31e5a15ef6fadc73eb8b57b374dfec0170499e00fc07a9d726584e5509ffaef|Rejected first candidate|
|canonicalhelpers2|a41963955baf6600dd86a20f97e84bdee1f79f722e4e9de4bdfb15cdefd88f35|Accepted candidate|
|slopecanonicalprofile1|48a684ebceeb7db13d0574e90269b3f360a887e9bcb40ae8d525bbdc8620f83a|Instrumented baseline|
|slopecanonicalprofile2|a935cee49a0f958532c736f59e2098d5770122e07b842eb318691cbbb462a0f6|Instrumented first candidate|
|slopecanonicalprofile3|6749df4757f2680d018326cd68fe0632243af8f39a84450217ecaa5f924f24ef|Instrumented accepted candidate|

Builds use isolated targets and pass the ABI panic-containment self-test.
Instrumented results are never substituted for production headline timings.
The pinned, unmodified fixest 0.14.2 archive SHA-256 is
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`;
the selected stock library SHA-256 is
`bece258f6c6fc3fc47accea45c7b004b0c495d46170d78fa2d6302e66de5fbf9`.
The comparable reference uses `glm.tol=1e-10`, `fixef.tol=1e-11`, matched
threads, robust inference and fitted means. BLAS threads are one. Default-reference
accuracy discrepancies remain separate evidence. Ordinary fixest weights are not
used as a Stata frequency-weight inference oracle.

The [immutable receipt archive](receipts-content-addressed.zip) contains 11,068
logical files in 2,371 unique payloads; SHA-256
`610b29c126e6f6dd105511bd542e805ff7b9dbcb6f23055aca8c0598630839b5`.
[Verification](archive-verification.json) records a complete restore and hash/privacy
audit. Generated DTA, binary row outputs and saved-output CSVs are hash-recorded
and remain local; source snapshots, controllers, declarations, scientific summaries,
all timing attempts, fixture return codes and filtered terminal protocols are
retained. No raw licensed log or private machine path is committed.

Restore to a new directory with:

```bash
python3 benchmarks/reports/development/ppml-canonical-helpers-local-2026-09-26/restore_receipts.py NEW_DIRECTORY
```

Receipt paths below refer to that restored directory. Top-level `trace`, `science`,
`pilot` and `paired-trace` concern baseline diagnosis or the rejected candidate.
**All accepted-candidate results are under `final-candidate/`.** Six full source
snapshots and identities are under `artifacts/`; build and instrumentation recipes
are retained. Rust test binaries are hash-recorded from separate target directories.

### Reference source attribution correction

The previous score report's two files labeled `pinned-stock-source` came from an
instrumented diagnostic tree. They were not the exact stock source. Its benchmark
controller selected the separate `R-stock` library; that binary still matches the
[original committed receipt](../one-fe-profile-local-2026-09-19/identities.json)
and differs from `R-traced`. This milestone records exact source extracted directly
from the pinned archive and adds a
[source-label adjudication](../ppml-directional-scores-local-2026-09-26/reference-source-adjudication.json).
Neither older archive is rewritten. This corrects the archived excerpt label; it
does not change selected comparator binaries, tolerances or measured times.

## Diagnosis and adopted implementation

The current slope path already uses rank-aware current-weight QR, a coefficient-
space Schur CG solve, prepared compact/dense coupling, independent column stopping,
joint ordinary Newton directions and generation-scoped geometry reuse. It is no
longer the original repeated-geometry observation-space MAP route.

Ten initial diagnostic fits showed that the low-memory four-slope target admitted
seven workers. Its control batches used six, while each separate response call
used one. Approximately nine ms of compact products lay in those two response
calls. Packing and square-root preparation were small; controls were not being
serialized in narrow windows. Measured columns needed one fresh reconstruction
check each and no observation-operator fallback steps.

For one RHS only, the candidate borrows the idle workers' existing coefficient
product and eliminated-group scratch. The ordinary joint solver already used
this mechanism. It partitions independent eliminated groups and merges partial
products in worker order. Multiple RHS and single-worker calls retain their
original direct product and scheduling, selected at compile time in one shared
recurrence. Dense coupling or unusable helpers retain direct products.

No geometry is reused outside its existing weight/design generation; no previous
solution becomes acceptance evidence. Original-input reconstruction, FE projection,
model-space, rank, finite-solve, score, objective and exact final-Hessian conditions
remain. There are no new numeric banks, worker pools or memory allowances. The
change serves canonical PPML slope response calls; public OLS routing is unchanged.

### Rejected first candidate

canonicalhelpers1 sent every column through the helper-capable product, including
those with no helpers. Its pilot improved the low target 112 to 108 ms but lost
wide fast 185 to 191 and serial 248 to 254. All scientific comparisons passed.
Paired diagnostic operator sums were about 10–14% slower on those excluded paths,
with identical iterations and check counts. canonicalhelpers2 selects the direct
or shared product at compile time and retains original multi-column dispatch.
No thresholds, graph rules, layouts or favorable seeds were tuned. Both candidates'
builds, science and timing evidence remain in the archive.

## Public timings

All times below are matched whole-command milliseconds including robust inference
and fitted means. Input generation and warmup are outside timing. Arms alternate
by rotation. Headline fits ran without concurrent compilation, profiling, archiving
or other fits; sampled monitors report no overlaps or monitoring failures. No
application failure, timeout or incomplete pair occurred. Sampled process monitoring
does not exclude every source of ambient OS variation.

[Ordered distributions](distributions.md) give every observation and C/stock range.
B/C/S denote directionalscore2/canonicalhelpers2/comparable stock. Three rotations
per case; medians are not pooled across designs.

|Pilot, seed202609347; N/K/T|B|C|S|C/S, all pairs|
|---|---:|---:|---:|---:|
|Unbalanced three FE, four slopes, low; 38400/6/8|112|106|117|0.899–0.929|
|Same design, fast|101|102|119|0.850–0.872|
|Unbalanced three FE, two slopes, fast; 76800/10/8|186|185|294|0.623–0.660|
|Two FE, two slopes, low; 30720/6/4|70|68|120|0.567–0.592|
|Four slopes, serial low; 38400/6/1|248|247|555|0.442–0.451|
|Disconnected categorical two FE, low; 61440/6/4|92|91|100|0.910–0.980|

Pilot native pairs: **10 wins, seven losses, one tie**. Four of six families meet
0.90 stock in every pair. The target gains 5.4% against baseline, with all three
native pairs improving, but its median is still 0.7 ms above 0.90 stock. Fast
four-slope median loses one ms. All 18 direct stock comparisons pass science.

Fresh seed202609452 was declared before initial timing inspection. Its cases
were preserved through the first candidate's rejection and second candidate's
measurement; it is now consumed and must not become a tuning seed.

|Fresh; N/K/T|B|C|S|C/S, all pairs|
|---|---:|---:|---:|---:|
|Unbalanced three FE, four slopes, low; 48000/8/4|167|158|329|0.477–0.489|
|Unbalanced three FE, four slopes, low; 61440/10/8|208|197|300|0.651–0.682|
|Four slopes, serial low; 48000/4/1|268|267|516|0.510–0.519|
|Disconnected two FE, two slopes, low; 23040/3/8|57|60|63|0.897–0.984|
|Weak slope barbell, low; 30720/6/4|61|60|3540|0.0169–0.0172|
|Wide two FE, two slopes, fast; 61440/14/8|155|155|280|0.543–0.567|

Fresh native pairs: **11 wins, four losses, three ties**. Five of six families
meet 0.90 stock in every pair. Both new four-slope targets improve about 5.3–5.4%,
with all six native pairs improving. The very large weak-barbell advantage belongs
to this particular design, not arbitrary weak graphs. All 18 direct stock
comparisons pass science.

One fixed-input five-rotation follow-up addresses the three-ms disconnected loss:
B/C/S medians **57/57/67**, native deltas **+1,0,0,0,0** and C/S range
**0.838–0.951**. All five stock comparisons pass. Full/warm input hashes match
the original case (`final-candidate/repeat-input-binding.json`). The original
loss did not recur; neither block is discarded, and not every pair reaches 0.90.
No further guard, code, layout, threshold or seed tuning followed.

### Protected OLS controls

Four cases, three rotations each; all 12 coefficient/covariance pairs are exact.
Two-FE slopes B/C **69/66** ms; three-FE categorical low **80/78**; three-FE
generated factors low **215/212**; N983040/T8 categorical fast **160/159**.
Individual pairs have **seven wins and five losses**. This is focused regression
evidence, not universal non-regression or a new OLS/fixest comparison.

## Final paired phase attribution

Three cases, two repetitions each, alternating instrumented baseline/candidate.
Values are mean ms from `final-candidate/paired-trace-summary.json`. These are
separate diagnostic fits; inclusive nested spans must not be added.

|Phase|Four-slope low B/C|Wide fast B/C|Serial B/C|
|---|---:|---:|---:|
|Whole plugin|113.951 / 110.568|185.097 / 187.385|239.498 / 240.330|
|Canonical construction|45.494 / 41.956|83.050 / 82.822|118.147 / 117.777|
|Column solving wall span|23.149 / 19.402|36.428 / 36.286|71.376 / 70.928|
|Slope solve wall span|26.558 / 22.790|44.021 / 44.216|75.700 / 75.320|
|Full MAP certificate|7.657 / 7.673|10.890 / 11.124|33.992 / 34.013|
|Operator time summed across columns|62.731 / 64.143|156.819 / 157.787|57.424 / 57.110|

The target column-solving wall span improves about 16%. Its summed operator
time increases, so this is improved scheduling rather than a claim of fewer
operations. Wide/serial direct operator sums no longer show the first candidate's
penalty, but their whole instrumented fits still lose. Per repetition, summed
column iterations remain 176/267/176 and fresh checks 14/23/14 in both arms.
No certificates were removed or made cheaper by this candidate.

## Science, memory and boundaries

- **19 distinct Rust tests pass**, with unique target and test-binary hashes.
  The independent analytic WLS projection oracle exercises no/automatic/forced
  compact coupling, T1/T4/T8, RHS widths 7/1/3/7, changed/extreme weights,
  disconnected topology, rank deficiency, duplicate and zero channels and
  slope-only partitions. Failure/invalidation and capacity checks remain.
- **13 licensed fixtures pass**, checked through process/Stata return codes and
  terminal sentinels: OLS topology and singleton controls, runtime validator and
  transaction, categorical/three-partition-slope/higher-dimensional independent
  oracles, scientific contract, weights/inference, and T1/T4 low-memory audits.
  Actual categorical and slope 16-iteration audits have zero events and growth.
- **147/147 public states, 12/12 explicit Poisson/Hessian/sandwich comparisons,
  96/96 anchored comparisons and 45/45 direct candidate/stock comparisons pass.**
  Final totals are in `final-candidate/science-counts.json`. All 51 native PPML
  pairs pass numerical envelopes; 38 are fully byte-exact across coefficients,
  covariance and means. Parallel reduction may change low bits. All 12 OLS
  coefficient/covariance exports are exact.
- Observed peak heap, peak delta and scratch deltas are **zero** in pilot,
  fresh and fixed-input follow-up. Modeled bounds vary **−48..+48 bytes**, with
  unchanged allowances. There is no new numeric storage. This is not a physical
  memory-cap or platform qualification claim.

The inherited prepared-IRLS checkpoint-count failure (0 versus 1) was not rerun
or repaired in this milestone. Static allocation-inventory drift, wire-minor
issues, weighted constrained-fit failures and insufficient-budget/nonretaining
CG instability remain open. Prior mean/saved-output discrepancies are not
declared repaired by these different cases. Linux T1/8/16 and constrained-memory
confirmation remain deferred; no SCC retry or stopped campaign was started.
The frozen 77 primary, 16 holdout and 111 native-control definitions are unchanged.

## Overall position at this checkpoint

The coefficient-space slope redesign and subsequent work have made the tested
multipartition PPML slope families competitive, often well beyond the requested
10% margin. Current examples are 68/120 ms for two-FE slopes, 185/294 for wide
three-FE two-slope fits, and 158/329 plus 197/300 for fresh four-slope low-memory
fits. Those are current-source comparisons; they are not exact before/after
multipliers against older designs or seeds. The remaining smaller target is
106/117, and disconnected cases still miss 0.90 in individual pairs.

The preceding **5a0f576d** score checkpoint measured large categorical two-FE
N983040/K6/T8 at **845/868 ms fast** and **937/931 ms low**. These cases were
not remeasured against stock on canonicalhelpers2. At those earlier medians,
the 0.90 targets require another **63.8/99.1 ms**. Earlier source-bound diagnostics
put canonical construction around **226–252 ms**, within whole plugin spans of
roughly 832–898 ms; that inclusive cost is not all avoidable. This is a larger
remaining opportunity than tuning the last 0.7 ms on the small four-slope median.

Categorical OLS has strong historical evidence, but its old 93/93 median wins
belong to their captured artifact. This milestone only supplies four current
native OLS controls. Historical serial weighted-slope AW 226/188 and PW 209/152 ms
losses remain unremeasured and unrepaired; that tuning round stays closed.
Higher-dimensional, weighted, wide and constrained-memory coverage is incomplete.
There is no universal 10% advantage or qualified release claim.

After this candidate, the evidence supports remeasuring large categorical two-FE
PPML on the committed artifact, then targeting canonical construction and repeated
whole-command work against the measured 64–99 ms deficit. This report ends the
current experiment; no next optimization candidate was started before the user's
requested overall assessment.
