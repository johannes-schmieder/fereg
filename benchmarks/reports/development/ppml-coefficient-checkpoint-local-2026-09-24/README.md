# Bounded full-batch PPML Schur checkpoints — local development, 2026-09-24

Adopt `coeffcheck1` under [ADR 0154](../../../../docs/adr/0154-bounded-full-batch-ppml-schur-checkpoints.md).
The target remains whole public PPML <=0.90 times comparable stock fixest per case.
This milestone materially improves checkpoint-limited cases; it does **not** finish
that objective. Move to ordinary categorical joint directions and their repeated
observation passes, preserving canonical checkpoint independence.

## Identity, scope and receipts

Baseline `selective1` is commit `d43d48694961ec218a54a2fcaffcb5e4a100d3a3`, plugin
`aab5ad93a4a43cf49cf8b6a72e91723e8ef65fc268082f7827bf28cd47224b3f`.
Candidate overlays only `crates/fereg-core/src/fe/borrowed_map.rs`; all **543 captured
computational inputs** match the adopted source. Candidate production plugin:
`d6c73d8493e58d2db2ef9ac656dcbe4b828752d99b45148df33539fea2b1c684`.
Diagnostic candidate `coeffcheckledger1`:
`2b28b1f6b437c68532630d245c426f15c5938b4b565aec310bbf4691529efc91`.
Diagnostic capture `checkpointcapture1`:
`64b6672b35a38e1bb12b34bdbdcdda8eb53184d5813ce8185e55199ffd3d980d`.

Pinned, unmodified fixest 0.14.2 is used with `glm.tol=1e-10`, `fixef.tol=1e-11`,
matched thread counts, serial BLAS and robust inference plus fitted means. Its
source archive SHA-256 is
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Development seed 202609347 and fresh **202609360**, predeclared before public
candidate results. Fresh 360 is now consumed/frozen; 359 and protected 211/212
were not tuning inputs. Frozen 77/16/111 definitions remain unchanged.

The [content-addressed archive](receipts-content-addressed.zip) contains **8,440
logical receipts**, **2,037 payloads**, **14,514,425 bytes**; SHA-256:
`fe986b048762e8756810280f79e468124508493eba6372d1935ccd5bcf93a8fa`.
[Restore and verify](restore_receipts.py) reconstructs source snapshots, declarations,
recipes, sanitized traces, all results, oracle checks and rejected engineering
attempts under `checkpoint/`. Generated data/output binaries and raw licensed logs
are omitted with hashes. The archive was verified and restored after collection.
All named jobs are terminal. No release/qualification claim, remote mutation or SCC run.

## What changed and why

The old checkpoint route separately ran an eight-step, at-most-three-column probe,
then copied the original full batch and rebuilt current-weight group state for a
bounded solve. A 16384-row guard sent the smaller measured cases directly to LSMR.
The capped helper also disabled the PPML component scaling already available to
ordinary MAP. Those are verified source facts, not hypotheses inferred from timing.

Use the existing coefficient Schur PCG engine for the whole checkpoint batch, with
one current-weight setup and original-input copy. The first eight steps are useful
solve work. An unfinished batch whose maximum per-column energy contraction exceeds
0.01 declines; otherwise continue within the unchanged 64-step cap/configured budget.
Keep individual column readiness. Reconstruct and independently certify under the
original weights and exact canonical tolerance. Finite capped/uncertified attempts
are discarded before ADR 0153's selective LSMR retry path starts from untouched inputs.

Separate MAP, OLS and checkpoint work limits internally. Bounded PPML retains
component scaling; OLS has no PPML component bank and keeps its previous numerical
route, probes, caches and continuation. The replacement reaches admitted two-FE
owners below the old row threshold, with the existing graph admission (N>=128,
owner capacity<=64) and transient allowance. It does not remove admission or
scientific checks. One FE, slopes, three-FE checkpoints and ordinary joint LSMR
remain separate. Eligible low-memory checkpoints can also reach the new route.

Initial/accepted projection checks, original reference scales, canonical rank,
finite solve, model-space, score/objective, strict profile, reconstruction, final
Hessian, inference, sample, weight semantics, DoF, separation and transactional
publication are unchanged. No bitwise equality requirement is imposed on a new
recurrence; independent oracles and established envelopes govern acceptance.

## Frozen identical-frame comparison

