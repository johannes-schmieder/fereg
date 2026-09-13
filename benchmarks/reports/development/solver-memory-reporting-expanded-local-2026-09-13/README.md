# Attempted solver memory reporting: expanded, unadopted prototype

This snapshot extends the [initial reporting prototype](../solver-memory-reporting-local-2026-09-13/README.md)
against adopted native `7663a87614e37bea73e5dbc037c3ad0c7afccdda`. It remains
**unadopted**: reporting coverage is incomplete, and wide weak OLS has repeated
command increases above the prescribed review envelope. Neither the public
selected-memory scalar nor its observed/model ratio changes in this snapshot.
This is P0 diagnostic work, not another accepted performance gain.

## Implemented coverage

The [patch](implementation-reviewed.patch) and [identities](identities.json)
bind thirteen changed native files and all 152 native inputs:

- CMG records its admitted construction estimate before building. Planned
  hierarchy rejection and recoverable construction failure retain that estimate;
  rejection before construction records no hierarchy attempt.
- CMG checks the joint retained hierarchy/workspace, retained coefficient product
  and CG-polish estimate before continuing. Its final-residual certificate keeps
  the earlier session peak. Prepared PPML callers retain their separate topology
  accounting and numerical route.
- Borrowed transient/retained LSMR reports its estimate even without a supplied
  memory limit. Admission and reporting use the installed Rayon team. The tests
  deliberately install seven workers while the retained config requests one.
- Generic intercept-only LSMR reports its attempted workspace, including an
  uncertified attempt followed by MAP and attempts discarded by automatic CG.
- Prepared fit reports carry the maximum across initial/final/batched solves.
  Fast and scratch score reconstruction propagate their separate solver session
  into that execution report instead of discarding it with the score matrix.

The counters contain **modeled incremental workspace for instrumented attempts**,
not observed allocations. A conservative estimate may exceed the actual work of
a certified-at-entry solver. Zero denotes unrecorded coverage, not an allocation-free
path. No extra observation-sized reporting bank or numerical recurrence is added.
The main-thread SPI, public result layout and transactional publication are unchanged.

The incomplete scope includes slope Schwarz signed-cover storage, generic CG/MAP
increments and owned-design overlap, saved-FE recovery attempts/overlaps/admission,
some retained coefficient products, and full construction/discarded-work counters.
Publishing execution bytes plus this partial report would still not establish a
complete public model. Opaque upstream factor bounds remain a separate audit.

## Source, artifacts and validation

Both plugins were isolated from installed dist. The baseline's 152 native inputs
match `7663a87`; its reviewed artifact is
`c9db0dceea82dc7b7b87a51979d0fd7ca1f83f1e71f04f034be32312527990f6`.
The frozen candidate plugin is
`8b6f3ffaa711581367de389c9341d47ef57d2810c76188048852b10083b5b4d5`.
Universal static audits and the actual native arm64 ABI/self-test pass.
The build retains an unused production-wrapper warning; the wrapper has test
callers. There is no x86 runtime, SCC, Windows, broad profile or qualification claim.

[Validation](validation.json) records **128 distinct focused Rust tests**:
50 CMG, 67 LSMR-filter, two report merges, ten robust tests and one Schur oracle,
with two overlapping test identities. Plugin test type-checking passes. The new
CMG test exercises a retained-state-plus-polish budget one byte short and exactly
at the modeled boundary, both with and without coefficient retention, and checks
unchanged residual/coefficient results and final-report persistence. The score
test preserves full covariance against its existing MAP composition while using
a separate LSMR score session in fast/low modes with unit/AW weights.

Three licensed fixtures pass in **both artifacts**, with explicit r(0) files and
both fixture/runner sentinels; see [oracle receipts](oracle-results.json).
`ols_lsmr_reuse.do` checks three-FE AW inference and saved effects against explicit
dummies across low pools 1/5/10 and fast execution. `within_weighted_ols.do` checks
weighted robust b/V/residuals against explicit-dummy `regress` at 1/7 workers.
The [CMG variant](within_weighted_cmg.do) changes only the requested solver and
fixture sentinel, retaining that same independent oracle and tolerances.

The earlier missing CMG test arguments and unqualified Rust type failed to
compile and were fixed before accepted tests. One mistaken test filter matched
zero tests; the actual named Schur oracle subsequently passed. The first timing
launch stopped at sandbox process-statistics enumeration, before Stata started;
the authorized retry succeeded. These are preserved setup failures, not discarded
numerical or timed observations. Raw licensed logs and binaries remain outside Git.

