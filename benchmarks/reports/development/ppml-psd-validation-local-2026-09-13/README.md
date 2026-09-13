# Published PPML covariance validation: adopted local repair

Adopt [ADR 0056](../../../../docs/adr/0056-published-ppml-covariance-validation.md)
against `fd6b970a24f79a78e05fda585760beed59eb74dc`. The final result validator
now inspects the published covariance's own eigensystem. Production CGM
inclusion--exclusion, clipping, rank and tolerance policies are unchanged.
The separate wider-moment prototype remains absent and unadopted.

[Identities](identities.json) bind 176 source/ado/fixture inputs and the exact
[implementation patch](implementation.patch). The isolated baseline plugin is
`7b7c6664132041e60425382edb99728a20bc258c628a68369de32852d0bb65fe`; candidate is
`411ed113101399adda2c893d0b3e51fa16cbc96e230871ec568e8b346a689126`.
Both native crates visibly compiled for both architectures. Universal static
audits and actual arm64 ABI/self-tests pass; x86 runtime was not executed.
Installed/dist was neither the baseline nor replaced.

## Failure and independent adjudication

The exact original PW K257 warm-up (N7680, seed104730, one FE, two-way cluster,
offset, missing, 16 workers) reaches result validation and fails r(430) on the
accepted baseline. The validator reapplied the PSD policy to an already clipped
matrix, then checked diagnostics for a newly re-clipped comparison matrix against
the original published report. Tiny roundoff eigenvalues change that second
matrix and its eigensystem. It is the wrong object to use for this report check.

The [failure witness](failure-witness.json) retains a failure-only diagnostic
artifact's message and hashes. Original orthogonality error is
1.7520328141340652e-13; reapplication raises it to 1.829499072221847e-13, exceeding
the report-comparison slack of 16 machine epsilons even though both are far below
1e-8. The actual rejected covariance independently has minimum eigenvalue about
-1.10e-18 and rank 65 at the registered scale/tolerance. No diagnostic artifact
is adopted or used for timing.

A separate 65-dimensional Householder reflection supplies a closed-form
covariance with 22 positive and 43 negative known eigenvalues. Its exact PSD
target reproduces the same invalid rejection in the actual result validator.
The regression fails on old source and passes the repair. Tests independently
check the target matrix and reject eight forged reports plus a materially
indefinite matrix. Nonfinite, dimension, overflow and tolerance errors still fail.

The repaired original K257 request passes. A [NumPy oracle](profiled-oracle.json)
independently maximizes the profiled one-FE Poisson likelihood, forms the
current-weight centered Hessian, independently groups all three cluster subsets,
and applies complete CGM inclusion--exclusion and slope-block PSD clipping.
It matches the 7651-row sample hash, 1536 FE groups, 64/3/192 cluster subset
counts, identified slopes, normalized constant and reported ranks. Maximum
coefficient/covariance errors are **3.97e-15 / 2.87e-16**, well within the unchanged
registered envelopes. Final covariance rank is 65. Near-zero negative eigenvalue
counts differ across eigensolvers (159 versus native 185); this is not asserted
as independent count parity. The known-spectrum test verifies clipping counts
away from that roundoff boundary. Full covariance agreement is retained.

The original N30720/K257/seed104729 full request also passes both arms with
byte-identical scientific exports. These four cold functional requests are
separate from the warm cost protocol. The baseline warm failure is not timing
or scientific acceptance evidence; the single full cold pair is not a repeated
speed claim. [Functional archive](functional-archive.json) preserves all four
receipts, full numeric exports, synthetic input identities and rejected matrix.

## Boundary and retained checks

The independent inspector verifies the actual matrix's minimum eigenvalue, rank,
reconstruction and orthogonality. It retains the existing bound on further PSD
repair: a tiny-negative multiway matrix still produces a certified clipped
comparison whose distance from the published matrix must fit tolerance. It
avoids eigendecomposing that hypothetical comparison again. It never rewrites
the covariance or its production report. All state/count/minimum/rank,
underreported-diagnostic, standard-error and independent certificate checks remain.

Existing fallible K-by-K scratch is used, with no observation-sized state,
retained cache, new worker team or SPI. This removes one eigendecomposition per
final inspection. That work falls in the existing **publication** timer, which
starts before `CertifiedPpmlResult::new`; inference timing ends earlier. No
inference-kernel or solver-iteration speedup is attributed to this change.

## Complete-command, phase and memory evidence

The [protocol](protocol.md) fixes nine paired cost controls, then allows two more
seeds after a >=5% and 20 ms gain in changed work. The K129 screen meets that
condition, and the [prospective follow-up](repeat-protocol.md) names both seeds
before they run. All **11 pairs / 22 measured commands** pass. Fresh local
M2 Ultra/Stata 19 processes use a small untimed warm-up and rotated arm order;
no tests/builds overlap timing. Native workers are distinct from Stata's license
processor cap; affinity is uncontrolled. These are local development findings.

| Case | N / K / workers | Command seconds, baseline → candidate |
|---|---|---:|
| FW, exposure/missing, robust (three seeds) | 61440 / 129 / 4 | 4.740 → 4.400 |
| Complete one-FE robust | 983040 / 50 / 16 | 7.414 → 7.644 |
| PW, offset/missing, two-way cluster | 245760 / 50 / 16 | 2.099 → 2.055 |
| Weak two-FE PPML | 7680 / 10 / 4 | 0.135 → 0.137 |
| Low-memory FW PPML | 30720 / 10 / 4 | 0.784 → 0.763 |
| Small one-FE PPML | 7680 / 2 / 1 | 0.030 → 0.028 |
| Saved/separated PPML | 7680 / 3 / 4 | 0.026 → 0.029 |
| AW robust OLS | 61440 / 129 / 4 | 0.580 → 0.577 |
| Narrow/no-FE OLS | 7680 / 2 / 1 | 0.008 → 0.008 |

