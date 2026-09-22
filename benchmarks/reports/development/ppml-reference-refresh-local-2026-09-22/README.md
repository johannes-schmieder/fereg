# PPML fresh reference statistics: bounded local adoption

Reuse of the existing numeric-column binding and broader independent-column
scheduling reduces repeated reference-statistic work while retaining fresh
current-weight calculations. [ADR 0103](../../../../docs/adr/0103-ppml-fresh-reference-columns.md)
records the scope and invariants. The baseline is `87b21d4c`/`transport1`;
candidate `reference1` is that source plus `ols/linear_session.rs`, with plugin
SHA-256 `357118cece7f4eaf1813e5e7ab25458ca999ae20fb0bed8afc7dd4d6354d58ea`.
All 523 computational inputs match the final working source. The archive
contains the exact patch and full baseline/candidate identities.

## Cost and change

Separate diagnostic builds of `7189e57a` show reference refreshes taking
roughly 4.8–5.5 ms at N61,440/K10. One-FE invokes four refreshes, spending
about 20 ms there. Strict retained-rank assembly adds about 7.7 ms per solve;
degree-six two-FE final residual projection alone takes 38 ms. These clocks
overlap enclosing phases and must not be added as disjoint costs.

The accepted change binds immutable numeric sources once per refresh and
allows independent column work below the old 65,536-row cutoff when N*K is
large enough. It also carries to eligible multi-FE categorical sessions.
Fresh normalized weights, means, stable norms, residuals, scores and final
Hessian certificates remain mandatory. No changing-weight geometry cache is
introduced. Generated recipes retain their accessor, existing one-FE scheduling
and scientific policy; low-memory/slope paths retain serial scheduling.

Uninstrumented one-FE screen phase medians corroborate the location of the
gain: projection 31.15→23.80 ms, finite solve 19.94→17.04, strict/final solve
55.54→48.36. Preparation is 16.45→16.36, inference 1.70→1.60, publication
9.66→9.59. Whole-command medians below include command parsing, native
loading/preparation, fitting, inference and publication. External data import
and evidence export are outside both timers. Diagnostic artifacts are kept
separate from all headline measurements; stock fixest is uninstrumented.

## Whole-command evidence

Each cell has three alternating baseline/candidate pairs, with stock fixest
in rotated order for matching requests. The initial screen uses seed 202609221.
The confirmation and excluded-path blocks were frozen before their runs with
fresh seed 202609223. The 49,152-row confirmation was added after inspecting
the initial screen and before running the confirmation block; candidate code
was unchanged. No branch depends on a design name or seed. All frozen primary,
holdout and native-control definitions remain unchanged.

There are 108 attempts: 102 completed, six native warmup failures, no timeout,
detected competing fit/build or process-monitor error. No compilation,
profiling, archiving, oracle computation or competing fit ran alongside these
headline blocks.

| PPML case, N / K / threads | Baseline / candidate / stock median, ms | Paired candidate/baseline min / median / max |
|---|---:|---:|
| One FE, 61440 / 10 / 4 | 179 / 162 / 54 | .899 / .915 / .933 |
| Degree-six two FE, 61440 / 10 / 4 | 348 / 347 / 101 | .986 / 1.000 / 1.006 |
| Dense three FE, 61440 / 10 / 4 | 476 / 466 / 335 | .969 / .979 / .981 |
| Fresh one FE, 131072 / 17 / 4 | 451 / 445 / 234 | .987 / .991 / 1.004 |
| Fresh sparse two FE, 61440 / 10 / 4 | 359 / 347 / 2195 | .951 / .969 / .972 |
| Fresh unbalanced three FE, 61440 / 17 / 8 | 502 / 491 / 185 | .978 / .988 / 1.000 |
| Fresh one FE, 49152 / 12 / 4 | 158 / 143 / 51 | .887 / .911 / .911 |
| One FE with zeros, target 24576 / 12 / 4 | failed / failed / 38 | Unranked: native warmup fails |

Successful reference comparisons use positive outcomes, no separation removals,
the same full sample and numeric regressors, robust VCE and no saved outputs.
Stock `fepois` covariance is HC0 times N/(N−1), matching the native request.
All 21 successful stock pairwise coefficient/covariance/objective/sample checks
pass. The seven successful rows comprise one win and six losses to stock;
most remaining losses are substantial. This is bounded development evidence,
not the full pinned-ppmlhdfe/F0 comparator-adjudication gate.

