# Remaining performance opportunity: current-source local inventory

Development screen on macOS arm64, September 22, 2026. This starts a new bounded
investigation; it does not resume either stopped empirical/reference campaign.
All 523 computational inputs at `7189e57a` match the captured `dtk1` artifact
(plugin `77b2b35764ae216267503b34a9965834bd3a6c8ef60e0f307bdf62c379f5b151`).
`source-binding.json` verifies that binding independently against the Git archive.
Stock fixest 0.14.2 uses the unchanged archive pin in systematic/references.json.

## Coverage inventory

| Category | Evidence and interpretation at this starting point |
|---|---|
| Covered and competitive on captured earlier artifacts | One-/two-FE categorical OLS: both historical 93-cell blocks won all medians; [closeout](../ols-round-closeout-local-2026-09-22/README.md) expressly limits that claim to `owf1`. Threaded absorbed slopes and narrow inference improvements have source-bound development evidence. None of those aggregate wins is transferred to this source. |
| Current measured losses, matching public requests | Dense three-FE numeric OLS, IID, K10/T4 and K50/T16; public feppml versus fepois, robust, one FE and degree-six two FE. Exact samples and pairwise coefficient/covariance/objective checks pass. Independent small counterparts pass. See below; this is development evidence, not the complete PPML qualification gate. |
| Current known closed losses | The `dtk1` AW K10 robust/full-output and PW K6 serial slope losses (226/188 and 209/152 ms) remain closed. No further serial-slope tuning is proposed. |
| Historical losses needing remeasurement | Small robust/clustered one-FE overhead and saved-output costs in [the breadth screen](../one-fe-inference-output-breadth-2026-09-19/README.md), pre-improvement slope losses, and older systematic three-FE/PPML timings. Later accepted work reaches some of these paths. |
| Scientifically unresolved comparisons | Current sparse three-FE default fixest covariance disagrees at both widths; the wide case also disagrees on beta/RSS. Small native results pass explicit-dummy rank/inference; small fixest covariance fails. These are not accepted speed wins. Historical sparse reference failures, FW comparisons and PPML reference reconstruction discrepancies remain separate. |
| Tested implementation, incomplete current performance coverage | Low-memory OLS/PPML, generated/factor terms, weighted/disconnected/weak graphs, multi-partition slopes, saved effects, robust/multiway-cluster inference and K>50. Relevant frozen manifests and Rust/Stata tests exist; this inventory does not mistake test definitions for current timings. |
| Untested in this current screen | Four-plus FE, strongly unbalanced three-FE, separation-heavy PPML, weighted/clustered PPML, large generated models, constrained-memory throughput, Linux T1/8/16. Local extensions will be adaptive; SCC remains deferred. |

Relevant frozen definitions include `ols_optimization_20260921_v1.json` (77
primary, 16 holdout, 111 native controls), `ols_v1.json`,
`ppml_memory_performance_v1.json`, `feppml_solver_development_v5.json`, and
`feppml_bounded_auto_validation_v1.json`. None was changed. The stopped
systematic report remains historical; its PPML results were pilot-only.

## Current screen

Three repetitions per large cell, fresh processes with 7,680-row warmups,
alternating native/reference order, identical generated doubles and audited
sample IDs. Seed 202609221 is a new tuning seed. Dense panels use 20% mobility;
sparse panels use the existing 2% design. `strong_d6` means **two** effects and
degree six. Thread requests match, BLAS uses one thread. Timers include the
public command, loading from Stata into Rust, estimation, inference and
publication; external file import and diagnostic exports are excluded on both
sides. No saved outputs are requested. PPML uses positive outcomes, no
separation removals, the same full sample, and HC0 multiplied by N/(N−1) for
the stock-reference export. No reference diagnostic build enters these timings.

