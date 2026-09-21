# Ordered serial slope passes: local development evidence

The `sf1` experiment combines coefficient projection, residual reconstruction,
row-ordered norm accumulation and fresh group maxima for an eligible serial
single-partition slope fit. The sorted-code guard preserves observation order.
The existing coefficient allocation supplies the maxima storage; no additional
matrix is introduced. Shuffled and threaded inputs retain their prior traversal.
Every result still receives its independent rank-aware QR certificate.

The three focused slope fixtures pass, including exact old-path comparisons
over unit/constant/dispersed weights, widths 1/12/52, slope-only and intercepted
terms, dependent channels, sorted/shuffled rows and T1/T4. Independent QR
certificates, strict-tolerance rejection and unchanged-input fallback pass.
The actual plugin ABI fixture passes.

The paired screen completes 87 application calls across ten cells, all without
detected competing work. All thirty native pairs have bit-identical finite
exports and samples. All eighteen output pairs also have identical saved
vectors, with passing independent output audits. FW uses the native comparator;
ordinary fixest weights are not treated as Stata frequency-weight inference.

| Cell | wr2 median | sf1 median | fixest median | Paired sf1/wr2 |
|---|---:|---:|---:|---:|
| Unit K10, T1, IID | 118 ms | 113 ms | 92 ms | 0.9573 |
| Unit K10, T16, IID | 50 ms | 52 ms | 52 ms | 1.0196 |
| Unit K50, T1, IID | 406 ms | 407 ms | 700 ms | 1.0025 |
| Unit K50, T16, IID | 147 ms | 144 ms | 196 ms | 0.9724 |
| AW K10, T1, robust/all | 266 ms | 262 ms | 199 ms | 1.0000 |
| FW K10, T1, two clusters/all | 307 ms | 307 ms | — | 0.9902 |
| PW three slopes only, K6, T1, robust/all | 237 ms | 230 ms | 151 ms | 0.9705 |
| PW three slopes only, K6, T1, two clusters/all | 261 ms | 262 ms | 164 ms | 1.0038 |
| PW three slopes only, K6, T16, robust/all | 95 ms | 96 ms | 114 ms | 1.0000 |
| PW three slopes only, K6, T16, two clusters/all | 113 ms | 111 ms | 129 ms | 0.9735 |

These are exploratory three-pair screens. Small rounded timings and modest
margins do not establish final-block acceptance. Serial K10 and weighted output
losses remain explicit. The compact actual-channel Gram representation beneath
sf1 has its separate measured history in the
[finishing archive](../two-fe-finishing-experiments-local-2026-09-21/README.md).

Fourteen additional diagnostic calls compare ordinary and instrumented builds
and pass their scientific checks. Unit K10/T1 places about 12 ms in fresh slope
checks and 7 ms in reconstruction plus residual norms. A serial PW three-slope
clustered/all trace spends about 26 ms in preparation, 85 ms in the fit and
38 ms preparing outputs. Direct saved-effect recovery accounts for about 34 ms,
including 14 ms in group solves and 18 ms in finishing/certification. These
phases overlap named subtotals and must not be added indiscriminately. The fit's
`within_norms` interval also includes score-cache construction; it is not a
measurement of norms alone. Traced calls are not headline timing evidence.

The archive contains 101 successful calls, including all 87 screen calls and
14 diagnostics. `verify.py` verifies 2,529 stored entries, 59 omitted local
output arrays, the independent audit binding and complete reconstruction of
all three captured source identities. Raw licensed logs are omitted; extracted
native traces retain their original stream hashes. The small post-measurement
indentation cleanup is separately identified in `source-indentation.json` and
preserves every non-whitespace byte of the captured slope source.

| Artifact | Plugin SHA-256 |
|---|---|
| wr2 | `2b956b420b141b2d5c1ddb119451f48e69fa50510ab77f7ff7ee52c202f23b03` |
| sf1 | `90d4d382e9f7030e7db2b152b5b1cfe097d4dca147f49c1843a2ef4bdd550b62` |
| wr2-profile | `853b84009836fa107083613c5437ddfc67044b0db1e050349e79cacae5297a39` |

Broader protection, final independent blocks and Linux confirmation remain
open. This is development evidence, with no installation, push or qualification.

A subsequent four-call profiler compatibility check passes at T1 and T16.
All six numerical bundles and samples match the ordinary sf1 build exactly.
`profiler-check.json` binds the updated instrumenter, captured plugin source,
result hashes and extracted phase lines. These calls are diagnostic and are
separate from the immutable 101-call archive above.
