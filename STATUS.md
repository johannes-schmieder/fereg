# fereg 0.5.0 beta status

**Last updated:** 2026-09-08
**Branch:** main
**Mode:** prototype/development
**Release state:** public OLS and PPML beta surfaces; no qualified release,
tag, push, or Windows scientific qualification.

This is the sole mutable progress/handover record. Historical receipts remain
unchanged under benchmarks/reports/. This compact handover supersedes the
completed optimization/calibration investigation chronology formerly here.

## Current milestone: PPML auto admitted for development

ADR 0029 now admits the implemented automatic policy:

- Exactly two or three categorical intercept FE dimensions select LSMR when
  fast execution and the complete ordinary/checkpoint workspace fit the
  generation-zero memory plan.
- Other designs, absorbed slopes, low-memory execution, allocation audit, or
  modeled specialized-workspace veto select MAP. Auto never selects CMG.
- A modeled veto occurs before solver construction and is not a fallback.
  Actual allocation/interruption/data/internal errors remain fatal.
- Requested, selected, concrete, effective, checkpoint, retry and fallback
  reporting is preserved. Auto uses the same certified engine as explicit LSMR.

Admission: benchmarks/reports/development/ppml-auto-development-admission-2026-09-08.json.
This is development acceptance, not formal qualification, a successful
reclassification of the failed calibration campaigns, or a claim that every
scope was rerun on the repaired source.

## Scientific adjudication

All21 hard actual-auto scopes on9 original datasets have independent
coefficient binding, model-space, objective, finite/per-FE score, rank/DoF,
covariance and default/tighter stability evidence.

- 18 scopes pass the unchanged numerical comparisons using complete pinned
 reference records. Earlier complete rows in failed campaigns are prospective
 scientific witnesses only, not accepted timings or campaign passes.
- Task17 has a fresh optimum-accuracy proof on the repaired Linux capture:
 coefficient radius4.882e-12 and covariance radius1.762e-12. All5 complete
 reference repetitions have17 coefficients and50 covariance cells proven
 outside their existing envelopes; other compared science passes.
- Tasks7/8 share the many-level N75000/K50 dataset with no complete reference
 row. Both pass independent optimum inclusion. The task7 coefficient and
 covariance bounds are6.345e-12 and1.793e-12. Reference error is not claimed.

The apparent four additional VCE discrepancies were truncated-record artifacts.
The new strict reference parser rejects incomplete matrices/records, invalid
stripes, unsuccessful fits and invalid objective pairs. It does not treat
missing evidence as a numerical mismatch. ADR0026 finite/domain/Poisson-identity
and no-worse objective rules apply without weakening coefficients
(rtol1e-7/atol1e-10) or VCE (rtol1e-6/atol1e-10).

All9 preserved SCC dataset hashes were reverified read-only; the reference
closure matches original setup. Original input/source and audit bindings are
in ppml-auto-adjudication-completion-2026-09-08/work/aggregate-bindings.json.
No projected reference state is used as an additional objective witness.

## Common checkpoint-cycle repair

The Linux K0 N75000 many-level two-FE tighter fit cycled: a certified warm
checkpoint could reconstruct the older canonical predictor, moving eta by
1.599e-10 against the requested1e-10 profile gate despite a passing score.

A fit-owned checkpoint target now tightens100x after a certified unchanged-basis
profile rejection, bounded below by1e-14 without loosening an already-tighter
target. The refined target also governs final-Hessian synchronization. The
next actual checkpoint counts the extra work as a retry. Existing RAII,
warm-state invalidation, two consecutive deviance-plus-score convergence,
canonical rank and exact final q_i*mu_i certificates remain mandatory.

The formerly300-second timeout now takes1.873seconds at both default and
tighter outer tolerance. Strict/final eta movements are8.42e-12/7.29e-16.
Default b/V/objective/rank/DoF are unchanged. Repaired difficult2FE and task17
default results are identical to baseline; sparse3FE b/V differences are
1.05e-16/3.69e-19, with unchanged objective/rank/DoF and passing certificates.
Different hosts prohibit matched performance claims for those guard timings.

## Source, jobs and artifacts

