# Current-frame means and independent PPML scores: local development

Accept `framemom1`, based on `07d41bef`, with all **536 computational inputs**
matching the final source. Plugin SHA-256:
`12fd878cf359681240c23c43f46cdc3f541ff41c81bd980106510502db5c7509`.
Baseline is accepted `onefeshare2`, plugin
`26cef3ff3c30a727c34437c81025cf2d104c2807c391e4d1f637f51dce687316`.
This is a bounded improvement, not completion of the per-case public-command
target of <=0.90 times comparable stock fixest.

## Change

[ADR 0133](../../../../docs/adr/0133-current-frame-moments-and-ppml-scores.md)
shares already computed current-weight raw means with canonical retained fit
assembly, including low mode. Sample/topology/weight generations, dimensions,
coordinate identity and finite values gate the short borrow. Changed weights
still require fresh calculation. Independent residual/outcome means, rank,
bread, inverse and reconstruction checks remain.

Outer PPML scores still scan original inputs on every required check. Numeric
columns use the existing bounded binding and run independently on the admitted
team, retaining each column's arithmetic/row order and the canonical final
reduction. FE scores remain fresh. Small, generated and wider-column models
keep the generic evaluator. No new N-sized buffer, tolerance change or skipped
certificate is involved. Ordinary one-shot OLS retains its computation.

## Comparable public timings

Three alternating baseline/candidate/stock rotations per main cell. Values are
milliseconds, **median [minimum, maximum]**, including the whole public command,
robust inference, fitted means and publication. Generation/import, process
startup, warmup and exports are outside the timer. Every overlap and monitor
check passes; no compilation, profiling, archiving or competing fit accompanies
headline timing. The small independent-oracle pilots are science checks, not
performance rankings.

Stock is unmodified pinned fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched application threads, serial BLAS
and the established comparable covariance normalization. Every current stock
comparison passes. Historical/default-reference discrepancies remain separate.

Development screen, seed332:

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| One FE N61440 K10 T4 | 117 [116,118] | 103 [103,104] | 52 [49,54] | 1.981 |
| One FE N184320 K6 T8 | 275 [272,278] | 236 [235,237] | 135 [122,138] | 1.748 |
| Unbalanced three-FE slopes N61440 K10 T8 | 314 [313,321] | 300 [298,305] | 232 [231,236] | 1.293 |
| Dense categorical three FE N30720 K6 T4 | 203 [200,208] | 193 [191,200] | 232 [229,238] | 0.832 |
| Low one FE N30720 K6 T4 | 110 [100,131] | 98 [96,101] | 24 [23,26] | 4.083 |

Predeclared fresh seed333:

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| One FE N61440 K10 T1 | 154 [150,155] | 142 [141,147] | 97 [93,97] | 1.464 |
| One FE N76800 K8 T4 | 126 [126,126] | 112 [112,112] | 55 [53,57] | 2.036 |
| Unbalanced one FE N122880 K6 T8 | 186 [185,189] | 162 [162,163] | 96 [90,100] | 1.688 |
| Degree-six two slope partitions N23040 K6 T4 | 106 [104,107] | 99 [98,101] | 86 [86,87] | 1.151 |
| Wider unbalanced three-FE slopes N46080 K10 T8 | 286 [283,289] | 269 [268,271] | 930 [920,932] | 0.289 |
| Sparse categorical N38400 K8 T4 | 147 [146,148] | 133 [129,136] | 84 [83,84] | 1.583 |
| Low unbalanced three-FE slopes N30720 K6 T4 | 196 [195,198] | 194 [188,205] | 117 [117,121] | 1.658 |

Fresh median native gains are **7.8%, 11.1%, 12.9%, 6.6%, 5.9%, 9.5%, 1.0%**.
Every pair in the first six improves. Low slopes retain a **5.1% paired loss**;
the development low one-FE cell has a 1% paired loss. Keep those distributions.
Low native versus ordinary stock is **not physically memory-matched**; its ratio
is a timing aspiration, not evidence about equally constrained execution.

## Attribution and memory

Two isolated baseline-only diagnostic builds (`initdiag1`, `initdiag2`) preserve
source snapshots, patches, identities and trace receipts. All eight diagnostic
applications pass. Their clocks are not headline times. The larger one-FE case
spends about **3.844/3.894 ms** in initial/strict moments-factor-inverse work,
versus **5.768/5.881 ms** scanning raw means. The former includes all its moments
and factor work; it is not an isolated inverse time. Removing initial inverse
construction alone cannot close the public gap.

