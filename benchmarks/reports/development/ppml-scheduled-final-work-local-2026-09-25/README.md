# Scheduled Poisson preparation and independent final replay

Local development evidence, 2026-09-25; not qualification. Adopt **familyframe1**
under [ADR 0212](../../../../docs/adr/0212-scheduled-poisson-preparation-and-final-replay.md).
The multi-FE goal of at most 0.90 times comparable stock remains unfinished.

## Exact identities and scope

Baseline **implicitframe1**, accepted at **f2a3ab0f**:
`0f00d18b41c5a7588d9e1bd9448a8490120bd30737ae63ce85300c3a46692766`.
Candidate **familyframe1**, base f2a3ab0f plus three Rust overlays in
`ppml/family.rs`, `ppml/prepared_separation.rs` and `ppml/result.rs`:
`a88eb33da856f021563105040e90a08ede50797e901cadce9b452629cab02328`.
All 566 computational inputs match the measured candidate. Full identities,
build/ABI checks, source snapshots, controllers and declarations are archived.
Diagnostic **ownerprofile1/ownerprofile2** are separately built, instrumented
baseline/candidate snapshots; none supplies stock headline timings.

The stock fixest archive remains unmodified and pinned at SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Use `glm.tol=1e-10`, `fixef.tol=1e-11`, matched native thread counts, BLAS one,
robust or declared clustered inference, and fitted means. Default-reference
failures from earlier reports are not recast as comparable results here.
Public command clocks exclude harness import/setup; native phase clocks separate
loading, preparation, estimation and publication. No compilation, audit,
profiling, archiving or other task-owned fits ran beside headline clocks.

## Adopted work and measured attribution

Prepare immutable log-outcome/log-Gamma terms on the existing team, in the same
bank. Schedule fresh final objective contributions in a bounded three-double row
tile and merge all three compensated chains in original row order. Run independent
sample/outcome/report fingerprint streams concurrently, with unchanged bytes,
comparison order and generation binding. Small/serial or insufficient-workspace
calls retain scalar replay. No certificate or acceptance check is removed,
relaxed, or supplied by an IRLS cache. Projection, rank, finite solve, score,
model-space, separation, reconstruction and exact final q*mu Hessian requirements
are unchanged.

The initial diagnostic rejects output copies as the dominant opportunity: direct
final sample/output copies cost under 1 ms on the million-row cases. Baseline
fresh final replay costs 40–41 ms; separate fingerprint streams total about 62 ms
before overlap. Core preparation is about 60 ms and initial model generation 50
ms. These are distinct from final owner copies.

Two complete uncontended diagnostic pairs on N983040/K6/T8 fast show:

|Boundary|Baseline / candidate ms|
|---|---:|
|Final composite result validation|88.27 / 46.28|
|Fresh fitted/null objective replay, within that boundary|41.00 / 17.64|
|Initial-state work outside `initialize`, includes immutable terms|33.35 / 21.43|
|Initialization itself|57.32 / 57.87|
|Core preparation|61.71 / 65.04|
|Initial model generation|50.04 / 51.60|
|Both canonical retained solves combined|225.53 / 227.41|
|Plugin loading / publication, candidate|41.91 / 24.93|

Overlapping fingerprint stream durations must not be added to wall time. The
composite boundary includes their independent validation and the final replay;
its saving is not additive to the replay saving. Only this complete clean fast
family supports paired attribution. Low and slope diagnostic attempts remain in
the archive; one low call is contended and the slope block is incomplete.
Initialization does **not** include immutable-term preparation in these traces.

## Public performance

B=implicitframe1, C=familyframe1, S=comparable stock. Medians in ms; complete
ordered series and every paired ratio are in [distributions.md](distributions.md).
Development paired outcomes are 22 wins, four losses and one tie; four of eight
multi-FE families clear 0.90 in every pair. Nine development families have three
rotating B/C/S repetitions. The one-FE row
is a protected control outside the multi-FE objective.

|Development case; N/K/T|B / C / S|
|---|---:|
|Two-FE numeric fast;983040/6/8|1003 / 938 / 875|
|Same low memory|1181 / 1078 / 895|
|Two-FE numeric fast;491520/6/8|514 / 479 / 502|
|Unbalanced three-FE factors, clustered;153600/19/8|595 / 590 / 700|
|Unbalanced three-FE, two slopes;61440/10/8|160 / 157 / 232|
|Two-FE, two slopes, low;30720/6/4|75 / 74 / 120|
|Two-FE numeric serial;245760/6/1|559 / 555 / 940|
|Small unbalanced three-FE;30720/6/4|76 / 74 / 82|
|One-FE protected control;61440/6/4|66 / 62 / unranked|

Largest numeric gains are **6.5–8.7%**. The one-FE stock mean comparison fails in
five coordinates (maximum absolute error 7.37e-7, tolerance ratio 1.667); its 41-ms
clock is retained but cannot support a speed ranking. Coefficients, VCE and
objectives pass that comparison.

Fresh **seed 202609442**, declared before the pilot and unchanged thereafter,
uses five rotations. The source and scheduling constants were not tuned on it.

|Fresh case; N/K/T|B / C / S|Interpretation|
|---|---:|---|
|Two-FE numeric fast;737280/6/8|765 / 713 / 769|6.8% gain; still short of 0.90|
|Matched low memory|Unranked clocks|Science passes; repeated timing contention|
|Two-FE numeric fast;368640/6/4|510 / 487 / 653|All pairs below 0.90|
|Unbalanced three-FE factors, clustered;184320/19/8|753 / 737 / 3280|All pairs below 0.90|
|Unbalanced three-FE, two slopes;76800/10/8|202 / 200 / 299|All pairs below 0.90|
|Unbalanced three-FE, four slopes, low;38400/6/8|122 / 123 / 111|Loss retained; still slower than stock|
|Two-FE numeric serial;245760/6/1|574 / 573 / 957|All pairs below 0.90|
|Weak_d3;61440/6/4, complete unchanged retry|170 / 166 / 770|All pairs below 0.90|

