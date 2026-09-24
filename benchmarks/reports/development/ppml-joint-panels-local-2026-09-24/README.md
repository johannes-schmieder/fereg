# Bounded PPML reconstruction and scratch moments adopted

Local macOS arm64 development evidence, 2026-09-24. Adopt **`jointpanels2`**
under [ADR 0165](../../../../docs/adr/0165-bounded-ppml-reconstruction-and-moment-panels.md).
Fresh wide low PPML improves **17.1%**, unbalanced low **8.7%**, and low-memory
slopes **6.1%**, with unchanged measured heap peaks, modeled memory and scratch
size. The whole-public <=0.90-times-comparable-stock goal remains unfinished.
Small excluded-path losses are retained below; no universal non-regression claim.

## Exact sources, comparison and receipts

Baseline **`threeschur3`** is the accepted `a1bf9ce0` computation (unchanged by
`d482395e`), plugin SHA-256
`a8366911510287757fb4644ff47ab04030521991a2067c3964abbf8c52ef4df0`.
Reconstruction-only **`jointpanels1`** is `d482395e` plus the captured `joint.rs`
overlay, plugin
`a0ac01d44b582d328ed1facb710b2b875aab75a61f1b5182e27f99f40a80d496`.
Final **`jointpanels2`** adds the captured `finite_wls_kernel.rs` overlay, plugin
`4d4c669d6716583b14be437e2732133d61f733840b610414429d8df5614168b1`.
All **546 computational inputs** match the adopted worktree before this report's
commit. Complete sources, overlays, build logs and identities are archived.
These identities, rather than later source revisions, govern the timing claims.

Stock is pinned, unmodified fixest **0.14.2**, source archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Comparable stock uses `glm.tol=1e-10`, `fixef.tol=1e-11`, matched requested
threads, serial BLAS, robust inference and fitted means. The public command and
requested prediction are timed; startup, import, warmup and exports are outside
both clocks. Low is native execution mode, not a matched physical memory cap.
No compilation, profiling, archiving or competing fit overlaps headline runs;
all overlap and monitor-error arrays are empty. All applications succeeded;
there were no timeouts in these timing blocks.

The [content-addressed archive](receipts-content-addressed.zip) contains **10,298
logical receipts**, 2,384 payloads, 16,524,381 bytes, SHA-256
`372c27c2a48c780c440d046a2af36e4913b9701044bee1ec03afccc97345dfc4`.
The [restore tool](restore_receipts.py) verified and restored all logical files.
It includes three complete artifact source snapshots, declarations, controllers,
implementation scripts, builds, passing and failing test logs, fixture protocols,
public exports, audits and `summary.json`. Data/binaries and raw licensed logs
are omitted with hashes; machine-specific paths are sanitized. All recorded
builds, tests, fits, audits and archive/restore processes are terminal.

## Computational change

The preceding rejected storage/checkpoint experiment did not justify replacing
scratch with lazily reconstructed rows or rebuilding dense two-FE checkpoint
geometry per call. This candidate instead changes two existing consumers without
introducing a new solver, cache lifetime or extra memory bank.

1. **Original-column reconstruction and scores.** Where all selected original
   columns expose immutable slices, reconstruct disjoint 1,024-row panels on the
   existing worker team. Each panel streams contiguous source columns; each
   residual keeps its original selected-column FMA order. Validate complete
   direct access before any write; recipes/dense sources without it retain their
   existing path. After eliminated-effect projection, independent fresh finite
   scores scan original columns concurrently, keeping each score's row order.
   This mainly helps low-mode numeric joint PPML; it adds no normalized N-by-K
   bank, thread team or scratch allocation.
2. **One scratch read/standardization per observation.** Where the existing
   row-tile workspace fits multiple complete Gram/RHS partials, partition rows
   instead of assigning Gram rows that each rescan scratch. For selected width D,
   stored width S and admitted B elements, each partition occupies D²+D+S. Let
   P=min(floor(B/(D²+D+S)), existing partitions, N). Require P>=2 and
   `ceil(N/P)*(D*(D+1)/2+D) < N*(L*(2*D-L+1)/2+L)`, where L is the old
   columns-per-partition value. This is a conservative multiply-add critical-path
   comparison, not a fitted timing threshold. If it refuses, keep the old method.
   Checked dimensions and the existing immutable 8-KiB reader bound storage.

