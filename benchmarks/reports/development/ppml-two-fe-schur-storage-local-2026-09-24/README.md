# Two-FE Schur and coefficient-storage experiment closed

Local macOS arm64 development evidence, 2026-09-24. **Reject both experimental
organizations for production** under [ADR 0164](../../../../docs/adr/0164-two-fe-schur-storage-closeout.md).
Restore the accepted `a1bf9ce0` computation. Kernel improvements do not justify
mixed public results, consistent unbalanced regressions and additional memory.
The whole-public PPML <=0.90-times-comparable-stock goal remains unfinished.

## Sources, work and receipts

Accepted baseline is `threeschur3`, computational revision `a1bf9ce0`, plugin
SHA-256 `a8366911510287757fb4644ff47ab04030521991a2067c3964abbf8c52ef4df0`.
Rejected final candidate **`twodense4`** is that revision plus the captured
six-file overlay, plugin
`8d84a7eff0a547226001cdd1a2515abcbca583e868a4c134c0fb1287311fd8b7`.
All 547 candidate computational inputs matched before restoration. All 546
accepted inputs match after restoration; the candidate-only fixture is archived,
not left in the production test tree. No candidate numerical or routing change
is adopted, and no new OLS performance claim is made.

Stock is pinned unmodified fixest 0.14.2, archive
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Comparable stock calls use `glm.tol=1e-10`, `fixef.tol=1e-11`, matching requested
threads, serial BLAS, robust inference and fitted means. The public command and
requested prediction are timed; process startup, data import, warmup and exports
are outside both clocks. Low is native execution mode, not a matched physical
cap on stock. Kernel diagnostics are separate from these stock headline clocks.
No compilation, profile, archive or competing fit overlaps headline timings;
all recorded overlap and monitor-error lists are empty.

The [content-addressed archive](receipts-content-addressed.zip) contains **11,753
logical receipts**, 2,446 payloads and 17,724,293 bytes, SHA-256
`8a8d887e7c204a436070ed99439ad1a5a16f033c549751d7c5aaf9cfbbcf9f57`.
The [restore tool](restore_receipts.py) verified and restored every logical file.
It includes five complete artifact sources/identities, two isolated diagnostic
sources, source patches, all declarations, builds, test results, public exports,
audits and failure receipts. Data/binaries and raw licensed logs are omitted
with hashes; machine-specific paths are sanitized. Prior receipts are unchanged.

## Coefficient-backed rows: measured and rejected

The first probe takes identical final-weight frames from development seed
202609347: narrow N12000/K3, disconnected N20160/K12 and wide N16320/K32.
It compares production scratch rows with original controls minus retained FE
coefficients. Reconstruction, moment arithmetic and scan results are bit-identical.
All **50 independent SciPy sparse-LSMR projection/score checks pass**.

Seven paired diagnostic rotations at T1/T4/T8 show mixed access costs. At T8 the
wide moment calculation improves 5.572 to 3.859 ms, but its row scan increases
0.950 to 1.996 ms. At T1 both lose: moments 3.472 to 4.344 ms and scan 0.915 to
1.896 ms. Disconnected T8 moments lose 2.237 to 2.989 ms and scan 0.494 to
0.876 ms. Narrow T4 moments improve 0.317 to 0.170 ms, scan loses 0.120 to
0.130 ms. Full distributions and setup times are in `storage-summary.json`.

Wide scratch construction takes about 2.16 ms at T8 and stores 4,177,920 bytes;
the coefficient bank stores 736,032 bytes. Avoiding that write does not prove a
whole-fit gain when reconstruction repeats at each downstream scan. Do not
adopt the row-view replacement or start threshold tuning from this evidence.
This does not reject a materially different algebraic product organization.

## Dense two-FE checkpoint: kernel versus public organization

The second probe generalizes the accepted centered three-FE reduced matrix to
two partitions. It eliminates the largest partition, prepares weighted group
covariances once per call, solves columns independently and reconstructs from
original inputs before the unchanged full fresh certificate. With D partitions,
require q² <= 2DN; two FE therefore require q² <= 4N. Checked, fallible temporary
allocations consume the checkpoint LSMR allowance, with no cross-weight reuse.
Unadmitted two-FE shapes retain matrix-free Schur; numerical rejection returns
to original-input LSMR. No tolerance or acceptance invariant changes.

