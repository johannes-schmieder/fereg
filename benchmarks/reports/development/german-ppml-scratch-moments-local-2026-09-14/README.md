# German PPML retained scratch moments: local development adoption

The candidate batches complete row-major scratch rows inside the existing
Gram-column partitions. Each narrow tile holds at most 1,024 doubles; rows wider
than that retain their existing complete-row capacity. Observation order, selected
column identities, weight normalization, reference scales and FMA order remain
exact. Dense retained and one-shot OLS paths do not acquire this workspace.

Parent is `1f76986` with accepted plugin `e1c72f56`. Candidate plugin is
`2acecd294e35bb1ac4378c20c149987fd1b8eabb3e1eca8ec0fc250e843fcb44`, built only in an
isolated source snapshot. [Inputs](inputs.json) and [patch](source.patch) bind the
native source and artifact. Installed dist remains `4187efdf`; no push or cluster
work occurred.

## Evidence

214 OLS/session tests, 351 PPML tests, 97 plugin tests and two constrained-memory
tests pass. Universal static audit and actual ARM64 ABI/self-test pass. Two
licensed Stata fixtures contain 26 native fits, with independent dummy Poisson,
weak weighted mobility, one/two/three FE, pool boundaries, frequency/probability
weights, clustering/missingness and singleton/separation cascades. Initial memory
assertions expected the old row bank; final assertions charge the new tiles and
preserve the same imposed admission limits. The warmed allocation tests pass.

The new scalar-oracle kernel test preserves Gram/RHS values exactly across unit
and extreme positive weights, selected-rank/stored-width differences, tile tails,
wide rows and one/four workers. At 18 stored columns, 65,536 and 131,072-row cases
make over 50 times fewer positional read chunks than the previous per-row loop.
This is a read-count result, not a kernel wall-time claim.

Three alternating pairs per German subset use `exp(logrwage2)`, the same ten
age/education controls and year effects, two absorbed effects, robust inference,
low memory, pool four and eight native threads. Each command runs in a fresh
Stata process. No own builds/tests/oracles run concurrently with timing. Both
arms have solver tracing enabled. The command and phase values below are
independent medians and need not sum.

| Input / retained rows | Parent command | Candidate command | Change | Finite solve parent → candidate | Strict/final parent → candidate |
|---|---:|---:|---:|---:|---:|
| 7,759 / 6,654 | 1.194 s | 0.845 s | −29.23% | 0.327158 → 0.039424 s | 0.221112 → 0.137586 s |
| 75,350 / 65,503 | 11.688 s | 8.224 s | −29.64% | 3.171349 → 0.390776 s | 2.379641 → 1.679041 s |

FE-projection medians are 0.534018→0.558558 s and 5.874241→5.833079 s.
Preparation is 1.938→2.212 ms and 19.147→19.644 ms. The gain is in finite
scratch-read work. Every pair has exact samples/order, coefficients, covariance,
means/absorbed outputs, status, fit/DoF/drop tables, non-time solver fields and
certificates. Independent mean stationarity and coefficient/model-space checks
pass. Pinned reference masks/order agree exactly, slopes within 5.17e-15 and
covariance within 1.26e-12. Comparator objective policy remains ADR 0026.

Native allocated peaks rise 5,011,531→5,059,051 and 39,953,316→40,000,836 bytes:
47,520 additional bytes each. RSS medians are 72,433,664→71,647,232 and
105,316,352→111,460,352 bytes; this is not an RSS reduction. Selected plan stays
11,497,506 bytes for the small case and rises 74,367,908→74,432,324 for the larger.
Logical scratch stays 958,176 and 9,432,432 bytes. Fallback/retry counts and all
non-time solver work fields are unchanged, with zero solver fallbacks. Full
per-run values and phase costs are in [paired results](paired-results.json).

## Memory bound and scope

Low preflight charges `min(threads,K)*min(N*K,max(K,1024))` doubles for every
possible selected rank and stored width within the generation-zero envelope.
Actual partition count is not monotone in selected rank: K=5 on four workers
creates three partitions, while rank four creates four. The previous row-bank
formula could undercount that case. The new checked bound covers it without
changing headroom, route policy or adding observation-sized preparation state.
A trace at validated IRLS entry now exposes original and retained row counts.

The preceding full run had reached per-row moments but was interrupted before a
fit; it is not a completed baseline. A full retry is pending at this checkpoint.
Residual construction, QR and other scratch consumers remain separate work.
These are bounded local development findings, not qualification or universal
performance claims. Raw licensed evidence remains local under the basename in
[the immutable hash inventory](local-evidence-hashes.json).