The scratch route changes the summation tree: consecutive row partials merge
with the existing compensated reduction in fixed partition order. Each row
retains the same normalized weight, column identity, scale and FMA products.
ADRs 0020/0138's accumulation organization is superseded only for this admitted
route. Their ban on an independently allocated partitions-by-K² bank remains:
all partials fit the already charged tile bank. Dense/implicit moments, one-shot
OLS and declined scratch paths retain their existing calculation.

Reordered kernel tests use an explicit forward-error envelope for both sums and
the compensated merge, bounded by `4*N*epsilon` times column/response norms.
Dense and declined paths still require exact recurrence equality. No runtime
acceptance tolerance or public comparison envelope changes. Current-weight
projection, model-space, canonical rank/omission, finite solve/inverse, fresh
original-input score, objective, reconstruction and final-Hessian checks remain;
none is removed or rescheduled. No weighted state gains cross-frame authority.
Weight semantics, sample/separation, DoF and transactional failure behavior remain.

## Development distributions

Development seed **202609347**. Three rotations; every repetition is shown in
milliseconds. B is `threeschur3`, J1 reconstruction-only, C final `jointpanels2`.
First screen, 63 applications, all public scientific exports exactly equal
between B and J1; all 21 direct stock comparisons pass:

| Case | B ms | J1 ms | Stock ms |
|---|---|---|---|
| Wide low 2FE N16320/K32/T8 | 137;137;140 | 130;129;128 | 85;88;89 |
| Narrow fast 2FE N12000/K3/T1 | 45;43;45 | 43;45;44 | 37;37;35 |
| Disconnected fast 2FE N20160/K12/T4 | 72;72;72 | 71;72;73 | 63;64;64 |
| Unbalanced fast 3FE N23040/K8/T4 | 84;82;83 | 84;84;83 | 85;83;83 |
| Low 3FE N14400/K7/T4 | 62;61;63 | 61;61;62 | 117;116;118 |
| Two slope partitions fast N16320/K6/T4 | 71;71;72 | 71;71;72 | 63;62;65 |
| Unbalanced low 2FE N16320/K12/T4 | 72;72;72 | 69;68;69 | 41;43;41 |

Reconstruction alone improves wide medians 137 to 129 ms and unbalanced low 72
to 69. The subsequent four-way screen isolates the additional moment change:

| Case | B ms | J1 ms | C ms | Stock ms |
|---|---|---|---|---|
| Wide low 2FE N16320/K32/T8 | 138;137;143 | 131;130;133 | 121;122;124 | 86;86;86 |
| Narrow fast 2FE N12000/K3/T1 | 44;44;42 | 44;44;44 | 44;44;43 | 38;36;37 |
| Disconnected fast 2FE N20160/K12/T4 | 71;72;73 | 71;72;74 | 72;72;72 | 63;61;65 |
| Unbalanced fast 3FE N23040/K8/T4 | 84;83;83 | 84;87;85 | 84;87;86 | 87;85;84 |
| Low 3FE N14400/K7/T4 | 62;62;62 | 62;62;62 | 60;61;60 | 118;126;120 |
| Two slope partitions fast N16320/K6/T4 | 75;72;72 | 73;71;72 | 72;71;73 | 61;64;64 |
| Unbalanced low 2FE N16320/K12/T4 | 72;73;74 | 70;69;70 | 67;66;68 | 42;44;41 |

Wide medians improve 138→131→122 ms and unbalanced low 73→70→67 ms. The
excluded unbalanced fast three-FE loss, 83→86 ms, triggers a prospective fresh
control. No benchmark-specific exclusion is introduced in routing.

## Fresh confirmation and losses

