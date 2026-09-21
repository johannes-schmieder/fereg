# OLS action owners and saved-slope recovery

Development checkpoint on main `5240e589400c888690666265263ad96f9b694aa6`.
The changes are uncommitted. This is not a qualified production candidate.

## Implemented and checked

- Retained request metadata, nested strings/components, bindings, cluster
  identities, output registries and prepared C names now enter OLS admission
  before observation loading and remain charged beside fit/output parents.
  Independent allocator deallocation matches the capacity counter, including
  deliberately unused capacities; the counter itself allocates nothing.
- Saved-slope recovery previously bypassed the intercept-only memory guard.
  Retained coefficients, LSMR and MAP now have separate checked envelopes.
  The LSMR slope transform model is shared with owned absorption. It includes
  the caller certificate workspace that remains live during finishing.
  Topology-validation scratch is admitted before validation. The smaller
  certified MAP fallback remains available when LSMR does not fit.
- Exact/one-byte-short budgets, unit/unequal weights, one/four workers,
  retained coefficients, full/partial outputs and known-span reconstruction
  pass. The recovery envelope also bounds an independently observed allocator
  peak across repeated calls. Existing interruption/allocation-failure and
  PPML recovery policies remain intact. Public base-memory fields are unchanged.

[Incremental implementation](incremental-implementation.patch) is relative to
snapshot `5240e58-ols-prepared-parent2`, including the earlier unadopted
publication/prepared-owner prototypes. The two prior inventories and every
historical failed receipt remain unchanged.

## Validation and identities

Final snapshot `5240e58-action-coverage3` binds 288 files in
[its identity](final/identity.json). All 186 numerical, Stata and build inputs
still match the worktree. Two inventory-renderer defaults were subsequently
advanced and tested separately; [source verification](source-verification.json)
records that distinction.

Job 7590299 passes **289 distinct native checks**: 174 core, 110 plugin,
3 testkit and 2 constrained-memory checks. The terminal native sentinel is
`ACTION_COVERAGE_NATIVE_PASS`. Its subsequent build wrapper invoked Cargo
outside the source directory and the job exited 101; that original failure is
preserved. Corrected wrapper job 7590309 passes binary export/ABI audits,
mock-SPI panic containment, and the fixed Stata pairs. It has scheduler
failed=0/exit=0, terminal `OLS_PARENT_PAIRED_COMPLETE`, 125 seconds including
build, and 2.486G maxvmem. No numerical retry was performed.

All **38 Python checks** pass (35 source-contract, 3 action inventory), as do
the repository checker, OLS v22 and action v5 audits and `git diff --check`.
The action inventory watches 125 files and 2,500 allocation syntax sites;
these remain structural counts, not a full byte-coverage certificate.

- Baseline: `5240e58-ols-prepared-parent2`, Linux artifact
  `a1c53228f37a23d02ea1ac06fbb2a7a0c68aedb21a1b072f9b6bf8d2adb7bd19`.
- Tested combined prototype: `5240e58-action-coverage3`, Linux artifact
  `e87a902c1a94a187513a6369312d8cd8879a0ce1601be78ab762e1cd2a49c3a0`.
- Installed Mac artifact remains
  `4187efdfbd6235e63c369d9b42dede6381b8a9e50b5360fd63ee237bd6c11b08`.
  Local Rust linking is blocked by the unaccepted Xcode license.

These comparison baselines are preceding unadopted development snapshots,
not a claim about improvement over committed main. Input values, sample order,
source, artifacts and the reference lock are hash-bound.

## Slope timing investigation

Original jobs 7590187/7590196 remain failed/inconclusive as recorded in the
[preceding report](../candidate-ols-prepared-parents-2026-09-16/README.md).
The first new check (7590274) selected four physical cores on one NUMA node,
but ran concurrently with a Rust build on the same host and overlapping CPUs.
Its identical-artifact control shifted by 83 ms; it is inconclusive.

One corrected check (7590284) ran after the native build, with the same two
preserved artifacts and input. Three alternating fresh-process pairs yield
0.193/0.200 seconds; three identical-artifact controls yield 0.229/0.220.
Both median shifts are below the declared combined 5%/10 ms threshold. All
scientific exports match. The original slowdown does not reproduce in this
controlled development check. Single-NUMA pinning is verified; scheduler
binding was requested but the inherited mask remained broad, so exclusive
core ownership is **not** established. Individual timings remain variable;
this does not establish the unique cause of every earlier outlier.

## Combined prototype cost checks

The [seven cases](final/fixed-cases.json) were fixed before measurement.
Twenty-one alternating pairs (42 fresh Stata processes) all pass with exact
coefficients, covariance, rank/status, sample/order, DoF, solver non-time fields,
certificates and saved outputs. All command medians pass the declared gate.

| Case | Baseline seconds | Candidate seconds |
| --- | ---: | ---: |
| Small/narrow | 0.024 | 0.023 |
| Tiny weighted bridge | 0.169 | 0.174 |
| Three FE, frequency weights/generated controls | 0.427 | 0.406 |
| Absorbed slopes | 0.132 | 0.136 |
| Singleton cascade, PW/cluster, low memory | 0.204 | 0.205 |
| Saved slopes, fast | 0.160 | 0.140 |
| Saved slopes, low | 0.249 | 0.253 |

Native allocation peaks, scratch and public selected-base bytes match between
arms in every pair. [Cost summaries](cost-summary.json) include phase medians,
RSS and memory; [per-command receipts](paired-results.json) retain runtime and
solver tables plus scientific hashes. Independently computed medians need not
sum. Discarded-attempt costs are included in elapsed times but are not separately
measured.

The tiny weighted bridge has a **phase-level review flag**: native medians
0.1042/0.1168 seconds (+12.6 ms), despite only +5 ms in complete command time.
This intercept-only branch has no numerical/kernel/solver-policy changes;
its new native accounting adds fixed scalar parent arithmetic, and all
non-time solver fields match. The source review does not prove the cause of
this shared-host phase variation. It remains explicit; no additional ambiguous
retry or universal speed/adoption claim is made.

## Remaining acceptance gaps

Whole-action coverage and its ratio remain undefined. Compiler/factorization
construction, complete finite/inference transient frames, complete PPML
parent/generation overlap proof, and opaque dependency/worker-pool growth still
require byte-and-lifetime closure. Versioned whole-action protocol diagnostics
also remain pending. The component fixes and passing command screen do not
waive those requirements or the phase-level review flag.

The Mac/Linux candidate remains unready. No installed package replacement,
commit, push, tag, release, German estimation rerun or stopped campaign restart
occurred. See [validation.json](validation.json) for machine-readable scope and
[raw hashes](raw-receipt-hashes.json) for original receipt identities. Logs are
path-normalized; raw licensed Stata logs and datasets are not included.