Baseline main commit:7c1fc09207864b41f851c0cb58c2dcbe80b1e8be.
Frozen dirty snapshot:da29adb7f6061a933dccbedbae923d96957361f3aa495f75780be8a05755362e.
The sole subsequent compiled-production overlay is
crates/fereg-core/src/ppml/prepared_irls.rs SHA
977522f1b878c2f96b5147f85fd927e739cb65b5111a9062f25d5f3ff50d3c93.
Documentation, adjudicator and fixture changes are separately identified.

- 7490248: actual-auto Linux smoke, task17, pass.
- 7490294/7490295: baseline actual-auto arrays;16 successful scope receipts.
- 7490361: independent direct-ring reaudits repair auditor failures4/7/8;
 original failures remain recorded.
- 7490499: trace-only diagnosis, no numerical policy change.
- 7490653: repaired K0 task3, successful fits and independent audit.
- 7490705: repaired guard scopes1/11/17; all scheduler/application/audit passes.
- 7490776.1–4: strict reference attempts all timeout1200seconds,
 failed0/exit1, application124, no fit sentinel. None accepted.

Every substantive job reserved28cores. No job is currently pending for this
goal and the completed campaign monitors were deleted.

Baseline Linux plugin:12f17d5873a0092e2ac6baee6b3ced2732504550c2983e5ada4a2d1a07f3450a.
Repaired Linux plugin:01af450c8fa6d841584661608fa45db41b0f9afb14cd7750eeaa9a3f51bc5124.
Repaired universal Mac plugin:47e578679f81ba9f0ffbb087c94864019427fa15c1f1ecac7ecc3bac5791caee.
The isolated Mac build used installed target libraries; only unavailable rustup
provisioning was omitted. Standard compilation, binary audit and actual ABI
loader tests passed. Formal exact-source qualification remains separate.

## Validation and handoff

- 745 repaired-core tests;18 focused strict-checkpoint tests;2 constrained-memory
 tests; formatting and core all-target Clippy passed.
- 71 focused auto Python tests,10 accuracy tests,8 optimum-adjudication tests,
 16 PPML help-contract tests,31 allocation-source-contract tests and10 build-
 closure tests passed. Existing independent oracle/memory evidence is retained.
- 12 licensed Mac auto-versus-explicit route pairs pass with exact b/V/objective/
 rank/DoF/sample/mu agreement. Low memory selects MAP; failed auto restores
 prior results. An additional3FE pair exactly matches saved effects and eta/d.
- Terminal Stata sentinel and explicit application returncode0 were checked.
- Git diff whitespace validation passes.

The full evidence chain is listed in the development admission receipt.
The admitted universal artifact is installed in dist and passes the same
fixture in a fresh Stata process, including its explicit terminal sentinel.
Activation receipt: ppml-auto-local-activation-2026-09-08.json.
Final whitespace-only cleanup of the independent coefficient-proof script
preserves its Python AST and nonwhitespace bytes exactly;10 accuracy tests were
rerun. The original executed source is preserved and historical proof hashes
are not rebound to the cleaned file.
Historical optimization evidence remains in its named immutable reports;
nothing is promoted across source revisions. The coherent development commit
includes this handover and the source-bound admission/activation records.

## Remaining performance work (not part of this admission)

- Sparse/mixed3FE ordinary joint LSMR conditioning and operator work remain
 high-leverage; do not restore the rejected retained all-pairs Schwarz policy.
- CMG keeps its bounded nuisance envelope and fresh certificates. Guarded
 weak-edge grouping, graph batches and certified warm starts are retained;
 unconditional grouping and ineffective response-start experiments are not.
- Explicit MAP is still slow on poorly connected graphs. Auto now avoids it
 within its admitted two/three-effect envelope.
- Easy one-FE wide-K weighted-design passes remain a useful optimization target.
- No further upstream API expansion, auto-policy refit or new campaign is implied.

## Release boundary

Development acceptance does not close the beta's formal exact-source
scientific, memory, performance or cross-platform release gates. Windows and
the formal aggregate remain deferred. No push, tag, release or broader
qualification is authorized by this completed development admission.