Fresh seed **202609377 is consumed/frozen**, five matched rotations. The initial
nine cases were declared before execution; after observing the development
three-FE loss, a tenth corresponding excluded-path case was added **before any
fresh run**, preserving the original declaration and every other case. No tuning
uses this seed afterward. All scientific comparisons below pass; historical
unresolved comparisons remain at their original source identities.

| Case | B ms | C ms | Stock ms |
|---|---|---|---|
| Wide low 2FE N19200/K30/T8 | 142;153;154;146;141 | 119;121;120;137;129 | 87;89;89;89;89 |
| Serial low 2FE N13440/K18/T1 | 96;99;99;97;99 | 95;93;96;94;95 | 137;137;139;139;138 |
| Unbalanced low 2FE N24960/K14/T4 | 104;102;105;109;102 | 98;95;95;93;94 | 64;63;63;67;62 |
| Low 3FE N16800/K9/T4 | 73;74;73;74;74 | 73;73;70;72;73 | 279;277;278;277;283 |
| Two slope partitions low N18240/K10/T4 | 98;98;100;99;98 | 94;92;92;91;91 | 94;100;98;94;95 |
| Excluded narrow fast 2FE N14640/K5/T1 | 54;54;52;53;53 | 54;54;55;55;54 | 54;52;51;52;53 |
| Excluded disconnected fast 2FE N23040/K13/T4 | 90;82;89;87;83 | 86;85;83;86;84 | 78;79;76;77;77 |
| Excluded weak fast 2FE N10080/K5/T1 | 38;38;35;38;39 | 38;37;36;37;38 | 907;903;902;907;908 |
| Excluded two slope partitions fast N18240/K8/T4 | 84;81;82;82;83 | 83;82;83;81;82 | 90;90;91;90;92 |
| Excluded unbalanced fast 3FE N25440/K10/T4 | 90;90;90;90;91 | 90;93;92;91;92 | 97;95;96;93;96 |

Medians and remaining target distances (target is 0.90 times stock):

| Case | B ms | C ms | Stock ms | C/stock | Further saving to target ms |
|---|---:|---:|---:|---:|---:|
| Wide low 2FE N19200/K30/T8 | 146 | 121 | 89 | 1.360 | 40.9 |
| Serial low 2FE N13440/K18/T1 | 99 | 95 | 138 | 0.688 | 0.0 |
| Unbalanced low 2FE N24960/K14/T4 | 104 | 95 | 63 | 1.508 | 38.3 |
| Low 3FE N16800/K9/T4 | 74 | 73 | 278 | 0.263 | 0.0 |
| Two slope partitions low N18240/K10/T4 | 98 | 92 | 95 | 0.968 | 6.5 |
| Excluded narrow fast 2FE N14640/K5/T1 | 53 | 54 | 52 | 1.038 | 7.2 |
| Excluded disconnected fast 2FE N23040/K13/T4 | 87 | 85 | 77 | 1.104 | 15.7 |
| Excluded weak fast 2FE N10080/K5/T1 | 38 | 37 | 907 | 0.041 | 0.0 |
| Excluded two slope partitions fast N18240/K8/T4 | 82 | 82 | 90 | 0.911 | 1.0 |
| Excluded unbalanced fast 3FE N25440/K10/T4 | 90 | 92 | 96 | 0.958 | 5.6 |

Every wide, serial-low, unbalanced-low and low-slope pair improves. Wide paired
changes are -23,-32,-34,-9,-12 ms; the 137-ms candidate tail remains visible.
Unbalanced low changes are -6,-7,-10,-16,-8; low slopes -4,-6,-8,-8,-7.

Excluded narrow fast loses 53→54 ms (pairs 0,0,+3,+2,+1), and excluded unbalanced
fast three FE loses 90→92 (0,+3,+2,+1,+1), confirming its small development loss.
Their cause is not established; direct-column/scratch organization does not
establish a causal explanation for dense excluded paths. Disconnected medians
improve 87→85, but individual pairs lose up to 3 ms. Fast slopes tie at 82,
with individual ±1-ms changes. These controls prevent a universal non-regression
claim. The reliable low-mode gains with unchanged storage justify adoption;
small residuals do not justify another tuning cycle on the frozen controls.

