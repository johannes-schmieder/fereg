# PPML accepted-state construction — local development, 2026-09-23

Adopt `acceptstate2` under [ADR 0140](../../../../docs/adr/0140-ppml-accepted-state-products.md).
It reduces representative low one-FE whole-command medians by about 5–11%,
fast one-FE by 5–7%, and the sampled categorical/slope controls by 3–6%.
The K32 low screen beats stock by 3.3%, short of the requested 10% margin.
The fresh fast two-slope screen reaches approximate parity; narrow one-FE and
low three-FE remain substantial losses. This does not complete the broader goal.

## Source, comparator and scope

Baseline: committed `2c23a7ba`, artifact `groupcanon3`, plugin SHA-256
`ba667b523455f10fa47be89e1385e928d810797467973d750c42a0749eaddb3e`.
Candidate: that source plus the six Rust overlays in `candidate-source.json`,
artifact `acceptstate2`, plugin SHA-256
`aa3cbf99c229b7d7780c01a871225ff6684476dd8bf5b2251b2f127ab283fd11`.
All **541 computational inputs** match the adopted worktree. Exact input hashes,
build recipes, source snapshots and patch are archived. Artifact ABI and panic
containment pass. `acceptstate1` was built but never publicly timed: review found
that moved predictor construction bypassed the earlier generated-term kernel;
`acceptstate2` preserves it. Iterative compile failures and preliminary tests are
retained separately from the final evidence.

Stock fixest is the pinned, unmodified 0.14.2 library from archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads, robust inference and
fitted means. Data loading/setup precedes both whole-command timers. Fereg's
public command includes native loading, preparation, estimation, inference and
publication. Default-reference accuracy failures are not reinterpreted. No
frequency-weight inference claim uses ordinary fixest weights as its oracle.

These are bounded Mac development measurements, not qualification. Frozen
77/16/111 definitions and stopped campaigns remain unchanged. Headline blocks
ran with no compilation, profiling, archiving or other detected fits; all
application returns, sentinels, isolation records and science checks are retained.

## Adopted computation and scientific checks

A finalized sample borrows immutable outcome/prior/offset inputs and prepares
per-row log-outcome/log-Gamma terms once. Changing means, Hessian weights,
responses and weighted geometry remain fresh. Cached and direct family arithmetic
is bitwise equivalent in focused tests. Composite final objective replay and the
null likelihood retain their direct independent calculations.

The canonical draft constructs one original finite predictor and transfers it
through session installation. Its independently accumulated raw-residual mean
and original-column weighted-mean formula retain the intercept check. Canonical
within residual construction binds implicit columns once per scan and uses
parallel disjoint rows; each row preserves its scaled FMA order. Dense uses the
same parallel construction; scratch keeps sequential reads. The old second
original-column reconstruction is removed. The ordinary OLS route is unchanged.

Every initial/final canonical rank, inverse, dropped-span, fresh FE residual,
finite score, outer score, objective, reconstruction and final-Hessian condition
remains. No tolerance or checking frequency changed. Transaction rollback retains
prior predictor owners. Independent oracles, not old recurrence bits, adjudicate
the changed raw-residual evaluation order.

For the measured large one-FE fit (two canonical solves, six objectives including
publication, four working frames), source work counts change as follows:

| Work | Baseline | Candidate |
|---|---|---|
| Original finite construction | Four N*K scans | Two N*K scans, transferred products |
| Implicit canonical residual recipe binding | Two N*K per-cell lookups | Two K-column bindings |
| Log-Gamma, including null objective | Seven N evaluations | Three N evaluations |
| Working-frame log-outcome | Four N evaluations | One N preparation |

`work-counts.json` states scope and retained checks. These are source-derived
counts, not an assertion that every scan has equal cost.

## Distributions and losses

Milliseconds: median [minimum, maximum]. Screen and fresh one-FE blocks have five
rotations; PPML controls have three. The last column counts candidate wins/ties/
losses against the paired current baseline, not against stock. Screen/pilot use
the existing development seed 202609335. Fresh seed 202609342 and control seed
202609343 were declared before their runs; they are now consumed. The added fast
fresh counterpart was declared before any fresh data or fit.

