# Saved-FE recovery admission: tested prototype, still open

The five-file candidate remains **unadopted**. It extends the existing saved-Schur
prototype with route-specific OLS intercept-recovery admission. It does not
finish P0/P6, qualify whole-action memory, or adopt the saved-Schur optimization.
Accepted numerical source is `dadd956`, at evidence HEAD `f32c5b2`.

[Source inventory](v2-source-manifest.json), [implementation](implementation.patch)
and [built identities](v2-built-identities.json) bind 176 inputs. The incremental
baseline is the already unadopted saved-Schur plugin `18427c19`; candidate is
`247f889a852b618d6f0884048c19318141194e117b799c1d96e01064e4a11161`.
The patch is relative to accepted main and includes the original two-file saved
route. The other changes are memory accounting, recovery admission and a
`cfg(test)` enum re-export. The installed plugin remains unchanged.

## What is implemented

OLS recovery receives its existing joint execution/solver budget. Finalized
intercept topology supplies N, level counts and requested channel count;
planning uses the installed Rayon team. Only FE-scale metadata is allocated.
Retained normalization, retained CG, SD/remainder-LSMR, generic LSMR and MAP
have separate incremental reserves. Each route is admitted before its working
allocations. An over-budget route is skipped; an already certified retained
product or the cheaper MAP fallback does not require an unused Schwarz reserve.
If no route fits, recovery returns the allocation error before public storage.

Normalization/finishing conservatively reserves `8N(q+4)+128L+256D+4096` bytes
for requested channels, target/residual/combined products, component gauges,
coefficients, row/level certificate partials and metadata. CG adds the existing
one-RHS estimate and five coefficient products. SD-LSMR adds its still-live
residual/direction/group/two-coefficient frame and the existing LSMR bound.
The one-FE direct SD route does not reserve LSMR. Generic LSMR uses the existing
maximum preconditioner estimate, which includes locality copies, source/sorted
weights and construction scratch. Earlier retained-versus-maximum-only planning
is preserved in [previous versions](previous-versions/first-budget-version.patch).

Explicit-Hessian PPML recovery still passes no new budget and retains its
existing allocation propagation and disabled LSMR policy. Absorbed slopes are
outside this new model. No recurrence, tolerance, sample/weight/rank/VCE/DoF,
normalization, certificate or publication arithmetic is changed.

This is a candidate incremental model. It inherits the pinned Schwarz model's
opaque-factor slack; it is not a new proof of every upstream allocation. The
caller execution charge remains conservative across its prepared/final-fit
state. Public selected-memory reporting still omits initial solver increments.

## Correction to the preceding memory diagnosis

The [preceding screen](../saved-schur-probe-local-2026-09-13/repaired-baseline/README.md)
suggested retained inference score storage overlaps final recovery. That was
not established and is **incorrect for these ordinary robust/clustered fits**.
`prepared_scores.rs::build_prepared_ols_score_design` consumes the fit cache;
`prepared_robust.rs` and `prepared_clustered.rs` drop the score design before
returning the inference result. The earlier receipt is preserved, not rewritten.

A separate [instrumented plugin](diagnostic.patch) opens an existing secondary
allocator window around recovery without resetting the whole-action peak.
It changes only plugin diagnostics relative to the candidate and builds for
both architectures. The exact original PW request, data and warm-up are reused.
[Diagnostic results](diagnostic-summary.json) show:

| Original PW request, N244274 retained/K50/16 workers | Native bytes |
|---|---:|
| Whole-action peak, already reached before recovery | 581,409,185 |
| Live allocation on recovery entry, relative to action baseline | 126,919,661 |
| Peak during recovery, relative to action baseline | 140,701,653 |
| Additional allocation above recovery entry | 13,781,992 |
| Unchanged public selected model | 424,694,088 |

Recovery takes 0.6972 seconds in this diagnostic; its 2.665-second command is
excluded from ordinary comparisons. Full b/V, sample/status/DoF and saved
exports exactly match the original prototype's capture. Initial solver fields
retain the charged 8-step probe, 64-step Schur attempt and 66-step CG continuation.
The 581 MB discrepancy therefore precedes recovery. The source's initial
Schur/CG joint admission includes a solver increment that public reporting
omits; it must not be confused with the reported 424.69 MB base. The diagnostic
localizes the peak to earlier work, without identifying its exact allocation site.

## Constrained-budget and independent validation

The [prospective fixture](cap_protocol.md) uses 4096 rows, four FE levels and no
finite controls, with a full-dummy OLS oracle. The execution estimate is 212,992
bytes and the minimum recovery reserve 168,704. After 512 MiB headroom, exact
admission requires 537,252,608 available bytes.