Serial low, weak fast and low three FE exceed the 10%-faster target in this
block. Low slopes, fast slopes and fast unbalanced three FE beat stock medians
but miss that margin. Wide, unbalanced low, narrow and disconnected still lose
to stock. Earlier source-bound wins do not transfer to these fresh cases.

## Independent science and observed failures

All **309 timed applications** succeed: 63 reconstruction-only, 84 four-way
combined development, 150 fresh PPML and 12 OLS controls. The two final-candidate
PPML blocks contain **234 public applications/public-state checks**, **71
final-candidate native pairs** and **71 direct candidate-stock comparisons**,
all passing. The four-way development block also includes intermediate J1 checks.
No discrepancy in these new final blocks is excluded from ranking.

Public-state checks independently establish samples, original-design scores,
model-space feasibility and objective replay; they are not full covariance
oracles for every large cell. Separate explicit-design fixtures establish the
Poisson/Hessian/sandwich boundary. Across final blocks maximum native absolute
differences are coefficient 1.10155e-16, covariance 1.00018e-17, means 2.13163e-14,
likelihood zero and deviance 7.27596e-12. Reconstruction-only exports are exact.

**102 distinct focused Rust tests** pass on the final computational source,
the union of `unit-moments-3.log` (26), `unit-irls.log` (48) and
`unit-joint-final.log` (33), with five overlapping names. Coverage includes
explicit weighted projection/finite-product oracles, rank/span boundaries,
weight extremes, joint frame invalidation, memory refusal, Poisson truth,
changing-weight iteration and direct-column no-write refusal. Reconstruction
tests exercise T1/T4/T8, panel tails 1/1023/1024/1025/2083, widths 1/7/33/65,
noncontiguous selection and scales 1e-200/1/1e200. Scratch tests cover serial,
parallel, selected/dropped columns, capacity admission/refusal, exact page-read
counts, immutable contents and the derived roundoff bound.

Six final licensed fixtures pass process code, Stata code and terminal sentinel
in `fixture-3-results.json`:

- `feppml_categorical_final_oracle.do`;
- `feppml_multipartition_slope_oracle.do`;
- `feppml_one_fe_dummy_oracle.do`;
- `feppml_joint_categorical_audit.do`;
- `feppml_joint_column_products_oracle.do`;
- archived `feppml_categorical_low_panels_oracle.do`.

The last is the existing categorical final oracle with low/MAP execution:
24 combinations of two/three FE, T1/T4, unit/FW/PW and robust/cluster, including
offset and mu/eta/xb/d/residual outputs, against explicit Poisson plus independent
Hessian/sandwich calculations. The joint-column fixture covers K8/N4800 low
slopes with T1/T4 and FW/PW/robust/cluster. The allocation fixture passes warmed
zero-event/growth and fresh/warm parity checks. Ordinary fixest weights are not
used as a frequency-weight inference oracle. These are bounded local development
checks, not a whole-suite or release qualification result.

Failures remain visible:

- The broader first joint test selector passes 33 and fails
  `zero_active_control_lsmr_uses_joint_state_but_reports_fused_identity` on the
  checkpoint-count assertion (actual 2, expected 3). An isolated run against
  accepted `threeschur3` fails identically (`unit-baseline-zero.log`). This
  pre-existing metadata-count discrepancy is unresolved, not silently repaired;
  the final selector explicitly skips it. No all-tests-green claim is made.
- The first moment test run has two failures from old exact-sum and old I/O-count
  assertions. Their successors check the changed accumulation order against the
  explicit roundoff bound and the new reader's exact count. An intermediate
  test-only integer-type compile error is also captured, then corrected.
- The first generated low fixture completes its science with Stata rc0 but its
  wrapper rejects a filename/sentinel mismatch. The file was copied to the
  matching fixture name and all six fixtures rerun successfully. The original
  failure is retained in `fixture-2-results.json`; no acceptance threshold changes.

## OLS controls, memory and phase observations

Frozen OLS control seed **202609374**, three alternating pairs. All six scientific
comparisons are exact. Categorical medians improve 18→17 ms; slope medians tie
at 15 with one +1-ms pair. No universal OLS speed claim follows.

