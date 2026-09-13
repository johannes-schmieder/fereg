# Saved-Schur comparison: primary gains, incomplete adoption screen

**Not adopted.** The five-file saved-Schur/recovery-admission prototype remains
under evaluation. Nine primary pairs pass; protected controls expose three
baseline residual discrepancies and an independent publication bug. The latter
is addressed in the separate [omission repair](../ols-derived-output-omissions-local-2026-09-13/README.md).
The original remaining controls must still be completed against that repaired
baseline. Earlier screens, failures and timing flags remain immutable evidence.

## Source, admission and validation

[Identities](built-identities.json) bind 176 inputs, accepted baseline
`834703b798a560e4a52cb8df7ed0719777149f0c`, plugin `fdc3e141`, and candidate
`fb87d903969760c6b7b1220af763f0760e98bf19fa4756d92c6e681b979c046b`.
The [patch](implementation.patch) allows ordinary OLS to recover saved FE from
its certified final target after an admitted Schur solve. Rejected probes retain
their original coefficient products. Existing weight probes, width/iteration
guards, raw scales, independent certificates and fallback families remain.
PPML and absorbed-slope policies are unchanged.

The [overlap audit](recovery-overlap-audit.md) accounts for initial selection,
released Schur state before CG continuation, one combined retained coefficient
RHS, inference-cache release, final targets, normalized components, requested
channels, CG/SD/LSMR/MAP recovery and retries. Each intercept recovery route has
its own checked reserve; expensive rejected routes may use an admitted cheaper
MAP fallback. Existing opaque Schwarz allowances and incomplete public whole-
action reporting remain limitations, not universal memory proofs.

Fifteen focused core checks and five plugin saved-output checks pass. All 48
independent saved-OLS dummy fits pass with actual new Schur admission on eligible
fits. Both architecture builds/static audits and native arm64 ABI checks pass.
Public final-recovery exact-cap success and one-byte-short r(909) rollback pass.
Original unit/PW initial checks succeed at usable budgets 827,753,800 and
831,179,336 bytes and return r(909) one byte below. Exact-cap scientific/saved
exports match earlier captures. The canonical large-cap worker checks return
codes; the separate small recovery fixture establishes transaction rollback.
The initial collector expected text suppressed by the quiet worker; it was
corrected to use the explicit return-code sentinel without rerunning completed
captures. All attempts remain in [validation](validation.json) and log hashes.

The source-derived AW initial execution-plus-CG reserve is 3,310,918,664 bytes;
its retained-CG counterpart is 4,358,839,304. Unit/PW retained counterparts are
1,089,733,960 and 1,091,614,056. These are modeled reservations, not observed
allocations. In particular, a rejected probe must restore retained admission;
its observed peak cannot be judged against the smaller nonretaining reserve.
The public scalar still reports only the execution base.

## Nine original primary pairs

The [prospective protocol](protocol.md) uses the original specifications at
seeds 104729, 8675309 and 20260819. All requests include residuals. Calls use
fresh local M2 Ultra Stata/MP processes, unchanged original/verified input files,
small warm-ups and rotated arms. No build/test/profile/data generation overlaps
timing. Affinity is uncontrolled; these are local development findings.

| Saved-output target | N / K / workers | Command medians, seconds | Median paired change | Native peak medians, MB |
|---|---|---:|---:|---:|
| Unit, both FE | 245760 / 50 / 16 | 2.896 → 0.694 | −76.1% | 839.05 → 378.71 |
| AW, both FE | 983040 / 50 / 4 | 82.925 → 65.283 | −22.3% | 3363.96 → 2316.04 |
| PW, partial FE, missing, two-way cluster | 245760 / 50 / 16 | 3.499 → 2.582 | −25.8% | 841.84 → 581.41 |

Each target accepts Schur on two seeds and rejects it on one; the medians include
those rejections. Median native savings are approximately 54.9%, 31.2% and 30.9%.
No primary increase crosses both 5% and 10 ms. Rejected unit/PW probes add
42/33 ms (1.45%/0.94%); rejected AW changes −406 ms/0.49%.