| Model/design | N / K / threads | Native median [min,max], ms | Stock median [min,max], ms | Scientific status |
|---|---:|---:|---:|---|
| OLS sparse three FE | 61,440 / 10 / 4 | 113 [113,118] | 925 [924,931] | V discrepancy; unranked |
| OLS sparse three FE | 245,760 / 50 / 16 | 1,194 [1,173,1,202] | 5,782 [5,773,5,793] | beta/V/RSS discrepancy; unranked |
| OLS dense three FE | 61,440 / 10 / 4 | 105 [104,108] | 82 [79,86] | Pairwise pass; 28% slower |
| OLS dense three FE | 245,760 / 50 / 16 | 919 [885,930] | 595 [594,598] | Pairwise pass; 54% slower |
| PPML one FE | 61,440 / 10 / 4 | 175 [173,181] | 51 [50,53] | Pairwise pass; 3.43× |
| PPML strong degree six | 61,440 / 10 / 4 | 349 [346,354] | 98 [97,98] | Pairwise pass; 3.56× |

The four bounded explicit-design cases use 3,600 rows for three FE (rank audit
bounded to 1,200 dummy columns) and 7,680 rows for PPML. All native beta/V/objective
checks pass; dense OLS and both PPML reference counterparts also pass. Sparse
fixest V fails by up to 1.60e−7. Large sparse discrepancies remain unadjudicated.
Independent PPML scores are freshly checked at the independently fitted optimum.
This does not supply the full same-cell pinned-ppmlhdfe/F0 qualification matrix
required by the performance contract; no qualification or universal timing claim
is made.

## Cost and mechanism inventory

Native diagnostic traces put dense wide OLS initial absorption near 712 ms,
versus about 16 ms for within norms and 11 ms for the finite solve. Explicit
MAP/CG/IT diagnostic command times are 5.303/1.239/3.927 s versus auto 0.854 s.
Explicit LSMR fails the warmup with r(459), before the target invocation. Auto
uses its existing certified routing. Removing routing guards is not justified.

An isolated source-hashed diagnostic build splits the wide LSMR boundary into
50 ms of input transport, 38 ms of output transport, 559 ms upstream solve
(41 ms setup), 13 ms workspace/norms, and 26 ms fresh certification. Sparse
wide transport costs are nearly identical but upstream work is 801 ms.
The first implementation candidate is a bit-preserving tiled transport using
the existing admitted buffers. It can carry over to other LSMR callers without
changing routing, weighting, arithmetic or certificates.

PPML runtime medians are not summed from unrelated runs. Representative r0
native phases, one-FE / degree-six in milliseconds: preparation 16/19,
projection 30/98, finite solve 19/21, objective/line search 15/30, strict-final
55/141, inference 1.6/1.3, publication 9.6/9.2. Outer iterations are 3/6;
reported inner iterations 5/58; no fallback or strict-profile rejection.
Tracked peaks are 32.8/94.5 MB, modeled plans 36.3/288.3 MB. PPML strict
checkpoints and current-weight projection are the next profiling priority.

Shared numeric binding/loading, preparation ownership, score-cache and small
covariance kernels already reach these public paths where their data conditions
hold. One-/two-FE Schur geometry, same-weight graph reuse and scoped retained
serial OLS windowing have narrower contracts. Slope geometry is rank- and
weight-frame-specific. PPML already retains topology and session storage;
changing working weights invalidate weighted geometry/norms even when labels
are unchanged. Its joint LSMR ordinary steps, canonical checkpoints and final
Hessian synchronization must remain distinct. Low-memory categorical PPML
already uses bounded blocks under ADR 0069. No OLS-only cache is transferred.

## Receipts and limitations

`evidence.zip` retains plans, captured workers/generator, input hashes,
application results, all numeric exports, independent audits, phase traces,
and original/portable file hashes. Raw licensed logs, data and binaries are
excluded. Fifty-seven calls were attempted: 56 completed and one explicit
LSMR warmup failed; no timeout or detected competing fit/build occurred.
The diagnostic source identity and instrumentation are separate from the
uninstrumented baseline. An initial diagnostic build failed before compilation
because the cargo launcher was absent; the successful build used the same
installed rustc 1.85.1 toolchain as `dtk1`.

Remaining source-allocation inventory drift, weighted constrained-fit failures,
nonretaining CG instability and deferred Linux checks are unchanged. The first
screen intentionally leaves many combinations unmeasured rather than claiming
coverage from a large nominal Cartesian product.
