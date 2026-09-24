# Categorical PPML accepted-frame finalization — local development

Adopt `catfinal2` under [ADR 0152](../../../../docs/adr/0152-categorical-ppml-accepted-frame-finalization.md).
This is a bounded development experiment, not qualification. The broad whole-public
PPML target of **at most 0.90 times comparable stock fixest remains unfinished**.
Fresh unbalanced two FE improves 23.2%; dense two and sparse three FE improve
8.0% and 5.5%. Other fresh medians improve 2.4% or tie. No new universal speed,
memory, platform or accuracy claim follows.

## Exact scope and identities

Baseline `unifiedproduct1` matches all 542 captured computational inputs of
`20d9b25c`. Candidate `catfinal2` uses that commit plus the two PPML source files
and the new independent Stata fixture listed in its identity. All **543 candidate
inputs match the accepted worktree**, including tests. No frozen 77/16/111
benchmark definition, reference pin, tolerance or sample rule changed.

| Artifact | Role | Plugin SHA-256 |
|---|---|---|
| unifiedproduct1 | accepted baseline | `8bc61627701864b4bd88c2ed7307c8014ea2cffbc47cd28a389f35e2d8db3bda` |
| catfinal1 | same Rust candidate; first output fixture had a double-offset error | `ac1d493e01a1095a139de72907f2c183850a49fadc78a2ba2811939d7f73f74b` |
| catfinal2 | accepted production candidate | `288e034239e08c623aa37364360ab794fbdb786106d10298002c5a0d29907307` |
| catfinalbaseledger1 | isolated baseline diagnostic | `14f2b65c6f4799e33be17197877a98b4973ae772dbae91935c09d9bd8f97aa3a` |
| catfinalledger1 | isolated candidate diagnostic | `3adeb173cd44d359638eba5f4807ffd75585c5818766de03dd326fb9fbf5eb2e` |

The stock library is pinned, unmodified fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Its comparable Poisson call uses `glm.tol=1e-10`, `fixef.tol=1e-11`, matched
threads, robust inference and fitted means, including R prediction time.
BLAS is serial. Native low execution controls are not physical-memory-matched
to stock. Ordinary fixest weights are not used as a Stata FW inference oracle.

Development seed 202609347; fresh seed **202609358**, now consumed and frozen.
Alternating baseline/candidate/stock rotations use the existing public harness,
scientific audits, command timeouts and process-isolation monitor. Data preparation
and warmups finish outside measured fits. Compilation, diagnostics and archiving
are separate from headline fits. No detected overlap or monitor error. Diagnostic
clocks below are explanatory, never stock-comparator headline timings.

## Computational change and scientific acceptance

Reuse the categorical LSMR owner for a new strict current-weight Newton witness,
then share the existing accepted-frame finalizer. Keep the accepted point fixed;
perform one canonical final rank/bread/Hessian construction instead of a canonical
Newton construction followed by another final construction. This changes the
computational organization, not numerical acceptance.

The witness recomputes current weights, operator, target, predictor and finite/FE
normal moments. Current active finite slots govern coefficient agreement; the
canonical final fit owns absorbed-intercept normalization. The independent final
transaction retains all-width rank/basis, projection, finite solve, inverse,
reconstruction, strict objective/predictor/coefficient agreement and exact final
`q * mu`/generation checks. Outer objective and score remain current because the
accepted state never moves. Refusal uses canonical retry; errors and publication
remain transactional. No new solver, workspace bank or benchmark-specific guard.

Only the final response refreshes the canonical session on the external-LSMR
route. A preliminary implementation refreshed it twice without projecting between
refreshes, breaking its response-delta history. The three-FE test caught that
error before public measurement. The adopted path fixes the ownership boundary;
it does not suppress the checking failure. The existing session-owned coefficient
joint path preserves its original refresh sequence.

## Science and failure receipts

- **79 distinct final-source Rust tests** pass across prepared IRLS, categorical
  LSMR and solver diagnostics, plus production ABI/panic checks. All 163 Rust
  source hashes match the tested state. Tests cover analytic weighted truth,
  T1/T4, strict witness refusal, predictor movement, basis rollback, nested/zero
  controls, exact final weights, generation/storage checks, budget refusal and
  the existing canonical checkpoint injections.
- **Four final-artifact licensed fixtures** pass explicit process/Stata return
  codes and terminal sentinels. The new two-/three-FE N4800/K8 fixture crosses
  unit/FW/PW, T1/T4 and robust/cluster inference, independently constructing the
  Poisson fit, Hessian and sandwich. It also checks mean, response residual and
  eta/xb/absorbed reconstruction. Further fixtures cover factors/products and
  shifts 0/1000, post-publication rollback and separation.
- **234 public applications**, **222 public-state checks**, **nine small
  independent Python Poisson/Hessian oracle results**, **80 native pairs** and
  **74 direct candidate/stock comparisons** pass. Six separate diagnostic fits
  also pass. No timed application failure, timeout or scientific exclusion.
