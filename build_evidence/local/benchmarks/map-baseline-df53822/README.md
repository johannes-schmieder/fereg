# Frozen MAP baseline at df53822

These four immutable paired-case receipts are the last completed performance
evidence before ADR 0008 redirected development from deterministic MAP-only
qualification to certified solver routing.

They bind source `df53822b3263d7425a9a2a42b16b58f7b306dc91` and candidate
plugin SHA-256
`ec3c4f0c71b9e0fc0581ec83fd16794ab8bf34727dee7d43c0ecc83252550ef9`.
Each case contains one cold and five warm fresh-process runs for both the
candidate and the exact pinned comparator, plus scientific identities,
certificates, host metadata, and memory telemetry.

| Case | Threads | reghdfe warm median | fereg MAP warm median | Speedup |
|---|---:|---:|---:|---:|
| dense two FE | 1 | 22.011 s | 9.123 s | 2.41x |
| dense two FE | 8 | 18.053 s | 7.150 s | 2.52x |
| sparse two FE | 1 | 31.801 s | 14.860 s | 2.14x |
| sparse two FE | 8 | 25.807 s | 10.751 s | 2.40x |

These receipts are implementation-baseline evidence only. They cannot qualify
a later source SHA, solver policy, artifact, or release candidate.
