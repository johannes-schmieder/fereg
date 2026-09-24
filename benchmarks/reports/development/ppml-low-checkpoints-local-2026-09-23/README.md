# Low three-FE PPML canonical checkpoints — local development, 2026-09-23

Adopt the bounded, component-scaled LSMR checkpoint path in
[ADR 0143](../../../../docs/adr/0143-low-memory-three-fe-checkpoints.md).
The ordinary joint solver is unchanged. The final artifact improves all 18
confirmation pairs by 19–88% in whole-command median terms. Two of six cases
beat comparable stock by at least 10%; three are near parity and the narrow T8
case still loses. This does not complete the broader PPML performance objective.

## Source and measurement boundary

Base: `d71ad743`, production baseline `profilemom1`, plugin
`abcbf06ea8d0403d72950412f39b735f10b89fe80f0249a0f6c7632396b050a9`.
Final: `ckptlow5`, the eight overlays in `confirmation-declaration.json`, plugin
`c07ad1086ef53f4fa7e79b6c2ee6a8b2a4c61decfce757077d57006f65337eef`.
All 541 computational input hashes match the committed source.

The principal five-rotation development/fresh blocks used `ckptlow4`, plugin
`d86b692e66e4918cc21df7b00548f73e13e0d173f3c56b3e6ff6383972598829`.
`ckptlow5` changes only `crates/fereg-plugin/src/ppml_estimate.rs`: its publication
guard checks **selected** low storage, including an automatic request, rather than
requiring an explicit low request. It adds a positive/negative boundary test.
The other 540 inputs, including the complete computational core and Stata adapter,
match exactly. Four same-data fresh exports and mean vectors match byte for byte
across these artifacts (`publication-bridge.json`). Older timing claims remain
attached to their named artifacts; the final artifact has a separate confirmation.

Local Mac arm64 development only. Headline blocks rotate baseline/candidate/stock,
with one untimed warm-up per fresh process. Timers cover the public command,
robust inference and requested fitted means; input import is outside both timers.
All 212 adopted-path headline applications have passing status/sentinels and
empty overlap/monitor-error lists. Builds, tests, diagnostics and archiving were
separate from these headline fits. This is not a qualification matrix.

Stock is the pinned unmodified fixest 0.14.2 archive, SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`, with
`glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads, robust correction and means.
Headline inputs are positive, unweighted numeric PPML. Weighted fixtures provide
separate native/oracle evidence; ordinary fixest weights are not used as a Stata
frequency-weight inference oracle. Default-reference accuracy failures are not
reinterpreted here.

## Final-artifact confirmation

Three predeclared rotations, seed 202609348. Milliseconds shown as
**median [minimum, maximum]**. The 0.90 target is a goal computed from this block's
stock median, not an attainable-runtime forecast. Full ordered samples and paired
changes are in `summary.json` and `confirmation/progress.json`.

| Low three-FE case | Baseline | Final | Stock | Final/stock | 0.90 target |
|---|---:|---:|---:|---:|---:|
| Dense N30720/K6/T4 | 339 [337,341] | 241 [237,246] | 236 [234,238] | 1.021 | 212.4 |
| Dense N61440/K16/T8 | 1031 [1029,1059] | 744 [731,747] | 771 [767,776] | 0.965 | 693.9 |
| Unbalanced N24580/K10/T1 | 348 [345,354] | 282 [278,284] | 284 [279,288] | 0.993 | 255.6 |
| Dense N16384/K3/T8 | 176 [175,176] | 135 [134,142] | 80 [75,80] | 1.688 | 72 |
| Sparse N15360/K6/T4 | 1030 [1030,1036] | 125 [125,127] | 2319 [2318,2343] | 0.054 | 2087.1 |
| Dense N12288/K24/T4 | 700 [699,703] | 251 [243,253] | 328 [328,330] | 0.765 | 295.2 |

Candidate-minus-baseline pairs, respectively:
`[-100,-98,-95]`, `[-282,-328,-287]`, `[-61,-70,-72]`,
`[-33,-41,-42]`, `[-905,-905,-909]`, `[-449,-450,-456]` ms.
Every final confirmation public-state and pairwise/direct stock check passes.
The remaining deficits to the 10% goal are 28.6, 50.1, 26.4 and 63 ms in the first
four rows. The narrow T8 deficit remains substantial.

## Five-rotation evidence and excluded-path controls

These rows belong to **ckptlow4**, not a later artifact. Dense development uses
seed 202609347; fresh and controls use 202609348. Fresh cases were declared before
measurement; no thresholds were fit to these seeds or names.

