# Selective current-frame PPML checkpoints — local development

Adopt `selective1` under [ADR 0153](../../../../docs/adr/0153-selective-current-frame-ppml-checkpoints.md).
Prepared checkpoint setup, selective tighter retries and one canonical certificate
per attempt reduce development wide PPML by **17.7%** and fresh unbalanced two-FE
PPML by **17.8%**. Other fresh median gains are 1–3 ms, with losing pairs retained.
The whole-public **<=0.90-times comparable stock fixest objective is unfinished**.
This is bounded local development evidence, not qualification.

## Exact identities and measurement

Baseline `catfinal2` matches all 543 captured computational inputs of `950aa44a`.
Candidate `selective1` uses that commit plus `fe/borrowed_map.rs` and
`fe/lsmr_batch.rs`. All **543 candidate inputs match the accepted worktree**;
all **163 Rust source hashes** match the focused test state. Frozen 77/16/111
definitions, reference pins, tolerances and sample policies are unchanged.

| Artifact | Role | Plugin SHA-256 |
|---|---|---|
| catfinal2 | committed baseline | `288e034239e08c623aa37364360ab794fbdb786106d10298002c5a0d29907307` |
| selective1 | production candidate | `aab5ad93a4a43cf49cf8b6a72e91723e8ef65fc268082f7827bf28cd47224b3f` |
| catfinalledger1 | isolated baseline diagnostic | `3adeb173cd44d359638eba5f4807ffd75585c5818766de03dd326fb9fbf5eb2e` |
| selectiveledger1 | isolated candidate diagnostic | `7615568a3dcd0d2c4783ae83b1ffb2717e3a6acd3286f93072d044c6bd412783` |

