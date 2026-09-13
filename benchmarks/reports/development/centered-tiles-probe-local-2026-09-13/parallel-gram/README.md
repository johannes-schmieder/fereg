# P5 bounded parallel score certificate: unadopted native revision

This [182-input snapshot](receipt.json) changes only score certification from
[native-mean-bank](../native-mean-bank/README.md). Centered scores of at most 64
finite coordinates share the existing dense Gram certificate's fixed row chunks,
bounded partials and compensated ordered merge. Each partial uses one 64-double
stack row, at most 32 KiB across 64 partials; no N-by-K score allocation is added.
The independent certificate, current reference weights and tolerances remain.
Wider centered prototypes retain the serial path. Existing dense entries retain
their arithmetic/partition order; strided and scratch implementations are unchanged.

The partial bank is bounded by the existing 32 MiB budget (at most 2 MiB for
64 coordinates), plus two K-by-K merge arrays. Reservations remain fallible and
all new partial allocations are exercised by the existing injected-failure and
exact/one-byte-short component tests. This does not complete the full overlapping
FE/inference memory proof or change public admission.

All 51 distinct final-source centered/robust/clustered tests and core checking
pass. Coverage includes independent weighted HC1/frequency expansion, clustered
inference, 1/4/16-worker scoped inference, 65-column serial control, rank-deficient
QR, saved outputs and allocation/stale-frame rejection. Both plugin slices
visibly rebuild; universal static audit and actual arm64 ABI pass. Candidate
plugin is `b18717c2a422ee241f744d0cfead538c1bc0ddf60d592a016eff7cb894657158`;
accepted baseline is `c64ead07`. Installed dist is unchanged.

| Original first-screen target | Accepted baseline → candidate seconds | Native peak reduction |
|---|---:|---:|
| Unit IID, N983040 / K50 / 16 workers | 0.585 → 1.037 | 42.7% |
| AW robust, missing, N245760 / K50 / 4 | 0.420 → 0.495 | 42.3% |
| PW two-way cluster, missing, N245760 / K50 / 16 | 0.322 → 0.419 | 40.4% |

All three remain outside the <=10% slowdown condition for memory adoption.
The earlier serial-score candidate took 1.044 / 2.037 / 1.987 seconds on the
same screen recipes and seed. These are separate source-bound screens, not a
repeated paired comparison between the two unadopted candidates. All primary
native peaks remain identical to that earlier candidate and within unchanged
models. Reported scratch is zero. Full paired scientific exports pass; samples,
status/rank/DoF and FE/cluster structure are exact.

One separate AW profile pair uses seed67280421: 0.413→0.494 seconds. Candidate
score-design time is 0.03425 seconds (baseline 0.09068), compared with 1.58208
in the earlier separately profiled serial-score candidate. Candidate fit remains
0.22064 seconds against baseline 0.09856; covariance formation is 0.10249 versus
0.08719. This localizes the principal removed cost and the remaining fit cost.
Profiles are not pooled with first screens. Raw licensed logs stay outside Git;
full numeric exports and sanitized phase lines are archived with checksums.

Next evaluate the serial FE certificate with bounded fixed group partitions,
then the same original command screen. No additional primary seeds, protected
command campaign, PPML centered evaluation or adoption has occurred. Every P5
checkbox remains open, including the full memory/rank-fallback boundary.