## Original command comparisons

The [protocol](protocol.md) predates timing. Fifteen pairs / thirty commands run
sequentially in fresh local Stata/MP 19 processes with small untimed warm-ups on
the M2 Ultra. Primary rows rotate three seeds and arm order. Native thread counts
are distinct from the Stata license's processor cap; affinity is uncontrolled.
No build/test runs alongside timing, and optional phase/native tracing is off.

| Case | N / K / threads | Command medians, s | Median paired change |
|---|---|---:|---:|
| Weak OLS, AW robust | 122880 / 65 / 4 | 1.006 → 1.054 | +4.8% |
| Explicit CMG, low-memory AW robust scores | 30720 / 10 / 4 | 0.128 → 0.128 | 0.0% |
| Explicit CMG, wide | 61440 / 50 / 4 | 0.214 → 0.210 | −0.9% |
| Weak PPML, PW/offset/cluster checkpoints | 30720 / 17 / 4 | 0.412 → 0.413 | −1.0% |

Independent medians and median paired ratios need not agree. The weak OLS command
differences are +49/+9/+51 ms; the last crosses max(10 ms, 5%). Native differences
are +50.83/+7.37/+50.11 ms. Input and outer work do not explain the larger losses.
Reported solver setup differences are about +7.35/+2.21/+4.73 ms, and solve
differences +25.89/−0.36/+30.37 ms. These are accounting scopes, not a complete
causal decomposition; they must not be summed with overlapping phase timers.

Single-pair controls are small weak AW robust 22→22 ms, complete IID 172→178 ms,
and saved one-FE AW 26→25 ms. Full successful scientific exports and non-time
solver/retry decisions are exact throughout, including saved outputs. No new
fallback occurs in these captures; forced continuations remain covered by tests.

Native peaks are unchanged for both CMG targets and the complete/saved controls.
Wide weak OLS has two 16-byte reductions and one 655376-byte reduction; these are
not adopted memory gains. Public OLS modeled bytes remain unchanged and incomplete:
wide weak is about 351.32 MB observed versus 268.55 MB reported; small weak is
54.93 MB versus 1.045 MB; low CMG is 13.964 MB versus 13.092 MB. None closes a
model-bound gate. Native peaks, scratch, sampled process RSS and all runtime
components are retained separately in the [OLS](ols/summary.csv) and
[PPML](ppml/summary.csv) summaries and numeric archives. No general RSS saving
is claimed.

## Prescribed follow-up and disposition

The [follow-up protocol](followup-protocol.md) prescribes three fresh-seed A/A
pairs interleaved with three A/B pairs on the same wide weak workload, using the
original review threshold and unchanged binaries. The
[same-artifact receipts](same-artifact-controls.json) require matching baseline
plugin hashes and selected-model bytes for every A/A pair.

| Comparison | Pairs | Command medians, s | Candidate minus baseline, ms |
|---|---:|---:|---|
| Identical accepted binary A/A | 3 | 1.013 → 1.033 | −9, +20, +21 |
| Accepted/candidate A/B | 3 | 1.020 → 1.043 | −4, +13, +52 |

The last A/B pair again exceeds the envelope. A/A shows variation but does not
explain the roughly 50 ms losses. All six follow-up comparisons preserve exact
science and non-time solver fields. These observations remain separate from the
original fifteen pairs: **21 pairs / 42 measured calls total**, including three
same-binary pairs. No observation is removed or reclassified as noise.

Do not adopt this prototype yet. Finish the missing workspace/ownership coverage
and localize the persistent weak-graph cost before proposing the public report
change. The earlier LSMR batching adoption remains the native baseline. This
checkpoint neither closes P0 nor advances the later #27, agent, upstream-CMG or
SCC phases. Installed dist and the separate Veneto work remain intact.

Replay `compare.py` and then `followup.py` with the repository's existing
`benchmarks/local_fixest_gap` on `PYTHONPATH`, `FEREG_LOCAL_BENCH_ROOT` containing
the isolated source/artifact trees, and `STATA_MP_PATH` set. The archived replay
scripts replace local paths only; hashes of the scripts actually executed are
recorded in validation. `run_oracles.py` uses the same environment and separate
fixture outputs. Historical receipts and the initial prototype are unchanged.