Stock is pinned, unmodified fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Comparable Poisson calls use `glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads,
serial BLAS, robust inference and fitted means, including R prediction time.
Generation/import, process startup, warmup and export are outside both clocks.
Native low execution is not physical-memory-matched to stock. Ordinary fixest
weights are not a Stata frequency-weight inference oracle.

Development seed 202609347; predeclared fresh seed **202609359**, now consumed
and frozen. Fresh has five alternating baseline/candidate/stock rotations;
development and controls have three, and small pilots one. One fixed-input
low-three-FE follow-up has five rotations and exactly matching generated values.
Builds, licensed fixtures, diagnostics and archive work do not overlap headline
fits. The process monitors report no competing fit, overlap or monitor error.
Only one production candidate was needed; no threshold or seed tuning followed.

## Computational and verification change

One checkpoint owner borrows immutable topology, solver weights and original
RHSs, and retains the existing prepared LSMR solver only within that absorption.
On a failed canonical certificate, independently passing columns retain their
bits. Only failing columns restart from their original RHSs at the existing
tighter tolerance. The complete combined result receives a fresh original-weight
canonical certificate at the unchanged tolerance. No retained residual or prior
PPML weight frame can authorize acceptance.

The old generic helper checked a temporary residual certificate, then the caller
recomputed the canonical certificate and replaced its report. Only the latter
was the checkpoint acceptance gate. Remove the temporary workspace, duplicate
reference-norm construction and duplicate certificate at this boundary. The
specialized attempt returns an explicitly uncertified report until the caller
performs its full check. The initial certificate and early return remain.
Generic direct/OLS adapters retain their separate certificates.

The complete original-input projection, model-space, rank, finite solve, inverse,
score, objective, reconstruction and exact final-Hessian requirements remain.
Keep the existing single tighter retry and untouched-input MAP fallback.
Prepared state is dropped before fallback, a changed frame or another call.
Fatal errors preserve transactional publication. Ordinary joint LSMR, one-FE,
slopes and OLS routes are unchanged. The shared column transport remains a
bitwise copy, including on paths outside the specialized adapter.

The partial-retry overlap must fit the already admitted full-batch allowance:
charge its narrow solve, retained full candidate, full metadata and actual
selection capacity. Otherwise release the rejected output before a full retry.
Runtime conversion checks actual capacities. This retains numerical setup and
some accepted columns longer, with no new whole-fit memory admission formula.
It does not claim a universal physical-memory reduction.

## Scientific and regression evidence

- **55 distinct Rust tests** pass. Independent weighted paired contrasts cover
  two/three FE, disconnected components, changing weights and T1/T4. A deliberately
  damaged column exercises selective refinement; forced budget refusal exercises
  full retry. Both recover known residuals, retain untouched inputs, reuse one
  solver and fit the modeled allowance. Passing columns remain bit-exact.
  Transport tests cross row/column tile boundaries and preserve arbitrary bits.
  Existing tests cover original-weight/component scaling, forced failure of both
  attempts, MAP fallback, fatal retry errors and prepared IRLS boundaries.
- **Four licensed production-artifact fixtures** pass process/Stata return codes
  and terminal sentinels. Independent two-/three-FE Poisson/Hessian/sandwich checks
  cross unit/FW/PW, T1/T4 and robust/cluster inference. Means, residuals, eta/xb/
  absorbed reconstruction, generated factors/products, shifts 0/1000, separation
  and transaction rollback are covered. Production ABI/panic build checks pass.
- **198 public applications**, **186 public-state checks**, **nine independent
  Python oracles**, **68 native pairs** and **62 direct candidate/stock comparisons**
  pass. Six isolated diagnostic applications pass separately. There are no timed
  failures, timeouts, scientific exclusions or detected contention.
- **59 native export pairs are exact**, including every OLS and excluded PPML
  control. Remaining pairs pass unchanged envelopes: maximum absolute coefficient
  difference 3.631e-12, covariance 3.300e-16, means 2.265e-9, likelihood and
  deviance each 2.274e-13. Algorithmic recurrence equality is not required.

All 62 PPML pairs retain outer iteration count, rank/DoF, step halvings, fallback/
profile-rejection metadata, modeled peak and scratch bytes. Inner totals are
unchanged in 53 pairs and fall by one or three in nine; those totals mix batch
maxima and joint recurrence steps. Tracked peak changes range **-329,592 to
+73,232 bytes**, with 22 exact matches. Some changes reflect longer setup
lifetimes or runtime allocation behavior; no universal peak reduction or causal
attribution to every allocator delta is claimed.

## Whole public timing distributions

Milliseconds. All PPML rows include robust inference and means. The complete
distributions, paired losses, scientific checks and metadata are in the restored
`checkpoint/summary.json`.

### Fresh seed 202609359

| Case | Baseline | Candidate | Stock | Candidate/stock | 0.90 stock target |
|---|---:|---:|---:|---:|---:|
| Dense two FE N18000/K13/T4 | 81 | 79 | 58 | 1.362 | 52.2 |
| Sparse three FE N27000/K8/T8 | 233 | 231 | 5411 | 0.043 | 4869.9 |
| Disconnected two FE N21600/K9/T4 | 82 | 81 | 54 | 1.500 | 48.6 |
| Unbalanced two FE N15840/K11/T1 | 135 | 111 | 79 | 1.405 | 71.1 |
| Narrow two FE N12960/K3/T1 | 68 | 66 | 35 | 1.886 | 31.5 |
| Weak barbell N8640/K4/T1 | 37 | 36 | 595 | 0.061 | 535.5 |

| Case | Baseline samples | Candidate samples | Stock samples | Paired deltas |
|---|---|---|---|---|
| Dense | 82/82/78/80/81 | 80/83/79/79/79 | 58/56/61/57/58 | -2/1/1/-1/-2 |
| Sparse | 233/233/231/236/236 | 248/232/227/231/230 | 5408/5411/5432/5419/5381 | 15/-1/-4/-5/-6 |
| Disconnected | 82/83/84/80/81 | 84/82/80/80/81 | 56/53/54/52/54 | 2/-1/-4/0/0 |
| Unbalanced | 135/135/134/135/134 | 110/111/111/111/111 | 78/78/79/79/81 | -25/-24/-23/-24/-23 |
| Narrow | 67/68/68/67/68 | 66/66/66/66/66 | 35/37/34/36/35 | -1/-2/-2/-1/-2 |
| Weak | 37/38/37/37/41 | 36/36/36/36/36 | 597/591/594/596/595 | -1/-2/-1/-1/-5 |

The unbalanced gain is consistent in all five pairs. Dense has two +1-ms losses,
sparse one +15-ms loss, and disconnected one +2-ms loss and two ties. Narrow
and weak improve in every pair, but only by small absolute amounts. Sparse and
weak already beat the stock target; the other four still lose. No new universal
speed or non-regression claim follows.

### Development, controls and follow-up

| Block/case | Baseline | Candidate | Stock | Paired deltas |
|---|---:|---:|---:|---|
| Screen: two FE N16320/K16/T4 | 130 | 107 | 61 | -21/-23/-25 |
| Screen: unbalanced three FE N30720/K10/T4 | 165 | 161 | 112 | -4/-5/-6 |
| Screen: narrow two FE N12000/K3/T1 | 66 | 66 | 35 | 1/0/-2 |
| Control: one FE N16320/K12/T4 | 45 | 44 | 24 | 1/-1/-1 |
| Control: low two FE N17280/K12/T1 | 108 | 109 | 118 | 2/2/-3 |
| Control: multiple slopes N19200/K10/T4 | 124 | 123 | 144 | 1/-1/-3 |
| Control: low wide N18240/K64/T8 | 321 | 318 | 222 | 1/-5/-3 |
| Control: low three FE N18000/K8/T4 | 100 | 99 | 70 | 0/60/-83 |
| Same low-three-FE input, follow-up | 102 | 99 | 72 | -1/-2/-3/-4/-3 |
| OLS categorical N16320/K16/T4 | 35 | 35 | — | 1/0/-1 |
| OLS slopes N30720/K6/T4 | 72 | 71 | — | 0/2/-3 |

Low three-FE initial samples are baseline 99/100/180, candidate 99/160/97 and
stock 72/70/69. Follow-up samples are baseline 100/102/102/102/102, candidate
99/100/99/98/99 and stock 72/73/73/72/72. Every output pair is exact and iteration
metadata are unchanged. The original spikes remain unexplained and preserved;
the follow-up supports a modest improvement without proving their cause.

The one-shot small pilots are 11→12 vs13, 19→18 vs18 and 12→12 vs14. Preserve
the +1-ms small loss; these are science pilots, not timing confirmation. The
excluded low-two-FE control loses 1 ms at its median and includes two +2-ms pairs.
Other excluded/OLS rows also contain losing pairs despite tying/improving medians.
Their exact exports and unchanged routes do not establish timing non-regression.

## Diagnostic attribution and residual cost

Instrumented clocks are separate from stock headline timings; spans nest and
must not be added. In the wide development case, native roots are
**122.334→98.475 ms**. Four checkpoint attempts remain, but their inner adapter
work falls **53.849→30.951 ms**. Complete FE certificate calls fall **12→8**,
weighted-norm calls **16→8**, and response/control reprojection falls
**42.817→22.615 ms**. Canonical rank constructions remain two (10.579→10.052 ms).
Ordinary plus strict joint solves remain six (23.812→22.985 ms).

Both first strict attempts produce canonical residual 1.0046085e-12, failing the
unchanged 1e-12 target. Baseline restarts all 17 RHSs; candidate restarts one using
the same prepared solver. Retry solve time falls **15.196→2.058 ms**, setup
**2.255→0.171 ms**, and certificate time **3.361→1.722 ms**. Full original-weight
acceptance passes afterward. Across all checkpoint attempts, RHS solves fall
**53→37**, summed per-column recurrence steps **555→334**. These count comparable
operations, unlike aggregate batch maxima.

The narrow diagnostic has unchanged 10 RHS solves and 87 summed recurrence
steps. Its intrusive native clock is 71.340→58.343 ms, but the public development
median ties and fresh median improves only 2 ms. Do not transfer that diagnostic
clock difference into a headline claim. The separate fresh unbalanced diagnostic
has four attempts and adapter work 70.869→45.371 ms; its 164.232→120.883-ms roots
and large exclusive-span variation likewise do not replace public distributions.
All raw spans and operation traces remain in the archive.

The remaining wide checkpoint adapter still costs about 31 ms, ordinary/strict
joint solves about 23 ms, and the public target is **54.9 ms versus 107 ms**.
Removing duplicate checks alone cannot close that deficit. The next bounded
implementation should compare an efficient component-scaled coefficient-space
checkpoint solve against current LSMR on identical frozen frames, reusing the
existing Schur topology/geometry where scientifically valid. Preserve the
disconnected dynamic-range boundary and strong sparse/weak routes; do not merely
remove old size guards. Use development seed 347, not consumed 359 or protected
211/212. Limit coherent candidates, measure complete public costs, and retire
superseded paths after replacement is established.

Narrow original-input passes, one-FE execution and threaded-wide low-memory
work remain subsequent targets. Existing one-FE owners already borrow controls
and share certified products under ADRs 0132/0137/0139; inspect those current
paths instead of proposing their already completed predecessors again.

Known static allocation drift, weighted constrained-fit failures, nonretaining
CG span instability, historical one-FE stock-mean discrepancy, 1e6-shift failure,
synthetic exact-fit three-FE ordinary-LSMR refusal and deferred SCC/physical-cap
checks remain unresolved. No SCC retry, stopped campaign, push, release or
qualification was run.

## Immutable receipts

[Content-addressed archive](receipts-content-addressed.zip), SHA-256
`ce5d9200c4fd93a24ce8832a37706017e1dd820634fc6a419fba4ee4004c6949`:
**7,968 logical receipts, 1,944 payloads, 13,103,840 bytes**. Includes exact
captured sources, identities, build/test results, declarations, all rotations,
follow-up, science audits, phase spans and recipes. Binaries, generated data and
raw licensed logs are omitted with hashes; machine paths are redacted.

The archive was verified and restored with [restore_receipts.py](restore_receipts.py):

```bash
python3 benchmarks/reports/development/ppml-selective-checkpoint-local-2026-09-24/restore_receipts.py target/checkpoint-receipts
```

Restored `checkpoint/summary.json`, `metadata-deltas.json`,
`maximum-native-differences.json`, `checkpoint-work.json`, `*-ledger-summary.json`
and `final-check.json` bind the claims to the measured artifacts and sources.
