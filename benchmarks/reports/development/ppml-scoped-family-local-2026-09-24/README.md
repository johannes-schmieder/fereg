# Scoped PPML execution and bounded family rows — 2026-09-24

Adopt [ADR 0179](../../../../docs/adr/0179-scoped-ppml-family-execution.md).
Fresh threaded whole-public PPML medians improve **3.2–6.7%**; serial is flat.
This is a shared execution milestone, not completion of the multi-FE goal of
at most 0.90 times comparable stock fixest, or qualification evidence.

## Source and measurement boundary

Baseline `lazygraph2`, plugin SHA-256
`91e59cd5fee3413d9e12eaf47494c1ec374e43a50352db8a89ed57f8d9a108bc`,
matches all **552 captured inputs** at `3bcf225db255853171d275007793e92a398403cf`.
Its older captured base/overlay identity is preserved. Accepted `familyrows2`
captures that full commit plus five PPML core files and three plugin files;
all **554 captured inputs match final computational source**. Plugin SHA-256:
`4a326b8d3b210084620478dfd857ffdca672db5aacfc7acbd82fbc3194b98af8`.
Rejected `familyrows1` captures the same base plus only the five core files,
SHA-256 `fafd41206c3574dc65229f6d7f4919e7b00719d76fe4c38856047d8834e4d2a6`.
Both isolated builds pass the actual-plugin ABI/panic harness.

Stock is unmodified fixest 0.14.2, source archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Every comparison uses glm.tol=1e-10, fixef.tol=1e-11, matched threads,
robust inference and requested fitted means. All timed native models here have
numeric unweighted controls. Weighted/clustered science is covered separately
by licensed oracles; no weighted timing or frequency-weight reference claim follows.

Data generation/loading into the host application is outside the command clock
for both programs. Native command loading, fitting, output construction and
publication are inside its clock. Public phase telemetry is not a separately
instrumented headline build, and differently named stock/native phases are not
equated. Baseline/candidate/stock order rotates by cell and repetition.
No compilation, profiling, audit, archive or competing fit ran alongside these
headlines. All **333 timed applications** and their isolation monitors passed.
A sampled process monitor does not exclude all ambient OS variation.

Development seed202609347 was already a tuning seed. Fresh seeds
**202609404–202609408 are now consumed and frozen**. Pre-fit declaration changes
and generator failures are retained below; no fitted outcomes selected those
corrections. The frozen 77 primary, 16 holdout and 111 native-control definitions
are unchanged.

## Computational change and attribution

The family path had serial exponentials/objective rows and a separate score
residual pass. The immutable owner already cached log(y), but the deviance's
non-near branch recomputed it. The candidate uses that cached value, combines
working responses/Hessian weights/scores, and schedules disjoint mean and family
rows on the existing team. An admitted contribution tile permits parallel row
work followed by exactly the original ordered compensated objective reductions.
Final independent objective replay and every scientific acceptance boundary stay.

The first candidate made repeated external `ThreadPool::install` submissions.
Science passed, but the real T4 allocation audit recorded **one event with zero
net growth**. Pinned Rayon/Crossbeam source shows external submissions entering
a block queue with periodic successor allocation. The accepted plugin keeps
native work in one scoped task. A stack exchange carries interrupt requests and
replies to Stata's calling thread; no SPI callback crosses to workers. Both peers
close on panic, and scope completion precedes publication. This removes repeated
external submissions, not the interrupt checks. T1 remains direct.

Fresh measured phase medians explain most of the benefit:

| Case | Weight/response B→C ms | Objective B→C ms | Whole B→C ms |
|---|---:|---:|---:|
| Narrow fast |1.10→0.52|3.72→2.47|45→42|
| Disconnected |2.16→0.67|7.75→4.61|118→112|
| Low large slopes |7.34→3.73|15.51→10.06|342→330|
| Two slope partitions |4.31→2.75|8.44→5.94|126→121|