The frozen-input diagnostic uses widths 1, min(10,K+1), K+1 and T1/T4/T8,
seven rotations. Narrow width4/T4 improves 1.668 to 1.068 ms; wide width33/T8
improves 9.247 to 7.475 ms, while wide width33/T1 barely moves 11.568 to
11.521 ms. Disconnected q=336 refuses the structural rule rather than being
ranked. All **37 independent SciPy projection/score checks pass**. More reduced
CG iterations can replace fewer expensive observation operators, so nominal
iteration counts are not equivalent work units.

Public low two-FE initially did not enter this checkpoint owner. The integrated
candidate admits its full optional LSMR allowance while preserving the ordinary
joint owner and prior affordable route if the extra budget does not fit. It also
reuses already established connected topology instead of recomputing components;
no weighted authority is reused. This complete low-mode route, publication
validation and memory-audit identity are tested in the final candidate.

## Public development and fresh distributions

Development seed **202609347**, three alternating baseline/candidate/stock
rotations. Values are all repetitions, milliseconds:

| Case | Accepted baseline ms | Rejected candidate ms | Stock ms | Candidate/stock |
|---|---|---|---|---:|
| Wide low 2FE N16320/K32/T8 | 144;132;135 | 137;135;125 | 92;90;89 | 1.500 |
| Narrow fast 2FE N12000/K3/T1 | 49;49;48 | 45;44;41 | 38;39;39 | 1.128 |
| Disconnected fast 2FE N20160/K12/T4 | 69;67;70 | 70;70;68 | 66;69;66 | 1.061 |
| Unbalanced fast 3FE N23040/K8/T4 | 80;83;81 | 83;81;80 | 87;88;87 | 0.931 |
| Low 3FE N14400/K7/T4 | 61;61;60 | 62;60;60 | 122;123;122 | 0.492 |
| Two slope partitions N16320/K6/T4 | 70;69;71 | 69;69;69 | 65;72;66 | 1.045 |
| Unbalanced low 2FE N16320/K12/T4 | 70;70;73 | 72;71;73 | 45;47;44 | 1.600 |

Narrow development improves 49 to 44 ms, but wide low ties at 135 ms and
unbalanced low loses 70 to 72 ms. The narrow gain is not confirmed below.

Fresh seed **202609376 is consumed/frozen**, five predeclared rotations. All
original cases remain; the low-storage counterpart of unbalanced two FE was
added before first fresh execution to confirm the development loss. Candidate
identity updates and that prospective amendment are preserved separately.

| Case | Accepted baseline ms | Rejected candidate ms | Stock ms | Candidate/stock |
|---|---|---|---|---:|
| Wide low 2FE N18720/K28/T8 | 124;123;124;123;122 | 122;119;122;120;122 | 84;79;85;81;85 | 1.452 |
| Serial low 2FE N12960/K16/T1 | 84;83;84;84;84 | 81;85;83;84;83 | 122;123;120;121;119 | 0.686 |
| Narrow fast 2FE N14160/K5/T1 | 60;54;60;56;55 | 55;56;56;57;56 | 58;50;51;52;56 | 1.077 |
| Unbalanced fast 2FE N24480/K12/T4 | 72;74;71;75;73 | 78;78;76;76;78 | 56;61;61;61;59 | 1.279 |
| Disconnected fast 2FE N22560/K13/T4 | 77;77;78;79;81 | 80;81;83;80;83 | 71;68;72;74;75 | 1.125 |
| Weak fast 2FE N9840/K5/T1 | 37;40;41;41;42 | 40;42;38;35;38 | 770;772;776;766;762 | 0.049 |
| Low 3FE N16320/K9/T4 | 73;74;74;72;74 | 74;74;73;74;76 | 191;192;193;183;184 | 0.387 |
| Two slope partitions N17760/K8/T4 | 78;77;78;77;77 | 79;79;77;79;76 | 93;86;91;85;90 | 0.878 |
| Unbalanced low 2FE N24480/K12/T4 | 88;89;89;91;92 | 97;100;101;96;97 | 58;56;59;60;59 | 1.644 |

