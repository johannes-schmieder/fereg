# Independent FE score directions — local development, 2026-09-26

Accept **directionalscore2**, the repaired candidate. Independent FE score
accumulation now uses owned scalar destinations and existing group membership
on the admitted thread team. Large public pilot medians improve 1.5–2.2%; the
fresh block has 32 native wins, nine losses and four ties. This is a modest,
broad PPML improvement without added numeric storage. The multi-FE <=0.90-stock
goal remains active; several measured families still miss it.
[ADR 0220](../../../../docs/adr/0220-owned-fe-score-directions.md) records the decision.

## Source and measurement

Baseline **objectiveblocks1** matches all 567 computational inputs of
**d15b907e7aa2031644fec9fb8dba7b50499dba7b**, plugin
`fe596205eef0faf28680d6682a778ad3f871e9530d57fd9b361ebf36cce84c04`.
Candidate is that revision plus the archived ppml/prepared_irls.rs and parallel.rs
overlays, plugin
`aec92cd86fc9152bf06de471eafb1f8aedc3e976e1391d0901310c7bb70b5a73`.
All 567 final production inputs match the measured candidate; no computational
input changed after its build.

Separate **scoreprofile1/scoreprofile3** diagnostic plugins:
`80c9de1a1c722cf642c6f0ec357a45bfdbf8a052db543980bc273041edaffee0` /
`109a412960393d4c2abde3f1873e74969c363e3b53b9a33ace3ce8b632b8590c`.
Builds use distinct Cargo targets and pass actual ABI panic-containment self-tests.
Diagnostic clocks do not enter stock headline comparisons.

Unmodified pinned fixest source archive:
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Comparable stock uses glm.tol=1e-10 and fixef.tol=1e-11, matched threads,
inference and requested outputs, and single-threaded BLAS. Public feppml timings
include fitted means with robust or specified cluster inference. Three pilot
rotations use development seed202609347; five fresh rotations use predeclared
seed202609451. Variant order rotates. Monitor receipts are clean; no builds,
profiles, audits or archiving overlap headline fits. All ordered observations,
including losses and outliers, appear in
[final distributions](final-receipts/distributions.md). Sampled process monitoring
does not rule out all ambient operating-system variation.

## Attribution and implementation

Baseline diagnostic evidence identifies serial FE accumulation as about 26–27
of 41–43 ms of large outer-score work. Scale sums and finite-column checks are
separate. These are full-fit totals over five current-state checks.

At N>=4096 with multiple workers, each FE direction owns its two existing scalar
destinations and traverses its immutable group members. Original member order
preserves the direction's arithmetic; certificate reduction remains canonical.
Small/T1 calls keep the original row scans and execute directly. The resolved
team size is read without submitting a Rayon task. Each call freshly reads
original y, q, current mu and bases, and resets every direction, including empty
levels. No cached score, new numeric bank, pool or generation authority is added.

Paired diagnostic inclusive milliseconds (nested spans are not additive):

| Case | FE scores B/C | Entire score check B/C | Whole plugin B/C |
|---|---:|---:|---:|
| Million fast | 26.358 / 12.445 | 40.364 / 26.560 | 829.239 / 832.097 |
| Million low | 26.838 / 12.061 | 41.093 / 26.505 | 922.168 / 897.878 |
| Two slopes low | 1.357 / 1.098 | 2.780 / 2.518 | 68.920 / 68.500 |
| Three FE, two slopes | 3.100 / 1.851 | 5.576 / 4.482 | 147.063 / 152.817 |
| Factors | 7.555 / 1.752 | 13.767 / 7.958 | 497.914 / 487.940 |
| Four slopes low | 2.685 / 2.092 | 4.575 / 4.085 | 112.529 / 111.553 |

Scale/finite costs remain essentially unchanged. Whole diagnostic fast and
three-FE slope losses remain visible; the local span improvement is not a promise
that every full fit improves. Uninstrumented public rotations govern headline
claims. No scheduling constant or source layout was tuned after measurement.

## Public performance and remaining gaps

