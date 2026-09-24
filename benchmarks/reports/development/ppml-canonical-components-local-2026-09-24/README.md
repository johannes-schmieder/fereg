# Componentwise canonical PPML Schur systems: local development evidence

This milestone adopts **canonicalcomponents2** under
[ADR 0170](../../../../docs/adr/0170-componentwise-canonical-schur-factors.md).
Packed component matrices, factors and solves replace the global dense geometry
in the private low two-categorical-FE canonical product engine. Four fresh
previously refused disconnected designs improve 15–22% against the accepted
baseline; every corresponding native pair improves. Scientific acceptance and
the outer memory allowance are unchanged. The broader goal of <=0.90× comparable
stock fixest on designs with at least two FE remains unfinished.

## Exact identities and comparison

| Artifact | Captured source | Plugin SHA-256 |
|---|---|---|
| Baseline `canonicalownership2` | `341477b58d802f039b0a48d0109615e3f11f03a0` + canonical_products.rs; computation matches accepted `a8257266` | `3fcc35a1759e78d451cda4dc01a9d22d167cc46a79af848ba3a29c9ccea5bc0b` |
| Intermediate `canonicalcomponents1` | `a82572668d51504d7c059d3de43942a644b56bb1` + canonical_products.rs | `9e14f7c808bde7d4f9894858ffb73e0223d251ba9d2879be1709764a65317216` |
| Adopted `canonicalcomponents2` | same base + canonical_products.rs | `55639c6687a26b2ad8f2edd6cc248d26a6baa764c38a13b9a936d41e996896c3` |

All 202 captured native/build-interface inputs match final working sources.
Of all 548 captured inputs, only the nonnative Python timing-isolation test
changed after the candidate build. The corrected benchmark monitor and its
seven-test file are separately archived under recipes; their revision is not
represented by the plugin hash. All native source, builds, ABI pass receipts,
full captured source snapshots and identities are retained. No estimator source
changed after final candidate freeze or in response to fresh seed 202609382.

Stock fixest 0.14.2 is the pinned unmodified library, with `glm.tol=1e-10` and
`fixef.tol=1e-11`. Its source archive SHA-256 is
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
All PPML calls request robust covariance and fitted means. Rows, designs,
requested outputs and thread counts match; BLAS threads are one. All fresh
native cases use low mode; stock uses its supported execution, so this is not
an equal-memory reference experiment. No instrumented reference contributes
headline timings. Prior default-reference accuracy failures remain separate.

Times cover the complete public command, including loading, preparation,
estimation, inference and output. External data generation, process startup,
and post-run audits are outside its timer. Fresh processes execute rotating
baseline/candidate/stock order. Native phase clocks are diagnostic and overlap;
they must not be added to reconstruct public timing.

## Timing defect, adjudication and final selection

The original process monitor recognized `stata-mp` but missed the separate
`StataMP` GUI process. After severe variability in `fresh-packed`, host inspection
found an unrelated GUI using about 790% CPU. Its recorded launch was 14:59:44
local time. File timestamps establish possible lifetime overlap, not continuous
historical CPU use. The process was neither stopped nor modified by this work.

All original packed-candidate `oracle-packed`, `comparison` and `fresh-packed`
timings are **unranked**, including favorable results. For example, fresh case 0
originally had candidate median 290 ms, range 150–518 ms, versus baseline 248 ms
and stock 175 ms. This result is preserved, not silently replaced. The original
`overlaps: []` fields reflect the defective monitor and confer no acceptance.
`timing-adjudication.json` records hashes and possible overlap; the original
monitor is also archived. Ambient backup/indexing processes and approximately
14–37% sampled host CPU use are retained as limitations. No thermal warning was
observed. We do not claim an otherwise perfectly idle operating system.

The monitor now recognizes GUI Stata names, resolves ancestry, and samples CPU
activity before and throughout each fit. At least 10% of one core over the sample
marks possible activity; unavailable activity or ancestry fails closed. An idle
open GUI is allowed. This bounded sampling cannot rule out every short burst.
Seven focused tests cover CLI ancestry races, own/foreign/idle/busy GUI processes
and inaccessible process state. No monitor source changed during corrected fits.