| Case | Baseline | Candidate | Stock | Candidate/stock | 0.90 target | Paired W/T/L |
|---|---:|---:|---:|---:|---:|---:|
| screen 0: low N30720 K6 T4 | 51.0 [50, 53] | 46.0 [46, 47] | 26.0 [25, 28] | 1.769 | 23.4 | 5/0/0 |
| screen 1: low N61440 K10 T4 | 105.0 [103, 107] | 94.0 [92, 97] | 54.0 [49, 55] | 1.741 | 48.6 | 5/0/0 |
| screen 2: low N30720 K32 T4 | 127.0 [125, 130] | 116.0 [114, 117] | 120.0 [115, 128] | 0.967 | 108.0 | 5/0/0 |
| screen 3: fast N61440 K10 T4 | 105.0 [102, 107] | 98.0 [97, 102] | 54.0 [48, 55] | 1.815 | 48.6 | 5/0/0 |
| screen 4: low N3840 K3 T1 | 22.0 [17, 23] | 21.0 [18, 22] | 12.0 [10, 13] | 1.750 | 10.8 | 2/1/2 |
| fresh 0: low N49152 K12 T4 | 96.0 [94, 98] | 85.0 [83, 88] | 53.0 [49, 55] | 1.604 | 47.7 | 5/0/0 |
| fresh 1: low N16384 K48 T4 | 138.0 [132, 138] | 124.0 [123, 128] | 108.0 [107, 111] | 1.148 | 97.2 | 5/0/0 |
| fresh 2: low N49152 K10 T1 unbalanced | 123.0 [122, 125] | 117.0 [113, 122] | 77.0 [72, 81] | 1.519 | 69.3 | 4/1/0 |
| fresh 3: fast N49152 K12 T4 | 94.0 [89, 96] | 89.0 [88, 91] | 54.0 [53, 55] | 1.648 | 48.6 | 4/0/1 |
| ppml-controls 0: fast N30720 K6 T4 three FE | 198.0 [195, 204] | 192.0 [187, 194] | 238.0 [234, 238] | 0.807 | 214.2 | 3/0/0 |
| ppml-controls 1: low N30720 K6 T4 three FE | 356.0 [340, 357] | 337.0 [331, 343] | 235.0 [229, 236] | 1.434 | 211.5 | 3/0/0 |
| ppml-controls 2: low N30720 K6 T4 two slope partitions | 163.0 [155, 163] | 154.0 [154, 157] | 130.0 [130, 131] | 1.185 | 117.0 | 3/0/0 |
| ppml-controls 3: fast N30720 K6 T4 two slope partitions | 136.0 [135, 137] | 129.0 [126, 131] | 130.0 [128, 131] | 0.992 | 117.0 | 3/0/0 |

All 30 larger multithread low/fast screen and fresh-low pairs improve. The fresh
serial low case has four gains and one tie; fresh fast has four gains and one
1 ms loss. Small one-FE has two gains, one tie and losses of 4/5 ms; its median is
not a reliable precise margin. All twelve PPML control pairs improve, but three
rotations do not establish a whole-family speed claim. Fast three-FE has a
19.3% stock lead in this fresh screen; broader sparse/weak/disconnected designs
still need confirmation. Two-slope fast is 129/130 ms, short of the 117 ms target.
Scientific discrepancies are absent from these blocks; no failing state is ranked.

Four OLS controls have two rotations and exact scientific exports. Their medians
are 71/70, 68.5/67, 18/17.5 and 69/107 ms (baseline/candidate). The generated-factor
control's last median includes a **+79 ms paired loss**, followed by a 3 ms gain.
A separately declared unchanged-source, same-seed five-pair recheck is 67 [65,72]
versus 66 [65,69] ms, with differences +1,+3,-6,0,-2 ms. Preserve both blocks;
the initial spike is unexplained and not deleted or pooled away. These controls
support scientific non-regression, not universal timing non-regression.

## Attribution and remaining cost