Fresh inner and outer iteration counts match in every native pair. This supports
an execution-cost explanation, not changed convergence or fewer accepted checks.
Other phase fluctuations are retained. Serial family phases lose time even though
the whole-command median is flat; no claim of universal kernel improvement follows.

## Timing distributions

Values are median [minimum, maximum] milliseconds. Fast memory unless specified.
Slope cases put one slope on each of two partitions. Paired losses count C>B;
ties are separate. Ratios apply only to the stated current-source cases.

### Accepted development, three rotations

| Case | B ms | C ms | Stock ms | C/stock | Paired losses |
|---|---:|---:|---:|---:|---:|
| Two FE N21120/K2/T4 | 40 [40, 41] | 38 [37, 38] | 31 [29, 31] | 1.226 | 0/3 |
| Same, low | 40 [39, 40] | 39 [39, 39] | 29 [29, 30] | 1.345 | 0/3 |
| Disconnected N42240/K10/T8 | 94 [94, 99] | 88 [87, 90] | 77 [77, 77] | 1.143 | 0/3 |
| Three FE unbalanced slopes N61440/K10/T8 | 235 [234, 245] | 224 [221, 231] | 232 [228, 232] | 0.966 | 0/3 |
| Same, low | 253 [251, 255] | 241 [240, 243] | 231 [230, 231] | 1.043 | 0/3 |
| Two FE N30720/K10/T1 | 92 [90, 92] | 93 [93, 96] | 199 [195, 199] | 0.467 | 3/3 |
| Three FE unbalanced N30720/K6/T4 | 91 [90, 94] | 88 [87, 89] | 82 [82, 83] | 1.073 | 0/3 |
| One FE control N21120/K6/T4 | 36 [36, 36] | 34 [33, 35] | 22 [20, 22] | 1.545 | 0/3 |

### Fresh, five rotations, seed202609404

| Case | B ms | C ms | Stock ms | C/stock | Paired losses |
|---|---:|---:|---:|---:|---:|
| Two FE N24480/K2/T4 | 45 [44, 46] | 42 [42, 44] | 32 [31, 34] | 1.312 | 0/5 |
| Same, low | 45 [44, 45] | 43 [42, 43] | 32 [32, 33] | 1.344 | 0/5 |
| Disconnected N49200/K12/T8 | 118 [116, 121] | 112 [110, 113] | 90 [84, 91] | 1.244 | 0/5 |
| Unbalanced N29520/K6/T4 | 55 [55, 56] | 53 [52, 54] | 44 [43, 45] | 1.205 | 0/5 |
| Weak barbell N20400/K8/T4 | 45 [44, 45] | 43 [42, 43] | 1077 [1076, 1081] | 0.040 | 0/5 |
| Three FE unbalanced slopes N69120/K12/T8, low | 342 [339, 346] | 330 [326, 332] | 290 [284, 294] | 1.138 | 0/5 |
| Two intercept+slope partitions N34560/K6/T4 | 126 [125, 127] | 121 [120, 122] | 124 [120, 124] | 0.976 | 0/5 |
| Two FE N36960/K10/T1 | 106 [103, 109] | 106 [105, 110] | 204 [196, 207] | 0.520 | 3/5 |
| Three FE unbalanced N35840/K8/T4 | 108 [106, 108] | 104 [103, 106] | 124 [124, 126] | 0.839 | 0/5 |
| Two FE N41040/K32/T4 | 125 [124, 127] | 121 [118, 123] | 285 [282, 288] | 0.425 | 0/5 |

Of 45 threaded fresh pairs, 44 improve and one ties; the serial median is unchanged, with three
paired losses and a candidate maximum of 110 versus baseline 109 ms. Nine fresh
medians improve and one ties. This targeted sample is not a global win-rate claim.
Four of these ten cases meet 0.90× stock; six do not. The low-slope stock comparison
is slower than the previous milestone's different case; earlier ratios do not
transfer to these new data and dimensions.

