# Concentrated PPML moments — local development, 2026-09-23

Adopt `profilemom1` under [ADR 0142](../../../../docs/adr/0142-contiguous-concentrated-ppml-moments.md).
All 35 K24/K32/K40/K48/K80 baseline/candidate pairs improve. Fresh whole-command
reductions range from 6.1% to 14.9%. Screen K32 reaches 107 ms versus stock's
121 ms, an 11.6% lead. Narrow K10 does not improve. This completes one bounded
milestone, not the broader objective of PPML at most 0.90 times comparable stock.

## Exact comparison

Baseline is the computational source at `14c397ca`, production artifact
`psdowner1`, plugin SHA-256
`ae620228d87df873839c06436af6a55f72c0461d4f531d1b1b766bcdad685da1`.
Candidate is that source plus only `ppml/one_fe_profile.rs`, artifact
`profilemom1`, plugin SHA-256
`abcbf06ea8d0403d72950412f39b735f10b89fe80f0249a0f6c7632396b050a9`.
All 541 computational source inputs match the adopted worktree. Both production
artifacts are separate from diagnostic `finiteledger1`/`finiteledger2`.
Snapshots, patch, source hashes and ABI/panic-containment receipts are archived.

The comparator is unmodified pinned fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched threads, robust inference and
fitted means. Both timers exclude data import/setup; the public fereg timer
includes loading and publication. Default-reference discrepancies remain separate.
Ordinary fixest weights supply no Stata frequency-weight inference oracle.

The declaration precedes candidate results: seed 202609335 for pilot/screen,
202609346 for fresh widths and 202609347 for controls. These are now consumed.
Every completed application passes its status/sentinel checks; no overlap,
monitor error, timeout or scientific discrepancy was detected. Compilation,
profiling and archival did not overlap headline fits. This is local Mac
prototype evidence, not frozen-suite or platform qualification.

## Computation and attribution

A group anchor is fetched once per column per concentrated direction. Both
centered-moment passes use it instead of repeatedly reevaluating the same first
row. The triangular Hessian uses contiguous FMA updates. Current weighted
means, observation order, fixed 64-group blocks, compensated block merging,
standardization, solve and independent unfactored residual check are unchanged.
The same engine serves fast/low storage and numeric/generated columns.

The [operation counts](work-counts.json) are source-derived: moment value fetches
fall from `4*N*K` to `(2*N+G)*K` per direction, while Hessian FMA count remains
`N*K*(K+1)/2`. This excludes unchanged predictor/profile reads. Anchors refresh
on each evaluation, including when FE-equivalent representatives have changed.
No weighted products persist across changing frames. The ordinary pre-step and
canonical accepted weights differ, so a proposed cross-step Hessian cache was
rejected in source analysis. Existing exact-frame final-product reuse already
avoids the redundant canonical solve when its conditions hold.

Separate one-rotation diagnostic intervals subtract the union of nested child
spans. Direction work below excludes its profile-evaluation child. Neither
unchanged phase fluctuations nor whole-plugin differences are attributed to the
patch; the experiment does not separate anchor binding from contiguous updates.

| Case | Direction baseline/candidate, ms | Native plugin baseline/candidate, ms |
|---|---:|---:|
| Low N61440/K10/T4 | 4.415 / 2.869 | 83.532 / 92.199 |
| Low N30720/K32/T4 | 16.449 / 8.151 | 92.498 / 87.156 |
| Fast N8192/K80/T4 | 18.815 / 5.985 | 141.549 / 133.057 |
| Low N8192/K80/T4 | 26.583 / 9.971 | 136.996 / 120.993 |

The diagnostic K10 whole-plugin loss is retained despite its faster changed
phase. Stock timings use only the uninstrumented library. These are attribution
measurements, not replacements for the following public-command rotations.

## Whole-command distributions

