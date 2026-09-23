# Bounded retained-scratch readers: accepted development milestone

Local macOS arm64 development evidence, September 23, 2026. Accept `scrread1`,
based on `73c73dfc`; all **534 computational inputs** match the measured source.
Plugin SHA-256:
`e6d3bd7035556bb77d501b4aa5de2a030002af10068c32b97ce0913e1ba46c4f`.
Baseline is accepted `oneprof6`, matching `4ad7ade5`, plugin
`c1aa681c6c160d60381d8483b783a1bbed057c63d4f0d81c89f9e02485ac67b3`.
This completes one bounded milestone, not the broader PPML performance goal.

## Mechanism

[ADR 0129](../../../../docs/adr/0129-retained-scratch-read-cursors.md) adds an
immutable positional-read cursor for retained finite QR, its direct dropped-span
certificate and fresh/frozen residual reconstruction. Reuse one 8 KiB encoded
page across rows. Existing finite moment tiles already batch reads under ADR
0070 and remain unchanged. Inference still uses its previous row reader.

Every requested value is decoded and validated in the original order. Weight
normalization, rank, QR rotations, finite solve, residual FMA expressions and
all acceptance conditions are unchanged. The immutable borrow excludes owner
mutation; the cursor cannot survive a weight/design update. Failed page loading
revokes its length before I/O. A 519-row, six-stored-column QR test verifies
four positional reads, with exactly the same weighted QR products and rank as
the dense source, including a deficient design.

## Public timings

Stock is the unmodified pinned fixest 0.14.2 archive, SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
using `glm.tol=1e-10`, `fixef.tol=1e-11`, matched application threads,
single-thread BLAS and the established comparable robust normalization.
Public timers include loading/preparation, estimation, robust inference,
fitted means and publication. Import, process startup, warmup and exports are
outside. No compilation, profile, archive or competing fit overlaps occurred.
Low-memory native versus ordinary stock is not a matched-memory comparison.

Development seed **202609327**, three rotations. Milliseconds are median
[minimum, maximum]; the fast slope row is a control.

| Case | Baseline | Candidate | Stock |
| --- | ---: | ---: | ---: |
| Unbalanced three FE slopes N30720 K6 T4 low | 344 [343,355] | 217 [215,226] | 116 [115,119] |
| Larger slopes N61440 K10 T8 low | 731 [726,748] | 503 [483,515] | 229 [226,231] |
| One FE N61440 K10 T4 low | 450 [422,460] | 264 [247,268] | 54 [53,55] |
| Sparse two categorical FE N61440 K10 T4 low | 680 [656,694] | 449 [431,457] | 133 [131,135] |
| Unbalanced categorical three FE N30720 K10 T1 low | 762 [759,763] | 659 [656,667] | 320 [316,321] |
| Two slope partitions N30720 K6 T4 fast | 165 [163,166] | 166 [165,166] | 115 [115,119] |

Fresh confirmation, seed **202609328**, declared before development results,
three rotations. All rows use native low memory. No candidate source changes
were made between development and confirmation.

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Unbalanced three FE slopes N23040 K6 T4 | 251 [243,268] | 162 [160,171] | 94 [92,95] | 1.72 |
| Larger slopes N46080 K12 T8 | 794 [770,806] | 593 [569,596] | 942 [942,943] | 0.63 |
| One FE N61440 K16 T4 | 467 [443,477] | 325 [310,339] | 110 [108,112] | 2.95 |
| Sparse two categorical FE N30720 K10 T1 | 477 [475,479] | 359 [358,360] | 193 [193,197] | 1.86 |
| Unbalanced categorical three FE N23040 K8 T4 | 330 [304,342] | 242 [237,244] | 90 [89,91] | 2.69 |
| Multiple slopes N7680 K6 T4 | 100 [95,103] | 64 [64,64] | 54 [49,57] | 1.19* |

Fresh native gains are **35.5%, 25.3%, 30.4%, 24.7%, 26.7%, 36.0%**. All large
public states and pairs pass. The wider slope cell clears the 0.90 target on
this source/design; most cells still lose. *The smaller independent multiple-slope
stock pilot fails covariance accuracy. Its family remains unresolved; a passing
large pair does not erase that discrepancy. The disconnected stock pilot also
fails convergence and has no ranked time. Do not infer universal family wins or
pool failures away.

## Attribution and remaining costs

Medians of native phase records on the fresh inputs, baseline/candidate ms:

| Case | Ordinary FE projection | Finite work | Strict-final | Inference |
| --- | ---: | ---: | ---: | ---: |
| Unbalanced slopes K6 | 59.34 / 58.93 | 78.01 / 11.16 | 53.75 / 32.87 | 12.10 / 11.72 |
| Wider slopes K12 | 312.81 / 312.32 | 213.67 / 51.91 | 135.14 / 92.77 | 31.86 / 34.87 |
| One FE K16 | 62.94 / 62.73 | 130.66 / 35.66 | 142.15 / 85.67 | 46.87 / 54.74 |
| Sparse categorical T1 | 174.80 / 171.10 | 112.90 / 29.00 | 110.69 / 83.12 | 15.85 / 15.30 |
| Unbalanced categorical K8 | 115.12 / 114.35 | 67.34 / 10.26 | 78.79 / 54.47 | 17.89 / 18.66 |
| Multiple slopes | 21.37 / 21.05 | 30.61 / 4.32 | 17.72 / 10.18 | 4.14 / 3.88 |

