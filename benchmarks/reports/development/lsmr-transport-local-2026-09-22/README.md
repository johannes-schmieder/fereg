# LSMR transport: bounded local adoption

The shared LSMR boundary now copies in tiles through its existing buffers.
This preserves all values and scientific policy while improving threaded wide
three-FE OLS. [ADR 0102](../../../../docs/adr/0102-tiled-lsmr-transport.md) records
the decision. The baseline is `7189e57a`/`dtk1`; candidate `transport1` is that
source plus the three FE files in `transport.patch`, with plugin SHA-256
`ec1b35b114548f69ea3e8c1ddac94f615b7ac43308984839ce4eed73acf15b49`.
The archive verifies every computational source hash against the working tree.

## Whole-command measurements

Each cell has three alternating native baseline/candidate pairs, with stock
fixest in rotated order where the requests match. External import and evidence
export are excluded; public command loading, fitting, inference and publication
are included. No compilation, profiling, archiving or competing fits occurred
alongside these timings. All 150 attempts completed; no timeout, application
failure, overlap or process-monitor error was detected.

| Case, N / K / threads | Baseline / candidate / stock median, ms | Candidate/baseline paired ratios: min / median / max |
|---|---:|---:|
| Dense three FE, 245760 / 50 / 16 | 854 / 785 / 587 | .896 / .903 / .998 |
| Sparse three FE, 245760 / 50 / 16 | 1153 / 1064 / unranked | .833 / .970 / .991 |
| Fresh dense three FE, 153600 / 31 / 4 | 646 / 607 / 558 | .937 / .937 / .971 |
| Fresh disconnected three FE, 122880 / 31 / 8 | 417 / 400 / unranked | .954 / .959 / .963 |
| Fresh unbalanced three FE, 122880 / 31 / 4 | 331 / 311 / 176 | .901 / .943 / .954 |
| Fresh sparse three FE, 122880 / 17 / 1 | 1119 / 1119 / unranked | .992 / 1.000 / 1.021 |
| Fresh four FE, 61440 / 10 / 4 | 148 / 146 / 98 | .973 / .986 / 1.057 |

The initial screen uses seed 202609221; the complete confirmation block was
declared before running with fresh seed 202609222. Its topology extensions are
additive and leave all frozen manifests unchanged. Disconnected groups split
each label by four worker components; unbalanced groups merge workers through
a deterministic square-root map. Four-FE data add an independent crossing
23-level dimension. No branch depends on benchmark identity or seed.

Narrow dense/sparse medians are 103→104 and 112→111 ms. PPML one-FE is 177→175
versus stock 52 ms. Degree-six PPML is 346→351 versus stock 99 ms; the fresh
K17 control is 708→718 versus stock 363 ms. One predeclared regression recheck
gives 381→377 ms for degree-six K10. These support no PPML speedup claim.
Other controls give one-FE robust 42→41 ms, two-FE clustered 125→124, AW
three-FE robust 69→66, and low-memory three-FE 104→105. No tuning followed
these results. Remaining dense/unbalanced/four-FE and PPML losses are explicit.

## Science and memory

All 55 native baseline/candidate pairs pass unchanged sample, coefficient,
covariance and objective tolerances. Scientific exports are byte-identical
except the four-FE control, whose tiny covariance/certificate variation also
appears within each unchanged artifact's repeats (roughly 5e−23 maximum
covariance difference on the large case). Its coefficients/RSS match and all
independent/reference checks pass. This is not a new exact-repeat guarantee.

Both artifacts pass bounded explicit-dummy beta/V/RSS oracles for dense,
disconnected, unbalanced and four-FE designs. Dense/unbalanced/four-FE stock
checks pass; disconnected stock covariance fails. Initial sparse stock
discrepancies also persist and are never counted as speed wins. PPML science
from the starting inventory remains diagnostic; no full ppmlhdfe/F0
qualification campaign was authorized or run.

Seventy-six focused release-mode Rust tests pass, including transport bits at
zero/tail shapes, signed zero/subnormal/nonfinite bit patterns, one/four workers,
weighted and slope LSMR, workspace overflow/admission, batch equivalence and
later-window failures. The actual-plugin ABI harness passes. Three licensed
Stata fixtures pass with explicit zero-return and terminal sentinels:
`ols_narrow_weighted_schur`, `estimate_ols_slope_only_multiple_parity`, and
`feppml_weights_inference`. They exercise independent weighted dummies,
frequency replication, clustered inference, saved outputs, multiple slopes,
PPML weights and failure atomicity.

The change adds no matrix-sized owner or storage class. Exact column-bank
reservation slightly reduces headers in several narrow cases. Native tracked
peaks vary in threaded upstream LSMR: paired differences include increases
up to 68.1 MB on sparse K50 and decreases up to 124.2 MB. The same solver may
keep different concurrent RHS lifetimes; all observations remain in the
summary. PPML peaks are equal or 120 bytes lower, serial K17 is 312 bytes
lower, and the excluded one-/two-FE controls are equal. There is no universal
peak-neutrality claim. No admission model or budget behavior was loosened.

The allocation source audit still fails with the existing drift plus the new
transport allocation-site delta. Its failure is archived, not refreshed away.
Linux/constrained-memory qualification, weighted constrained-fit failures and
nonretaining CG instability remain open.

`evidence.zip` contains exact artifact/source identities, the patch, plans,
captured controllers, numeric exports, pair/repeat/peak summaries, independent
oracles, sanitized Rust checks, fixture receipts and original licensed-log
hashes. It excludes binaries, data and raw licensed logs. The separate
[inventory](../next-subsystem-inventory-local-2026-09-22/README.md) contains the
diagnostic transport profile. The next bounded milestone is PPML's fresh
current-weight reference passes and strict checkpoint cost.