FW saves 337/340/345 ms, **7.2% median paired command time**. Publication medians
fall **0.68449→0.34734 seconds**. In the first pair, preparation is
0.10791→0.10949, finite solving 1.23234→1.23419 and inference 0.81180→0.81477.
This locates the gain in changed validation. The large complete control adds
230 ms/3.1%; its unchanged finite/strict phases rise about 136/111 ms while
publication falls 8 ms. The cause of that variation is unestablished. No control
crosses max(10 ms, 5%); saved output's +11.5% is +3 ms. All observations remain.
Overlapping phase medians must not be added, and the remaining single pairs are
cost controls, not causal speed claims.

Native peaks and modeled budgets are exact in all 11 cost pairs and remain
admitted. The K129 native action delta is 287.97 MB against 347.32 MB; raw live
peak is 288.01 MB. These scopes are distinguished in the [PPML comparisons](ppml/comparisons.json).
Sampled process RSS rises on K129 (median 0.334→0.357 GiB) and AW OLS
(0.267→0.313 GiB). No memory saving is claimed. Full native/model/scratch/phase
fields remain in the numeric archives. All full b/V and scientific exports are
exact; samples, rank/status/DoF and solver decisions match. PPML has zero fallback,
step-halving and strict-profile-rejection counts throughout. No new private
iteration audit or fallback-heavy performance claim is made.

## Validation and preserved harness failures

[Validation](validation.json) records 47 distinct core tests (29 result, four PSD,
14 VCE), 33 PPML plugin tests, and three distinct licensed fixtures on both arms.
Weighted OLS, independent Poisson/Hessian/sandwich, long-request covariance and
rollback fixtures all have explicit r(0) and terminal sentinels. Existing focused
plugin checks include allocation, worker switches and failed publication.

Two harness assumptions were corrected after preserving their failures:

- The cold full pair has a common **+576-byte** shift in every modeled phase and
  both mode totals, with exact admitted native peak. The unchanged model charges
  the actual embedding allocator snapshot before core entry equally to every
  phase. That precedes the changed validator; the particular allocation was not
  traced. [Inspection](functional-memory-inspection.json) records the offset
  separately while retaining every scientific exactness guard. All subsequent
  warm cost pairs satisfy the original exact-model comparator.
- The first independent oracle confused 257 identified slopes with reported
  finite rank 258. The existing result contract includes the normalized
  structural constant. The corrected oracle also verifies kept statuses,
  zero constant/covariance, FE nesting and reported ranks. No policy changes.

The [amendment](protocol-amendment.md), initial oracle script and log hashes
preserve those decisions. An initial diagnostic build launched from the wrong
Cargo working directory was excluded, terminated and rebuilt correctly; neither
its artifact nor its measurements are used. Raw licensed logs, local paths,
binaries and synthetic datasets remain outside Git.

## Supplemental control-label correction

The original `weak_d3` manifest denotes a **two-FE, degree-three** graph. Its
recorded commands correctly absorb g1/g2; the earlier report/protocol description
of three FE was wrong. The table above is corrected, while all original numeric
archives remain unchanged. An initial explicit-g3 setup attempt failed r(111)
during warm-up because that source has no g3; it is preserved and not counted
as a measured estimator result.

One additional pair uses the existing `three_fe_sparse` manifest (mobility 0.02),
N7680/K10/t4/seed31415927 with explicit g1/g2/g3, alongside the original weak
two-FE control. It passes **0.083→0.082 seconds**, with exact science. Native
peak delta is 18,453,252→18,452,084 bytes, both below the unchanged 42,341,675-byte
model; it is not exactly equal or evidence of a causal memory saving. An initial
supplemental collection assertion expecting exact peaks failed on that 1168-byte
difference; the scientific comparison passed and no estimation was rerun.
[Supplemental evidence](control-correction/comparisons.json) records this twelfth
successful pair separately. Code/artifacts and the original 11-pair performance
estimates are unchanged. This closes the label/coverage correction without
claiming that degree and FE dimension are interchangeable.

## Replay and next step

Bind the existing local harness through `FEREG_LOCAL_BENCH_ROOT` to isolated
baseline/candidate trees with the recorded source identities and set
`STATA_MP_PATH`. Copy the [replay scripts](replay/compare.py) into that experiment
root. `compare.py` runs nine cost pairs; `repeats.py` runs the two recorded FW seeds.
`run_oracles.py` runs the three independent fixtures on both arms.

For functional reproduction, use the original wide-moment recipe for
`pw_257-n7680-s104730` and `pw_257-n30720-s104729`; the input manifests and hashes
are in the functional archive. `functional.py` preserves the original failed
model-equality assumption; then run `functional_compare.py` for its explicit
adjudication and `profiled_oracle.py` for the independent oracle. The latter
requires NumPy/pandas. Full numeric oracle arrays are retained separately.

Resume the original wider-moment protocol against this accepted repair, including
its original PW K257 warm-up/full request. Do not substitute smaller passing data
or reuse the two earlier incomplete probe pairs as adoption evidence. Broader
#27 items remain open. No agents, SCC, Windows, qualification or push occurred.