These phase medians are not additive decompositions of the public median. They
locate the improvement in finite/strict work. Projection remains nearly unchanged.
Inference was not optimized and some phase medians increase; preserve those
losses. It remains a separate per-row scratch consumer. The next substantial
algorithmic target is joint finite-plus-FE ordinary solving, especially the wider
slope projection cost, followed by shared final-state construction. One-FE low
execution still excludes the concentrated fast owner and needs its own ownership
and final-acceptance redesign. This is concrete remaining work, not parity.

## Science and controls

**202 distinct Rust tests** pass on the reviewed source: 16 scratch, 25 finite
kernel, 156 prepared PPML and five linear-session/reconstruction checks. New
tests cover positional-read counts, signed zero, backward/empty/overflow requests,
post-scan writes, widths above a page, concurrent independent readers, nonfinite
bytes, truncated reads, selected columns, unequal weights and deficient rank.
Existing independent reconstruction and current-frame/session tests remain green.

Actual-plugin ABI and panic-containment checks pass. Four licensed fixtures pass
process rc=0, Stata rc=0 and terminal sentinels: a captured low-memory adaptation
of the multipartition slope explicit-design oracle, saved slopes, separation and
OLS memory modes. The low adaptation changes only the memory option and sentinel;
it independently checks unit/FW/PW coefficients, fitted means and current-Hessian
robust/cluster covariance with offsets. Its exact script/hash is archived.

All native independent pilots pass. The six fresh pilots add multiple slopes,
four FE, weak connectivity and disconnected components. Stock's multiple-slope
pilot fails two covariance coordinates: maximum absolute error
**4.13753946e-10**, tolerance ratio **1.60028014**. Its other oracle coordinates
pass. The disconnected pilot reports **nonconvergence**; its elapsed time is
preserved only as a failed application. No tolerance, solver pin or sample was
changed. Earlier reference discrepancies remain unresolved.

Across development, fresh and control blocks there are **184 applications**:
183 pass and one stock application reports nonconvergence. No overlap or monitor
errors occur. Scientific verdicts remain separate. All **69 native pairs** retain
exact scientific exports. Of these, 23 are fresh excluded/control pairs, including
eight public OLS comparisons across slopes, low-memory categorical, one-FE
robust/full outputs and low-memory factors.

Fresh excluded baseline/candidate medians, two pairs: dense categorical three
FE **188/192**, one FE **130.5/127.5**, sparse two FE **238/234**, unbalanced
three FE **180/178**, fast slopes **165.5/163** ms. The dense loss receives five
additional pairs on identical full/warm numerical hashes: **194 [189,195] /
195 [194,196]** ms. Preserve both blocks; a small residual loss remains amid
overlapping ranges. OLS controls give **69/69**, **65.5/64**, **17/17**, **65.5/65**
ms. This bounded evidence does not establish universal non-regression.

An attempted formatting-only cleanup used a mutating grouped-diff operation
and briefly truncated unchanged file edges. Compilation failed before any
artifact or timing. Restoring the exact original edges and inspecting the diff
removed unrelated formatting; the final reviewed source passes all checks.
The failure log is preserved alongside initial and reviewed passing logs.

## Memory and receipts

No heap owner, file or worker is added. One immutable scan retains an 8 KiB stack
page plus a descriptor; its lifetime can overlap other stack frames under existing
headroom. Tracked heap peaks match baseline exactly in every fresh application:
**18,275,751; 44,224,224; 39,061,005; 18,509,640; 18,227,245; 6,401,541 bytes**,
in table order. Modeled peaks and selected memory policies are unchanged. This
is not physical-cap qualification or a complete stack-accounting claim.

Static allocation-inventory drift, constrained-fit failures, nonretaining-CG
instability, the obsolete allocation-fixture assertion, prior comparator issues
and Linux/SCC/physical-cap deferrals remain open. Frozen 77/16/111 definitions
are unchanged. No push, release, SCC retry or stopped campaign restart occurred.

The restored immutable container verifies **6,564 logical receipts**, **1,928
distinct payloads**, **17,528,786 bytes**, SHA-256
`00e2f4779eebc0723c2c10e1f8e74262794cb3305d587779d4aff2efdb561b75`.
It preserves declarations, artifact/source identities, exact source snapshots,
controllers, tests, failures, distributions, scientific audits and data hashes.
Raw licensed logs, binaries and generated datasets are excluded. Restore into a
new directory with `restore_receipts.py`; `verification.json` records successful
restoration and source matching.