- Twenty-three native export pairs are exact, including every excluded PPML and
  OLS control. Other pairs pass the unchanged envelopes: maximum absolute
  coefficient difference 6.126e-11, covariance 4.193e-14, mean 1.682e-8,
  likelihood 2.274e-13 and deviance 7.276e-12. Bit equality with the old
  finalization recurrence is not the acceptance criterion.
- Initial unit work contains a test-only compile typo and an overly narrow
  checkpoint-count assertion; both failure receipts remain. A synthetic exact-fit
  three-FE case also refuses the ordinary LSMR normal certificate **before any
  shortcut attempt**, including with the shortcut disabled. Its reproducible
  diagnostic remains open; no repair is claimed.
- The first new licensed output assertion subtracted the offset twice. Public
  `xb` already includes it. Only that fixture assertion was corrected, retaining
  the same model and tolerances; `catfinal1` and the failed receipt remain.
- Fresh generation initially rejected N24576 because the last five-row worker
  had one row. No fits had started. N24580 completes the worker; the original
  declaration, failure and correction remain. No generator or sample rule changed.

## Whole public timings

Milliseconds. Fresh has five rotations; development and excluded controls three.
All include robust inference and saved means unless marked OLS. Each line below
retains the complete time distribution and paired candidate-minus-baseline losses
in `categorical/summary.json` within the archive.

### Fresh cases, seed 202609358

| Case | Baseline | Candidate | Stock | Candidate/stock | 0.90 stock target |
|---|---:|---:|---:|---:|---:|
| Dense two FE N17280/K17/T4 | 88 | 81 | 67 | 1.209 | 60.3 |
| Sparse three FE N24580/K7/T8 | 181 | 171 | 3453 | 0.050 | 3107.7 |
| Disconnected two FE N20160/K12/T4 | 84 | 82 | 68 | 1.206 | 61.2 |
| Unbalanced two FE N15120/K10/T1 | 138 | 106 | 72 | 1.472 | 64.8 |
| Narrow two FE N12000/K3/T1 | 63 | 63 | 39 | 1.615 | 35.1 |
| Weak barbell N7680/K4/T1 | 36 | 36 | 414 | 0.087 | 372.6 |

Dense two, sparse three and unbalanced two improve in all fifteen pairs.
Disconnected has one +3-ms losing pair; narrow has two +1-ms losses and one tie;
weak has one +2-ms loss. Only sparse three and weak clear the stock target in
this fresh block; both were already stock wins. The other four remain losses.
The unbalanced case still needs 38.9% further reduction to reach 64.8 ms.

| Fresh case | Baseline samples | Candidate samples | Stock samples | Paired deltas |
|---|---|---|---|---|
| Dense two FE N17280/K17/T4 | 88/84/88/88/86 | 81/80/81/80/81 | 65/70/70/63/67 | -7/-4/-7/-8/-5 |
| Sparse three FE N24580/K7/T8 | 175/188/181/196/180 | 163/171/173/175/168 | 3437/3445/3453/3454/3460 | -12/-17/-8/-21/-12 |
| Disconnected two FE N20160/K12/T4 | 88/83/84/87/83 | 84/86/80/80/82 | 68/67/69/66/70 | -4/3/-4/-7/-1 |
| Unbalanced two FE N15120/K10/T1 | 138/135/141/138/136 | 106/101/106/108/102 | 73/74/71/72/67 | -32/-34/-35/-30/-34 |
| Narrow two FE N12000/K3/T1 | 64/61/65/61/63 | 63/62/63/62/63 | 40/39/39/36/41 | -1/1/-2/1/0 |
| Weak barbell N7680/K4/T1 | 34/36/38/42/35 | 33/38/36/37/33 | 412/422/414/414/409 | -1/2/-2/-5/-2 |

### Development, controls and fixed-input follow-ups

| Block/case | Baseline | Candidate | Stock | Paired deltas |
|---|---:|---:|---:|---|
| screen: Two FE N16320/K16/T4 | 130 | 124 | 66 | -6/-5/-4 |
| screen: Unbalanced three FE N30720/K10/T4 | 162 | 160 | 115 | -2/-4/3 |
| screen: Narrow two FE N12000/K3/T1 | 64 | 65 | 37 | 2/-2/1 |
| controls: One FE N16320/K12/T4 | 44 | 44 | 29 | -1/0/3 |
| controls: Low two FE N17280/K12/T1 | 109 | 108 | 116 | -2/0/-1 |
| controls: Unbalanced multiple slopes N19200/K10/T4 | 118 | 118 | 146 | 11/-2/-4 |
| controls: Low wide N18240/K64/T8 | 358 | 325 | 229 | -22/-18/-33 |
| ols-controls: OLS categorical | 36 | 36 | — | 0/0/0 |
| ols-controls: OLS slopes | 74 | 72 | — | 3/-2/-3 |
| small-followup: Small two FE N240/K3/T1 | 11 | 11 | 14 | 0/0/-1/0/1 |
| small-followup: Small unbalanced three FE N480/K4/T4 | 18 | 18 | 17 | 0/0/0/0/0 |
| small-followup: Small disconnected two FE N480/K3/T1 | 13 | 13 | 13 | 0/0/0/0/1 |
| slopes-followup: Excluded slopes, identical inputs | 115 | 116 | 145 | 0/0/5/2/2 |