### Initial candidate, rejected on allocation, three rotations

| Case | B ms | C ms | Stock ms | C/stock | Paired losses |
|---|---:|---:|---:|---:|---:|
| Two FE N21120/K2/T4 | 40 [39, 40] | 39 [39, 41] | 30 [26, 30] | 1.300 | 1/3 |
| Same, low | 40 [39, 41] | 39 [39, 39] | 30 [29, 30] | 1.300 | 0/3 |
| Disconnected N42240/K10/T8 | 94 [94, 94] | 90 [90, 92] | 78 [77, 80] | 1.154 | 0/3 |
| Three FE unbalanced slopes N61440/K10/T8 | 237 [232, 238] | 228 [224, 230] | 234 [230, 234] | 0.974 | 0/3 |
| Same, low | 251 [251, 251] | 241 [241, 243] | 232 [230, 232] | 1.039 | 0/3 |
| Two FE N30720/K10/T1 | 92 [87, 92] | 93 [91, 96] | 198 [198, 199] | 0.470 | 2/3 |
| Three FE unbalanced N30720/K6/T4 | 91 [90, 93] | 88 [88, 88] | 81 [74, 81] | 1.086 | 0/3 |
| One FE control N21120/K6/T4 | 37 [36, 37] | 34 [34, 35] | 22 [22, 23] | 1.545 | 0/3 |

These timings do not authorize its adoption. The successor addresses execution
ownership; the same numerical family implementation remains in the accepted path.

### Small, independent and OLS controls

Small-fit seed202609407 has three rotations: two FE N1920/K2/T4 is
13 [13,13]→13 [12,13] ms against stock 17 [17,18]; one FE N1500/K2/T4 is
12 [11,12]→11 [11,13] against 11 [11,12]. The one-FE control has one paired loss.
The parallel N4560 disconnected explicit oracle loses 16→18 ms (single calls),
with stock 20. The valid N2400 slope oracle is 19/19/19 ms. These single oracle
calls establish science, not a timing distribution. The warned N1440 case is
21→22 versus stock24; its timing remains unranked.

OLS seed202609406, three native rotations, has six exact scientific export pairs:
three FE sparse N32768/K10/T4 is 62 [61,66]→62 [61,66] ms; two FE strong
N32880/K10/T4 is 43 [43,43]→41 [41,41]. Individual paired losses are retained in
receipts. OLS receives no scoped-executor or family-row code path.

## Science, failures and memory

All **321 exported PPML states** pass independent current score, model-space and
objective replay checks. All **107 native pairs** and **107 direct stock pairs**
pass numerical comparisons. Of these, the accepted-candidate screens comprise
249 states and 83 pairs; the other 72 states/24 pairs belong to the rejected
candidate. Nine small states pass explicit-design Poisson, Hessian, sandwich and
mean oracles: six are valid-graph checks, three are retained warned-data diagnostics.
Older sparse comparator discrepancies were not rerun or repaired by this work.

**215 distinct focused Rust tests pass**: family18, memory53, IRLS50,
prepared estimator58, scoped executor3, plugin PPML33. The unchanged inherited
`refined_checkpoint_preserves_accuracy_on_a_small_dynamic_range_model` assertion
still fails zero versus one strict rejection. The executor tests verify non-Send
caller callbacks, exact team width, both peer panics, cancellation, retry and
256 warmed cycles with zero allocations. Initial nonexistent `checked_add`
compilation failure is retained; no numerical assertion was relaxed.

The final candidate has **eleven successful licensed fixture runs**: the six
science/OLS fixtures, saved slopes, T1/T4 actual 16-iteration allocation windows,
real Ctrl-C with prior/null results, and the current-wire transaction copy.
The unmodified transaction fixture fails for baseline and candidate because it
asserts minor wire2 instead of current wire3. The diagnostic copy changes only
that assertion and its terminal sentinel; all remaining checks pass for both.
The original fixture and its failures remain unchanged.