The exact 12 frozen cases and seed were rerun as `recheck`. It stopped at 165 of
180 calls when the GUI became active again: cases 0–9 completed cleanly; the
last candidate call of case 10 detected overlap; case 11 never ran. All 165
completed scientific states pass. Missing comparisons remain explicitly
incomplete in its audits. The complete case 10 is excluded from final timing.

`supplement` then reran exactly cases 10–11, five rotations each. All 30 calls
completed with no detected overlap or monitor error; both full generated DTA
hashes match their frozen `fresh-packed` counterparts. Final adoption evidence
is **recheck cases 0–9 plus supplement cases 0–1**: 180 calls, five rotations
for each of the 12 cases. This selection follows contamination/missingness,
not timing outcomes. No compilation, profiling, archiving or agent-owned
competing fit accompanied these calls; the corrected monitor detected no
foreign fit in the selected runs. Original interrupted and contaminated blocks
remain archived. Their scientific evidence remains usable.

## Fresh performance, including losses

Seeds **202609381** (intermediate validation) and **202609382** (final validation)
are consumed/frozen. Case definitions were declared before generation. The table
uses the corrected selection above: median [minimum–maximum] milliseconds and
paired ratios within rotations. All 180 public-state checks, 60 native pairs,
60 baseline/stock pairs and 60 direct candidate/stock comparisons pass.

| Case | Baseline ms | Adopted ms | Stock ms | C/stock medians | Paired C/B median [range] |
|---|---:|---:|---:|---:|---:|
| 0: disconnected N26400/K18/T4 | 118 [117–122] | 98 [98–100] | 101 [99–104] | 0.970 | 0.831 [0.803–0.847] |
| 1: disconnected N45600/K26/T8 | 233 [231–235] | 181 [178–183] | 202 [198–203] | 0.896 | 0.774 [0.764–0.789] |
| 2: disconnected N20160/K11/T1 | 107 [106–108] | 91 [91–95] | 144 [138–145] | 0.632 | 0.850 [0.843–0.888] |
| 3: disconnected N15600/K9/T4 | 63 [62–70] | 52 [51–54] | 48 [48–50] | 1.083 | 0.825 [0.729–0.871] |
| 4: disconnected N48000/K10/T8, work refused | 144 [143–147] | 148 [145–149] | 87 [87–88] | 1.701 | 1.014 [1.000–1.035] |
| 5: connected N21600/K2/T4, work refused | 53 [51–55] | 52 [50–53] | 31 [27–32] | 1.677 | 0.981 [0.926–1.039] |
| 6: connected N31200/K76/T8 | 319 [316–319] | 316 [316–324] | 459 [452–469] | 0.688 | 1.000 [0.991–1.016] |
| 7: unbalanced N23520/K16/T4 | 82 [81–83] | 84 [82–93] | 71 [68–73] | 1.183 | 1.012 [1.012–1.120] |
| 8: two slope partitions N21600/K10/T4 | 104 [104–106] | 106 [104–107] | 110 [109–111] | 0.964 | 1.019 [0.990–1.019] |
| 9: three FE N23520/K10/T4 | 113 [113–115] | 115 [112–116] | 278 [274–280] | 0.414 | 1.009 [0.991–1.027] |
| 10: weak graph N12000/K8/T1 | 50 [49–53] | 51 [47–52] | 1794 [1785–1801] | 0.028 | 1.020 [0.940–1.020] |
| 11: connected N21600/K100/T8, memory refused | 514 [511–519] | 520 [513–524] | 436 [434–437] | 1.193 | 1.010 [0.994–1.019] |

