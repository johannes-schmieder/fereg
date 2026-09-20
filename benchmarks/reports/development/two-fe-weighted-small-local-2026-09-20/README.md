# Weighted dense two-FE and small-width development screen

Measured the committed endpoint first, then extended only initial Schur
admission. The retained change admits 3–15 actual RHS columns, including supplied
weights, on at least eight requested workers. Existing actual-weight probing,
memory admission, certificates and continuation remain. See
[ADR 0076](../../../../docs/adr/0076-narrow-weighted-ols-schur-admission.md).

This is local Mac development evidence on the M2 Ultra, N=983,040 and sixteen
native workers unless noted otherwise. It is separate from the completed
unweighted K=10/50 campaign and from release qualification. Stock fixest 0.14.2
is pinned by archive and library hashes. Inputs, plugins, controllers, samples
and numerical results are audited. Import/export is outside the timer;
Stata-to-native loading, estimation and requested outputs are inside. Each arm
starts a fresh process with a small warm-up; arm order rotates across repeats.
No timed benchmark ran alongside compilation, tests, data generation or archiving.

## Ordered design: fitting and inference

Each row contains three pairs/triples on the same data. Numbers are median
command milliseconds; paired ratios are preserved separately in `summary.json`
and `paired-native-ratios.json`. K counts finite regressors, excluding the
outcome and explicit constant. Moderate weights are independently seeded
integers from one to five, not selected using estimator results.

| Weights and request | K | Baseline | Candidate | Stock fixest |
|---|---:|---:|---:|---:|
| Unit, IID | 1 | 350 | 212 | 338 |
| Unit, IID | 3 | 402 | 240 | 353 |
| Unit, IID | 5 | 424 | 274 | 365 |
| AW, IID | 1 | 423 | 277 | 426 |
| AW, IID | 5 | 492 | 360 | 478 |
| AW, IID | 6 | 490 | 318 | 481 |
| AW, IID | 13 | 770 | 458 | 581 |
| PW, robust | 6 | 509 | 336 | 484 |
| PW, two crossing clusters | 6 | 658 | 491 | 543 |
| FW, robust | 6 | 516 | 353 | — |

All nine directly comparable ordered cells beat stock fixest in all three
pairs. Their median paired candidate/fixest ratios span 0.634218–0.900552.
Native fitting gains are approximately 25–42%. FW is compared to native baseline
and independent frequency-expansion checks: ordinary fixest weights do not
implement the required frequency covariance/effective-N semantics.

The pre-change screens include K=1/3/5/6/13/14 unit weights and K=1/5/6/13/14/50
AW. The boundary discontinuities precede the prototype. Separate profiles show
the weighted K=13 setup/RHS bucket fall 94.2→21.7 ms and initial absorption
440.6→208.3 ms. Unit K=5 falls 85.8→14.6 ms and 213.2→143.3 ms. The setup/RHS
bucket includes absorber construction, so it is not solely matrix copying.
The instrumented fixest K=13 weighted profile spends 426 ms in demeaning,
including about 360 ms in group means. Instrumented times are diagnostics.

## Remaining gaps and rejection costs

The shuffled holdouts use a fresh seed, 31415927, and three repeats. They are
kept even though the candidate loses to fixest:

| Request | K | Baseline ms | Candidate ms | fixest ms | Median paired candidate/fixest |
|---|---:|---:|---:|---:|---:|
| Unit IID, shuffled | 1 | 357 | 233 | 227 | 1.026432 |
| Unit IID, shuffled | 5 | 426 | 298 | 253 | 1.206478 |
| AW IID, shuffled | 6 | 518 | 369 | 294 | 1.250847 |

The remaining initial-absorption costs are about 155 ms for shuffled unit K=5
and 187 ms for shuffled AW K=6, including probes of 23 and 34 ms. Input loading
is about 37 and 47 ms. These profiles point to further narrow/weighted operator
work; reducing loading alone cannot explain the entire gap.

AW K=6 with saved FE, residuals and predictions improves 1163→1026 ms in three
native pairs. A separate three-pair comparison gives candidate/default-fixest
medians of 1008/707 ms and median paired ratio 1.427157. Default fixest misses
the unchanged output tolerance in every such comparison. One separate fit and
FE-recovery run at 1e-11 passes the output comparison; its 1069/1023 ms timings
are accuracy diagnostics, not the headline benchmark. Weighted strict final
projection and output recovery remain the next important development target.

