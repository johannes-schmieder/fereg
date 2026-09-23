# Immutable PPML inference scans: local development

Accept `infscan1`, based on `3295603f`, with all **537 computational inputs**
matching the final source. Plugin SHA-256:
`b4af5b56185bcd6e13d544e58e1ed90b75db3e71f3ca3c87e98d7a8bdcd5803c`.
Baseline is accepted `lowjoint4`, plugin
`853bd995ff3783f7223e5e29c18b343aa5bd4d05da42ca9bc0452629809837f2`.
This is a bounded improvement toward whole public PPML **<=0.90 times comparable
stock fixest per case**, not completion of that objective.

## Change and measured attribution

[ADR 0135](../../../../docs/adr/0135-immutable-ppml-inference-scans.md)
extends the existing immutable retained-scratch reader to robust PPML inference.
Each fixed row block keeps one reader, replacing a positional read per row.
The reducer's mathematics, fixed blocks, separate production/certificate scalars
and storage, row order, compensated merges and acceptance remain unchanged.
Clustered PPML and one-shot OLS keep their existing access paths. No changed
rank, sample, weight, tolerance, DoF, separation or final-Hessian policy.

An isolated release probe compares the actual reducer with a wrapper that
retains the old pointwise reads, the production cursor adapter, and dense input.
All 180 timed attempts assert identical production and certificate matrices.
Four shapes, T1/T4/T8 and five rotations are declared before execution. Data/file
construction is outside the clock; reducer allocation is inside for all arms.
These are diagnostic kernels, not whole-command or stock timings. There was no
whole-machine contention monitor; no assistant-launched competing build or fit.

Selected medians in ms:

| N/K/T | Pointwise | Cursor | Dense | Positional reads, pointwise -> cursor |
| --- | ---: | ---: | ---: | ---: |
| 2400/6/1 | 1.965 | 0.107 | 0.088 | 2400 -> 15 |
| 30720/6/1 | 15.923 | 0.724 | 0.533 | 30720 -> 180 |
| 30720/6/4 | 15.206 | 0.457 | 0.327 | 30720 -> 180 |
| 30720/24/4 | 23.899 | 4.101 | 3.296 | 30720 -> 720 |
| 61440/10/1 | 29.820 | 2.170 | 1.597 | 61440 -> 600 |
| 61440/10/4 | 33.064 | 0.834 | 0.647 | 61440 -> 600 |
| 61440/10/8 | 32.314 | 1.378 | 1.002 | 61440 -> 600 |

The probe source snapshot and binary identity are separate from production.
Full distributions and every assertion are in `probe-summary.json`, `probe.log`,
`probe-run.json` and `artifact-sources/probe/` in the restored archive.

## Whole public command

Three alternating baseline/candidate/pinned-stock rotations per main case.
Times below are **median [minimum, maximum] ms**. The timer includes the public
command, robust inference, fitted means and publication; generation, import,
startup, warmup and exports are outside. No compilation, profiling, archiving or
competing fits accompanied headline runs; every contention/monitor check passes.
All current application and scientific checks pass. Small pilots establish
science and are not ranked as timing evidence.

Stock is unmodified pinned fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched application threads, serial BLAS
and the established robust N/(N-1) normalization. Low versus ordinary stock is
**not physically memory-matched**. Historical/default-reference failures remain
separate evidence, not silently reclassified wins.

Development seed334:

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Low one FE N30720 K6 T4 | 102 [102,102] | 87 [85,87] | 26 [26,26] | 3.346 |
| Low two slope partitions N30720 K6 T4 | 182 [182,193] | 169 [166,170] | 120 [120,122] | 1.408 |
| Low unbalanced three-FE slopes N61440 K10 T8 | 403 [376,411] | 354 [351,361] | 230 [227,231] | 1.539 |
| Low three categorical FE N30720 K6 T4 | 581 [580,581] | 556 [553,568] | 243 [236,243] | 2.288 |
| Fast one-FE control N61440 K10 T4 | 102 [102,102] | 105 [104,105] | 55 [48,55] | 1.909 |

The four low median gains are 14.7%, 7.1%, 12.2% and 4.3%; every native pair
improves. The excluded fast one-FE loss is retained, including its 2.0–2.9%
paired losses. Its predeclared fixed-input follow-up appears below.

Fresh predeclared seed335, all low mode:

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| One FE N61440 K10 T1 | 256 [255,258] | 225 [215,229] | 95 [92,96] | 2.368 |
| Three categorical FE N46080 K8 T8 | 3086 [3044,3192] | 3112 [3054,3150] | 10973 [10952,11005] | 0.284 |
| Wider unbalanced three-FE slopes N46080 K16 T8 | 471 [461,495] | 448 [444,450] | 1359 [1354,1367] | 0.330 |
| Multiple slopes N23040 K6 T4 | 165 [154,167] | 145 [144,146] | 130 [127,131] | 1.115 |
| Weak barbell, two slope partitions N7680 K4 T4 | 41 [41,42] | 37 [36,38] | 209 [209,209] | 0.177 |
| Small one FE N3840 K3 T1 | 23 [21,26] | 20 [19,20] | 11 [11,12] | 1.818 |

Five fresh cells improve by **4.9–13.0%**, every native pair in those cells
improves. The categorical cell retains a **0.8% median loss**, with paired ratios
1.0207, 1.0033 and 0.9749. Three fresh cells clear the stock target throughout
the observed ranges, but were already stock wins in baseline. No family-wide
claim follows from those wins. Small-case timer precision limits interpretation.

## Phases and memory

