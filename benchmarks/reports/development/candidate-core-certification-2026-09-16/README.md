# PPML certification lifetime and validation-mask accounting

This development checkpoint releases obsolete PPML estimation state before
independent certification and includes omitted validation masks in the checked
publication allowance. It passes focused native, mock-SPI and licensed Linux
checks. It does not establish complete whole-action memory coverage or qualify
a production candidate.

## Change and scope

Once the certification input owns its products, the estimator releases its
final Hessian/fit, prepared generation, original separation reasons and mapping
temporaries. The independent certifier then runs without those obsolete owners.
The existing conservative preflight parent reserve remains in place.

The checked validator workspace now includes the original-row generation mask,
candidate-row active/reason replay and simultaneous reporting-slot/saved-channel
masks. It uses immutable original/candidate axes after separation reduces the
final sample. Sequential validators combine by maximum; simultaneous masks add.
The method-attribution mask is explicitly released before independent hash
replay. PSD/Wald rules, tolerances, samples and publication semantics are unchanged.

The [incremental patch](incremental-implementation.patch) isolates the four Rust
files changed since the preceding checkpoint. OLS source contract v26 and action
inventory v9 retain the new allocation inventory; earlier manifests remain
unchanged. Public field definitions and wire layout are unchanged. Computed
publication allowances increase where they previously omitted these masks.

## Identity and validation

The source snapshot is `5240e58-core-certification1`, based on main
`5240e589400c888690666265263ad96f9b694aa6` plus the preserved uncommitted changes.
The [identity manifest](receipts/identity.json) binds 290 candidate files, 291
baseline files and the reference lock. The baseline is the unpublished
`5240e58-publication-lifetime4` checkpoint, not committed main.

- Baseline Linux plugin SHA-256:
  `c3ada630de1278c771bb7c128591ff4e9104dae3abc8e0584475598966116e83`.
- Candidate Linux plugin SHA-256:
  `a8e5a0078de990c22eb581566f40d58e20cd1b64e90ec4792a2794cc420208f3`.

All **260 distinct native tests** pass: 128 plugin, 46 PPML memory, 30 PPML result
and 56 prepared-estimator tests. The new ownership probe checks plain,
weighted/clustered saved-output, actual exact-FE separation and low-memory
fixtures. Publication-end live allocation equals live allocation after core
return; the explicit source drops establish release before certification.
The new mask fixture tests a small final sample with one million original rows
and different candidate counts. Existing exact/one-byte-short, allocation-failure,
interruption, repeated-call and rollback checks continue to pass.

All **38 Python checks**, repository checks, OLS v26 and action v9 audits pass.
The native test executables are [hashed separately](receipts/final-artifact-verification.json).
Linux export and actual-loader checks pass. The audit-enabled loader emits 287
valid terminal records: three OLS successes and 284 failures, including one PPML
syntax error. Successful PPML terminal tracing is not exercised by this loader;
separate passing mock-SPI and licensed fixtures cover PPML publication.
The preexisting prepared-estimator test formatting difference remains recorded
in [local/remaining-format.txt](local/remaining-format.txt); no full rustfmt pass
is claimed.

Job **7598585** passed the 260 native tests and compilation/export audit, then
exited 126 because the snapshot copier had dropped the loader script's execute
permission. Its original failed receipt is preserved. Restoring source modes
changed no file bytes. The one focused wrapper retry, **7598655**, reran the build
wrapper and loader, verified the candidate binary was byte-identical, and ran
the licensed comparisons. It finished in 61 seconds, scheduler failed=0/exit=0,
maxvmem 1.394G. The initial build job used 240 seconds and 3.245G maxvmem.
Both jobs used the requested 28-core reservation; execution was capped at four
physical cores. No additional retry or empirical estimation ran.

## Fixed paired costs

The [fixed manifest](receipts/fixed-cases.json) predates measurement. Three
alternating fresh-process pairs ran for each of the same three controls:
saved-slope low-memory OLS, narrow robust PPML, and frequency-weighted clustered
PPML with saved effects. Input, generator and harness hashes are retained in
[input bindings](receipts/input-bindings.json), [requests](receipts/requests.json)
and [generated-data bindings](receipts/generated-bindings.json).

All **nine pairs / 18 licensed commands** preserve exact scientific matrices,
sample/order, solver non-time fields and saved outputs. Worker return codes,
terminal sentinels and absence of Stata errors are independently checked.
Thirty-six scheduler snapshots confirm exclusive scheduled node occupancy before
and after each command. No full German reference fit or stopped campaign restarted.

| Complete-command median | Baseline | Candidate | Change |
| --- | ---: | ---: | ---: |
| Saved-slope OLS, low memory | 0.319 s | 0.296 s | -23 ms |
| Narrow robust PPML | 0.124 s | 0.126 s | +2 ms |
| Weighted/clustered PPML, saved effects | 1.053 s | 1.033 s | -20 ms |

No command or reported phase median crosses the declared slowdown threshold of
both 5% and 10 ms. Narrow PPML strict-final-solve time rises by about 4.5 ms,
below the absolute threshold. The earlier saved-slope flag remains cleared for
these bounded development comparisons; all historical flags are preserved.
The OLS control's timing improvement cannot be attributed to a PPML source change.
These observations support absence of a material timing regression in the fixed
cases, not a general speedup claim.

[Phase/memory records](receipts/phase-memory-summary.json) and the
[additional phase summary](cost/extra-phase-summary.json) retain all observations.
Saved-slope native peak is unchanged at 3,301,780 bytes, with 1,573,056 scratch
bytes. Narrow PPML peak is unchanged at 2,429,937 bytes. Weighted saved PPML
peak medians are 64,688,484/65,723,031 bytes, an increase of 1,034,547 bytes;
this checkpoint does **not** demonstrate a reduction in whole-command peak.
The changed publication allowance rises from 2,365,632 to 2,380,680 bytes for
narrow PPML and from 37,411,355 to 37,538,467 for weighted saved PPML. Other
stages still determine each case's selected memory estimate.

Sampled process-RSS medians in GiB (baseline/candidate) are 0.0734/0.0745 for
slopes, 0.0716/0.0738 for narrow PPML and 0.1352/0.1369 for weighted PPML.
Both PPML cases retain zero scratch and fallback counts, with identical inner
iteration/step-halving counts of 6/0 and 49/0. Exact non-time solver diagnostics
show no new retry/discarded-attempt behavior. Independently computed phase
medians need not sum. These observations and existing public base ratios do
not certify whole-action coverage or establish a cap breach.

## Remaining scope

Whole-action coverage remains incomplete and its ratio remains undefined.
Construction/decoder/string storage, numerical and old/new PPML generation
overlaps, nested dependency/worker lifetimes and remaining result assembly
still need complete capacity/admission proofs. Passing these fixtures does not
clear an unknown-coverage bit. The Xcode-license prerequisite, exact candidate
freeze, universal Mac/Rosetta checks and deterministic Mac/Linux review-package
installation checks remain. Windows and formal release qualification are later
gates. No commit, push, tag, release or installed-package replacement occurred.

The installed Mac plugin remains
`4187efdfbd6235e63c369d9b42dede6381b8a9e50b5360fd63ee237bd6c11b08`.
All checkpoint jobs are terminal; no monitor remains for them. Raw licensed logs
remain in the preserved run directory. This report stores synthetic diagnostics,
sanitized native/scheduler receipts and hashes of original evidence.
