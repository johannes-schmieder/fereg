# P3/P7 bounded cluster-boundary searches

Adopt [ADR 0047](../../../../docs/adr/0047-bounded-cluster-boundaries.md) against
main `fdf46244aa4d286c1d4ac30565236458ed1ef648`. OLS ordered clustered inference
and PPML's paired-independent reducer now share a bounded search for the end
of each sorted cluster. The [patch](implementation.patch) changes those two
call sites and adds the common helper/tests. Every provisional/final boundary,
empty chunk, within-group row order and forward/reverse merge is unchanged.
There is no new workspace, worker pool, chunk count or numerical policy.

## Mechanism and scope

The former loop advanced each provisional boundary one row at a time until its
cluster ended. Several boundaries inside one giant group rescanned the same tail.
The new helper immediately returns an already aligned boundary; otherwise it
uses an upper-bound search over the sorted suffix with the original group's key.
Equality is a prefix predicate because the existing cluster order is lexical.
The final boundary is exactly the one returned by the old adjacent-row scan.

The complete primary recipe has 983,040 physical rows: 884,736 in one group and
1,024 in each of 96 other groups. The existing 16,384-row policy gives 60 chunks;
53 of 59 provisional boundaries lie inside the giant group. The old loop makes
23,445,563 adjacent-group comparisons; the new search uses at most 1,172 group
comparisons under the tested bound. These [logical counts](logical-boundary-work.json)
are derived from the exact recipe and source policies, not instrumented command
counters or measured physical traffic. Sorting, score arithmetic and the giant
group's serial reduction still occur. This addresses repeated scheduling setup;
it does not claim to balance the giant group's numerical work across workers.

## Complete-command evidence

The [prospective protocol](protocol.md) requires a 10% paired total-command gain
or repeatable 20 ms savings with setup/inference attribution. Twenty completed
pairs / forty measured commands comprise six primary pairs, nine controls, two
separate traced pilots and three targeted rechecks. Primary seeds and arm order
rotate; each arm uses a fresh Stata/MP 19 process with a small untimed warm-up.
Runs were sequential on the local M2 Ultra, without competing build/test work.
Native thread requests differ from Stata's eight-processor license limit; macOS
affinity is uncontrolled. This is bounded local development evidence.

| Skewed one-FE target | N / K / native threads | Command medians, seconds | Median paired gain |
|---|---|---:|---:|
| OLS PW, one-way cluster | 983040 / 10 / 4 | 0.430 → 0.401 | 6.5% |
| PPML FW, offset, one-way cluster | 983040 / 10 / 4 | 2.914 → 2.893 | 1.1% |

OLS saves 37, 27 and 28 ms across the three pairs and meets the 20 ms criterion.
PPML saves 33, 7 and 47 ms; its smaller and more variable total gain does not
independently establish repeatable 20 ms savings. Adoption rests on OLS. The
PPML inference medians fall 0.06969→0.03532 seconds, while the much larger solve
phases remain. Its pilot similarly halves inference but improves total command
time by only 5 ms; kernel speed is not the claimed command gain.

The OLS pilot keeps preparation 0.07901→0.07902, fit 0.11217→0.11200, and score
design/certificate 0.01745→0.01794 seconds. Native total falls 0.31324→0.28075.
Subtracting those measured phases leaves 0.10461→0.07179 seconds of remaining
native work, including clustered inference and publication. This is a phase
remainder, not a new direct covariance timer. The source change and comparison
counts locate the removed setup there. Primary native medians fall
0.31599→0.28093, with similar input/outer work. Independent medians and nested
phase scopes need not sum.

## Controls, regression investigation and memory

All full scientific exports, sample hashes, saved outputs where requested, and
non-time solver decisions are exact. Controls cover balanced clusters, small/
narrow OLS, weak graphs, multiway clustering, AW/FW/PW, missing rows, 1/4/16 native
threads, low memory, saved FE/residuals/means and PPML separation. OLS controls
are unchanged or faster. PPML balanced is +6 ms/1.6%, weak +11 ms/4.3%, and low/
saved improves 11 ms. Those remain within max(10 ms,5%). Single pairs are controls,
not repeatability claims.

