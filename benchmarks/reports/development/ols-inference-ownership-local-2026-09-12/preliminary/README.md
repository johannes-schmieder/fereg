# P3 inference ownership: preliminary local evidence

This is a source-bound experiment, not an adopted production change. Baseline
is `f4cebe71bb8afcd29e5acb2e97a1d1875a340ba7`. The [patch](implementation.patch)
lowers only ordinary robust/clustered OLS's default packed-score limit from
512 MiB to 32 MiB, selecting the existing strided path earlier. PPML's separate
weighted-linear entry and caller-specific overrides are unchanged.
[Identities](identities.json) bind both isolated plugins; installed dist remains
unchanged. The [protocol](protocol.json) predates measurements.

There are 26 completed pairs / 52 measured calls: nine primary pairs, thirteen
controls, three separately traced rank-deficient QR pairs and one traced pilot.
All full-command comparisons use the existing local M2 Ultra/Stata 19 harness,
with fresh processes, small warm-ups and rotated primary order over three seeds.
Native thread counts are distinct from the Stata license's eight-processor cap.

| Target | N / K / native threads | Command medians, seconds | Median paired change | Native peak medians, bytes |
|---|---|---:|---:|---:|
| Robust sparse missing | 983040 / 50 / 16 | 1.312 → 0.803 | −38.8% | 1245142607 → 925443580 |
| PW robust complete | 245760 / 50 / 16 | 0.368 → 0.253 | −31.4% | 313492811 → 233452576 |
| FW cluster sparse missing | 245760 / 50 / 4 | 0.539 → 0.409 | −24.1% | 315828839 → 236091748 |
| QR, exact duplicate column (traced) | 98304 / 50 / 4 | 1.507 → 1.447 | −4.0% | 125258779 → 93312368 |

The trace locates more than a copy saving: the existing strided route also
selects fused raw reference moments and residual construction. On the pilot,
raw-moment work falls 0.1214→0.0202 seconds and bread/raw-residual work
0.4119→0.0103. The scope containing packed-cache construction falls
0.0521→0.000002. Strided score-design certification costs more, 0.0564→0.1019,
while covariance remains about 0.094 seconds. These scopes are separately timed;
this is a full-route comparison, not a claim that the covariance kernel sped up.

Both QR arms report `reabsorbed=1`: the existing packed cache is also discarded
when its generated-column identities disagree with the rank-reduced fit.
Measured score-storage reconstruction stays about 0.12 seconds in both arms;
QR savings therefore do not mean recovery/reprojection was eliminated. Every
pair passes native certificates and full b/V comparison under unchanged
registered tolerances. Sample/status/DoF, cluster/FE tables and solver decisions
are exact. Numerical certificate metrics can differ at roundoff scale.

The initial pilot assertion mistakenly compared those numerical metrics as exact
solver decisions. Its failure is retained in the local log; inspecting the
fields found residual-certificate differences around 1e-15, with successful
independent certification and b/V discrepancies around 1e-16/1e-22. The corrected
collector checks structural decisions exactly and retains the original numerical
acceptance. No scientific tolerance or production check changed.

Controls include all six graph families, complete IID, small/narrow, low memory,
saved outputs and fragmented restrictions. A newly eligible weak graph at
N122880/K50 improves 0.459→0.411 seconds; its native peak is essentially unchanged
because an earlier solver phase dominates. The largest absolute control slowdown
is complete IID 571→583 ms (+2.1%); the largest percentage is small/narrow 6→7 ms.
Neither exceeds max(10 ms, 5%). All primary gains are reported alongside these
controls in [comparisons](comparisons.json) and [summary](summary.csv); phase,
modeled/native memory, RSS and fallback receipts remain in the numeric archive.

The [independent Stata oracle](ols_inference_ownership.do) passes on both
[baseline](baseline-fixtures.json) and [candidate](candidate-fixtures.json), with
r(0) sentinels. It compares full slope covariance against explicit-dummy regress
at N98304/K50 for unit/AW/FW/PW, robust/clustered inference, full rank and an exact
duplicate. Four existing strided/cache tests and nine robust tests pass (twelve
distinct tests); these include weight semantics and frequency-replication science.
The universal static audit and native arm64 ABI/self-test pass. No x86 runtime,
SCC, Windows or qualification run occurred.

`compare.py`, `qr_compare.py` and `pilot.py` replay the captured experiment using
the existing local harness modules, an isolated baseline/candidate root and the
Stata executable supplied through environment variables. QR input recipes and
hashes are in the archive's derived-inputs directory. Raw licensed logs, binary
artifacts and datasets stay outside Git.

Production adoption, final source/documentation review and the remaining P3
cluster-encoding, score-tiling, skew and low-memory investigations remain open.
