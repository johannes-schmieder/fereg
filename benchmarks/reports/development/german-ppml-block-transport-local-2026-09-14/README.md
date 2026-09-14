# German PPML changing-weight scratch transport: local adoption

Retained low-memory projection now extracts and patches pool-width column blocks
using the scratch file's existing bounded complete-row methods. The two removed
loops each issued a read or write per observation per block. The projector,
floating-point operations, selected identities and changing-weight generations
are unchanged. No production allocation or admission policy is added.

Parent source is `94d7780` (native `0531450`), plugin `2acecd29`. Candidate plugin
is `fdaf3e16361542cf04ee2c535a7468e3fe13ab4739f0686f837d6b7fc0f1a10d`, built with
label `94d7780-block-transport` in an isolated snapshot. [Inputs](inputs.json)
and [patch](source.patch) bind the comparison. Installed dist stays `4187efdf`.

## Results

Three alternating pairs per case, each in a fresh local Stata process, use low
memory, eight native threads, robust inference and saved absorbed effects. German
cases use exp(logrwage2), ten age/education controls, year indicators and the
original person/establishment effects; pool four. Synthetic crossed-FE controls
use pool one, N=512/K=1 and N=2048/K=129. Data generation and independent oracles
occur outside timing, with no own concurrent builds/tests/oracles. Both arms have
solver tracing enabled. Independent phase medians need not sum to command time.

| Case: input / retained rows | Parent command | Candidate command | Change | FE projection parent → candidate | Strict/final parent → candidate |
|---|---:|---:|---:|---:|---:|
| German 7,759 / 6,654 | 0.817 s | 0.323 s | −60.47% | 0.544676 → 0.132161 s | 0.126333 → 0.040235 s |
| German 75,350 / 65,503 | 8.109 s | 2.245 s | −72.31% | 5.794687 → 1.186771 s | 1.654008 → 0.355504 s |
| Narrow 512 / 512 | 0.124 s | 0.086 s | −30.65% | 0.023385 → 0.001289 s | 0.013350 → 0.001665 s |
| Wide pool-one 2,048 / 2,048 | 3.030 s | 0.907 s | −70.07% | 1.741691 → 0.313120 s | 0.859163 → 0.163103 s |

Every pair has exact row order/sample, b/V, fitted means/absorbed effects,
status/fit/DoF/drop tables, non-time solver work and certificates. All four
candidate cases pass independent mean scores and fixed-coefficient model-space
checks. German pinned-reference masks/order are exact, slopes differ by at most
5.17e-15 and covariance by 1.26e-12. Reference mean/objective policy remains
ADR 0026. The wide oracle audits all 129 scores in bounded chunks and projects
log(mu) minus the complete X beta onto the FE span; no column is omitted.

The primary passes the prospective 5%/10 ms command-gain gate. No control crosses
the 5%/10 ms slowdown trigger. Larger German inference rises 23.36→36.88 ms;
the report retains that phase variation rather than implying every phase sped up.
Finite-solve medians there stay 0.393492→0.390065 s and preparation stays
19.557→19.558 ms. All per-run phases/scalars are in [paired results](paired-results.json).

Native peaks are identical in each pair: 5,059,051 and 40,000,836 bytes for the
German cases, 277,661 and 7,329,672 for narrow/wide. Scratch sizes are unchanged:
958,176 / 9,432,432 / 4,096 / 2,113,536 bytes respectively. RSS medians are
71,778,304→71,352,320; 113,704,960→112,672,768; 37,371,904→37,388,288;
53,755,904→53,477,376 bytes. German plans are exactly unchanged. Synthetic
selected-plan medians vary by +144/+48 bytes, with within-arm variation and
unchanged native peaks; no memory-plan improvement is claimed. All reported
solver fallbacks are zero and non-time solver work is exact.

## Validation and tradeoff

215 OLS/session, 351 PPML and 97 plugin tests pass. Universal static audit and
actual ARM64 ABI/self-test pass. Two licensed fixtures contain 26 native fits,
including independent dummy Poisson, weak weighted mobility, one/two/three FE,
pools 1/2/10000, FW/PW, clustering/missingness and singleton/separation cascades.
The existing interrupted/error/scratch lifecycle checks remain in the suites.

A new repeated-block test verifies exact bits (including signed zero and extreme
powers of two), untouched columns, file identity/length and buffer capacity.
It covers partial blocks/tiles, 1/5/18/129 columns and scale doubling at
65,536/131,072 rows with 18 columns. Per changing-weight pass, B blocks and T row
tiles cause 2*B*T encoded reads and B*T writes, replacing 2*B*N per-row I/O
requests. These counters describe bulk library calls, not literal OS syscalls.

The reused 1 MiB encoded bank reads full stored rows. This exchanges more
sequential bytes for far fewer calls: at full N=88,876,264, 18 stored columns and
pool four, extraction plus patch-read plus patch-write transfer about 192 GB
logically, versus 25.6 GB selected payload in the old loops (7.5 times as much).
Physical disk traffic depends on caching; no hardware-byte claim is made.

The [preceding full attempt](preceding-full-interruption.json) verified
88,876,264 active rows and finished one IRLS iteration, then was interrupted
after 1,453.38 process seconds. The third of three one-second stack samples
located per-row changing-weight extraction. Maximum RSS was 65.16 GB, swaps zero;
no estimate was published. Mixed-state scratch and raw logs remain local. That
interrupted run is not a completed performance baseline. A full candidate retry
and pinned-reference comparison remain pending at this adoption checkpoint.

Earlier analyzer-width and preparation-script syntax failures are preserved in
the local evidence inventory. No native test or timed fit failed. These are local
development findings, not full-data or cross-platform performance qualification.
Other scratch consumers, residual/QR costs and broader roadmap work remain open.