| Block/case | Baseline median | Candidate median | Stock median | Candidate/stock |
|---|---:|---:|---:|---:|
| Development dense N30720/K6/T4 | 339 | 240 | 234 | 1.026 |
| Development dense N61440/K16/T8 | 1014 | 725 | 746 | 0.972 |
| Fresh unbalanced N24580/K10/T1 | 348 | 281 | 281 | 1.000 |
| Fresh dense N16384/K3/T8 | 190 | 132 | 82 | 1.610 |
| Fresh sparse N15360/K6/T4 | 1030 | 128 | 2331 | 0.055 |
| Fresh dense N12288/K24/T4 | 698 | 247 | 335 | 0.737 |
| Disconnected N30720/K6/T4, 3 native rotations | 4807 | 1357 | unranked | — |

All 54 paired observations on the changed route, including pilots, disconnected
native checks and final confirmation, improve. This is a bounded local sample,
not a universal guarantee. The disconnected stock comparison is unresolved:
`screen-public-fixed` fitted means differ by up to 1.9696e-5, about 49 times the
registered envelope, despite passing coefficients, VCE and objective comparisons.
Its stock timings are preserved and not ranked. No later stock replay replaced
that discrepancy.

Three-rotation controls at N30720/K6/T4, except one-FE K32, also belong to ckptlow4:

| Unchanged route | Baseline median | Candidate median | Stock median | Paired changes, ms |
|---|---:|---:|---:|---|
| Fast categorical three FE | 178 | 183 | 239 | -4, +12, -5 |
| Low two slope partitions | 157 | 157 | 120 | +2, -3, +1 |
| Fast two slope partitions | 127 | 127 | 120 | +3, -1, +2 |
| Low categorical two FE | 133 | 130 | 58 | 0, -3, -3 |
| Low one FE, K32 | 107 | 105 | 124 | -2, -2, -9 |

All 15 control pairs have exact scientific exports and mean vectors. Two OLS
controls (multipartition slopes and low three-FE, two rotations each) have exact
science: 68.5→68 ms with pairs -2,+1; 66.5→66.5 ms with pairs 0,0. Losses remain
visible; their causes are not established and no repeat replaces them. Slope and
narrow two-FE PPML still have meaningful stock deficits.

## Attribution and residual costs

Diagnostic builds `canonledger3` and `canonledger4` are isolated from production
and stock timing. Their complete source, instrumentation recipes, native spans,
solver traces and audit results are preserved. They are whole public-command
profiles on the same data, **not a frozen-weight native/stock inner-solver experiment**.
No claim of a complete stock operation ledger follows from them.

For dense N30720/K6/T4, diagnostic plugin time is 378.475→223.967 ms. The baseline
makes 137 symmetric observation projections (109.838 ms), 246 grouped-statistic
calls (30.554 ms), and 82 canonical certificate calls (41.124 ms inclusive).
The candidate replaces that CG work with five LSMR batches totaling 79.210 ms,
48 grouped-statistic calls and 16 canonical certificate calls (7.735 ms inclusive).
Norm calls fall 82→21. Every accepted tile still receives the independent
original-weight certificate; the recurrence does not require CG's repeated
intermediate checks.

At N61440/K16/T8, diagnostic plugin time is 1039.104→709.948 ms. The baseline's
198 symmetric projections consume 276.293 ms and its seven CG calls 538.143 ms
inclusive. Seven replacement LSMR calls consume 238.586 ms; grouped-statistic
calls fall 345→66 and norm calls 115→29. Strict/final public phase medians in the
five-rotation blocks fall 163.3→84.6 ms and 477.2→265.8 ms respectively. Sparse
strict/final construction falls 764.5→44.2 ms.

Candidate trace totals separate checkpoint setup/iteration/certification:
21.972/53.692/7.032 ms for K6, and 55.589/173.148/19.306 ms for K16. They are trace
categories, not additive to overlapping span totals. Component-topology/scaling
construction costs 2.429 ms across the five K6 calls. Current-frame factor reuse
could reduce repeated setup, but it cannot by itself remove the larger remaining
iteration and ordinary-direction cost. Unattributed IRLS-exclusive spans remain
93.135 and 314.353 ms; they include work outside the instrumented child functions,
not a measured lower bound for scientifically mandatory checks. The slope control
is 138.055→141.469 ms diagnostically and receives no claimed optimization here.

## Science, memory and failures

- 444 distinct Rust tests pass: 395 core plus 46 plugin tests on final ckptlow5,
  and three additional checkpoint/fallback/Schur tests on the byte-identical
  ckptlow4 core. A fresh Cargo target binds each final artifact. Production ABI
  self-test and injected-panic containment pass.
- Six final-artifact licensed runs of five fixtures pass with explicit return
  codes and sentinels: weighted explicit-dummy Poisson/fast comparison, slope
  oracle, separation, new-route publication rollback, and two allocation-audit
  configurations. The weighted oracle retains its original numerical thresholds
  and tests two/three FE at pool widths 1, 2 and 10000; its local derivative omits
  an unrelated historical one-FE scratch assertion.