Fresh uncontended B/C pairs include 26 wins, seven losses and two ties. Five of
the seven fresh families with valid clocks meet 0.90 in every pair.
The large factor result is specific to this seed and scientifically checked
stock artifact; it is not a universal fourfold advantage. No universal timing
non-regression or cross-platform claim follows.

## Scientific checks, interruptions and memory

**141 distinct focused Rust tests pass:** family 23, separation 29, result 31 and
prepared-estimator 58. They cover serial/parallel bit identity, tile/budget/error
boundaries, changed/extreme weights and means, seal byte identity and mutation
rejection, analytic likelihood/replication and existing acceptance contracts.
Release build, ABI and panic-containment self-test pass.

**Six original licensed fixtures pass:** runtime validator, hidden transaction,
72-fit categorical final oracle, joint column-product oracle, public scientific
contract and weight/inference fixture. The original hidden-allocation fixture
fails on both baseline and candidate because it asserts wire minor 2 while the
current protocol is 1.3. A separately retained diagnostic copy changes exactly
that assertion to 3 and the terminal sentinel; both artifacts pass all remaining
allocation/rollback checks. Do not count those as original-fixture passes or
reinterpret the captured failure. This issue was already recorded in ADR 0179.

All **262 completed PPML public states** pass independent original-input
score/model-space/objective checks: 245 production and 17 diagnostic applications.
The four small cells provide **12 independent explicit-design Poisson/Hessian/
sandwich arm checks**. Small oracles do not alone cover large parallel final
replay: scalar/parallel Rust tests cover that branch, and large public independent
SciPy objective checks plus **164 fsum/SciPy null-statistic states** supply
additional evidence. Maximum null-likelihood absolute difference is 1.63e-9.

All **86 completed B/C comparisons** pass and have byte-identical coefficient,
covariance and mean exports (full identified outputs too where requested).
There are 81 production and 5 diagnostic pairs. Of 167 completed anchored pairs,
163 pass; 21 further planned pairs are incomplete. The four discrepancies are
one small stock full-output residual comparison (four coordinates; maximum
absolute error 8.13e-9, tolerance ratio 2.667) and three one-FE stock mean comparisons.
Direct production C/stock means comparisons pass 78/81; 10 further planned pairs
are incomplete. No discrepancy is ranked as a speed win or silently removed.

All **17 OLS scientific export pairs are exact**. Initial B/C medians are
67/69, 80/82, 10/9 and 222/244 ms. The last loss prompted one predeclared five-pair
unchanged repeat: 210/210 ms, with two candidate wins and three losses. Both blocks
have clean monitors and are preserved; the original loss is not replaced with a
non-regression claim. These are narrow local controls, not OLS qualification.

The original fresh screen has two overlaps, including its incomplete last family.
Select its complete uncontended families 0,2–6 and the full clean weak retry. Three
subsequent low retries also record an overlap each; the last two are incomplete.
No fresh low-memory clock is ranked, despite passing completed science. One
diagnostic attempt stops before measurements; its retry has one contended low
call and an incomplete slope block. Six recorded overlaps are retained overall,
with process identities where the monitor captured them. Some between-call
preflight stops retain only the controller error, not a process identity. No
unrelated workload was stopped. Quiet-interval preflights do not prove future
isolation; per-fit records govern every timing decision. All failures, aborted
work and omitted attempts remain visible.

The final replay tile is at most 768 KiB at T8 and fits within the existing
publication-validator allowance after earlier scratch has dropped. Preparation
uses the same immutable-term bank; concurrent digests add no observation bank.
Tracked peak and scratch deltas are **zero** in the selected production series;
modeled bounds vary by **-48..+48 bytes**, with no allowance increase. No RSS,
physical-cap, new warmed-allocation or platform qualification follows.

## Remaining work

The million-row fast/low development medians still need about **151/273 ms** to
reach 0.90 stock. Fresh fast737280 needs about **21 ms**. Current fast initialization
58 ms, core preparation 65 ms and initial model generation 52 ms are concrete next
budgets. Investigate the global log-sum start and the per-partition conditional
seed, then share valid numeric/sample/topology preparation across generation-zero
construction. Preserve fallback for changing samples, weights and separation;
independent final authority remains mandatory. Do not expect an owner-copy rewrite
to recover those times.

Low finite rank and scratch writing remain additional costs: clean single-pair
traces show roughly 67/26 ms across canonical calls. This is descriptive evidence,
not a clean completed low-profile distribution. Four-slope low PPML remains a
separate loss requiring its own attribution. Do not retune this milestone's row
threshold or restart rejected slope extrapolation probes.

Frozen 77-primary, 16-holdout, 111-native definitions are unchanged. Static allocation
inventory drift, constrained weighted-fit failures, nonretaining CG instability,
Linux/constrained-memory confirmation and qualification remain open. No push,
release, SCC authentication retry or stopped campaign restart.

## Receipts

[The immutable receipt archive](receipts-content-addressed.zip) uses content
addressing to preserve complete logical files without duplicate source payloads.
[restore_receipts.py](restore_receipts.py) verifies the archive and every logical
hash while restoring to a new directory. Generated data/output binaries and raw
licensed logs are hash-bound but omitted; source-bound generators, import hashes,
protocol lines, scientific summaries, attempt outcomes and distributions remain.
Machine-specific paths and license data are excluded. See
[archive-verification.json](archive-verification.json) for restoration/privacy
checks and [receipts-content-addressed.sha256](receipts-content-addressed.sha256)
for the container identity.
