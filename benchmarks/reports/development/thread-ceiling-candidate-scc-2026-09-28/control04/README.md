# OLS control confirmation and separate CPU attribution

Ratios use command clocks. Case medians require all registered repetitions to pass. Scaling pairs the corresponding repetition, including repeated seeds, on native-certified same-input/sample fits. A bounded sample does not establish a statistical guarantee.

Completed 9/9 registered pairs.

| Configuration | Candidate/baseline min / median / max | Candidate/stock min / median / max |
|---|---:|---:|
| ols-control-confirmation | 0.947 / 1.000 / 1.044 | 1.410 / 1.545 / 1.649 |

ols-control-confirmation: selected native baseline / candidate teams = 8 / 8.

NR retains missing results, application failures and scientific discrepancies in the JSON; it is never a timing win.

## Native thread scaling

| Lower → higher setting | Higher/lower min / median / max | Slower on every repetition |
|---|---:|---|

Independent-oracle failures: 0.

Per-fit command/process clocks, sampled RSS, failure reasons, all seed ratios and source-file bindings are retained in the JSON.

Job **7762812** completes all **27 headline applications**, **nine native** and
**nine default-stock** scientific comparisons. This repeats the registered
N61,440/K10/T8 two-FE OLS seed202609284 bytes; it is confirmation, not a fresh
holdout. Both native arms publish the requested eight-worker team. IID inference
is matched, with no optional saved observation outputs. Candidate snapshot
`1f290d1c…` / Linux plugin `8e2d5af2…` and baseline `e9d9f706…` are unchanged.

Candidate/baseline paired median is **1.000**, spanning **0.947–1.044**. There
are four clearly faster pairs, four clearly slower pairs and one 191 ms clock
tie. The exact exported binary floats make that tie's ratio 1.0000000000000007;
the machine-readable receipt preserves it. The initial three-pair 5.5% loss
does not persist. There is no basis for additional OLS policy or ABI tuning.
Candidate/default-stock is **1.545**, spanning **1.410–1.649**: the underlying
small OLS loss remains. Median native process RSS is 0.989 of baseline and
0.344 of stock. These process peaks are sampled, not native allocation bounds.

Only after all headline fits ended, three separate perf processes ran 20 OLS
baseline fits, 20 OLS candidate fits and 12 small two-FE PPML candidate fits.
All **52 diagnostic fits** converged; application and perf terminal checks
pass, with zero lost samples. These are full-public-process user-cycle samples,
including input loading and output extraction. Diagnostic clocks never enter
timing ranks. The profiles use `cycles:u`, 499 Hz, and list self shares at or
above 0.5%; listed shares need not sum to 100%.

| Listed sampled self cycles | OLS baseline | OLS candidate | Small PPML candidate |
|---|---:|---:|---:|
| Stata executable | 61.78% | 62.89% | 49.24% |
| Native plugin | 21.03% | 21.02% | 16.94% |
| libm FMA entry/return | below listing threshold | below listing threshold | 10.14% |

ELF mapping and recorded build IDs resolve the hot Stata offsets to a polling
region containing `pause`, shared-state loads and `rdtsc`. Perf's unresolved
executable offsets require the ELF 0x400000 load bias; directly disassembling
the raw offsets would identify the wrong code. This supports a host-waiting
hypothesis, but does not establish wall-time savings or authorize assigning
those CPU shares to avoidable native work. No host setting changed in this
measurement. The earlier passive-wait slope probe reduced visible host activity
without establishing a timing benefit; a bounded matched test is still needed.

The PPML profile separately places 5.90% in `normalized_product`, alongside
10.14% at the out-of-line hardware FMA entry/return. The latter is hardware FMA,
not software emulation. The helper serves both current-weight finite Gram
construction and speculative canonical reduced-CG matrix products. This profile
has insufficient unwind information to divide those callers. Any proposed
portable optimization must preserve the independent Gram/certificate authority;
the sampled percentages are not a prediction of command-time gain. Native OLS
profiles instead emphasize parallel group means and norm scans, with similar
shares in both artifacts. No candidate-specific kernel regression is identified.

Accounting is clean: **123 s**, 28 isolated slots on scc-wo4, **1.998G maxvmem**,
failed=0 and exit_status=0. Rotation, profile and wrapper terminal sentinels
pass. Raw licensed logs/disassembly are not committed; sanitized profile
receipts and binary/evidence hashes preserve attribution.

Together with the initial and fresh blocks this supports bounded-development
acceptance of ADR 0233. It does not meet the universal 1.20 stock ceiling or
complete the larger repair plan. Small OLS/PPML, low-memory OLS, original
campaign discrepancies and broader fresh scaling remain open.