Pilot medians, milliseconds; B is committed baseline, C is candidate, S is stock:

| Family | B | C | S | Paired C/S range |
|---|---:|---:|---:|---:|
| N983040/K6/T8, low | 958 | 937 | 931 | 1.005–1.057 |
| N983040/K6/T8, fast | 858 | 845 | 868 | 0.971–0.980 |
| Two slopes, N30720/K6/T4, low | 72 | 70 | 118 | 0.561–0.619 |
| Three FE/two slopes, N61440/K10/T8 | 152 | 149 | 234 | 0.618–0.662 |
| Factors, N153600/K19/T8 | 571 | 565 | 714 | 0.782–0.796 |
| Serial, N245760/K6 | 545 | 547 | 953 | 0.573–0.580 |

There are **11 native paired wins, seven losses, no ties**. Four of six families
meet 0.90 stock in every pair. Large fast and low still need **63.8 and 99.1 ms**
at their respective median targets. The large low case remains slower than stock.

Fresh seed202609451 medians:

| Family | B | C | S | Paired C/S range |
|---|---:|---:|---:|---:|
| N737280/K6/T8, fast | 652 | 643 | 761 | 0.824–0.854 |
| N737280/K6/T8, low | 708 | 705 | 766 | 0.901–0.943 |
| N368640/K6/T4, fast | 454 | 447 | 661 | 0.669–0.687 |
| Factors, N184320/K19/T8 | 686 | 679 | 3157 | 0.215–0.222 |
| Three FE/two slopes, N76800/K10/T8 | 192 | 189 | 292 | 0.620–0.660 |
| Four slopes, N38400/K6/T8, low | 115 | 112 | 117 | 0.925–1.000 |
| Serial, N245760/K6 | 539 | 541 | 941 | 0.572–0.582 |
| Weak, N61440/K6/T4 | 121 | 120 | 837 | 0.142–0.150 |
| Disconnected, N61440/K6/T4, low | 93 | 93 | 98 | 0.929–0.959 |

There are **32 wins, nine losses, four ties**. All 45 direct stock comparisons
pass science; six of nine families meet 0.90 in every pair. Large low,
four-slope low and disconnected low still miss by **15.6, 6.7 and 4.8 ms** at
the median targets. Preserve the 1107-ms baseline outlier. Neither earlier
seed449 mean discrepancies nor the first candidate's seed450 weak-case loss
is declared repaired by this different seed.

Serial PPML loses two ms in each initial block. One predeclared identical-input
follow-up gives B **535,536,542,554,540**, C **540,543,545,541,541**: medians
**540/541**, four paired losses and one win. Protected OLS initial B/C medians
are **65/63, 76/77, 14/15, 208/211, 160/158, 17/18**. The single follow-up of
small serial, three-FE factor and one-FE controls gives **14/14, 209/209, 17/18**.
Their paired wins/losses/ties are **1/2/2, 3/1/1 and 1/2/2**. Full/warm input
hashes match. Original blocks and outliers remain; there is no universal
non-regression claim. The cause of the small excluded-path losses is not
established. Accept these trade-offs for broader threaded savings without extra
numeric storage; no further threshold, layout, seed or graph-specific tuning.

## Allocation failure, repair and scientific validation

The initial candidate **directionalscore1** passed its threaded audits but failed
a late real T1 low-memory slope audit at
`assert e(feppml_audit_events_delta) == 0`, Stata r(9). The failed log does not
print the event count. Unchanged objectiveblocks1 passes the same T1 fixtures.
Candidate1 submitted the accumulation closure before testing the team size;
extra external Rayon jobs can allocate queue blocks. It was never committed.

Directionalscore2 reads the existing team size directly and bypasses submission
on serial calls. Real T1 and T4 categorical/slope 16-iteration audits now pass
with zero allocation events and growth. No allowance, scientific arithmetic or
acceptance schedule was changed. The [superseded first-stage report](stage1.md)
and its original archive remain unchanged; later failure, baseline and repair
receipts are preserved in the supplemental final archive.