Milliseconds, median [minimum, maximum]. W/T/L counts paired candidate gains,
ties and losses against baseline. One-FE blocks use five rotations; PPML
controls use three and OLS controls two. All displayed PPML cases request robust
inference and means. `strong_d6` has two intercept-plus-slope partitions.

| Case | Baseline | Candidate | Stock | Candidate/stock | Paired W/T/L |
|---|---:|---:|---:|---:|---:|
| screen 0: low one_fe, N61440/K10/T4 | 96 [94, 98] | 97 [95, 98] | 52 [49, 52] | 1.865 | 0/2/3 |
| screen 1: low one_fe, N30720/K32/T4 | 115 [114, 116] | 107 [106, 108] | 121 [120, 123] | 0.884 | 5/0/0 |
| screen 2: fast one_fe, N8192/K80/T4 | 182 [179, 182] | 166 [164, 167] | 127 [126, 129] | 1.307 | 5/0/0 |
| screen 3: low one_fe, N8192/K80/T4 | 172 [171, 179] | 155 [154, 156] | 128 [126, 131] | 1.211 | 5/0/0 |
| screen 4: low one_fe, N3840/K3/T1 | 17 [16, 18] | 16 [16, 19] | 11 [10, 11] | 1.455 | 2/2/1 |
| fresh 0: low one_fe, N16384/K48/T4 | 112 [109, 113] | 100 [98, 102] | 109 [107, 114] | 0.917 | 5/0/0 |
| fresh 1: fast one_fe, N12288/K40/T4 | 82 [79, 83] | 77 [75, 79] | 69 [62, 69] | 1.116 | 5/0/0 |
| fresh 2: fast one_fe, N8192/K80/T8 | 170 [169, 173] | 159 [159, 163] | 115 [107, 124] | 1.383 | 5/0/0 |
| fresh 3: low one_fe, N24580/K24/T1 | 134 [131, 134] | 114 [112, 116] | 125 [119, 131] | 0.912 | 5/0/0 |
| ppml-controls 0: fast three_fe_dense, N30720/K6/T4 | 183 [180, 192] | 186 [186, 187] | 234 [231, 235] | 0.795 | 1/0/2 |
| ppml-controls 1: low three_fe_dense, N30720/K6/T4 | 341 [336, 346] | 346 [331, 352] | 234 [226, 234] | 1.479 | 1/0/2 |
| ppml-controls 2: low strong_d6, N30720/K6/T4 | 151 [149, 153] | 150 [149, 152] | 120 [116, 120] | 1.250 | 1/0/2 |
| ppml-controls 3: fast strong_d6, N30720/K6/T4 | 126 [124, 126] | 125 [123, 126] | 120 [118, 123] | 1.042 | 2/0/1 |
| ols-controls 0: fast strong_d6, N30720/K6/T4 | 66.5 [65, 68] | 69 [67, 71] | — | — | 1/0/1 |
| ols-controls 1: low three_fe_dense, N30720/K6/T4 | 65.5 [65, 66] | 66.5 [65, 68] | — | — | 0/1/1 |

All 35 wider one-FE pairs improve; no threshold/seed tuning or second candidate
was needed. K10 pairs are +1,+1,0,+1,0 ms. K3 pairs are 0,0,-2,-1,+2 ms.
Excluded fast/low categorical pairs are +6,-5,+3 and +16,+5,-15 ms; low/fast
slope pairs are +1,+1,-4 and -3,-1,+2 ms. OLS pairs are -1,+6 and 0,+2 ms.
These losses remain visible. The private one-FE change does not run on those
excluded routes, but that fact is not evidence that their timing differences
are zero. No universal performance non-regression claim is made, and no repeat
was used to replace an unfavorable initial block.

## Science and memory

- 391 distinct Rust PPML tests pass, including independent full-dummy weighted
  normal equations, changing/extreme weights, shifted representatives, lost rank,
  zero-outcome groups and capacity checks. No scientific acceptance was relaxed.