| Native excluded-path control, N / K / threads | Baseline / candidate median, ms | Paired min / median / max |
|---|---:|---:|
| One FE serial, 61440 / 10 / 1 | 200 / 196 | .960 / .995 / .995 |
| Two FE low memory, 30720 / 10 / 4 | 355 / 354 | .914 / 1.000 / 1.011 |
| One FE narrow, 61440 / 4 / 4 | 118 / 120 | .967 / 1.017 / 1.025 |
| Two FE PW/cluster, 30720 / 10 / 4 | 204 / 200 | .957 / .971 / 1.025 |
| Two FE with absorbed slope, 30720 / 10 / 4 | 1260 / 1250 | .976 / .994 / .995 |
| OLS three FE, 153600 / 31 / 4 | 609 / 606 | .974 / .982 / 1.018 |

These controls support no broad excluded-path speed claim. The narrow control
loses about 1.7% by medians; low-memory and large already-parallel one-FE cases
barely change. OLS never enters the changed method. No further tuning followed
these results. Per-call times, phase medians, repeat checks and peaks are in
the three summary JSON files.

## Independent science, memory and failures

All 39 completed native pairs pass the existing tolerances and have
byte-identical scientific exports, including final certificates and separation
receipts. Successful within-artifact repeats pass. The new licensed
`feppml_reference_refresh` fixture exercises N49,152/K12, one/two/three FE,
unit/FW/PW weights, offsets, robust/clustered inference, saved means and T1/T4.
Its nine independent dummy Poisson fits and directly formed current-Hessian
sandwiches check coefficients, covariance and means for 18 native fits.
Ordinary fixest weights are never used as an FW inference oracle.

The final source passes 375 distinct release-mode Rust tests: 373 PPML tests
and two certified-session generation/route tests. The actual-plugin ABI
harness passes. Four licensed fixtures pass with explicit zero return and
terminal sentinels: the new refresh fixture, `feppml_native_preparation`,
`feppml_weights_inference` and `feppml_saved_slopes`. Controls include generated
terms, missingness, weighted inference, slopes, saved outputs and transaction
failure injection. This supplements the previous transport milestone's 76
focused tests and three fixtures; it is not a release qualification campaign.

No heap buffer or matrix-sized owner is added. Tracked peaks match exactly in
one-FE, serial, low-memory, weighted-cluster and slope controls. Multi-FE
threaded peaks vary: candidate-minus-baseline spans −2.51 to +3.49 MB in PPML,
with all individual differences preserved. This does not establish exact peak
neutrality or constrained-memory qualification. The static allocation-source
audit still fails on documented drift; the inventory was not regenerated.
Two failed audit launch attempts (old system Python and omitted subcommand)
are retained separately from the completed source-drift audit.

The zero-outcome cell fails all six native attempts during its 7,680-row warmup
with r(3499), before the target timing. Direct replay on both `reference1` and
the original `7189e57a`/`dtk1` reports **“PPML separation execution bounds are
inconsistent”** from `_fereg_estimate_ppml.ado`'s publication validator. The
specific invalid sequence/receipt field has not been localized. Stock completes
the target, but this is an application failure, not a valid speed comparison.
`zero-replay/failure.json` binds the data, scripts, source, log hashes, return
codes and terminal sentinels. Neither the validator nor separation policy was
bypassed. This pre-existing failure remains open.

## Scope and next subsystem

The [starting inventory](../next-subsystem-inventory-local-2026-09-22/README.md)
and [transport milestone](../lsmr-transport-local-2026-09-22/README.md) remain
exact-artifact evidence. The current round adds selected four-FE, disconnected,
unbalanced, weighted, low-memory, clustered and slope controls. It does not
cover every high-dimensional/generated/output combination or establish
disconnected PPML performance. Sparse/disconnected OLS reference discrepancies,
weighted constrained-fit failures, nonretaining CG instability, serial-slope
losses and deferred Linux checks remain open.

The next bounded subsystem should be PPML separation/publication accounting:
localize the zero-outcome execution-bounds failure, verify sample and separation
certificates independently, and add a focused regression fixture. Then return
to strict checkpoint residual projection/assembly costs on scientifically
comparable samples. Do not extend this modest reference-refresh experiment.

`evidence.zip` contains 3,108 verified portable receipts, captured controllers,
plans, numeric exports, summaries, source overlays, diagnostic traces, Rust
checks, fixture receipts and original log hashes. It excludes data, binaries
and raw licensed logs. The stock shared library matches the earlier immutable
stock receipt; the fixest archive pin is unchanged. No push or release occurred.
