# Low-memory OLS: remove redundant QR column reads

Accept a small general improvement to scratch-backed QR: calculate the next
pivot norm from each just-written target while it remains in memory. Eleven
low-memory rows improve by paired medians of 1.8–4.5%, with no new allocation
and identical native scientific exports. Large stock-reference losses remain.
The next substantial target is the compensated scratch Gram/covariance work,
not further tuning of these norm reads. See [ADR 0109](../../../../docs/adr/0109-low-memory-qr-target-norms.md).

## Source and measurement scope

Baseline `factor_names1` is the computational checkpoint committed at
`f69cf21f`; its unchanged plugin is
`a191585c5a156809f8a572bbce37d948b2ede1e0ca8579a85f15e4cb32547d14`.
Candidate `qrnorm1` is an isolated build of `f69cf21f` plus the archived
`crates/fereg-core/src/ols/prepared_fit.rs` overlay. All 529 captured inputs
match the accepted working source. Candidate plugin SHA-256:
`f106400d4ed2561fcde5eedb367bcbe384e0c768aa563cc4df6840f09eb8846a`.
The overlay hash is
`a7b2fda2a667421ec96719139f20a81b0e5c04ca3020a534de2e968ed41864cf`.

Stock fixest 0.14.2 remains uninstrumented, with pinned archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`
and loaded library SHA-256
`bece258f6c6fc3fc47accea45c7b004b0c495d46170d78fa2d6302e66de5fbf9`.
The additive surface worker requests `memory(low)` for native fits; stock uses
its ordinary memory strategy. These are matched scientific workloads, not an
equal-budget or constrained-memory comparison. Numeric/factor coefficients,
full covariance, sample, RSS and requested normalized outputs are compared.
Dataset import/export is outside the timer. Fit/publication and requested
prediction are recorded separately; their sum is the command time below.

Every headline fit runs alone, after data generation. Builds, tests, profiling,
independent oracles and archiving run separately. Frozen 77/16/111 definitions,
reserved validation seeds and stopped campaigns are unchanged. No SCC attempt,
qualification, push or release is involved.

## Opportunity and mechanism

The initial current-source screen has six passing scientific comparisons per
repetition. Native/stock medians in ms are 154/25 (one FE robust), 152/32
(two FE IID), 187/87 (three FE IID), 471/68 (factor interactions robust),
2158/122 (wide robust) and 56/21 (saved outputs). Three separate phase traces
locate the dominant costs in low-memory finite fitting and inference.

For N=122880, K=31, T8, the full-size trace records 2122 ms of native inference,
including about 1051 ms before the robust inference composer. Inside that
composer, score storage costs 52 ms, its fresh Gram certificate 310 ms,
covariance construction 368 ms and the independent covariance certificate
339 ms. The retained-score copy costs only 7 ms. The files and bounded score
tiles accepted in earlier work already reach this path; file copying is not
the dominant remaining cost. Fast dense parallel kernels are deliberately
separate from scratch-backed, serial compensated accumulation.

Code inspection found a redundant read of every remaining QR column before
each pivot, immediately after the preceding reflector loop had owned and
written those same values. The candidate computes the identical stable segment
norm from that resident target. It removes the duplicate first-pivot scan and
later read-backs while retaining the initial scan, final independent scratch
rank certificate, all numerical thresholds and every residual/inference
certificate. No recursive norm approximation, topology guard, width cutoff,
new buffer or retained cache is introduced. This applies across the low-memory
OLS finite-QR path; it does not transfer timings to PPML iteration kernels.

## Alternating pairs and fresh confirmation

Five rotated baseline/candidate/stock repetitions per row. Screen seed
202609233, fresh seed 202609234 and small-oracle seed 202609235 were declared
before candidate timings. `K` is numeric width; factors add 11 or 14 finite
columns as stated. Ratios are candidate/baseline within each repetition.

| Case, N / K / threads | Baseline / candidate / stock medians, ms | Paired min / median / max |
|---|---:|---:|
| One FE robust, 61440 / 10 / 4 | 152 / 147 / 24 | .955 / .961 / .980 |
| Two FE degree-six IID, 61440 / 10 / 4 | 154 / 149 / 34 | .949 / .968 / .974 |
| Three FE dense IID, 61440 / 10 / 4 | 187 / 180 / 87 | .904 / .963 / 1.011 |
| One FE robust, 4×3 factors, 61440 / 8 / 4 | 472 / 460 / 69 | .956 / .977 / .989 |
| One FE wide robust, 122880 / 31 / 8 | 2169 / 2081 / 122 | .951 / .959 / .986 |
| One FE robust + all outputs, 61440 / 4 / 4 | 56 / 55 / 22 | .753 / .982 / 1.036 |
| Fresh one FE robust, 98304 / 14 / 1 | 444 / 430 / 101 | .966 / .970 / .980 |
| Fresh two FE IID, 96000 / 14 / 8 | 325 / 313 / 37 | .943 / .963 / .975 |
| Fresh one FE robust, 5×3 factors, 81920 / 10 / 4 | 983 / 953 / 138 | .962 / .973 / .986 |
| Fresh one FE wide robust, 98304 / 40 / 4 | 2765 / 2687 / 211 | .959 / .971 / .983 |
| Fresh one FE robust + all outputs, 81920 / 8 / 1 | 179 / 171 / 47 | .944 / .955 / .989 |
| Excluded fast robust, 61440 / 10 / 4 | 26 / 26 / 22 | .926 / 1.000 / 1.038 |
| Excluded fast wide robust, 122880 / 31 / 8 | 87 / 89 / 125 | .966 / 1.023 / 1.060 |

All eleven low-memory medians remain slower than stock, by roughly 2.1–17.1×.
The small saved-output screen has one 73 ms baseline outlier; retain its full
distribution rather than advertising that pair's 25% improvement. The wide
fast control loses 2 ms by median, with individual differences from −3 to +5 ms.
These controls and timer noise limit claims about small effects. The consistent
fresh low-memory gains justify this inexpensive change, despite falling below
the preferred 10% margin. No further norm-read tuning is warranted.

## Science, memory and failures

All 231 headline attempts pass application sentinels, with zero timeouts,
detected overlaps or monitor errors. All 65 candidate/baseline pairs have
byte-identical scientific exports; all ten native saved-output binary pairs
are also identical. Repeat checks and all 65 direct candidate/stock checks
pass. The three trace calls and eight corrected pilot calls complete separately.
Four pilot designs pass independent explicit-dummy coefficient, full covariance,
sample and RSS checks on both native artifacts, including normalized outputs.

Fifty-one distinct Rust tests pass: 38 selected by `low_` and 13 scratch tests.
They include weighted rank-deficient/wide fits, multiple pool widths, fresh
weight divisors, scratch corruption, I/O failure and cleanup. The actual-plugin
ABI harness passes its deliberate panic-containment injection. Three licensed
fixtures pass: `ols_low_one_fe`, `ols_cluster_low_memory` and
`estimate_ols_eclass_transaction`. They use independent Stata dummy regression,
unit/AW/FW/PW inference, literal FW expansion, common-weight rescaling,
collinearity, T1/T7 and rollback. Ordinary fixest weights are not an FW oracle.

There is no new allocation or scratch ownership. Measured native peaks are
identical outside threaded three-FE variation (−1,237,192 to +869,624 bytes).
This is not an OS-RSS or constrained-budget guarantee. Static allocation source
drift remains unresolved; weighted constrained failures and nonretaining CG
instability remain open.

Two pre-fit generator refusals are preserved: pilot N=1536 left a singleton
in the final five-period group, and fresh degree-six N=98304 violated the
required multiple of 240. Amended sizes are 1920 and 96000. Both amendments
precede any fit in their block; original declarations/controllers remain in
the archive. No failed application or scientific comparison was discarded.

## Separate factor-rebasing investigation

The diagnostic `factor_rebase_probe.do` reproduces the earlier ordinary
`absorb(g)` r(459) after missing outcomes remove factor level 1. Candidate
metadata for `i.f` and `ib1.f` is identical, yet final `fvexpand` requires
`x 2b.f 3.f` versus `x 1b.f 2.f 3.f`. Native `i.f`, `ib2.f`, `ib3.f` and
`ibn.f` fail; `ib1.f` succeeds. Every failed call preserves the previous b, V,
sample and absence of the requested output. The probe's sentinel certifies
these observations, not a repair. Printed long dummy names truncate; direct
expansion assertions establish the key distinction. An initial relative-path
launch failed r(601) before probing and is recorded in the receipt.

The legacy preparation path preserves candidate coefficient slots; the
categorical tuple path rebases with its deliberately restricted minimum-base
policy. Routing ordinary factors there would change supported explicit-base
behavior. A safe general repair needs original base/level-selection provenance
in the model wire, including dynamic, explicit and no-base intent. No blind
minimum-base rewrite or protocol change was adopted. Keep this scientific
compatibility task open; this bounded investigation does not claim it fixed.

## Evidence and next boundary

[Evidence archive](evidence.zip) and [SHA-256 receipt](evidence.sha256) contain
source identities/overlay, declarations and amendments, captured controllers,
all numerical exports, direct and independent checks, test receipts and
sanitized protocol traces. Original and portable hashes are retained; licensed
logs, datasets and binaries are excluded.
All 6,184 manifest entries verify. The archive is 63,713,024 bytes, SHA-256
`9746a19f5dc3dd2e70c21585cfe106d6daab4416969e8c87708ec97e99e48e07`.

The next performance milestone should target the scratch Gram and covariance
loops, preserving each coordinate's compensated arithmetic and fresh traversal.
Avoid replacing production influence accumulation with a reordered sandwich
formula. Remaining low-memory topology, weighted timing, generated time-series,
multi-partition slopes and constrained-memory coverage must stay explicit.
Earlier stock discrepancies and SCC deferrals are unchanged.