At N=245,760, constant AW K=6 improves 128→108 ms. Dispersed weights spanning
1e-4–1e4 reject the probe at contraction 0.06237 and cost 228→236 ms; sparse
unit K=5 rejects at 0.09848 and costs 99→106 ms. Separate profiles attribute
8.7 and 8.1 ms to probing. Median paired penalties are 3.5% and 7.1%; these
costs are not discarded or presented as regressions that passed a release gate.

Controls for existing unit K=6 and weighted K=14 admission give 262/256 and
447/445 ms before/after. The four-thread AW control gives 157/156 ms. One-FE
controls on the shared input, absorbing only g1, give 74/76 ms at unit K=1 and
196/193 ms at AW K=6. All 21 control pairs, including rejected probes, have
byte-identical b, V, status, DoF, certificate and fit bundles. Their native
allocation peaks are identical.

## Science, memory and provenance

- Twelve focused Schur tests cover narrow unit/constant/varying weights, thread
  admission, weighted cuts, scale invariance, memory bounds, certified
  continuation and final overlap. The first run had an obsolete guard
  expectation; the corrected guard test passes. The initial failure remains
  described in `focused-tests.json`.
- The completed artifact passes its ABI/panic-containment harness and three
  Stata fixtures: the new narrow weighted dummy/expansion fixture, existing
  multi-FE input/oracle fixture and e-class transaction fixture. The new fixture
  checks K=1/3/6/13, AW/FW/PW inference, saved FE/reconstruction and common scaling.
- Small independent weighted dummy oracles check coefficients, covariance,
  RSS and requested outputs. FW coefficients are also checked by literal
  expansion; the Stata fixture checks expanded FW robust covariance and DoF.
- Seven independent large weighted-output audits pass group constancy,
  component normalization, reconstruction, RSS and weighted normal equations.
  Maximum group residual mean is 1.040e-11; maximum normalized finite score is
  1.615e-14. Native-vs-baseline and tight-reference comparisons retain the
  repository tolerances.
- None of the 70 measured native pairs increases native allocation peak.
  Ordered unit/AW IID peaks fall approximately 33–66% because the newly admitted
  route avoids the legacy setup. No new heap owner was introduced.

Baseline is commit `b84cea023de07a750bcb019c03519edb05c01279`, verified against all
513 files of its reused historical artifact. Candidate `narrow2` contains that
commit plus the core admission/test file and new Stata fixture overlays; all
514 captured source entries match. Candidate plugin SHA-256:
`1e0bf355e5e9cd9b37494386ee6808c33122dffb4aac699122517beb12ecffdd`.
The helper's embedded build date is 2026-09-19; measurements occurred on
2026-09-20. `implementation.patch`, source audit and full identities bind the
result. No installed package, remote state or release qualification changed.

## Preserved discrepancies and attempts

The archive contains 289 application attempts: 287 successful application
calls and two worker failures. Application success is separate from scientific
comparison status. It includes stopped comparisons and their inputs/results.

The first weighted oracle retained redundant dummy columns, causing an unstable
SVD rank decision. Grounding one right-side level per connected component fixes
the oracle. On the N=960 disconnected clustered case, the independent oracle
and native correction use K=243, while pinned fixest reports covariance K=242;
that reference covariance discrepancy remains a failed comparison. The large
connected clustered comparisons pass. The first unweighted R worker passed an
explicit NULL weight argument; the corrected worker omits that argument. The
first one-FE control omitted the no-output environment flag and failed export;
the corrected control passes. The initial strict default weighted-output
comparison stopped on reference output disagreement before the explicitly
marked default-output series and tight-reference check.

`series.json`, `measurements.json`, `profiles.json`, test/fixture receipts and
independent audits preserve these distinctions. The archived harness is the measurement snapshot. The live harness additionally
restores the original unweighted FE-normalization arithmetic for future output
benchmarks; all unweighted timed cases here requested no outputs, and the
weighted branch is unchanged. No tolerance, estimator
semantics or reference design was changed to obtain a favorable timing result.