Wide low improves only 123 to 122 ms (0.8%); its five paired changes are
-2,-4,-2,-3,0 ms. Narrow medians tie at 56 ms. Unbalanced fast loses 73 to 78 ms,
with every pair worse by 1–6 ms. Unbalanced low loses 89 to 97 ms, every pair
worse by 5–12 ms. Disconnected loses 78 to 81 ms, every pair worse by 1–5 ms.
The slope control loses 77 to 79 ms, though both medians beat the 0.90 stock
target. Weak and serial-low gains do not compensate for the other losses.
These distributions reject broad adoption without attributing all small changes
to a particular kernel. No benchmark-specific guard is fitted afterward.

## Science, integration failures and memory

On the final candidate, **198 public applications**, **198 independent public-state
checks**, **66 native comparisons** and **66 direct candidate-stock comparisons**
pass. No scientific discrepancy, timeout or application failure remains in these
two final-source blocks. Public-state checks independently establish sample,
original-design scores, model-space feasibility and objective replay; they are
not full covariance oracles for every large cell. Separate explicit-design
Poisson/Hessian/sandwich fixtures cover that boundary.

Across final blocks, maximum native absolute differences are coefficient
5.107e-13, covariance 1.001e-17 and fitted means 1.041e-10; likelihood and
deviance exports are equal. **68 distinct focused Rust tests** pass for the final
core/plugin changes. A separate full-width/weight/T1/T4/T16 fallback control
passed on the preceding `twodense2` core, before low admission and connectivity
reuse; it is not represented as an exact-final-source sweep.

Seven final licensed invocations pass their process/Stata codes and terminal
sentinels: six repository fixtures plus the exact failed-warmup replay. The new
fixture covers 24 low-memory combinations of two/three FE, T1/T4, unit/FW/PW,
robust/cluster, offset and mu/eta/xb/d/residual outputs against explicit Stata
Poisson and separately formed Hessian/sandwich results. Existing categorical,
slope, one-FE, joint-product and allocation fixtures also pass. These are bounded
Mac development checks, not Linux, physical-cap or release qualification.

Earlier integration failures remain visible:

- The first diagnostic build missed test fixtures and a private module export;
  the repaired isolated source is separately identified.
- The first Schur harness incorrectly panicked on valid structural refusal;
  its successor records refusal explicitly, without changing the algorithm.
- Test assumptions that zero iterations must reject a fully nested system,
  that a tiny accepted accelerator must report used LSMR, and that width65
  necessarily selects the LSMR reference were corrected. The last became an
  explicit test-only dispatch seam. Failed logs remain archived.
- `twodense2` passes 63 public runs but does not admit the low two-FE route.
  `twodense3` attempts 63 runs; six low two-FE warmups fail publication with
  r(3499), before timed targets. The plugin validator still recognized only
  three-FE low checkpoints. Final `twodense4` fixes this integration boundary
  and adds the dedicated public oracle. Failed runs are not ranked.

Memory costs also argue against adoption. Development wide selected bytes rise
23,749,880 to 58,087,176 (+34,337,296); tracked peak delta rises 21,025,668 to
23,901,468 (+2,875,800), with scratch unchanged. Fresh wide selected bytes rise
25,717,193 to 65,090,329 (+39,373,136). Fresh unbalanced low rises 23,878,251 to
39,990,971. Optional refusal retains the old route, but conservative admission
still consumes substantial headroom. This is not a physical-cap claim.

Development wide first-run strict-final time falls 41.45 to 34.71 ms while
ordinary projection is 38.08 versus 36.78 ms. On fresh wide, strict-final falls
35.94 to 31.81 ms, while projection remains 34.49 versus 34.63 ms. On fresh
unbalanced low, projection rises 21.40 to 25.88 ms and strict-final 21.22 to
24.85 ms. These exported phase spans do not explain every whole-command
millisecond and must not be added to nested diagnostic timings. Faster frozen
projection is not sufficient to reach the public target.