The four newly admitted disconnected cases all improve in five of five native
pairs. Case 1's marginal stock ratio is 0.896, but its paired stock median is
0.901 [0.877–0.914]: the 10% target there is borderline. Case 2 is comfortably
below 0.90 in every stock pair. Case 0 nearly reaches stock parity; case 3 remains
8.3% slower. Five of these 12 selected medians reach <=0.90, including previously
fast paths; this is not a coverage-wide win rate or proof of universal superiority.

Five cases remain slower than stock: 3, 4, 5, 7 and 11. The narrow work-refused
cases are 1.70× and 1.68× stock. Large work-refused case 4 rises 144→148 ms against
the baseline (four slower native pairs, one tie). The topology pass is a real
additional cost before its refusal. The unbalanced case rises 82→84 ms, with
all five native pairs slower (paired range 1.012–1.120). Wide memory-refused case
11 rises 514→520 ms (four slower pairs). Excluded slope and three-FE controls
also add 2 ms to their medians. These small samples do not establish zero
regression; all ranges and individual results remain visible. The weak-graph
stock loss is scientifically comparable here but is one specific graph, not a
claim about all weakly connected models.

## Computational change and attribution

The prior engine used a global Q² matrix, a global grounded dense factor and
cross-component triangular loops. Its Q³ work bound refused disconnected
systems even when each component was small. The first prototype packed only
factors/solves and discovered components during weighted assembly. It reduced
work but retained global matrix allocation; larger cases still failed memory
admission and narrow refusals paid weighted setup before declining.

The adopted design discovers immutable components before weighted numerical
allocation, then packs both matrices and factors. Assembly keeps the existing
weighted pair arithmetic. Maximum-mass grounding, normalized columns, the
original global numerical pivot guard and direct residual/Gram/FE-moment
certificates remain. All response/control RHSs share the call's factors. No
weighted geometry or topology cache survives a call. Ordinary joint directions,
fast-mode routes, slopes, three-plus FE, OLS and publication retain their engines.

For component sizes Qc, matrix storage is sum Qc² and factor storage is
sum (Qc−1)². Work admission uses the same finite-Gram allowance NW(W+1)/2,
with bounded pair assembly plus sum floor(Qc³/3). Initial topology allocation
has a checked bound; complete phase admission preserves its live charges before
any weighted bank exists. The ledger changes its limit without losing charged
capacity. Full certificates, rank/QR/span/inverse checks, finite solve,
reconstruction, scores, objectives, strict final Hessian, weight/sample/DoF/
separation semantics and atomic publication remain unchanged. This replaces
actual global work and ownership, not just a threshold.

The following is a structural replay on frozen data, before numerical acceptance.
All rows survive. Baseline bytes are ADR 0169's maximum phase bound, not the older
all-live sum; full detailed replays remain in `summary-packed.json`.

| Case | Q components | Work B→C | Gram allowance | Phase bytes B→C | Existing byte allowance | C work/memory admitted |
|---|---|---:|---:|---:|---:|---|
| 0 | 110+110+110+110 | 28,460,666→1,840,664 | 5,016,000 | 3,371,296→1,660,912 | 4,224,000 | True/True |
| 1 | 190+190+190+190 | 146,439,333→9,259,332 | 17,236,800 | 9,712,672→4,739,696 | 7,296,000 | True/True |
| 2 | 84+84+84+84 | 12,694,752→840,672 | 1,572,480 | 2,015,744→819,808 | 3,225,600 | True/True |
| 3 | 65+65+65+65 | 5,897,666→405,164 | 858,000 | 1,244,128→607,840 | 2,246,400 | True/True |
| 4 | 200+200+200+200 | 170,786,666→10,786,664 | 3,168,000 | 10,735,200→3,106,608 | 7,680,000 | False/True |
| 5 | 90 | 297,000→297,000 | 129,600 | 367,952→370,120 | 691,200 | False/True |
| 6 | 130 | 810,333→810,333 | 93,693,600 | 4,674,048→4,674,048 | 4,992,000 | True/True |
| 7 | 98 | 1,090,215→1,090,215 | 3,598,560 | 372,272→374,632 | 3,763,200 | True/True |
| 10 | 24 | 142,608→142,608 | 540,000 | 147,120→147,704 | 1,536,000 | True/True |
| 11 | 90 | 297,000→297,000 | 111,261,600 | 4,540,096→4,540,096 | 3,456,000 | True/False |