Captured actual initial and strict checkpoint weights/RHSs for four public
development designs. Both native owners receive the same frames at T1/T4, five
alternating calls. The isolated `direct64` probe removes the discarded pilot and
uses component scaling, before production adds the bounded in-solve progress check.
Owner construction is recorded separately. Native call timings include reference
norms, initial checks, weighted preparation, solve, reconstruction and acceptance.
These diagnostic runs lack an external-process contention monitor and are not
headline public benchmarks.

The stock wrapper `demean` includes R validation/index preparation. A separate
unmodified `cpp_demean` replay precomputes the same symbolic IDs/tables and times
800 calls in five groups of ten per frame/thread configuration. Outputs are
bitwise equal to the wrapper, inputs unchanged; one-time symbolic setup is 0–1 ms
at the clock's resolution. Current-weight numerical preparation remains inside
both native and C++ calls. Do not attribute wrapper overhead to the C++ solver.
Only strict frames are accuracy-ranked; initial fereg frames use their prescribed
looser schedule and cannot supply a strict stock timing comparison.

Strict-frame medians, milliseconds:

| Frame | T | Baseline native | Isolated direct64 | Stock C++ |
|---|---:|---:|---:|---:|
| Dense N16320/K16 | 1 | 52.510 | 6.136 | 21.4 |
| Dense N16320/K16 | 4 | 20.218 | 5.799 | 7.3 |
| Unbalanced N15360/K10 | 1 | 29.710 | 3.613 | 9.8 |
| Unbalanced N15360/K10 | 4 | 11.189 | 3.444 | 3.4 |
| Narrow N12000/K3 | 1 | 9.274 | 2.236 | 4.9 |
| Narrow N12000/K3 | 4 | 3.621 | 1.916 | 2.0 |
| Weak N7680/K4 | 1 | 2.067 | 1.371 | **unranked** |
| Weak N7680/K4 | 4 | 1.257 | 1.436 | **unranked** |

Independent explicit weighted dummy normal systems, one gauge per component,
sparse direct LU and a separately equilibrated SciPy LSMR cross-check establish
projection and model-space accuracy. All **16 native strict outputs** pass the
established relative projection 1e-9 / span 1e-10 envelopes; maxima are 2.841e-12
and 2.469e-12. Six strict stock outputs pass; weak stock T1/T4 fails projection
at 6.946e-9. Its timing is retained in receipts (C++ medians 82.4/32.6 ms) but
unranked. This inner-frame discrepancy does not invalidate the separately audited
public weak fits below. The native weak T4 probe has a 0.179-ms loss; preserve it.

## Public command distributions

Rotating baseline/candidate/stock order; every application, public-state check and
native/reference pair passes. No timeout, application failure, scientific exclusion,
monitor error or detected competing process in these public blocks. No compilation,
profiling, archiving or competing fits alongside headline measurements. All numbers
below are milliseconds. Full distributions and metadata are in `summary.json`.

Development, three rotations:

| Case | Baseline median [range] | Candidate median [range] | Stock median [range] | Candidate/stock | Candidate−baseline pairs |
|---|---:|---:|---:|---:|---|
| Dense two FE N16320/K16/T4 | 109 [109, 109] | 82 [81, 85] | 64 [61, 69] | 1.281 | -27, -28, -24 |
| Unbalanced two FE N15360/K10/T1 | 108 [108, 110] | 67 [67, 69] | 68 [68, 68] | 0.985 | -41, -43, -39 |
| Narrow two FE N12000/K3/T1 | 64 [63, 66] | 51 [51, 52] | 35 [35, 36] | 1.457 | -12, -15, -12 |
| Weak two FE N7680/K4/T1 | 34 [32, 34] | 32 [31, 32] | 430 [427, 437] | 0.074 | -3, -2, +0 |

Fresh, five rotations, fast mode:

