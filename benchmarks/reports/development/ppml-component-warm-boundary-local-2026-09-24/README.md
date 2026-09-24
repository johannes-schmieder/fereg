# PPML component-aware warm checkpoints — local development, 2026-09-24

Adopt the narrow correctness prerequisite in [ADR 0156](../../../../docs/adr/0156-component-aware-ppml-warm-checkpoints.md).
The broad whole-command PPML <=0.90-times-stock objective remains unfinished.
This report does not adopt the previously rejected fast categorical engine or
claim a speed improvement from an additional accuracy check.

## Source and receipts

Parent: `96676a68`; candidate `componentwarm3`, overlay only
`crates/fereg-core/src/fe/borrowed_map.rs`. All **543 captured computational inputs**
match the final worktree. Production plugin SHA-256:

`5434193a5a9b0408ed9636c625f1d134996f2224bff2642049b129a47245ce52`

Baseline `densegram2` is the preceding accepted artifact, plugin
`a5768ce3774837d1968f21f956b9457cd35571760a6e2b0a01a88a1b26095a4c`.
Both complete source maps, release build, ABI/panic evidence, test recipes,
captured diagnostic changes, declarations, controllers, numerical exports and
summaries are in the content-addressed archive. No diagnostic binary supplies
headline timings. The same pinned unmodified fixest archive/library is used with
`glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads and serial BLAS.

Archive: **2,413 logical files**, **948 distinct payloads**, **5,933,173 bytes**;
SHA-256 `606a23a83b3475bb39b9e55a5ea1362ec3ebdced1a9300500a06a1c90407dde7`.
Every logical hash was verified by a complete restore. Restore into a new directory:

```sh
python3 restore_receipts.py /path/to/new-directory
```

Logical paths start with `component-warm/`. Data binaries and licensed logs are
omitted with hashes. Paths are sanitized. The archive preserves failed experiments
and harness errors; it is diagnostic development evidence, not qualification.

## What caused the rejected accuracy failure

The existing 160-row analytic Poisson fixture has two disconnected components,
with log means near 30 and 1. Duplicate covariate/FE rows have responses 0.9 and
1.1 times the truth, so their paired scores vanish at the known predictor.
Keep its 1e-10 predictor envelope and six-iteration bound.

Instrument initial, ordinary, strict and final states in isolated source copies.
Replay the rejected original-control integration against its `a1687ddb` source;
compare baseline MAP separately from explicit LSMR. Ground one firm coordinate
per component in an independent 79-column explicit dummy design. A separately
implemented 60-digit Decimal Cholesky solve checks 23 captured WLS frames; maximum
relative normal-system residual is 6.27e-59. This is an independent mathematical
calculation, not a replay of either native recurrence. Ordinary loose-tolerance
frames are diagnostic and are not relabeled strict passes.

| State in rejected fast MAP integration | Cold-component max truth error |
|---|---:|
| Fifth ordinary candidate | 9.04e-14 |
| Following canonical strict candidate | 1.2343253e-9 |
| Same final boundary with the adopted check | 8.94e-14 |

The cold WLS oracle independently measures the final rejected-frame discrepancy
as 1.2343254e-9. The canonical projector returned through its initial global-norm
certificate before preparing component-scaled weights. That global norm hid a
small component's drift. Enabling component scaling only for the recurrence
therefore could not repair it. Forcing the recurrence establishes the diagnosis;
the final implementation retains the shortcut when both original and scaled
checks pass. It changes neither the accepted point policy nor any tolerance.

Baseline MAP passes the truth fixture. Baseline explicit LSMR separately exhausts
six iterations with two strict profile rejections; the correction does not extend
component-scaling admission to that route or claim to repair it. The rejected
integration's medium-width public serial losses also remain valid reasons not to
adopt it. Its generic wrapper and routing guards are not reinstated.

The first diagnostic replay accidentally left the patch unapplied because of
`git apply` subdirectory scoping and exercised the old low configuration. Its
`rejected-*` receipts are excluded from the attribution. The corrected
`rejected2-*` source reproduces the recorded failure; `rejected3-*` forces solving,
and `rejected4-*` applies the adopted shortcut check. Source maps and recipes
make the distinction explicit.

## Retained change and validation

Only component-scaled checkpoint owners perform the extra check. The initial
original-weight pass is still required. A disconnected initial pass also needs
a fresh scaled certificate at the identical tolerance. Original norms are
restored; a declined shortcut enters the existing scaled solve and independent
original-weight acceptance. No new bank, route, factor, tolerance or memory
admission is introduced. Production eligibility remains low three-FE categorical
MAP; ordinary slopes, fast categorical, one FE and OLS retain their paths.

- **53 distinct Rust tests pass**, including all prepared IRLS tests, selective
  checkpoint retries and full-batch Schur/cap/fallback checks.
- The new analytic paired-contrast test fails on the parent and passes here. It
  covers two/three partitions, T1/T4, three changing component-weight frames,
  tiny nonzero drift, exactly absorbed inputs and a zero RHS. Exact inputs retain
  zero iterations; the inaccurate shortcut must perform projection.
- **Three licensed fixtures pass**: low categorical explicit Poisson/Hessian and
  sandwich (unit/FW/PW, robust/cluster, T1/T4); two/three-FE weak weighted low/fast
  comparisons at pools 1/2/10000; and the excluded low slope explicit Poisson/
  Hessian fixture. Process status, Stata return code and terminal sentinels agree.
- **42 public applications** pass: 36 PPML and six OLS. All 36 PPML public-state
  audits, 15 native pairs and 12 direct candidate/stock pairs pass. All 15 native
  coefficient/covariance/mean export pairs are exact. No detected contention.
- All 12 PPML pairs have identical outer/inner iterations, retries, rank/DoF,
  tracked allocation peak, modeled selected bytes and scratch bytes.

Keep the failed licensed fixture visible: the old full low-categorical fixture
asserts positive scratch bytes on a one-FE low fit. Both baseline and candidate
fail there after passing numerical checks. The immutable original is unchanged;
the unchanged multi-partition portion passes separately. This is not a repair of
that stale fixture assertion. The large-screen Python explicit Poisson oracle
also refuses its pre-existing bounded-design limit; the subsequent public-state
and parity audit succeeds without that oracle. Independent Poisson/Hessian
authority comes from the licensed explicit-design fixtures, not a silently
expanded Python bound. Earlier diagnostic/build setup failures are retained.

## Whole-command cost and remaining losses

Fresh **seed 202609365 is consumed and frozen**. Three rotations per case, matched
robust inference and fitted means. No compilation, profiling, archiving or other
fits run alongside headline measurements. Times below are milliseconds; each
list is in repetition order, followed by the median. These are current-artifact
claims only.

| Case | Baseline times; median | Candidate times; median | Stock times; median | Candidate/stock |
|---|---|---|---|---:|
| Low three FE N14400/K7/T1 | 158,152,150; **152** | 152,156,152; **152** | 388,399,384; **388** | 0.392 |
| Low disconnected three FE N17280/K4/T4 | 144,143,146; **144** | 145,148,146; **146** | 231,229,243; **231** | 0.632 |
| Fast categorical two FE N16320/K16/T4 | 79,78,79; **79** | 78,78,79; **78** | 70,58,65; **65** | 1.200 |
| Fast two slope partitions N15360/K6/T4 | 64,66,67; **66** | 66,65,67; **66** | 65,66,64; **65** | 1.015 |
| Excluded categorical OLS N16320/K16/T4 | 35,35,34; **35** | 34,35,34; **34** | — | — |

Candidate-minus-baseline pairs: low connected **[-6,+4,+2]**; low disconnected
**[+1,+5,0]**; fast categorical **[-1,0,0]**; fast slopes **[+2,-1,0]**; OLS
**[-1,0,0]**. The disconnected median cost is +1.4%. No performance gain is
attributed to the check, and no universal non-regression claim is made. The two
low cases already beat the 10% target; the fast controls still miss it. Static
allocation drift and physical-cap/Linux confirmation remain unresolved.

## Next coherent implementation

The numerical obstacle is localized. Continue with current-weight coefficient
Newton increments behind the incremental-direction boundary, avoiding generic
absolute-WLS score/mean/reporting work and preserving independent canonical
response history. Retain explicit original-control scores, fresh FE acceptance,
strict basis and final-Hessian authority. Do not transplant component rescaling
into the full finite regression objective: shared finite coefficients couple
components. Frozen inner-solve gains remain evidence for this redesign; the
rejected wrapper's public results remain evidence against blanket guard removal.

After scientific checks, first measure development 347 dense/unbalanced/narrow/
weak cases and whole-command phase costs. Promote only broad useful gains to
fresh cases, retain sparse/weak LSMR wins and low/OLS controls, then address the
separate one-FE and narrow-slope bottlenecks. This prerequisite is not completion
of the requested production solver redesign or the broad speed objective.