`acceptledger1` and `acceptledger2` are isolated diagnostic builds. Their single
rotation traces are **not headline timings**. They report observed wall intervals;
exclusive values subtract the union of captured child intervals and sum to each
plugin interval. Uninstrumented work stays in its enclosing span. Existing native
phase marginals are nested and must not be added.

The large-low diagnostic plugin interval falls 92.651 -> 78.203 ms. Exclusive
canonical-rank construction outside its separately instrumented children falls
9.165 -> 3.290 ms. Cache preparation itself costs time and is counted separately;
unchanged-kernel timing variation is not attributed causally to this change.
The whole-command paired rotations, not these individual samples, decide adoption.
The candidate's K32 diagnostic still spends 16.420 ms outside child spans in
concentrated direction preparation, 11.081 in group refresh, 10.470 in canonical
finite moments, and 9.313 in inference. This identifies remaining work beyond
likelihood caching.

In production five-rotation large-low phase marginals, plugin time is
89.567 -> 77.500 ms; finite solve 17.271 -> 12.529; strict/final 19.242 -> 15.521;
objective/line search 9.307 -> 7.666; publication 10.097 -> 10.015. These overlap.
Outer/inner work remains 3/4 (K32 remains 4/5). The fresh K48 inference marginal
is still **25.629 ms**, with 10.039 ms publication. Narrow large low still needs
48.3% off its 94 ms command to reach 48.6; K32 needs 6.9% off 116 to reach 108.

Next implementation should share current-weight finite/centered products between
concentrated direction and canonical construction where identities permit, and
address wide PPML sandwich/inverse publication. The K32 direction and K48
inference costs provide concrete targets. Then profile the current multi-FE/slope
checkpoint boundary on frozen identical weight frames and generalize accepted
ownership into the existing coefficient/Schur/joint engines. Do not restart
serial-slope window or residual-extrapolation probes. Public preparation and
small-fit overhead remain separate opportunities.

## Memory, validation and receipts

Immutable terms add **16*N bytes** to the charged iteration envelope. The large
low tracked peak delta rises 29,789,542 -> 30,773,022 bytes; modeled selected peak
44,342,215 -> 45,325,255. K32 tracked is 28,343,810 -> 28,836,650; modeled
36,220,441 -> 36,711,961. Scratch stays zero on the admitted route. K48's modeled
selected peak stays 26,384,465 because another phase dominates. These are allocator
and model observations, not physical-cap qualification.

A one-byte-below-optional-admission fixture passes its independent Poisson oracle:
modeled 1,823,129 -> 1,820,185 bytes, scratch 0 -> 65,536. This preserves the
optional grouped-storage refusal and does not repair historical constrained
weighted/nonretaining-CG failures or the documented static allocation drift.

Final source evidence:

- **495 distinct Rust tests:** PPML, prepared-fit and linear-session filters.
- **14 licensed runs of 11 fixtures**, including independent full-dummy
  Poisson/Hessian/sandwich checks, FW/PW, offset, robust/cluster inference,
  generated terms/interactions, separation, slope/categorical and OLS controls.
- Three fixed-process allocation audit configurations retain zero warmed events/
  growth, stable capacities and fresh/warm agreement.
- **206 headline-block applications**, 180 passing PPML states, 60 native PPML
  pairs, 60 baseline/stock and 60 additional direct candidate/stock checks;
  nine small independent Python oracles; thirteen exact OLS comparisons.
- Six additional diagnostic applications, separated from headlines. No observed
  overlap, timeout or application/scientific failure in completed benchmark blocks.

All runs are terminal. The content-addressed archive contains **8,837 logical
receipts**, 1,924 payloads and 17,012,513 bytes. SHA-256:
`87a2232119929a59fa0b810be1e06063dd838be825afa04303e4d1fdf8f3d0ba`.
Restoration verified every logical receipt. Raw licensed logs, binaries and data
are excluded; hashes, sanitized science/runtime exports, exact source/artifact
identities and reproduction recipes are retained.

```bash
python3 restore_receipts.py /path/to/new/directory
```

No SCC/Windows/Linux confirmation, release, push or full qualification was run.
Historical unresolved comparisons and all platform/memory deferrals remain open.