Fresh independent outer score checks total **21.714 ms** for the larger slope
case (seven calls), **14.331 ms** for one FE N61440 (five), **8.878 ms** for low
slopes (eight), and **29.140 ms** for one FE N184320 (five). This identified a
shared serial scan cost without assuming certificates themselves were optional.

Production receipts on the larger one-FE screen show line-search/profile time
**38.297 -> 24.909 ms**, strict/final **61.562 -> 51.116 ms**, finite solve
**25.034 -> 18.807 ms**, preparation **40.274 -> 35.927 ms**, and publication
**28.954 -> 29.123 ms**. On larger slopes, line search falls **31.142 -> 17.849 ms**,
while FE projection remains **107.045 -> 108.976 ms** and strict/final
**70.857 -> 70.400 ms**. These are phase medians, some nested; do not sum them as
disjoint kernel attribution. No candidate instrumented build is used to assert
exact per-kernel savings. Outer and reported inner counts remain unchanged.

Median tracked and modeled memory peaks are unchanged in every screen/fresh
cell. Larger one FE remains **84,087,312 / 94,575,762 bytes**; larger slopes
**58,339,134 / 65,262,268 bytes**; fresh low slopes **24,016,359 / 29,393,365 bytes**
(tracked/modelled). The means view borrows existing data; score pairs use 1,024
stack bytes and the existing team handle. Real T1/T4 warmed allocation audits
remain zero-event/zero-growth. Physical-cap and cross-platform checks are deferred.

## Science and OLS protection

**352 distinct final-source Rust tests**, actual-plugin ABI/panic checks and
**six licensed fixtures** pass. Fixtures cover one-FE and joint-slope allocation
audits, full-dummy Poisson/Hessian one-FE and multipartition-slope oracles,
the PPML scientific contract and OLS memory modes. They include unit/FW/PW
weights, offsets, robust/cluster inference, saved outputs and T1/T4. Ordinary
fixest weights are not a frequency-weight inference oracle.

The five benchmark blocks contain **163 passing applications**, **117 public
PPML states**, **39 native pairs**, **39 stock pairs**, **nine independent
Poisson/Hessian checks**, and **23 exact OLS scientific-export pairs**. No
application failure, timeout, current scientific discrepancy or detected timing
contention occurs. An initial development compile failure caused by two misplaced
arguments is retained; the corrected final source passed all named checks.

OLS two-pair controls, baseline/candidate medians: slopes **67/69 ms**, low
three categorical FE **66/64**, one-FE full outputs **16.5/17.5**, and generated
factor low mode **66.5/68**. Five-pair follow-up on exactly the same input hashes:
slopes **68 [67,74] / 68 [67,71]**, one FE **17 [16,17] / 18 [17,18]**, generated
factor **65 [65,68] / 65 [64,68]**. A **1-ms one-FE loss remains**. No changed OLS
computation explains it, but these measurements do not prove universal timing
non-regression. Preserve both blocks and stop chasing this small residual here.

## Remaining work

The one-FE N76800 fit needs **62.5 ms (55.8%)** more reduction to reach 49.5 ms.
Larger slopes need **91.2 ms (30.4%)** to reach 208.8 ms. Sparse categorical needs
**57.4 ms (43.2%)** to reach 75.6 ms. Dense categorical and the fresh wider slopes
clear the target at these inputs; neither result describes its whole family.

Prioritize a bounded low-memory joint-direction design using streamed finite
products and scratch access, without a hidden resident N-by-K copy. Compare
the fast/low operation counts first: the accepted joint path is deliberately
fast-only because of its RHS-bank ownership. Next address categorical joint
ordinary directions and one-FE end-to-end construction, loading and publication.
Avoid spending another cycle tuning these accepted mean/score kernels or
eliminating a small inverse phase. Required versus avoidable cost must be measured
before any remaining loss is treated as a limit.

Seed333 is consumed; seed334 remains unused for a future predeclared confirmation.
Frozen 77/16/111 definitions, static allocation drift, constrained-fit/CG failures,
historical comparator discrepancies and SCC/platform deferrals remain unchanged.
No push, release, qualification, SCC retry or stopped campaign restart occurred.

## Immutable receipts

The content-addressed archive contains **7,367 logical receipts**, **1,760 unique
payloads**, 18,892,621 bytes. SHA-256:
`c7964155884721759a83d75f5b11663254e73f5d0b78792d37b63feaf9e97bc2`.
It preserves four source snapshots, identities, build/test receipts, declarations,
controllers, distributions, diagnostic traces, science exports/audits and exact
follow-up input identities. Raw licensed logs, plugins and binary datasets are
excluded; omitted numerical files retain hashes. Machine paths are sanitized
with original/archive digests retained. `restore_receipts.py` verifies the
container and every logical file when restoring into a new directory.