[Four public checks](cap-validation.json) pass: the baseline succeeds uncapped
and one byte below the new boundary; the candidate returns r(909) below it and
succeeds exactly at it. Rejection identifies recovery admission and preserves
prior e(b), e(V), e(sample), row order and absence of requested output variables.
At the exact cap, [separate telemetry](cap-telemetry.json) observes 362,557 native
bytes against 381,696 usable. This is a named boundary, not a universal bound.

[Validation](validation.json) also records:

- 13 focused core recovery tests, including exact-boundary CG/MAP selection,
  failed retained-product fallback, correct component normalization with one
  and four workers, invalid dimensions/overflow, and PPML Hessian/atomicity tests.
- Five focused plugin saved-output tests, including store-failure transactions.
- All 48 independent saved-OLS dummy-oracle fits and rollback on the candidate:
  one/two FE, unit/AW/FW/PW, fast/low and saved/requested residuals. The identical
  old prototype's prior oracle receipt remains separate.
- Both architecture builds/static audits and native arm64 ABI/panic containment.
  No x86 execution, SCC, Windows or broad qualification ran.

The earlier new-test type-name failure and the later missing test-only enum
re-export are retained with their corrected passing logs. No existing tolerance
was loosened. Licensed logs remain outside Git and are represented by hashes
and explicit terminal-sentinel verification.

## Incremental cost screen and unresolved regression

The [screen protocol](cost_protocol.md) compares the two uninstrumented prototype
artifacts, not accepted main. Six fresh rotated pairs preserve exact full science,
non-time solver fields, native peaks, selected models and scratch. The original
unit screen increases 0.682→0.718 seconds (+36 ms/5.3%); PW is 2.573→2.586,
weak 0.044→0.044, weighted cut 0.052→0.053, low FW 0.208→0.206 and small saved
AW 0.005→0.005. The first unit increase triggers two additional seeds:
2.956→3.289 (+333 ms/11.3%) and 0.673→0.697 (+24 ms/3.6%). All remain evidence.

The +333 ms case rejects the Schur probe in both arms; 317 ms of the increase
is in initial CG solving, before the new recovery check executes. The separate
[bounded follow-up](cost_followup_protocol.md) gives:

| Same fixed request | Comparison | Seconds | Change |
|---|---|---:|---:|
| Rejecting unit seed8675309 | Identical binary A/A | 2.993→3.016 | +23 ms/0.8% |
| Original accepting unit seed104729 | Reversed A/B | 0.708→0.751 | +43 ms/6.1% |
| Original accepting unit seed104729 | Identical binary A/A | 0.713→0.702 | −11 ms/1.5% |
| Rejecting unit seed8675309 | Reversed A/B | 3.023→3.016 | −7 ms/0.2% |

The large rejecting-case increase does not recur, but the accepting-case cost
flag does. Its reversed pair adds about 30 ms in initial absorption and 14 ms
elsewhere. Attribution remains unresolved; A/A does not explain away all A/B
losses. No further seeds ran after this bounded follow-up. No speed or memory
saving is claimed for admission itself, and this evidence does not support
adopting the combined prototype yet.

[Summary](summary.csv) and [comparisons](comparisons.json) retain all 12 pairs,
including two A/A controls. All native peaks are exact. PW remains above its
424.69 MB public model; low FW is 16.18 versus 14.94 MB in both arms. Process
RSS moves both ways and is distinct from native allocation. The [numeric archive](archive.json)
contains all 25 captures, including the separate diagnostic, with identical
CSV blobs deduplicated and every run/file/hash mapping retained.

Next resolve initial-solver peak reporting/admission overlap and the repeated
accepting-unit cost signal before the saved-Schur primary adoption comparison.
The five changed source files remain uncommitted for that work. P0/P6 remain
open; no agents, cluster work, dependency update or push has occurred.

Replay scripts use explicit `EXPERIMENT_ROOT`, `REPOSITORY_ROOT`,
`CG_REPAIR_RUNS`, `SAVED_SCHUR_REPAIRED_ROOT` and `STATA_MP_PATH` environment
roles in place of local paths. Build isolated trees from the recorded source
and patch, prepare the referenced synthetic requests, and use the existing
`benchmarks/local_fixest_gap` harness modules on `PYTHONPATH`. Set
`FEREG_LOCAL_BENCH_ROOT` to the experiment root. Executed versus archived script
hashes and the path-only substitution are recorded in [script identities](script-identities.json).