- T1/pool1 and T4/pool2 measured ordinary windows have exactly zero allocation
  events/growth, stable capacities and independently checked fresh/warm science.
  The canonical checkpoint is outside that window and may allocate.
- All 204 final/adopted public PPML states and 69 native pairs pass. There are 66
  baseline/stock and 66 direct candidate/stock checks; two pilot stock comparisons
  fail covariance, leaving those pilot timings unranked. Seven of nine independent
  small Poisson/Hessian oracle results pass: all six native results and one stock
  result. Both other stock oracle covariance failures are retained. No tolerance
  or inference policy was changed to reconcile them.
- Additional tests cover analytically known weighted FE projections across
  component-mass changes, zero RHSs, threads, forced fallback, report aggregation,
  a one-byte-below optional-memory limit, rank/zero controls, and the existing
  extreme dynamic-range fixture. The final publication guard accepts selected-low
  automatic requests and rejects fast storage or the wrong FE count.

Speed uses additional optional memory. Across the seven ckptlow4 changed-route
headline shapes, tracked peak increments are **6.17–36.19 MB**; admitted model
increments are **19.74–314.83 MB** (decimal units). All sampled tracked peaks stay
below their model. Dense K6 increases 29.30→62.10 MB tracked and 34.76→98.87 MB
modeled; K16 increases 78.68→114.87 MB tracked and 88.90→403.74 MB modeled. Scratch
storage is unchanged. Low storage remains bounded; this is not a claim that its
fastest admitted route minimizes RAM. Tight-budget refusal keeps prior solving
and existing joint ownership. Physical-cap and Linux confirmation remain open.

Rejected/intermediate evidence is retained:

1. The unscaled LSMR prototype passes 389/391 PPML tests but fails the unchanged
   extreme-component-mass case and heterogeneous zero-work report merge. Its
   patch/log remains; component scaling and explicit report aggregation address
   these distinct failures.
2. `ckptlow1` passes Rust science but has 23 untimed warm-up failures across its
   pilot/screen because public validators disallow MAP ordinary + LSMR checkpoint.
   The original controller incorrectly proceeded after a zero audit process exit;
   successor controllers check application and scientific results explicitly.
3. `ckptlow2` fixes publication and supplies the first timing block. Two-effect
   gains are too small to justify extending the route; final admission uses three
   effects. The large disconnected stock mean discrepancy stays unresolved.
4. `ckptlow3` fails two allocation-audit publications because ordinary and final
   solver identities were required to match. `ckptlow4` separates those identities
   while retaining zero-event and numerical checks. `ckptlow5` fixes automatic-low
   publication admission without altering numerical computation.
5. Old command and transaction fixtures fail literal protocol/receipt-version
   assertions on both baseline and candidate. Baseline reproduction binds the
   old artifact and its ado tree. They remain failed historical fixtures, not
   accepted runs or regressions attributed to this solver. The new focused
   transaction fixture passes. Build/fixture-controller setup mistakes and an
   initial direct-mean summary shape error also remain captured; the corrected
   summary applies the audited retained sample before comparing means.

No SCC authentication retry, stopped campaign, push, release or full qualification.
The documented static-inventory drift and weighted constrained/nonretaining-CG
issues remain open. Frozen 77 primary, 16 holdout and 111 native-control definitions
and protected cycle seeds are unchanged.

## Receipts and next milestone

`receipts-content-addressed.zip` holds **13,612 logical receipts**, 2,739 payloads,
24,128,649 bytes. SHA-256:
`383f68b947d537bb317e0ac0e0979df9b8c12051acd1954ad95e8069fc03f317`.
Restore with `restore_receipts.py NEW_DIRECTORY`; every logical payload hash is
checked. All receipts were restored and verified locally. Raw licensed logs,
data and binaries are omitted; hashes, source, commands, protocol traces,
scientific exports, failures and controllers are retained. Key entries are
`summary.json`, `checkpoint-trace-summary.json`, `publication-bridge.json`,
`ckptlow5-licensed-results.json`, `identity-ckptlow5.json`, the declarations and
all source/artifact snapshots. Human-readable source-bound reports are not
substitutes for raw distributions.

Close this canonical-projector milestone. Next prioritize narrow one-/two-FE
whole-command work: the prior one-FE 97/52-ms result and this round's low two-FE
130/58-ms control indicate substantially more opportunity than another small
near-parity adjustment. First split native preparation, ordinary directions,
accepted-state scans and publication on a narrow N/K ladder. Then implement one
coherent ownership/dataflow change against a per-case <=0.90 stock budget. Keep
wide inference/binding and multi-partition slope setup as subsequent cost centers.
Current-frame checkpoint geometry reuse remains a specific secondary opportunity,
not a reason to prolong this bounded round.
