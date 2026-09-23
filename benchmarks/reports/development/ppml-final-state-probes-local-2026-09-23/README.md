# Final-state response probes: rejected

Local macOS arm64 development evidence, September 23, 2026. Baseline is
`oneprof6`, matching all 534 computational inputs at `4ad7ade5`, plugin
`c1aa681c6c160d60381d8483b783a1bbed057c63d4f0d81c89f9e02485ac67b3`.
Candidate `finalstate1` plugin is
`3cb45de24f14b1a18f5122e4adc140d813e85dd00814ce53ac93f129584a9258`;
`finalstate2` is
`f3c5223fc927cb8984e364e6489dd591cb89750cf6206aba1e9eee801e1facaf`.
Both candidate sources and exact identities are preserved. Neither is adopted.

## Experiment and decision

`finalstate1` retains the independently strict-confirmed accepted point for
publication PPML, preserving exact Hessian weights for final synchronization.
Mu exploration keeps its prior transition. The changed same-weight path avoids
control reprojection but may require a colder response solve. `finalstate2`
adds one-use, generation-bound response history to warm that response under the
unchanged weights. It excludes low memory and direct one-FE projection; those
structural exclusions were declared before timing. No new heap bank is added.
All existing acceptance checks remain.

The second probe improves some cells by 1–3%, leaves low memory unchanged and
slightly loses the wide slope case. Reject both under ADR 0128. The ownership
complexity is not justified by this bounded evidence. No fresh confirmation or
additional OLS screen was run after rejection. This does not reject a materially
different shared final-factor construction or joint finite-plus-FE solver.

## Public timing distributions

Seed **202609325**, three alternating baseline/candidate/stock rotations per
screen. Each entry is median [minimum, maximum] milliseconds. Timers include
loading/preparation, estimation, robust inference, fitted means and publication;
exclude process startup, data import, warmup and exports. No build, profile,
archive or competing fit overlaps were detected.

Stock is pinned unmodified fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched application threads, BLAS one
thread and the established comparable robust normalization. Low native versus
ordinary stock is explicitly not a matched-memory comparison.

| Case | Baseline, first screen | finalstate1 | Stock |
| --- | ---: | ---: | ---: |
| Two slopes N30720 K6 T4 | 169 [162,170] | 159 [157,164] | 118 [118,119] |
| Unbalanced three FE, two slopes N30720 K6 T4 | 152 [150,154] | 152 [151,153] | 114 [113,119] |
| Larger slopes N61440 K10 T8 | 371 [365,376] | 366 [357,382] | 229 [223,231] |
| Unbalanced slopes, low memory N30720 K6 T4 | 303 [299,308] | 308 [297,310] | 120 [119,121] |
| Sparse categorical two FE N61440 K10 T4 | 233 [228,235] | 228 [225,228] | 138 [135,138] |
| Unbalanced categorical three FE N30720 K10 T4 | 169 [167,171] | 177 [176,181] | 109 [109,110] |
| Dense categorical three FE N30720 K6 T4 | 207 [207,214] | 212 [209,215] | 233 [230,234] |
| One categorical FE N61440 K10 T4 | 128 [128,130] | 131 [130,131] | 54 [54,55] |

| Case, same order | Baseline, second screen | finalstate2 | Stock |
| --- | ---: | ---: | ---: |
| Two slopes | 163 [163,164] | 162 [160,163] | 118 [116,119] |
| Unbalanced three FE slopes | 154 [153,157] | 149 [148,151] | 119 [116,120] |
| Larger slopes | 366 [364,378] | 370 [364,370] | 226 [222,229] |
| Low memory | 293 [292,312] | 293 [288,300] | 121 [118,121] |
| Sparse categorical two FE | 231 [228,235] | 228 [225,229] | 140 [139,140] |
| Unbalanced categorical three FE | 172 [171,172] | 170 [168,174] | 116 [113,116] |
| Dense categorical three FE | 209 [208,215] | 203 [201,208] | 230 [228,232] |
| One categorical FE | 129 [128,130] | 128 [127,129] | 56 [55,56] |

Do not select the favorable clock block or transfer these claims to another
source. First-screen native records show two-slope strict-final time 33.780 to
28.621 ms but inner units 62 to 76; unbalanced categorical strict-final 56.010
to 63.404 ms and inner units 46 to 62. These individual records support the cold
response explanation; they are not decompositions of public medians. The
low-memory baseline spends 88.331 ms in finite work and 73.174 ms strict-final,
supporting a different investigation of scratch consumers and finite solving.

## Science, memory and preservation

All **168 public applications** complete successfully: four independent pilots
per candidate with three arms, plus both 72-application screens. Public-state,
pair and independent small explicit-design Poisson/Hessian checks all pass,
including stock on these particular inputs. Earlier stock discrepancies remain
open. No timeouts or overlap/monitor failures occur.

`finalstate1` passes 132 prepared-PPML Rust tests; `finalstate2` passes 134,
including new stale-response and strict-rollback history tests. Each candidate
passes four licensed fixtures with process rc=0, Stata rc=0 and terminal PASS:
multipartition slope oracle, saved slopes, separation and one-FE allocation audit.
Build receipts include actual-plugin ABI/panic checks. These are development
checks, not qualification. Per-application tracked/modelled peaks are preserved
in `campaign-summary.json`; the probes add no observation or factor bank.

Seed326 was declared but never generated. Accepted source is restored and all
534 inputs match oneprof6. Frozen 77/16/111 definitions, allocation-inventory
drift, constrained-fit/CG failures, obsolete allocation-fixture assertion and
platform/SCC deferrals remain unchanged. No push, release or stopped campaign
restart occurred.

The immutable container verifies **6,173 logical files**, **1,624 payloads**,
**14,695,193 bytes**, SHA-256
`7294400c7cc4ea7eca78d524a5909cd9ef63c2480193496595e934c2a12fd467`.
`restore_receipts.py` verifies and restores every receipt into a new directory.
Exact artifact sources, declarations, controllers, tests, timings and audits are
preserved; licensed raw logs, binaries and generated data are excluded, with
omitted data hashes retained. `verification.json` records successful restoration.