Fresh measured phase medians localize gains to projection and strict canonical
work, with smaller finite-product savings:

| Case | Projection ms B→C | Finite ms B→C | Strict final ms B→C | Inner diagnostic count B→C |
|---|---:|---:|---:|---:|
| 0 | 30.63→22.46 | 9.71→8.35 | 31.13→20.38 | 56→42 |
| 1 | 67.97→47.10 | 17.05→14.64 | 70.91→41.83 | 56→42 |
| 2 | 26.54→21.29 | 10.43→9.15 | 29.17→20.12 | 60→45 |
| 3 | 13.13→8.89 | 4.95→3.82 | 16.00→10.01 | 61→45 |

All four have four outer iterations in both versions. Inner counts mix direct
solves and iterative steps; they are not comparable operation units, and no
fixest internal attribution is inferred from whole-command timing.

Tracked whole-action peaks fall by 1,091,664, 13,520, 1,281,288 and 893,640 bytes
in cases 0–3, respectively. Remaining fresh tracked peaks are unchanged.
The small total reduction in case 1 reflects another live phase dominating the
whole action despite much smaller geometry. Modeled budgets and scratch sizes
are identical. Maximum candidate tracked/model ratio is 0.88656. Full allocation
and phase distributions are in `adoption-summary.json`. This is local tracked
memory evidence, not constrained-memory or cross-platform qualification.

## Independent science, rejected stages and controls

Thirty-six distinct focused core tests pass on final source. The new independent
weighted-contrast construction covers 24 unequal components, interleaved labels,
one-retained-level components, both FE role orders, changing weights, and threads
1/4. Known residuals and Gram products are derived separately from the solver.
The fixture fits the new byte allowance while global geometry exceeds it; one
byte below the complete plan declines without mutation. Ledger tests cover limit
changes and refusal atomicity. Existing extreme-scale, weight, rank, strict
rollback, scratch cleanup/poisoning, joint slope/categorical and interruption
checks remain. A plugin one-byte whole-action budget/publication test also passes.

The first prototype's original test run had four passes and one failure: an old
work-refusal fixture was actually disconnected and now admissible. Its exact
source and failing log remain. The same fixture is now an admission assertion;
a separate connected cycle still refuses under the same work rule. No tolerance
or sample was changed. Candidate1 then passed its six canonical and 36 focused
tests. A narrow K1 public screen also failed during data generation before any
fit because the shared DGP needs x2. The original declaration is preserved;
corrected `refusal2` uses K2/K3. No estimator failure was hidden.

A separate N6240/K8/T4 disconnected explicit-design Poisson oracle passes for
baseline, final candidate and stock. It independently fits the objective and
forms the final weighted Hessian and robust sandwich. FE rank is 1140, residual
DoF 5092, and its Newton solve takes five iterations. Candidate maximum errors
are 4.38e-14 for coefficients, 7.93e-18 for covariance, 6.00e-11 for means and
7.28e-12 for log likelihood. Its contaminated single timing is unranked; this
is independent scientific evidence. Candidate1's corresponding oracle also passes.

Five licensed fixtures pass with Stata rc0 and terminal sentinels: canonical
Poisson/Hessian/sandwich, categorical allocation audit, multipartition slope
oracle, generated-factor oracle and three-FE OLS oracle. This milestone does
not repeat every historical weight/cluster/output fixture or the full suite.

The intermediate candidate's seed-347 development screen (45 calls) completed
before GUI launch: disconnected N21120/K14/T4 improves 90→72 ms versus stock 68;
N30720/K18/T8 135→108 versus 96; serial N21120/K14 124→108 versus 180. Connected
controls are 61→60 and 316→310 ms. Its corrected narrow K2/K3 screen has 18
passing calls and approximately 1 ms baseline losses. All 150 intermediate
fresh scientific states also pass; the end of that block may overlap the GUI
lifetime, and none of its timings are transferred to the final candidate.
Final packed `comparison` (48 calls) and original `fresh-packed` (180 calls)
pass science but remain wholly unranked for timing. These stages and their
full distributions are retained rather than summarized only by favorable cases.