**65 distinct Rust tests pass**: 58 prepared-IRLS and seven threading. The
inherited checkpoint-count assertion still fails 0 versus 1, with baseline and
candidate evidence retained. The test controller initially reused a log filename
for the second filter and stopped after prepared-IRLS completed; this harness
failure is preserved. The seven threading tests then ran separately with a unique
log and the same source-bound Cargo target. No failed assertion was removed.

The new exact integer-design oracle covers dominant/disconnected groups, empty
levels, duplicate/zero slopes, slope-only partitions, changed means and weights,
2^-400..2^400 scales, T1/T4/T8 and stable buffers. Existing numeric/generated tests
compare all score fields and FE moments. **Thirteen licensed fixtures** pass with
explicit terminal receipts, covering independent categorical/three-partition/
higher-dimensional oracles, weights/inference, runtime/transaction behavior,
OLS controls and the T1/T4 allocation audits.

**238 public-state audits** and **15 explicit Poisson/Hessian/sandwich comparisons**
pass. The N4800/T4 unbalanced three-FE slope science cell exercises the parallel
path inside the explicit-design oracle. All **85 native PPML and 33 OLS export
pairs are exact**. Anchored comparisons pass **152/153** and direct C/stock
**67/68**; the older small three-FE saved-output discrepancy remains unranked.
The final screens have no fit application failures, timeouts, incomplete pairs
or detected contention. The first candidate's allocation failure is separate,
retained evidence and is not erased by that statement.

Every current-state projection, score, model-space, rank, finite-solve, objective,
reconstruction, separation and final-Hessian requirement remains. Pilot/fresh
observed heap and scratch peaks are identical; modeled bounds vary **-48..+112
bytes**, with unchanged allowances. There is no new numeric bank, pool or cache.
This is local development evidence, not physical-cap or platform qualification.
Static inventory drift, wire-minor/checkpoint-count failures, constrained-fit and
nonretaining-CG failures, SCC deferral and stopped campaigns remain open. Frozen
77/16/111 definitions and pins are unchanged. No push or release.

## Next subsystem

Move to **canonical multi-slope absorption and RHS scheduling**, beginning with
low-memory four-slope fits and applicable multi-partition OLS. Final paired
canonical preparation is **42.929/42.959 ms**, much larger than the remaining
four-slope score cost. The earlier diagnostic wrapper leaves about 25–27 ms in
absorb_in_generation after its instrumented descendants. Its name does not prove
a generic MAP fallback: the source delegates to the admitted coefficient solver.
Weighted geometry and ordinary joint-IRLS reuse already exist.

Instrument SlopeCoefficientWorkspace::solve, Worker::solve, operator sweeps,
acceleration/checking, packing/reconstruction and fresh certificates. Record
actual RHS windows, active workers, per-column stopping and geometry generations.
Whole-column scheduling may leave workers unused in narrow low-memory windows;
that is a source-based hypothesis, not measured attribution. Choose a coherent
scheduling or within-column coefficient-work change only after this split. Keep
independent original-input acceptance and valid current-weight geometry. Do not
revive rejected residual-space extrapolation, norm-handoff or incidence probes,
or remove memory/solver guards without proving their replacement.

## Reproducible receipts

Work root: `target/ppml-score-construction-20260926/final-candidate`.
[Final archive verification](final-receipts/archive-verification.json) binds the
supplemental archive. It contains full computational source snapshots, recipes,
declarations, controller/monitor code, all attempts and diagnostics, scientific
audits, tests and late T1 failure/repair evidence. Generated DTA and binary row
arrays are hashed and omitted; raw licensed logs are excluded. Restore into a
new directory with `final-receipts/restore_receipts.py`.

The original 11,419-receipt archive remains immutable at SHA-256
`55692ae2fb9f7fdfd18bdbf73818f38250f939d3e1b774733c2bf7ad492465d9`.
Its timing claims belong to rejected directionalscore1, not this repaired source.

The final supplemental archive contains **11,347 restored and hash/privacy-verified
logical receipts**, SHA-256
`04a59a1305da165a27abbd9bdc0350a1787e88fb6cd6a518f1f8505389c7ea28`.
