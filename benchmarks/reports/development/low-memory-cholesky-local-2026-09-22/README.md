# Low-memory OLS: guarded full-rank finite solve

Accept the existing guarded Cholesky trial for scratch-backed finite fitting.
Fifteen eligible low-memory rows improve by paired medians of 8.3–38.8%.
The wide public command improves 1661→1046 ms, with fresh K40 confirmation at
2126→1304 ms. Stock fixest still wins every scientifically comparable low-memory
row in this screen. See [ADR 0111](../../../../docs/adr/0111-guarded-scratch-finite-solve.md).

## Source, work and timing boundaries

Baseline `gramrow2` is the computational checkpoint committed at `f8c2a0d6`,
plugin SHA-256
`812326ed980df16ff9b60df3e21359971fad4437a0fe25fb02c9645d8f501b6f`.
Candidate `lowchol2` is an isolated build of that commit with the archived
`prepared_fit.rs` and `memory.rs` overlays. All 529 captured inputs match the
accepted working source. Candidate plugin SHA-256:
`45e2c724d80ee38e45d935b5c0d658c45f3d9c144b8f888041fd11a4d0b3b1da`.
The second overlay only corrects a stale comment about legacy base estimates.
The archive also binds controllers, diagnostics, fixtures and test receipts.

Stock fixest 0.14.2 is uninstrumented. Pinned archive SHA-256:
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`;
loaded library SHA-256:
`bece258f6c6fc3fc47accea45c7b004b0c495d46170d78fa2d6302e66de5fbf9`.
Native `memory(low)` and stock's ordinary memory strategy are not an equal-budget
comparison. Import and numerical export are outside timing. Native loading,
estimation and publication are inside the fit timer; requested postestimation
is recorded separately and added to command time. Regressors, thread requests,
sample, inference and outputs match except for the explicit DoF discrepancy
below. Weighted controls compare native artifacts only; independent Stata
frequency-aware regression and literal expansion validate FW inference.

No build, profiling, oracle computation, archiving or competing fit runs beside
a headline block. All 135 headline attempts pass with no timeout, detected
overlap or monitor error. Frozen 77 primary, 16 holdout and 111 native-control
definitions, reserved seeds and reference pins are unchanged. No stopped campaign,
SCC retry, qualification, push or release was performed.

## Profile and mechanism

A separate exact-baseline diagnostic reproduces all scientific exports in
three cases. For N=122880, K=31, T8, finite fitting costs 924 ms: 288 ms for
compensated moments, 596 ms for scratch Householder updates, 19 ms for initial
norms, and about 16 ms for standardization. This isolates a substantial solver
cost after the preceding scratch-certificate improvements.

Low mode already has the complete standardized compensated Gram and RHS before
QR. Reuse the fast path's full-rank Cholesky trial on those moments. Every pivot
must clear the unchanged stronger guard, `initial_pivot * sqrt(relative_rank_threshold)`.
Weak, deficient or otherwise rejected numerical trials leave scratch and the
response untouched and fall back to the existing QR, which continues to own
omission decisions. Allocation/internal errors remain fatal. No rank, omission,
weight, tolerance, sample or DoF policy changes.

The shared helper accepts an optional dense standardized design. Dense callers
retain their existing residual construction. Scratch callers allocate no dense
within design or extra residual: their caller reconstructs from original
columns, independently projects the final residual and certifies fresh scores
and reported bread. Robust/clustered inference still reads and certifies its
own score data. This is same-call computation, with no retained topology,
weight frame, response or PPML iteration cache added.

The candidate diagnostic is also numerically identical to its uninstrumented
artifact. Its wide finite phase costs 299 ms: 282 ms in unchanged moment loops,
16 ms in standardization and 0.14 ms in the guarded trial. Two-FE K10 finite
fitting changes 47.8→20.5 ms; 4×3-factor finite fitting changes 153.6→61.9 ms.
Both diagnostic builds and all twelve diagnostic calls are excluded from the
headline medians. The added `--profile-low-qr` harness flag instruments an
isolated source copy; production code has no new timing probe.

## Alternating pairs and fresh cases

Three rotated baseline/candidate/stock repetitions per unweighted row and
three alternating native pairs per weighted row. Screen seed 202609241,
fresh seed 202609242, pilot seed 202609243 and controls seed 202609244 were
predeclared. Diagnostic seed 202609240 is separate. K denotes numeric width;
4×3 and 5×3 factor crosses add 11 and 14 finite columns. Rows use low-memory
robust inference unless marked otherwise. Ratios are candidate/baseline
within each repetition, not ratios of independent medians.

| Case, N / K / threads | Baseline / candidate / stock medians, ms | Paired min / median / max |
|---|---:|---:|
| One FE, 61440 / 10 / 4 | 130 / 103 / 25 | 0.769 / 0.792 / 0.817 |
| One FE, 4×3 factors, 61440 / 8 / 4 | 389 / 303 / 74 | 0.775 / 0.795 / 0.797 |
| One FE wide, 122880 / 31 / 8 | 1661 / 1046 / 124 | 0.623 / 0.632 / 0.643 |
| Two FE degree-six, 61440 / 10 / 4 | 200 / 172 / 33 | 0.860 / 0.864 / 0.870 |
| Three FE dense, 61440 / 10 / 4 | 232 / 210 / 91 | 0.881 / 0.905 / 0.922 |
| Fresh one FE serial, 98304 / 14 / 1 | 371 / 267 / 99 | 0.700 / 0.720 / 0.755 |
| Fresh one FE wide, 98304 / 40 / 4 | 2126 / 1304 / 212 | 0.602 / 0.612 / 0.645 |
| Fresh unbalanced FE and 5×3 factors, 81920 / 10 / 4 | 807 / 532 / 144 | 0.633 / 0.659 / 0.666 |
| Fresh weak two FE, 61440 / 10 / 4 | 234 / 201 / 102 | 0.859 / 0.878 / 0.884 |
| Fresh disconnected three FE, 61440 / 10 / 4 | 315 / 297 / 779, unranked | 0.886 / 0.917 / 0.952 |
| Fresh one FE + all outputs, 81920 / 8 / 1 | 160 / 125 / 43 | 0.771 / 0.781 / 0.781 |
| Excluded fast wide, 122880 / 31 / 8 | 94 / 92 / 124 | 0.947 / 0.959 / 1.000 |
| Low IID two FE, 61440 / 10 / 4 | 151 / 122 / 30 | 0.808 / 0.813 / 0.848 |
| AW one FE, 61440 / 10 / 4 | 137 / 109 / — | 0.796 / 0.813 / 0.818 |
| PW clustered two FE, 61440 / 10 / 4 | 176 / 151 / — | 0.830 / 0.831 / 0.858 |
| FW one FE, 61440 / 10 / 4 | 136 / 109 / — | 0.790 / 0.794 / 0.801 |

Every eligible native pair improves. The excluded fast control has exact
scientific exports and ratios .947–1.000; this small timing shift is not a
new fast-path algorithm claim. Eleven of twelve stock-comparable rows still
lose; only the fast wide control wins. The disconnected stock row is unranked.
Three repetitions support this bounded large-effect decision, not universal
performance or constrained-memory claims.

## Science, memory and visible failures

All 48 native pairs and repeat checks pass the unchanged numerical envelopes.
Maximum coefficient and covariance differences are 5.22e−15 and 1.01e−17.
Maximum absolute RSS difference is 1.04e−7 in a weighted fit; its registered
relative check passes. Native status, DoF, subset and FE metadata exports are
identical. Low-mode numerical exports and descriptive condition proxies can
change because the finite algorithm changes; they are not claimed byte-identical.
All three saved-output comparisons pass, with maximum difference 8.89e−16.
Five small pilot cases pass independently constructed full-coefficient,
covariance, sample and RSS dummy-design oracles on both artifacts.

Twenty distinct focused Rust tests pass. They include direct dense/scratch
trial equivalence, wide weighted full/weak/deficient designs, exact omissions,
rank thresholds, rank zero, explicit weight divisors, low block/pool behavior,
retained-session weak/deficient controls, reservation failure at every low-fit
allocation, injected coefficient/bread corruption, checked frame arithmetic,
and an independent allocator peak fixture for fast/low IID/robust/clustered
execution. The direct trial test also passes optimized compilation.
Actual-plugin ABI/panic containment and three licensed OLS oracle/transaction
fixtures pass. The additional PPML low-categorical fixture passes nine
one-/two-/three-FE and pool combinations against explicit dummy Poisson,
including PW, means, covariance, rank, final certificate and memory checks.
No PPML speed claim follows from that control.

Direct candidate/stock comparisons pass 36/39. The remaining three are the
fresh disconnected sparse three-FE covariance comparison. Native residual DoF
is 48104; stock's is 46560. Original maximum covariance error is 1.78e−7.
Diagnostic rescaling of stock covariance by `46560/48104` agrees within
2.19e−13, but does not independently adjudicate rank or change the failed
comparison. Original exports and failed checks remain authoritative.

The largest simultaneous finite square owners in the new trial are the caller
Gram, trial Gram, factor, inverse factor and canonical inverse. The inverse-factor
owner drops before finite bread allocation, leaving at most five square banks
plus linear metadata. This fits the existing internal `128*q*q + 1024*q + 4096`
finite allowance. The trial adds no row-sized buffer and drops all local owners
before QR fallback. Published legacy base estimates, admission policy and
scratch file sizes are unchanged. Whole-action measured peaks are identical
outside parallel multi-FE variation, whose pair deltas span −3,541,216 to
+2,224,040 bytes. This is no tighter whole-action memory guarantee.

`lowchol1` was built before the exhaustive low-fit failure test and stale-comment
correction; it received no headline timing. The premeasurement amendment binds
`lowchol2`. The generic fixture runner initially rejected the standalone PPML
fixture's different sentinel convention. Its actual nine blocks, original
terminal sentinel and wrapper terminal sentinel all completed; no fixture or
assertion was changed. The initial static audit could not import `tomllib`
under system Python; the corrected environment run is retained separately.
Workspace formatting still reports extensive pre-existing drift; no bulk
reformat was made. The static allocation inventory remains failing for source
drift and was not regenerated. These controller/development failures remain
visible in the archive.

## Remaining opportunity

The large full-rank QR cost is removed without another tuning guard. Unchanged
compensated scratch moments now dominate finite fitting; the rest of the wide
command still costs roughly 0.74 seconds. The next bounded task should profile
current-source production covariance and its independent checks, then consider
a coherent shared improvement if a large avoidable pass or inner-loop cost is
confirmed. Do not infer the preceding artifact's phase timings apply here.
Factor-base provenance, generated time-series terms, multi-partition slopes,
broader topology/memory coverage and earlier comparator adjudications remain
open. Weighted constrained-fit and nonretaining CG failures, allocation
inventory drift, and deferred Linux/SCC checks are not repaired by this change.

This is a local development checkpoint, not completion of the broad optimization
goal or release qualification.

The first archive attempt encountered an unrelated Finder `.DS_Store` file
and failed before producing a manifest. It was not accepted; its hash and
failure are recorded. The final archive uses an explicit text-file allowlist.

## Immutable evidence

[Receipts](receipts.zip) and [checksum](receipts.sha256) contain 4,480 verified
entries plus their hash manifest (23,244,845 compressed bytes). SHA-256:
`30c4a6a54dda2301e46bde882ff1007d340e60720ad5f573f1add065410c35b0`. Every archived hash verifies; machine paths are sanitized.
Raw data, plugin binaries and licensed Stata logs are excluded. Original log
hashes, terminal protocols, source identities, overlays, amendments, failed
checks, distributions and all numerical exports are retained.