Final OLS controls have three baseline/candidate rotations, 24 calls and 12
passing science/output pairs, no detected overlap and identical compared
scientific values. No stock OLS ranking was collected in this milestone.
All use N16320; widths/threads are K8/T4, K12/T8, K6/T4 and K8/T4 respectively.

| OLS control | Baseline ms | Adopted ms | Paired C/B median [range] |
|---|---:|---:|---:|
| 0: one FE, all outputs, robust | 17 [16–19] | 17 [17–18] | 1.059 [0.895–1.062] |
| 1: two FE, iid | 30 [29–30] | 29 [29–30] | 1.000 [0.967–1.000] |
| 2: two slope partitions, low, robust | 104 [104–104] | 104 [104–105] | 1.000 [1.000–1.010] |
| 3: three FE, low, iid | 62 [61–63] | 62 [62–62] | 1.000 [0.984–1.016] |

The changed solver passes its scoped rustfmt check and `git diff --check` passes.
The optional whole-workspace formatting check fails on 65 pre-existing files,
all byte-identical to the baseline; they are left unchanged. `final-checks.json`
records this failure separately from scientific validation.

## Next bounded implementation work

First address narrow low-mode canonical work admission: case 4 still uses
35.4 ms projection and 39.4 ms strict final work; case 5 uses 7.8 and 10.4 ms,
with only 0.3 ms inference. Their Gram-based work cap can reject direct geometry
while repeated iterative projection is more expensive. Compare identical frozen
weight/RHS frames, count actual iterative passes versus component assembly and
factor operations, then choose a coherent work-based route or cheaper solve.
Do not simply raise the cap or tune on the consumed fresh seeds. Reusing immutable
topology within the same prepared sample may remove repeated refusal overhead,
but the target is the larger solve gap, not another cycle on 2–4 ms alone.

Second address the K100 certificate bank: 4,540,096 bytes are needed versus
3,456,000 available. Left-group coefficient storage and worker-private Gram/FE
reductions dominate. A bounded smaller reduction team or streaming coefficient
organization may admit the direct engine without increasing the allowance.
Preserve independent full-weight acceptance and measure any parallelism tradeoff.
The current case spends 113.8 ms in projection and 153.0 ms in strict final work;
this is a material opportunity even though inference also takes 35.6 ms.

The unbalanced case remains 84 versus stock 71 ms: projection 17.7 ms, strict
final 17.4 ms, preparation 8.2 ms, inference 1.2 ms. This remains a subsequent
canonical/joint/preparation target. Protect slope, wide and three-FE wins with
focused controls at each milestone. Do not reopen rejected residual extrapolation
or serial weighted-slope micro-tuning without materially new evidence.

The frozen 77/16/111 definitions, all consumed seeds, prior seed-378 comparator
discrepancy, static allocation-inventory drift, weighted constrained-fit and
nonretaining-CG failures and platform deferrals remain unchanged. No SCC retry,
stopped campaign restart, push, release or qualification is implied.

## Receipt restoration

`receipts-content-addressed.zip` contains sanitized immutable identities, full
captured inputs, monitor revisions, declarations, original failures/interruptions,
per-call numeric exports, checks, phase distributions and reproducible controllers.
Data/binary payloads and licensed logs are omitted with hashes; fixture return
codes and terminal protocol sentinels remain. Machine paths are sanitized.

```sh
python3 restore_receipts.py /tmp/fereg-canonical-components-receipts
```

The sibling SHA authenticates the container. `verification.json` records complete
logical-payload verification and a restored copy. Historical reports remain
unchanged. Use `adoption-summary.json`, not the original packed timing summary,
for this milestone's final performance claims.