The initial one-shot small pilots lost +2/+1/+3 ms. Five fixed-input repeats tie
all three medians; the original observations remain without a causal noise claim.
The excluded slope control first had a +11-ms pair with equal medians; repeats
retain a **115→116 ms** loss and +5/+2/+2-ms pairs. Its exports are exact, it still
clears the stock target, and the cause remains unattributed. No more tuning or
repeat loop is justified for this 1-ms excluded loss. Excluded threaded wide
appears 33 ms faster but is outside the changed route; do not attribute that
change to this finalizer. OLS categorical medians tie and all exports are exact;
OLS slopes include one +3-ms pair despite a lower candidate median.

## Phase attribution and memory

Diagnostic spans nest and must not be added. In the N16320/K16/T4 development
case, baseline/candidate native roots are 121.025/119.963 ms. Canonical response/
control reprojections fall 2→1 (45.623→41.400 ms) and canonical rank constructions
3→2 (12.098→9.676 ms), but **four batch LSMR calls persist: 53.274→52.808 ms**.
Ordinary plus strict joint LSMR calls rise 5→6 (18.261→24.083 ms). This explains
the modest gain and why deleting a nominal phase is insufficient by itself.

The fresh unbalanced N15120/K10/T1 diagnostic has roots 130.111→92.786 ms.
Response/control reprojections fall 2→1 (61.739→26.113 ms), canonical rank
constructions 3→2 (10.343→8.092 ms), batch LSMR calls **5→3 (73.241→42.395 ms)**,
and joint calls 4→5 (15.803→18.651 ms). The avoided full projections explain
most of the public gain. The narrow diagnostic worsens 53.712→59.167 ms despite
fewer rank constructions; its public fresh median ties. These are diagnostic
observations, not substitute distributions.

All 74 PPML pairs retain outer iteration count, sample/rank/DoF, step halvings,
fallback/profile-rejection metadata, modeled peak and scratch bytes. Inner totals
can rise while work falls because they mix joint recurrence steps and batch
maximum counts. Tracked peaks match in 58 pairs; 16 changes range from -549,071
to +594,042 bytes, with no new admitted storage and no established attribution.
Do not claim universal memory reduction. Physical caps and cross-platform checks
remain deferred; static allocation drift remains an explicit failure.

## Next bounded implementation

Close finalization-only tuning. Preserve this inexpensive shared mechanism, then
work on **canonical checkpoint batch organization**. On the wide diagnostic a
first strict batch misses the unchanged 1e-12 canonical target at approximately
1.005e-12, triggering a complete independent tighter batch. About 13.4+14.1 ms of
recurrence and duplicate setup/certification survive in the accepted code.

The current certificate already retains per-RHS infinity norms, and the pinned
solver exposes a reusable prepared batch interface. Investigate retaining only
immutable/current-weight setup within one absorption and refining only failed
columns, with a fresh full original-weight certificate afterward. Bound overlap
of accepted columns, retry transport and recurrence work explicitly; retain the
existing untouched-input full retry/MAP path when optional work cannot fit. Do
not replace independent acceptance with recurrence residuals or tune to seed 358.
If that cannot supply a meaningful broad gain, evaluate a coherent direct
coefficient-space categorical checkpoint engine using existing Schur geometry,
with evidence-based work/memory admission rather than removing guards blindly.

Subsequent priorities remain narrow original-input passes, one-FE/output overhead
and threaded-wide losses. Current local targets are explicit in the table; no
promise of parity follows from one phase improvement. The historical large
one-FE stock discrepancy, 1e6-shift failure, constrained-fit/CG issues, protected
cycle seeds 211/212, static inventory drift and deferred SCC/physical-cap work
remain unchanged. No stopped campaign, push, release or qualification was run.

## Receipts

[Content-addressed archive](receipts-content-addressed.zip), SHA-256
`00395c9c88d1ed56e399473c991fc42a6c86752604a072f1fdea1fb682f22c07`:
**9,537 logical receipts, 2,185 payloads, 15,526,975 bytes**. Exact input sources,
identities, build/test results, declarations, all rotations and follow-ups, audits,
phase spans and recipes are included. Binary plugins and raw licensed logs/data
are excluded with hashes; machine paths are redacted. The archive was verified
and restored with [restore_receipts.py](restore_receipts.py):

```bash
python3 benchmarks/reports/development/ppml-categorical-finalization-local-2026-09-24/restore_receipts.py target/categorical-final-receipts
```

The restored `categorical/summary.json`, `metadata-deltas.json`,
`maximum-native-differences.json`, `*-ledger-summary.json` and `identity-*.json`
provide the distributions, losses, phase evidence and source/artifact bindings.