| Case | Baseline median [range] | Candidate median [range] | Stock median [range] | Candidate/stock | Candidate−baseline pairs |
|---|---:|---:|---:|---:|---|
| Dense two FE N18000/K13/T4 | 77 [75, 80] | 75 [74, 78] | 57 [56, 57] | 1.316 | -2, -2, -2, -2, +1 |
| Sparse three FE N27000/K8/T8 | 210 [206, 216] | 211 [204, 215] | 5534 [5531, 5589] | 0.038 | +4, +1, +0, -4, -3 |
| Disconnected two FE N21600/K9/T4 | 81 [80, 82] | 80 [79, 82] | 54 [52, 56] | 1.481 | -2, +0, +1, -3, +1 |
| Unbalanced two FE N15840/K11/T1 | 123 [119, 124] | 70 [69, 70] | 78 [77, 78] | 0.897 | -54, -49, -54, -53, -53 |
| Narrow two FE N12960/K3/T1 | 72 [70, 73] | 53 [50, 54] | 36 [35, 38] | 1.472 | -19, -18, -22, -18, -19 |
| Weak two FE N8640/K4/T1 | 36 [36, 37] | 34 [33, 36] | 604 [601, 607] | 0.056 | +0, -4, -1, -3, -2 |

Fresh unbalanced improves 43.1%; its 70/78 ratio is 0.897. The 0.90 threshold is
70.2 ms, so the margin beyond the requested target is only 0.2 ms by medians;
do not claim a comfortable universal 10% lead. Narrow improves 26.4% but still
loses to stock. Dense, disconnected and narrow require approximately 31.6%, 39.3%
and 38.9% further reductions to reach their respective 51.3, 48.6 and 32.4-ms targets.
Sparse and weak already clear the target. Small median changes and every losing
pair remain visible. The large development wide gain does not transfer to the
fresh larger dense case: the latter already entered the old Schur route.

Focused controls, three rotations:

| Case | Baseline median [range] | Candidate median [range] | Stock median [range] | Candidate/stock | Candidate−baseline pairs |
|---|---:|---:|---:|---:|---|
| One FE N16320/K12/T4 fast | 44 [42, 45] | 44 [42, 44] | 26 [24, 26] | 1.692 | +0, +2, -3 |
| Two FE N17280/K12/T1 low | 110 [109, 110] | 108 [108, 109] | 117 [116, 118] | 0.923 | -1, -1, -2 |
| Unbalanced three FE, multiple slopes N19200/K10/T4 fast | 114 [114, 115] | 117 [114, 117] | 151 [149, 152] | 0.775 | -1, +3, +3 |
| Two FE N18240/K64/T8 low | 324 [323, 326] | 322 [318, 322] | 224 [223, 225] | 1.438 | -5, -4, -2 |
| Unbalanced three FE N18000/K8/T4 low | 98 [97, 100] | 97 [95, 98] | 70 [70, 70] | 1.386 | -3, -3, +1 |

| Case | Baseline median [range] | Candidate median [range] | Stock median [range] | Candidate/stock | Candidate−baseline pairs |
|---|---:|---:|---:|---:|---|
| Categorical OLS N16320/K16/T4 | 36 [35, 36] | 36 [35, 36] | — | — | +1, +0, -1 |
| Two-partition slopes OLS N30720/K6/T4 | 70 [69, 71] | 69 [68, 71] | — | — | -1, +0, -1 |

One-FE, slopes, three-FE and both OLS control exports are exact. The PPML slope
control loses 3 ms (2.6%) by medians with two +3-ms pairs; this route is unchanged.
Retain that observed loss, not a universal non-regression claim. The OLS categorical
control includes a +1-ms pair. No guard is adjusted to those observations. Three
small independent-oracle pilots (one rotation) have 12→11, 18→18 and 12→12-ms native
medians versus 14, 19 and 14 ms stock; they establish science, not a performance claim.

## Whole-command attribution and remaining cost

Eight separate diagnostic public applications pass all public-state/native-pair
checks. These are instrumented artifacts, not headline timings. Spans nest:

| Development case | Native root before→after | Checkpoint LSMR attempt solve time before | Full Schur calls/time after | Ordinary joint solves after |
|---|---:|---:|---:|---:|
| Dense | 98.617→72.235 | 32.679 (4 attempts) | 3 / 9.939 | 6 / 23.770 |
| Unbalanced | 96.161→55.376 | 44.869 (4 attempts) | 3 / 6.418 | 5 / 19.041 |
| Narrow | 54.865→40.184 | 17.891 (3 attempts) | 4 / 4.934 | 6 / 16.607 |
| Weak | 25.452→22.430 | 4.701 (3 attempts) | 3 / 3.165 | 5 / 5.779 |