Accepted unit fits charge 8 probe plus 57 Schur iterations. Accepted AW fits
charge 8 probe, a full 64-step attempt and 219 CG continuation iterations;
PW charges 8+64+66. Savings include that discarded attempt and final recovery.
Rejected fits add the eight-step probe to their legacy iteration counts. Solver
setup/solve times, fallback flags, input/native/outer costs and sampled RSS remain
in [comparisons](comparisons.json), [summary](summary.csv) and [captures](captures.json).
Overlapping phase medians are not additive. No favorable-seed subset or new
external-comparator claim is used.

## Protected controls and independent adjudications

Five ordinary control pairs pass exact science and saved exports: weighted cut,
small AW, low FW clustered, complete IID and narrow saved. Their command changes
are +1/0/0/+5/0 ms; native peaks are exact. Complete IID is 0.561→0.566 seconds.
The tiny weighted cut rejects Schur and retains its original numerical result.

Three other pairs return r(0) but fail the original saved-value parity assertion.
Each stops the batch before diagnosis. The same exact data are checked using an
independent grounded explicit-dummy direct solve, finite SVD regression and
separate explicit LSQR/LSMR solutions. Direct and iterative residuals agree
within 9e-13. Full paired b/V and sample/status/DoF pass; independent finite HC1
covariance errors are below 1.8e-15. FE normalization uses the actual components.

| Control | Baseline oracle failures | Candidate oracle failures |
|---|---|---|
| Weak degree-three, two FE | 1 residual row | None: residual and both FE channels |
| Disconnected, 16 components | 1 residual row | None: residual and both FE channels |
| Unbalanced | 155 residual rows; 216 first-FE rows | None: residual and both FE channels |

The saved-output tolerance remains atol1e-9/rtol1e-7. Candidate maximum tolerance
ratios are below one for every output on every control. Thus the candidate is
independently more accurate on these requests; matching the inaccurate baseline
would be the wrong numerical target. The original parity comparisons remain
**failed**, separate from accepted paired timing aggregates. Their 0.096→0.069,
0.070→0.044 and 0.066→0.048 second timings are diagnostic only. Detailed
[weak](weak-adjudication.md), [disconnected](disconnected-adjudication.md) and
[unbalanced](unbalanced-adjudication.md) adjudications, original arrays and failed
logs' hashes are preserved. This is not a general exemption for future failures
or a proof that native FE moments bound all pointwise residual errors.

The slope control then fails its baseline warm-up before any target measurement.
A noisy diagnostic reproduces r(498) on both artifacts: the derived-output
selector rejects a legitimately omitted generated column. The separate repair
preserves the same slope and rank-deficient requests; neither is replaced by a
smaller passing performance control. Three-FE saved, rank-deficient saved and
explicit-CG saved pairs were not executed in this screen. The prototype has not
met its full adoption protocol.

This archive contains 34 successful timed commands, one failed baseline warm-up,
four cap-worker captures and nine independent oracle CSV records (48 capture
entries total). Fourteen ordinary pairs pass; three paired scientific failures
receive the bounded independent adjudications above. No stopped call was rerun
as a passing original comparison. Original residuals, full covariance, solver
telemetry and input identities are stored in checksummed, deduplicated ZIP parts.

Earlier +36/+43 ms incremental recovery-planner cost flags remain disclosed in
the [prior report](../saved-recovery-admission-local-2026-09-13/README.md). These net
primary gains do not explain those earlier binary differences or demonstrate
that the planner itself speeds a command. P0 whole-action reporting remains open.

Next rebase on the independent omission repair, retain these baseline-error
adjudications and complete the original controls and source-bound adoption
comparison. No installed dist replacement, agents, SCC, dependency update,
qualification or push. Separate Veneto work and historical receipts are intact.
Replay scripts use environment roles; expand them in archived request JSON.
Licensed logs, binaries and datasets remain outside Git.
