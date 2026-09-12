# P3 cluster grouping: source audit and bounded experiments

Baseline is local main `2243b4db979469db29226e73fdd5744b5b9be112`.
These are local development investigations, not qualification or comparator
claims. The installed plugin and separate Veneto evidence are untouched.

## Actual ownership and consumers

| Branch | Current source-bound behavior | Remaining opportunity |
|---|---|---|
| Simple OLS cluster dimension | `ols/prepared_clusters.rs` stores an index into the authoritative final categorical source. `cluster_codes()` borrows it without copying N codes. | No second code owner to eliminate. |
| Interacted cluster term | `factorize_interaction()` sorts final rows, assigns collision-free codes, then releases that order. | Its construction order overlaps later inference grouping. Retention would add a fit-lifetime N-element allocation; assess construction and peak cost before adopting. |
| OLS covariance/certificate | `OlsQualifiedClusterSubsetProvider` calls one shared reducer per nonempty mask. Dense production and certificate share the same ordering and completed groups. | Caching every subset order has no second production consumer within this call. The additional legacy reducer is `cfg(test)` only. |
| PPML covariance/certificate | `ppml/vce.rs` borrows final code slices. The shared driver visits each mask once; `PairedIndependent` delegates to the parent's paired reducer, sharing grouping between both meats. | Neither prototype here changes that PPML route. |
| Dense OLS ordering | Counting radix on complete categorical codes, one pass per selected dimension; sparse huge codes use comparison sorting. | It does not perform four byte passes per code. The histogram can traverse codes sequentially instead of indirectly through the current permutation. |
| Scratch OLS ordering | One N-element tuple order; compensated group scores and per-group bread certificate. `PreparedOlsScratchScoreStorage::fill_row()` reads a forward tile on every cache miss. | Cluster-sorted rows can repeatedly reload tiles. A bounded group table could preserve within-group row order while reading scores sequentially. |

Consequently, a general persistent subset-order cache is not adopted for the
already shared simple-dimension consumers. This does not dispose of interaction
construction, all generated terms, or other P3 work.

## Sequential histogram: not adopted as a standalone change

The [isolated patch](histogram/implementation.patch) changes only OLS ordered-dense
histogram construction. A full row permutation has the same code frequencies as
the original row axis. Stable scatter, tie order, group boundaries, arithmetic,
reservations and comparison fallback are unchanged.

The [microbenchmark](histogram/order_micro.csv) extracts the actual baseline and
candidate ordering functions and compares each against an independent
lexicographic sort with original-row tie-breaking. All 168 results agree exactly.
At N983040, ordering improves about 16–28% for 97 groups and 11–17% for 65536
groups, across one-, two- and three-dimension masks. The absolute savings are
roughly 0.2–1.7 ms per order. Sparse-code comparison fallback stays essentially
unchanged. These are ordering-only measurements.

[Nine complete-command pairs](histogram/summary.csv), rotating three seeds and
arm order on local M2 Ultra/Stata MP 19, do not establish a sufficiently stable
standalone command benefit:

| Target | N / K / native threads | Command medians, seconds | Median paired change |
|---|---|---:|---:|
| PW, two-way cluster | 983040 / 50 / 16 | 0.927 → 0.928 | −1.3% |
| AW, three-way cluster | 983040 / 10 / 4 | 0.674 → 0.680 | −0.9% |
| Two-way targeted repeats | same | 0.940 → 0.952 | −1.8% |

Ratios of independent medians and medians of paired ratios need not agree.
The initial two-way native phase falls about 10 ms in all three pairs. Repeat
native changes are −2.6, +5.1 and +44.7 ms; the two favorable repeat command
differences also include 15.0 and 31.5 ms of savings in unchanged SPI input work.
The largest repeat command slowdown is 77 ms. No observation is excluded and
no external cause is established. The three-way target misses the predeclared
5% or 10 ms criterion. Although some two-way differences cross its absolute
threshold, attribution and repeatability are insufficient for adoption.

All full b/V, fit/certificate exports, samples, status, DoF and cluster tables
agree exactly; existing numerical checks and solver decisions pass. Native peaks
are exact in all nine pairs: 957335702 bytes for two-way and 279396634 bytes for
three-way, below unchanged models. Full phase/RSS values, synthetic input hashes,
artifacts and all numeric exports are in the [archive](histogram/archive.json).
Seven focused clustered tests, universal static audits and native arm64 ABI
checks pass. Two pre-launch setup failures (missing executable environment, then
its abandoned scratch directory) are preserved by hash in
[validation](histogram/validation.json). No protected campaign was run after the
decision not to adopt this standalone patch. Production source is unchanged.