- Ten licensed runs of seven distinct fixtures pass explicit process and Stata
  return codes and terminal sentinels. Coverage includes full-dummy Poisson/
  Hessian/sandwich, FW/PW, offsets, generated terms, robust/cluster inference,
  multipartition slopes, separation, and warmed allocation boundaries.
- The 188 headline applications yield 180 passing PPML public states, nine
  independent small explicit-design oracles, 60 baseline/stock and 60 direct
  candidate/stock comparisons. All 60 native PPML pairs have byte-identical
  scientific CSV exports and means. Four OLS pairs have exact science. All eight
  separate diagnostic public states pass. Outer/inner counts and scratch bytes
  are identical in every native PPML pair.
- The fixed group-block workspaces add `(8*K+24)*ceil(G/64)` bytes on this
  architecture. Actual allocation uses active finite width; admission uses full
  candidate width, including the normalization column where applicable. Actual
  peak increments span 216–26,880 bytes, modeled increments 240–27,648 bytes.
  All multi-FE/slope control peaks are unchanged. No N-by-K copy is added.
- Real warmed audits at T1/pool1, T4/pool1 and T4/pool2 retain zero allocation
  events/growth, stable capacities and cold/warm agreement. A one-byte optional
  implicit-state refusal changes modeled memory 1,823,177 -> 1,820,233 and scratch
  0 -> 65,536 bytes, still passing its independent Poisson oracle. It tests that
  admission boundary, not direct refusal of the new anchor storage.

This does not qualify a physical cap or repair the static allocation drift,
weighted constrained/nonretaining-CG failures, mixed-thread reclamation or
existing platform/comparator deferrals. Frozen77/16/111 and protected cycle seeds
202609211/212 remain unchanged. No SCC retry, stopped campaign, push or release.

## Remaining opportunity

Whole-command 0.90 targets on this source are concrete: narrow K10 needs
97 -> 46.8 ms; fresh K40 needs 77 -> 62.1; K80/T8 needs 159 -> 103.5;
low three-FE needs 346 -> 210.6; low/fast slopes need 150/125 -> 108.
K48 and unbalanced K24 are closer, at 100 -> 98.1 and 114 -> 112.5, but
those small residual gaps should not displace the larger targets. Screen K32
and the fast three-FE control reach the margin in these captured cases only.

Low three-FE's candidate strict/final marginal is 171.662 ms, versus 0.615 ms
inference. Investigate canonical projection, scratch passes and certificate
construction on identical frozen frames. Joint slope/categorical directions and
single-frame finalization are already implemented; another ordinary solver
layer is not a sufficient explanation of that final cost. Preserve the rejected
all-coefficient categorical dynamic-range evidence in ADR 0136.

Wide baseline diagnostics show roughly 14 ms in outer scores at K80, 10 ms in
reported inverse/intercept verification, 24–25 ms in inference and 10 ms in
publication metadata checks. These are separate exclusive ledger entries, not
all generic column scans. The >64 numeric score path does use serial generic
column evaluation; bounded all-width column binding is a concrete next candidate.
The complete original-input score remains independent and mandatory. Certificate
kernels can be reorganized while retaining both inverse sides and numerical
bounds; cached solver products cannot simply replace their independent work.

Narrow K10's deficit needs a whole-command ownership/pass redesign, not another
few-percent moment optimization. The public/native gap and loading, preparation,
initial/final construction, objective and publication work require matched stock
attribution before claiming a lower bound. Current diagnostics do not establish
that the target is impossible. STATUS contains the mutable prioritized plan.

## Immutable receipts

The content-addressed archive has 7,699 logical receipts, 1,798 payloads and
13,462,169 bytes. SHA-256:
`23f2570f48ec2f6f988171fdd37da0584d75e8eefdef3119b16fe77c3c81d3aa`.
`restore_receipts.py NEW_DIRECTORY` verifies the container and every logical
file. Source snapshots, identities, recipes, declarations, raw distributions,
public audits, diagnostic ledgers, memory comparisons and test evidence are
included. Licensed raw logs, binaries and datasets are omitted with hashes.