Fresh setup initially rejected paper row counts not divisible by240. A later
small setup warned on paper offsets below eight firms and rejected the one-FE
panel's final singleton. Both failures occurred before any timed fit. New
immutable declarations use valid whole graphs/panels and retain the originals.
The first slope oracle N1440 did fit after a modulo-zero generator warning.
Its actual exported design passes independent mathematics, but its timing is
unranked. Separately predeclared seed202609408/N2400 supplies a valid slope
oracle. No sample rule or frozen generator was altered to make these runs pass.

The contribution bank costs **16*min(N,4096*T) bytes** for N>=4096/T>1,
otherwise zero: 256 KiB at T4 and 512 KiB at T8 in these main cases. It is
charged on both routes beside all ten existing observation-vector equivalents.
The scoped task adds **1,392 measured heap bytes** on this artifact; the actual
live embedding snapshot charges it before core preflight. No N-by-K bank is added.

Fresh tracked peaks increase by 263,536 bytes at T4 and 525,680 at T8;
serial is unchanged. Narrow tracked peak is 11,337,008→11,600,544 bytes;
low large slopes 67,884,654→68,410,334. When another phase dominates, the modeled
peak rises only by the 1,392-byte embedding difference; otherwise it also includes
the tile. Every per-case modeled/tracked distribution is retained. This is a
speed/memory tradeoff, not RSS or constrained physical-memory qualification.
Static allocation-inventory drift remains a visible unresolved failure.

## Remaining opportunity

Stop adjusting these family scheduling constants. The accepted mechanism is
shared, deterministic in its reductions and allocation-audited, but cannot close
the remaining gap by itself. Required further savings to reach 0.90× stock on the
fresh cases are:

| Remaining case | Further ms | Fraction of current time |
|---|---:|---:|
| Narrow fast |13.2|31.4%|
| Narrow low |14.2|33.0%|
| Disconnected |31.0|27.7%|
| Unbalanced |13.4|25.3%|
| Low large slopes |69.0|20.9%|
| Two slope partitions |9.4|7.8%|

For low large slopes the candidate spends 33.91 ms in preparation, 135.17 in FE
projection, 24.03 in finite solve and 74.71 in strict finalization. Disconnected
spends 14.37/24.11/8.92/21.39 ms respectively. Narrow spends
5.44/6.41/3.73/3.73, plus 4.14 in publication. These phases do not include every
loading/control-plane cost and are not a complete additive call decomposition.

The next bounded design should share current-frame original-input statistics
and reconstruction across ordinary and canonical work, with explicit sample,
basis and weight-generation ownership. Now that the public numerical action
stays in one team, compatible independent row work can share its scope without
new external queues. Profile the remaining low-slope strict/canonical work and
narrow whole-command loading before selecting the next change. Preserve fresh
acceptance; do not start another tiny serial-family or solver-threshold loop.

Weighted constrained/nonretaining-CG failures, older comparator/reporting issues,
Linux/thread/physical-memory confirmation and SCC access remain open. No push,
release, SCC retry, stopped campaign or full qualification occurred.

## Receipts

The content-addressed archive preserves captured source, exact artifact identities,
declarations, failed setups, controllers, scientific exports, all timing/phase
and memory distributions, tests and derived audits. Raw data, binaries, machine
paths and raw licensed logs are excluded; input and omitted-log hashes and
protocol/status records remain. `verification.json` records archive and restore
verification as well as source matching. The restore utility checks every payload.

Verified archive: **10,963 logical receipts / 2,592 payloads / 20,180,351 bytes**,
SHA-256 `7febaa64a73ae75229cd77e4ddeaa10c2c1312bb8bc6617be3ef70501468e15c`.
All payloads and all restored logical files were independently rehashed.