Dense checkpoint LSMR solves 37 RHSs / 334 summed column recurrence steps; candidate
uses three Schur batches with 254 full-width iteration slots. Unbalanced is
27/246 versus 121 Schur slots; narrow 10/87 versus 78; weak 12/12 versus 90.
These are distinct operation types: a Schur step applies both weighted graph
couplings across the batch; LSMR uses separate per-column observation-space
forward/adjoint actions and preconditioning. Inactive Schur columns can still occupy
operator lanes. Do not interpret the counts, or fixest's accelerated iterations,
as identical units of computational work. `checkpoint-work.json` preserves attempts,
widths, steps, convergence and joint-frame diagnostics.

Dense Schur trace totals: weighted initialization about 0.575 ms, operator 4.704,
recurrence 0.104, reconstruction 0.353 and fresh final certificates **4.038 ms**.
Thus stronger acceptance remains material, but no longer consumes the old
32.7-ms LSMR attempt time. Seven total canonical certificate calls cost 4.709 ms
including initial checks, versus eight/5.966 before. Two canonical rank constructions
cost 6.688 ms (nested), versus 10.064 before. Ordinary joint solving still costs
23.770 ms, including 15.006 recurrence and 4.510 independent finish/certificate.
Narrow ordinary recurrence/finish costs 8.945/3.581 ms. These are measured remaining
costs; a projected gain from replacing that engine remains a hypothesis.

The next bounded implementation should compare ordinary categorical joint directions
on frozen frames against the existing coefficient-space joint mechanism used by
slopes/low-memory PPML. Repair/admit ownership and normalization as necessary while
keeping this canonical checkpoint owner and final-Hessian authority independent.
Inspect existing original-control borrowing and prepared topology before adding a
cache. Measure setup, observation passes, predictor warm starts and independent
normal checks. Do not reopen the rejected all-coefficient canonical route or spend
another round tuning this checkpoint cutoff. Joint solve savings alone cannot be
assumed to deliver every outstanding 0.90 target.

## Validation, memory and limits

**56 distinct Rust tests**, production ABI/panic checks, **four licensed fixtures**,
**192 public applications**, **180 public-state checks**, **nine independent Python
oracles**, **66 native pairs** and **60 direct candidate/stock comparisons** pass.
Four diagnostic capture fixtures and eight later diagnostic applications also pass.
Coverage includes independent weighted contrasts across frames/widths and component
mass disparities, full original-input fallback after cap/certificate rejection,
exact/insufficient transient admission, fatal allocation/interruption, shared OLS
probe/continuation/budget behavior, and prepared PPML strict-profile/dynamic-range
recovery. Licensed fixtures cover Poisson/Hessian/sandwich, unit/FW/PW, T1/T4,
robust/cluster, generated terms, reconstruction, separation and rollback. Ordinary
fixest weights are not used as the frequency-weight inference oracle.

Thirty-two native export pairs are exact. Remaining maximum native differences:
coefficients 1.232e-11, covariance 1.700e-16, fitted means 2.416e-9, likelihood zero,
deviance 7.276e-12. All sixty PPML pairs retain outer iteration counts, rank/DoF,
step halvings, fallback/strict-rejection metadata, modeled selected peak and scratch
bytes. Inner iteration counts intentionally change (-35 to +14); they are not
cross-engine work units.

No new retained heap bank: one bounded original-input candidate and existing graph
storage replace LSMR transport/workspace for successful calls. A fixed 64-double
(512-byte) stack bank holds progress energies. Actual tracked peaks are exact in
25/60 PPML pairs and change -5,845,725 to +304,332 bytes overall; no universal physical
memory reduction or scheduler/allocator attribution. Development dense peak falls
24,112,185→18,266,460 bytes (24.2%); unbalanced 15,124,671→11,930,420 (21.1%).

The first integration test caught missing fallback accounting when the discarded
pilot disappeared; corrected before production capture. A stale test-field compile
error and a missing Cargo PATH also preceded final green builds. Their logs and
source snapshots remain in receipts; no failed build supplies a timing claim.

Static allocation source drift remains an explicit qualification failure. Weighted
constrained fits, explicit/insufficient-budget nonretaining-CG instability, historical
large one-FE stock-mean discrepancy, separate 1e6-shift failure, synthetic exact-fit
three-FE ordinary-LSMR refusal and SCC/Linux/physical-cap confirmation remain open.
No stopped campaign, auth retry, frozen manifest change, push, release or qualification.
