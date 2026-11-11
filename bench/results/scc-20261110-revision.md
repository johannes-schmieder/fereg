# November revision campaign on SCC, November 10–11, 2026

The expanded revision campaign of the paper's November 6 protocol (paired
default/tight comparison blocks, package-default inference, raw numerical
deviations; GLFixedEffectModels.jl for PPML, reghdfejl for OLS) ran once,
complete, with nothing resampled. All 1,297 blocks and 5,188 timed fits are
accounted for; every native fit and every native diagnostic refit passed.

## Identity

| Item | Value |
| --- | --- |
| Source | `80b6768e` (runtime production-identical to the qualified `44cd1563`); archive sha256 `d333e648b4aadf12d90dc52db21c13f65979af224e7feafb6f02c83c6ccb0fb9` |
| Linux plugin | `a21b87b28b4e053f53e9f62360ed8902aa2c0bb3003a882c5d9173964f28a9ec`, built from `80b6768e` (carries `80b6768e7322`) by job 8549582 on scc-tb4 from an offline snapshot (vendored closure, CMG `368eceba` inventory checked; binary audit, actual-plugin ABI/self-test harness and batch guard pass), staged as a root package and admitted with all 24 frontend files identical |
| Policy | `bench/campaign_revision.json`, sha256 `0900bd73138d0484cf0ba03771e7b71601ef6bbbda477e946902edd44ebec9a6` |
| Manifest | sha256 `728beaccd3b71585e8ad31cea731d8c08413e9b088aa357f883818f4b74fd94b`, identical to the November 6 run's: 779 allocations, 1,297 blocks, 5,188 timed and 992 diagnostic fits, 63 source cells including the 10 Veneto cells |
| Tooling | the primary checkout's November 6–7 revision tooling (`da2eafac`), candidate plugin, task wrapper, submission and qualification (`38ec58cb`); collection fixes `4a96ea8f` and `10553433` |
| Comparators | fixest 0.14.2; PyFixest 0.60.0 `0f41fffd`; FixedEffectModels.jl 2.1.0; GLFixedEffectModels.jl 0.5.6 (Julia 1.11.7, FixedEffectModels 1.13.3); reghdfejl `e22427e7` with bridge `846fd585` (Julia 1.12.7); pinned reghdfe, ppmlhdfe, areg |
| SCC root | `/projectnb/welfgr/fereg/runs/20261110-paper-campaign/revision-production-80b6768e` |

reghdfejl runs only in OLS cells, as frozen: its tolerance option does not
reach GLFixedEffectModels.jl, so a tight PPML profile would repeat the default.

## Jobs

| Stage | Job | Host | Result |
| --- | --- | --- | --- |
| Repair validation of tasks 194 and 182 (44cd1563 candidate) | 8549157 | scc-wo4 | failed=0, exit 0; both blocks complete |
| Setup | 8549678 | scc-ei3 | failed=0, exit 0; 474 harness tests, pins, inputs identical |
| Smoke | 8549679 | scc-zj4 | failed=0, exit 0; qualified by job 8549795 (receipt `17106028…`) |
| Production array 1–779 | 8549825 | 47 hosts | 779/779 failed=0, exit 0; 175.6 node-hours; longest 3.46 h; Nov 10 08:37–22:26 EST |
| Collection, report, export | 8568800 | scc-gd4 | failed=0, exit 0 |

**Scheduling deviation.** At Johannes's request `qalter -tc 779 8549825`
(16:51:02 EST on Nov 10) lifted the task concurrency cap from 8; more than 8
tasks first ran at 16:53:00 (28). Each task reserves a whole node and pairs every
comparator with fereg inside its own block, so this is a scheduling-only change:
manifest, policy, per-task resources and no-resampling rules are unchanged. It
is recorded in the exported context.

Collection needed two harness repairs, each found on complete evidence and
retained as a superseded attempt: job 8568705 stopped because the collector
matched production sentinels by substring (task 1 matched tasks 10–19, …), job
8568763 because the export's capture hash could not serialize
GLFixedEffectModels.jl's reported log likelihood of −∞.

## Repair validation

The two native failures of the stopped November 6 production reran as their
frozen blocks with the 44cd1563 candidate (both pass; times descriptive, other
hosts):

| Block | Nov 6 | Now |
| --- | --- | --- |
| 194, two-sparse PPML, 1M, 4 cores, fixest | tight fereg r(430) after 13.05 s | tight 7.67 s (diagnostic 8.01 s, score 8e-15); default 5.67 s |
| 182, three-sparse PPML, 1M, 16 cores, PyFixest | both diagnostic refits r(430) after 3,089/3,124 s | tight 12.32 s, diagnostic 104.4 s; default 10.77 s, diagnostic 87.6 s |

In production the same blocks passed again on scc-wf2.

## Results

**Statuses.** 5,048 ok, 10 command timeouts and 130 comparator capture errors.
Native: 1,297 default and 1,297 tight fits ok; 260 and 260 diagnostic refits ok;
exact memory peaks at 5,178 positions. The timeouts (3,600 s) are tight reghdfe
in three-sparse OLS at 10M rows (repetitions 1–3) and tight GLFixedEffectModels.jl
in Veneto PPML (7 of 15). The capture errors are saved-effect fits whose
predictions fail the independent model score at package defaults (fixest,
GLFixedEffectModels.jl, FixedEffectModels.jl, ppmlhdfe in both profiles, PyFixest,
reghdfe, reghdfejl), and tight GLFixedEffectModels.jl fits whose reported −∞
likelihood cannot replay. Their timings and numerical captures are retained.