| Case | B ms | C ms |
|---|---|---|
| Categorical 2FE N10080/K8/T4 | 18;18;18 | 18;17;17 |
| Two slope partitions N10080/K6/T4 | 15;15;15 | 15;15;16 |

All **71 final-candidate PPML pairs** have identical modeled selected bytes,
tracked peak deltas, scratch bytes, outer iterations and reported inner iterations.
No new explicit heap bank or scratch file is introduced; the established 8-KiB
reader replaces a same-sized worker read temporary. Representative fresh values,
identical between B and C:

| Case | Modeled selected bytes | Tracked peak delta bytes | Scratch bytes | Outer / reported inner |
|---|---:|---:|---:|---:|
| Wide low 2FE N19200/K30/T8 | 27,011,299 | 24,143,862 | 4,608,000 | 4 / 53 |
| Unbalanced low 2FE N24960/K14/T4 | 25,180,982 | 21,329,194 | 2,795,520 | 4 / 38 |
| Two slope partitions low N18240/K10/T4 | 20,077,725 | 17,140,450 | 1,459,200 | 7 / 96 |

First fresh-run exported phase observations, milliseconds B→C:

| Case | FE projection | Finite solve | Strict/final solve |
|---|---:|---:|---:|
| Wide low 2FE N19200/K30/T8 | 39.688→31.605 | 14.589→8.204 | 40.796→33.752 |
| Unbalanced low 2FE N24960/K14/T4 | 26.337→22.289 | 9.896→8.144 | 25.744→24.650 |
| Two slope partitions low N18240/K10/T4 | 33.817→30.989 | 9.199→8.396 | 16.392→15.121 |

These are observed phase spans, not an additive causal decomposition; nested
spans must not be summed. Full per-repetition phase/memory records are in
`summary.json`. Whole-process physical-cap and Linux confirmation remain open.
Static allocation-inventory drift is preserved, as are weighted constrained-fit
and nonretaining-CG span failures. Frozen 77/16/111 benchmark definitions remain
unchanged. No SCC retry, stopped campaign restart, push or qualification occurred.

## Next bounded target

The largest comparable residuals are wide and unbalanced low two-FE PPML:
**40.9 ms (33.8%)** and **38.3 ms (40.3%)** further savings to reach 0.90 stock.
Their projection/strict phases remain much larger than inference. Do not spend
the next cycle chasing the 1–7-ms target residues in the nearly competitive
slope and three-FE anchors.

Source inspection of `borrowed_map.rs::absorb_two_fe_pcg_reusing` identifies
repeated weighted group-total/RHS preparation; its compiled weighted-edge route
is guarded to OLS (`allow_ols_rhs_scratch`, `SchurWorkLimit::Ols`, team>=8).
PPML retains observation operators. This is a concrete lifecycle hypothesis,
**not measured attribution** of the remaining 38–41 ms. Count setup, operator
applications, row passes, active RHS work and certificates across all compatible
control tiles/response/checkpoints in one exact current-weight frame first.

If repetition is substantial, give that frame explicit ownership of reusable
weighted topology/operator geometry and current-weight factors, shared only by
compatible RHS consumers. Immutable topology may outlive a weight frame;
weighted totals, scales and factors may not. Previous solutions can warm-start
new weights but retain no acceptance authority. Admission must account for the
complete live memory bound and drop optional state before fallback. This differs
from the rejected call-owned dense two-FE matrix and its large extra allowance;
simply enabling OLS guards is not the design.

Compare at most two coherent organizations: frame-owned projection reuse, and
algebraic canonical products from raw weighted moments plus certified FE
coefficients if profiling supports eliminating materialized intermediate passes.
The latter remains unimplemented and needs cancellation bounds, canonical
rank/QR/span authority and original-input final verification. Require a successor
ADR for any verification scheduling change. Use independent WLS/Poisson/Hessian
oracles, bounded alternating public screens and fresh predeclared controls before
adoption; stop after a small or inconsistent full-command result. The current
artifact is a useful accepted step, not completion of the broader speed goal.