The initial PPML two-way control increases 0.345→0.388 seconds (+43 ms/12.5%),
crossing the review threshold. Its inference improves 0.009389→0.008254, while
unchanged finite work rises 0.034769→0.059918 and strict/final work
0.090994→0.106859. A [prospective focused recheck](recheck-protocol.md) retains that
observation and runs exactly three fresh seeds. They improve 19, 53 and 17 ms;
the loss does not repeat. The precise source of timing variation is not
established, and the original slowdown is neither deleted nor averaged into a
headline win. Full paired receipts and phase values remain in the archive.

Primary native peaks are identical at 255,802,342 and 1,513,173,626 bytes, below
unchanged models of 416,809,928 and 1,726,458,982. Sampled RSS medians are
0.4599→0.4696 and 0.7474→0.7689 GiB; no general memory saving is claimed. OLS
scratch is unchanged, including 4,915,200 bytes in the low/saved control. PPML
control peaks remain within their models. Weak OLS retains the pre-existing
30,454,234→30,291,042-byte observed peak against a 13,394,888-byte model; it is
not a model-bound pass. The search does not change allocations or repair P0.

## Validation, identities and replay

[Validation](validation.json) records 25 distinct focused Rust tests. They compare
every boundary against the old scan across empty/small/larger balanced and skewed
orders and all seven three-way subsets. A million-row group test bounds the
search to 22 comparisons and an aligned edge to one. Existing exact clustered
meat, independent inclusion-exclusion, strided/packed equivalence, rank-zero,
allocation failure/undersize and scientific-mutation tests pass.

Both artifacts pass the independent [Stata/Mata oracle](../../../../tests/stata/development/cluster_boundaries_oracle.do)
with r(0) and its terminal sentinel. Both use 65,536 original rows, so OLS retains
four chunks after its two missing rows. Review caught that the initial OLS oracle
selected only one chunk; the [initial fixture](initial-oracle.do) and receipts are
retained, and both artifacts were rerun after strengthening it.
The oracle compares skewed-cluster OLS against full
FE dummies for unit/AW/FW/PW and full/deficient rank, including numeric/string
interactions. Independent full-dummy Poisson and Hessian/sandwich calculations
check PPML FW multiplicity, PW squared scores, offsets and saved means. The
PPML sandwich uses the existing reported correction scalar and is not a separate
DoF oracle. All twenty command pairs preserve exact b/V, samples, fit/status/DoF,
certificates, cluster/subset tables and solver decisions; PPML also retains
outer/inner counts, line searches, strict retries, separation and fallbacks.

[Identities](identities.json) bind all 150 baseline/candidate native inputs,
fixture, patch and plugins. Baseline inputs match the preceding accepted report;
its older embedded build label is not used as a substitute for source identity.
Adopted source files exactly match the measured candidate. Universal static
audits and native arm64 ABI/self-tests pass. Installed dist and unrelated Veneto
changes remain preserved. No SCC, x86 runtime, Windows, qualification or push ran.

[OLS](ols/comparisons.json) and [PPML](ppml/comparisons.json) summaries and numeric
archives retain every pair, including the initial slowdown, source/input hashes,
phase/runtime/memory values, full scientific tables and per-run workers. Raw
licensed logs, binaries and local paths are excluded. To replay, create isolated
baseline/candidate snapshots, apply the patch, and build from each snapshot's
own directory. Set `FEREG_LOCAL_BENCH_ROOT`, `STATA_MP_PATH` and `PYTHONPATH` to
the repository's `benchmarks/local_fixest_gap`. Run [paired.py](replay/paired.py)
with `--part pilot` or `--part primary`, then [controls.py](replay/controls.py) and
[recheck.py](replay/recheck.py). Run the oracle through `scripts/run_stata_tests.py`
with the actual plugin checksum. [collect.py](replay/collect.py) takes root/output
arguments. P0 memory/counter reconciliation, remaining score reconstruction and
the broader #27 work remain open.