On larger development slopes, public inference falls **51.860 -> 1.574 ms**;
projection is **141.118 -> 140.037 ms**, finite solve **21.800 -> 21.819 ms**,
strict/final **93.423 -> 94.230 ms**. The independently measured read reduction
explains the inference improvement; it does not imply changes to the solve.
On development categorical FE, inference falls **22.631 -> 0.595 ms**, while
projection remains **333.133 ms** and strict/final **164.888 ms** in candidate.
On the slower fresh categorical cell, inference falls **38.689 -> 0.803 ms**,
but projection rises **2076.183 -> 2114.743 ms** and strict/final
**816.419 -> 828.231 ms**. Keep this whole-command loss despite the isolated
kernel gain. Phase clocks are partly nested and must not be added as disjoint
removable costs.

All measured changed low-mode cells have identical baseline/candidate tracked
and modeled peaks. Larger development slopes use **57,446,907 tracked /
69,232,515 modeled bytes**. The reader adds no heap bank, thread, file or N-by-K
matrix. It keeps one 8 KiB encoded stack page per active block, replacing the
same temporary constructed per positional read. Longer stack lifetime uses
existing runtime headroom; physical caps and complete stack accounting are
not established here.

One excluded fast categorical attempt increases tracked peak by **463,312 bytes**;
the selected model is unchanged. All five exact-input follow-up pairs have the
same **58,321,915-byte** tracked peak in both artifacts. Preserve the initial
observation; there was no intrusive attribution of that transient difference.

## Science and regression controls

**390 distinct final-source Rust tests** pass: PPML, shared sandwich, retained
sessions and scratch boundaries. New tests exercise narrow/page-wide rows,
negative zero, fresh writes, invalid ranges/compaction, callback interruption,
different block widths, weights and T1/T4. Existing tests retain corruption,
truncation, allocation failure, rollback, rank and weighted inference coverage.
The actual plugin ABI/panic harness passes.

**Eleven licensed runs of eight distinct fixtures** pass process return code,
Stata return code and terminal sentinel. They include independent multi-partition
Poisson/Hessian and one-FE oracles; unit/FW/PW robust/cluster PPML; saved outputs;
fixed-process pool1/2 and T1/T4 allocation audits; OLS memory modes; and an
independent Stata regress oracle for unit/AW/FW/PW categorical OLS with IID,
robust and clustered inference and T1/T7. Ordinary fixest weights are not a
Stata FW inference oracle.

All **152 public applications** pass, with **136 PPML public states**, **50 native
pairs**, **36 stock pairs**, **nine independent Poisson/Hessian pilot checks**,
**50 exact native PPML export pairs** and **eight exact OLS export pairs**.
No application failure, timeout, scientific discrepancy or detected timing
contention occurs in these blocks. `verified-production.json` contains counts
by block; public-state and comparator checks remain separately inspectable.

Two-pair excluded PPML controls: fast two-partition slopes **138 [137,139] /
138 [137,139] ms**, fast three categorical FE **190 [189,191] /
196.5 [189,204] ms**. The latter includes a 6.8% paired loss. Five predeclared
identical-input follow-up pairs produce:

- Fast one FE: **105 [101,106] / 103 [101,105] ms**.
- Fast three categorical FE: **195 [189,199] / 191 [190,198] ms**.

Each follow-up retains one approximately 1% paired loss; both original losses
remain evidence. These follow-ups have no stock arm and must not be combined
with an earlier stock median to construct a new ratio. Every imported numeric
column hash matches its original case.

OLS controls, baseline/candidate medians: slopes **68/67**, low categorical
three FE **63.5/63**, one FE with robust/full outputs **17.5/16.5**, and low
factor interactions **65.5/64.5 ms**. These small screens protect the changed
shared reducer boundary; they do not establish universal timing non-regression.

The previous mixed-thread zero-growth allocation audit limitation remains open
and was not rerun or repaired here. Fixed-process zero-event/zero-growth audits
pass. Static inventory drift, constrained-fit/nonretaining-CG failures, historical
comparator disagreements and SCC/platform deferrals remain unchanged.

## Remaining budgets and next design

Candidate still needs **61 ms** on two-partition low slopes, **147 ms** on larger
low slopes, **337.3 ms** on development low categorical FE, **139.5 ms** on fresh
low one FE and **28 ms** on multiple slopes to reach 0.90 times matched stock.
The fast one-FE screen needs **55.5 ms**. These deficits cannot be closed by
more inference cursor tuning: candidate inference is already below 3 ms on
these shapes.

Next compare joint categorical directions with direct finite-block elimination
against the existing augmented LSMR and MAP routes on frozen current-weight
frames. Follow with whole-command one-FE ownership, including bounded analytic
profiling for low mode, and remaining slope setup/warm-start/finalization costs.
Already accepted one-FE profiling, joint slope solving, same-frame final products
and current-weight geometry must be the starting point, not proposed anew.
Keep one or two coherent candidates per milestone and protect existing wins.
The current implementation order and prospective declarations belong in STATUS.

Seed335 is consumed; seed336 remains unused/reserved. Frozen 77/16/111 inputs
are unchanged. No push, release, qualification, SCC retry or stopped-campaign
restart occurred.

## Immutable receipts

The archive has **6,228 logical receipts**, **1,639 unique payloads**, and
**14,216,470 bytes**. SHA-256:
`d0915422b0c529d867a04ba6f32e695ded9d96f940ba40e7edea2cbfa45a7f1a`.
It preserves baseline/candidate/probe source snapshots, artifact identities,
declarations, recipes, all public distributions, audits and test results.
Raw licensed logs, binaries and generated binary public inputs are excluded;
omitted inputs retain hashes. Machine paths are sanitized, retaining original
and archived hashes. `restore_receipts.py` verifies the container and every
logical receipt into a fresh directory; this restore passed.