**Within-block ratios** (comparator/fereg seconds in the same block, geometric
mean over source cells; tight comparators pair with tight fereg):

| Family | OLS default / tight | PPML default / tight | Veneto OLS default / tight | Veneto PPML default / tight |
| --- | ---: | ---: | ---: | ---: |
| fixest | 4.98 / 7.35 | 3.45 / 4.78 | 9.87 / 15.8 | 7.08 / 22.0 |
| PyFixest | 3.82 / 4.37 | 3.03 / 2.79 | 6.10 / 6.09 | 5.04 / 7.50 |
| FixedEffectModels.jl | 3.56 / 5.34 | – | 11.3 / 24.3 | – |
| GLFixedEffectModels.jl | – | 7.68 / 9.93 | – | 70.3 / 191 (3 of 5 cells) |
| reghdfejl | 13.4 / 17.2 | – | 35.2 / 52.7 | – |
| reghdfe | 31.0 / 33.0 | – | 122 / 139 | – |
| ppmlhdfe | – | 30.0 / 23.6 | – | 51.3 / 54.4 |
| areg | 9.39 / 9.44 | – | 15.2 / 12.0 | – |

Absolute medians across blocks include host variation; the paired summary keeps
counts, timeouts and sample comparisons per cell.

**Native time against the paper's beta.15 evidence.** Over the 63 source cells
that match beta.15 cell ids (synthetic and Veneto), the median default fereg time
is 0.572 of beta.15's (geometric mean; range 0.24–1.05; different hosts and
allocations, descriptive). For example, AKM OLS at 1M rows and 4 cores took
0.560 s against 0.819 s, AKM PPML 2.52 s against 5.72 s, three-sparse OLS at 10M
12.1 s against 51.6 s, and Veneto OLS and PPML at 4 cores 2.46 s against 5.33 s
and 18.6 s against 27.6 s.

**Numerical deviations** (against fereg in the same block; maximum over every
repetition): coefficient differences at most 4.7e-4 reference SEs for areg in
both profiles, 2.3e-4 for default FixedEffectModels.jl, 6.1e-5 for default
fixest and 9.5e-7/8.3e-7 for default/tight GLFixedEffectModels.jl; every other arm
and profile stays below 1e-7. SE differences up to 14% (fixest, PyFixest) and 12%
(areg) reflect package-default inference conventions, not stopping. The only different samples
are the gravity design's: fereg retains 960,000 rows, fixest and PyFixest 970,000
(no covariate-separation deletion) and GLFixedEffectModels.jl all 1,000,000 (no
separation detection; 1,200 against 1,133 rows in the smoke); objectives are not
compared across them.

## Exports

Retained at the SCC root's `-final/export` directory (row data never left SCC):

| File | sha256 |
| --- | --- |
| `scc-20261110-revision-summary.csv` | `c97469bf46b09405c8d8bfca12193e0959596e899df27b54992df072dbb35cb7` |
| `scc-20261110-revision-observations.csv` | `98bb66d5460699edeebea246bb6d389d30cb95f1e2c3f9a91bfe00e1d8111e29` |
| `scc-20261110-revision-inputs.json` | `240f4a084d9edc99a94d255ddb66fc86c35c69f4d727c148c544ccb6ea9f6f6f` |
| `scc-20261110-revision-paired-summary.csv` | `7da6ae65967552e7ce1e9ad6efed55818f854f0d0f9710869376dcc6e11d1477` |
| `scc-20261110-revision-deviations.json` | `2b67a9752c02ce0ea7991bb060488e3e35235c141dbd1994ac9816a082b0d4a5` |
| `scc-20261110-revision-outreach-deviations.json` | `11ca1cf1df79c0d657eb83059a41ca85801f70d0bc1126ac169d57693b7dfe92` |
| `scc-20261110-revision-coefficient-deviations.csv` | `e35583755cac3113c0c9f326f74439f4d41575f4ce1bc1fa01871d2d4c9e7135` |
| `scc-20261110-revision-context.json` | `69561d5b66e8685e37a186ffd140efeadbdde0fd9e6648b8e09e7a131515da1b` |
| `scc-20261110-revision-qualification.json` | `1710602850d50edf0f008ceb89b04a9f2ab29ba68556a5d54af50583445e4d28` |
| `scc-20261110-revision-finalize.json` | `1507a0c493b55a3b089fa0eca6639976ebbd6ec9cf4837e8599da71619050839` |
| report `fereg-scc-benchmark.pdf` | `3dbcaefffd3e28284de94dcb7841e97e5428f6e8a5774c288257566edf57ef89` |

The outreach-shaped deviations follow the paper's `outreach-20261106` layout
(`comparisons`, `unavailable`, coefficient CSV); predictions of unsaved timing
profiles come from the separate diagnostic refit of repetition 1.

## Limits

Descriptive Linux comparison on one fixed seed. Default and tight settings use
each package's own stopping criteria, so equal settings do not imply equal
accuracy. Ratios are paired within blocks; absolute medians and the beta.15
comparison cross hosts and allocations. Veneto rows remain private on SCC; only
aggregates are retained. The paper still reads the beta.15 bundle; integrating
these exports is Johannes's decision.