## Consequence and next implementation target

Production computation is restored byte-for-byte to the accepted checkpoint.
A further OLS timing block would not validate adopted code because none is
adopted; previous source-bound OLS controls remain as recorded. Frozen 77/16/111
definitions, scientific envelopes, static allocation-inventory drift and known
constrained-fit/CG failures are unchanged. No SCC retry or remote action occurs.

The new baseline observations provide a concrete per-family work budget:

| Accepted source, fresh case | Current ms | Stock ms | 0.90 target ms | Required reduction |
|---|---:|---:|---:|---:|
| Wide low two FE | 123 | 84 | 75.6 | 47.4 ms / 38.5% |
| Unbalanced low two FE | 89 | 59 | 53.1 | 35.9 ms / 40.3% |
| Unbalanced fast two FE | 73 | 61 | 54.9 | 18.1 ms / 24.8% |
| Disconnected fast two FE | 78 | 72 | 64.8 | 13.2 ms / 16.9% |
| Narrow fast two FE | 56 | 52 | 46.8 | 9.2 ms / 16.4% |

Serial low two FE (84/121), weak two FE (41/770), low three FE (74/191), and
the slope control (77/90) already beat 0.90 in this block. Preserve them. The
prior accepted report separately has unbalanced three FE at 95/100; its 5-ms
target residue is smaller than the losses above and belongs to that earlier
captured block, not these fresh designs.

Next investigate **joint reconstruction plus canonical product ownership** as
one lifecycle. The existing joint engine already forms reduced finite/FE blocks;
adding another wrapper around that algebra is not a new design. The prior
[source-bound attribution](../ppml-canonical-work-plan-local-2026-09-24/README.md)
places 10.796 ms in four streamed joint reconstructions, 9.817 ms in two moment
builds and 22.689 ms in eleven two-FE PCG calls on the wide development case.
Those spans are diagnostic, nested and nonadditive. The present experiment
shows that a standalone lazy row view or dense checkpoint extension is
insufficient. Measure and remove repeated passes across their consumers.

Two coherent candidate organizations warrant a bounded comparison:

1. Keep certified projected storage, but construct, normalize and consume it in
   admitted panels across canonical products and reconstruction. Give each
   current-weight frame one owner for compatible intermediates. Independent
   acceptance reads remain fresh; no retained recurrence grants authority.
   This concerns pass elimination and consumer ownership, not ADR 0159's
   rejected eight-row Gram tile or ADR 0157's scalar normalization hoist.
2. Test algebraic canonical products from raw weighted cross-products and FE
   projection coefficients, avoiding repeated evaluation of implicit within
   rows for every Gram entry. Cancellation, omitted coordinates and rank policy
   are first-class limits: retain canonical QR/span fallback and independent
   explicit-design checks. Materialize/stream certified final rows only where
   inference actually needs them. Do not assume normal equations can replace
   rank or residual certificates.

For both, map every required invariant to its authoritative current-frame read.
Any redundant check removal needs an explicit error/equivalence argument,
successor ADR and adversarial tests; required fresh score, objective, projection,
rank, reconstruction and final-Hessian conditions cannot be weakened. A changed
weight invalidates numerical products; a prior solution remains only a guess.

Aim for 35–45% whole-command reduction on the wide/unbalanced low anchors,
20–30% on unbalanced fast, and about 20% on narrow/disconnected. These are
engineering targets, not forecasts. Limit the milestone to the two materially
different candidates, require early public evidence, then move on if the full
pipeline does not improve. Do not keep tuning this rejected checkpoint on seed376.

After a useful change, confirm untouched fresh designs with five rotations,
independent WLS/Poisson/Hessian checks, matched stock work and explicit loss
reporting. Broaden adaptively to larger N, four-plus FE, multiple slopes,
generated/wide controls, clustered/full outputs, changing/extreme weights,
rank-deficient/disconnected/weak designs, threads and memory refusal. Protect
categorical/slope OLS and one-FE PPML. Adjudicate scientific discrepancies before
ranking, and keep unresolved historical cases unranked. The goal is per-family
practical speed, not an aggregate win rate that conceals remaining losses.