## Sequential scratch consumption: adopted locally

[ADR 0044](../../../../docs/adr/0044-sequential-low-memory-cluster-scores.md)
adopts the separate [low-memory patch](low-memory/implementation-reviewed.patch).
It excludes the histogram prototype. Eligible scratch subsets use the existing
compensated dense group accumulator, sharing its implementation with fast mode.
The group values, corrections and presence flags must fit within the displaced
`N*sizeof(usize)` order array. Larger/unrepresentable products retain sorting.
The independent per-group bread and covariance arithmetic remains unchanged;
this does not select the dense parallel certificate route.

[Thirteen pairs](low-memory/summary.csv) comprise six primary comparisons across
three seeds, six controls and one separately traced pilot:

| Target | N / K / native threads | Command medians, seconds | Median paired change | Native peak bytes |
|---|---|---:|---:|---:|
| Low FW, one-way cluster, sparse missing | 122880 / 10 / 4 | 0.307 → 0.259 | −16.4% | 33202406 → 33202406 |
| Low PW, two-way cluster, sparse missing | 245760 / 16 / 16 | 1.870 → 0.964 | −48.2% | 80093748 → 80093748 |

Native phase medians fall 0.285→0.238 and 1.825→0.917 seconds. Input and outer
Stata medians remain close. The pilot's score construction/certification stays
about 0.063→0.061 seconds while its native command falls 0.291→0.231. Neither
score residualization nor the number of subset passes is eliminated.

The [read-request replay](low-memory/read-request-replay.json) applies the
existing forward-tile cache algorithm to synthetic rows whose retained-ID hash
matches the actual Stata sample. Starting from an empty covariance cache, it
calculates 539→10 read requests (522.8→9.8 MB) for FW, and 8282→90 requests
(8.54 GB→94.1 MB) for PW multiway. These are source-derived software requests,
not instrumented calls, physical disk reads or measured hardware traffic. The
candidate still reconstructs each row once per subset; it avoids repeatedly
loading tiles in group order. Independent timing evidence is kept separate from
this explanatory replay.

All thirteen pairs have exact full b/V, sample/status/DoF, fit/certificate,
cluster/FE/subset exports and non-time solver diagnostics. Saved values also
agree exactly. Primary scratch peaks stay 19.6 and 62.7 MB. Primary modeled
bounds hold; sampled RSS stays about 0.091 GiB for FW and rises 0.169→0.192 GiB
for PW. No general process-RSS saving is claimed.

Every single-pair control improves: fast cluster −1 ms, high-cardinality fallback
−1 ms, small −1 ms, no FE −9 ms, saved outputs −10 ms and weak graph −12 ms.
These are protected diagnostics, not repeated speedup estimates. The weak
control has an important **pre-existing model discrepancy**: native peaks are
29933828→29827588 bytes against a reported model of 13460424 bytes. The candidate
does not worsen the peak, but neither arm qualifies as a successful model-bound
check. P0 memory-model reconciliation remains open. Other observed action peaks
fit their reported models; all scratch peaks remain unchanged.

[Validation](low-memory/validation.json) records nine distinct focused Rust tests,
including bit-exact legacy scratch arithmetic and allocation-failure injection
on the new route. A direct boundary test admits a 198-byte table under a
200-byte budget, rejects it under a 192-byte budget, and retains lexicographic
sorting when the Cartesian radix product overflows. The independent Stata
explicit-dummy b/V oracle passes unit/AW/FW/PW and full/deficient rank in both
artifacts at N98304/K50, with exact sample/order and terminal r(0) sentinels.
Universal static and native arm64 ABI audits pass. The reviewed rebuild is
byte-identical to the measured prototype, with [all native inputs bound](low-memory/identities.json)
to the applied source. No public model, rank, memory admission, solver, PPML,
worker/SPI or transaction policy changes.

Remaining P3 work includes feeding multiple subset consumers from each score
tile, interaction construction/order reuse, unequal-group scheduling and further
score reconstruction costs. The same-sample singleton code borrowing and shared
production/certificate orders were already present; they are not new gains.
