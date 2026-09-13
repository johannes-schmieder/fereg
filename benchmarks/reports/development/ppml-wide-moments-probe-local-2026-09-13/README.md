# P4/P6 wider moment tiles: unadopted probe and transport finding

The wider dense moment prototype passes focused mathematical tests and two
first-screen paired commands. It is **not adopted**: the third primary fails
on the unchanged baseline during warm-up, before measurement. Preserve this
checkpoint and its exact patch while addressing that transport boundary
separately. No repeated seeds, protected performance controls or private
allocation audit have run for this prototype. Accepted native remains
`22b5106fcffb40dbc729832c45a269890daa5c0b`.

## Prototype and source boundary

Above 64 active columns, scaled retained dense moment partitions use two fixed
64-value tiles: weighted left values and standardized right values. Nested
column tiles cover arbitrary admitted widths without a new width/rank cutoff.
Each Gram/RHS entry receives exactly the original observation-order FMA.
Reference scales, normalized weights, selected-column mappings, independent
certificates, QR and strict transactions remain unchanged. The existing up-to-64
cache, scratch, unscaled/single-column partitions and one-shot prepared OLS
moment routes retain their previous work.

Local stack payload is 1,024 bytes per partition, independent of K. No heap
allocation, Gram partial bank, observation-sized copy or new solver setup is
introduced. This bounds cache storage, not total Gram arithmetic or retained
design memory. It does not implement the centered-tile architecture of P5.

[Identities](identities.json) bind all 152 native input hashes, locked dependencies,
the exact [prototype patch](implementation.patch) and both isolated plugins.
Candidate inputs matched the measured main snapshot; baseline inputs match
accepted `22b5106`. The candidate plugin is
`0881c43f5f3de28494b75af3c8ab93b5005445303a496465e915317d72c4e160`;
the baseline is `ad39e2329579cf87295796379e12035805d923a3417b8021af37df0a198322f6`.
The prototype is archived and reversed from active source. The installed
plugin is unchanged and is not either comparison artifact.

## First screen: two completed pairs, one unavailable target

The [prospective protocol](protocol.md) requires repeated median paired savings
of at least 5% and 20 ms, changed-phase evidence and protected controls. This
screen does not satisfy that complete adoption protocol.

| Target | N / K / workers | Baseline → candidate command, seconds | Single-pair change |
|---|---|---:|---:|
| Unit, one FE, robust | 122880 / 65 / 4 | 2.269 → 1.636 | −27.9% |
| FW, exposure, missing, one FE, robust | 61440 / 129 / 4 | 4.726 → 3.458 | −26.8% |
| PW, offset, missing, two-way cluster | 30720 / 257 / 16 | Baseline warm-up r(1003); neither measured arm | unavailable |

Both completed full b/V exports are byte-identical; discrete sample/rank/DoF/
weights/clusters/separation/solver fields are exact and independent certificates
pass. Finite phases fall 0.7127→0.2953 and 1.2247→0.3959 seconds;
strict/final phases fall 0.6589→0.4480 and 1.0642→0.6404 seconds. Projection
and preparation remain close. Overlapping phase times must not be summed.
Native peaks are unchanged at 364.52 and 288.01 MB and fit their unchanged
431.45 and 347.32 MB models. The [numeric archive](archive.json) preserves
full results, RSS, phase models, iterations/retries/fallbacks and all input
recipes, including the unavailable case. These are local M2 Ultra development
observations, not repeated gains, a memory saving or qualification.
Sampled RSS rises 0.3481→0.3957 GiB for unit K65 and falls 0.3557→0.3343 GiB
for FW K129. Both pairs retain four outer/six inner iterations and zero reported
fallbacks, step halvings and strict-profile rejections. No QR-fallback timing
gain is established by these two ordinary cases.

## Independent validation and failure localization

[Validation](validation.json) records 62 distinct candidate Rust tests in an
isolated Cargo target. Exact scalar moments now cover 128/129/257 columns,
alongside 1/7/16/63/64/65, at 1/4/16 workers with extreme finite scales,
unit/provided weights and selected/gapped storage. Retained fresh/frozen/strict,
above-64 QR, rank-deficient, generation and stale-weight tests pass. Both
isolated artifacts pass the independent full-dummy Poisson/Hessian/sandwich
Stata oracle. The new wide arithmetic is covered independently by the scalar
moment oracle; the existing Stata dummy fixture itself is narrow. Universal
static/native arm64 ABI checks pass, with both architecture compilations
visible. No scientific tolerance changed.

The failed K=257 baseline's noisy replay returns `too many options`, r(1003).
A trace locates the failure at the single plugin invocation carrying the
framed request. A separate call with 270 variables and a short `version` action
reaches native code and returns zero. Passing a 1,024- or 4,096-byte dummy option
reaches the native `version action does not accept arguments` validation and
returns r(198). At 8,192/16,384/32,768 bytes, Stata instead reports `options too
long`, r(1003), before native dispatch. This local evidence identifies long
plugin-option transport as the next boundary; it does not establish an exact
universal byte limit or a numerical failure of the 257-column kernel.

The first combined diagnostic's `_dup` expression was invalid after its
successful 270-variable probe. A separate literal-string probe completes and
produces the size results above. Original failed request/result/worker and all
diagnostic do-files/logs are hash-bound in validation; raw licensed logs and
machine paths are excluded from Git. No failed receipt is overwritten or
counted as a successful comparison.

Next implement and independently test bounded request transport using the
existing main-thread macro bridge or another reviewed metadata mechanism.
Preserve canonical framing/validation, one substantive plugin call and atomic
publication. That repair needs its own source/artifact baseline before the
wider moment evaluation resumes. Do not substitute a smaller PW target merely
to obtain a passing performance cell. Other #27 packages remain open; the two
agents, upstream CMG assessment and small SCC campaign remain after #27.
