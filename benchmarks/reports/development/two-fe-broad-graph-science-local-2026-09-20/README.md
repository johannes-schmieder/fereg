# Broad two-way graph and weighted-oracle development evidence

Local Mac development on 2026-09-20, ordinary two-intercept OLS. The graph screen
uses N=245,760, K=6, sixteen requested native workers and one warmed command per
fresh application process. It identifies work for the broader milestone; it is
not its repeated acceptance campaign.

## Graph results and numerical adjudication

Initial native artifact `weightedfinish` is the committed `3a02a7da` endpoint;
`baseline/identities.json` binds its source and plugin. Command milliseconds:

| Graph | Unit fereg / fixest | AW fereg / fixest | Default reference science |
|---|---:|---:|---|
| Degree two | 215 / 631 | 240 / 552 | Coefficient mismatch |
| Degree three | 77 / 75 | 94 / 78 | Pass |
| Degree six | 71 / 64 | 85 / 78 | Pass |
| Sparse mobility | 108 / 796 | 129 / 726 | Coefficient mismatch |
| Unbalanced | 75 / 73 | 92 / 79 | Pass |
| Disconnected blocks | 70 / 175 | 87 / 195 | Pass |
| Weak degree three | 137 / 156 | 204 / 219 | Pass |
| Bottleneck ring | 1140 / 2586 | 202 / 1982 | Coefficient mismatch |
| Cycle | 150 / 3934 | 208 / 3221 | Coefficient mismatch |

All 36 application calls succeed. The eight reference coefficient disagreements
remain failures at the unchanged repository tolerances. An independently
implemented grounded sparse direct solve checks all 18 native and 18 reference
results, including coefficients, IID covariance, RSS and effective N. Every
native result passes, with maximum coefficient error about 9.9e-13. The same
eight default fixest results fail, with coefficient errors from 1.45e-8 to
1.27e-5. See `graph-baseline-direct-oracle.json`.

The independent implementation builds weighted dummy cross-products from the
canonical input, eliminates the left block, grounds one right coordinate per
connected component and uses SciPy SuperLU with fresh residual refinement.
Weighted group normal equations and finite scores are checked independently of
native solver state. Input-file and per-column IEEE hashes are verified.
Twelve small connected/disconnected, unit/AW/FW and reversed-order cases agree
with a separately implemented grounded explicit-dummy SVD. The first self-check
used redundant ungrounded dummy columns; its SVD numerically overcounted the
known null rank in weighted cases. `direct-oracle-initial-rank.json` preserves
that diagnostic. Grounding the self-check fixes its identification; no estimator
tolerance or benchmark input changes.

`precision/` retains eight further native/reference pairs at fixest tolerance
1e-11, using the committed weighted-input artifact `weightedinput2` from
`329bc17c` (source identity records its `3a02a7da` base and plugin overlay).
The degree-two and sparse pairs pass the unchanged comparison and the direct
oracle. Reference command times are 1.409/2.017 seconds for unit/AW degree two
and 4.143/16.315 seconds for unit/AW sparse mobility.

All four tighter bottleneck/cycle reference calls hit fixest's 10,000-iteration
limit after about 38 seconds and are recorded as nonconvergence. Their native
calls pass the direct oracle. No unsuccessful reference time is presented as
a matched-accuracy performance result. The original default-reference times
remain the speed target. A separate tighter-reference solve is not substituted
into the headline score.

The unit bottleneck's accepted Schur probe contracts by 0.001684, yet its
initial absorption takes 1.096 seconds. Its route therefore deserves a forced-
solver diagnostic. The well-connected narrow degree-three, degree-six and
unbalanced designs are the immediate losses. Graph labels never select runtime
routes; future admission changes must use measured graph/operator properties.

## Weighted inference and harness boundaries

The N=960 disconnected AW two-way cluster example has independently determined
model rank 243. The pinned fixest nonnested/exact correction reports 242; the
full/exact correction reports 243 when neither FE is cluster-nested. Rescaling
by 718/717 explains the covariance difference to 1.1e-19, and the full correction
agrees with native covariance to 1.1e-12. `dof-source.json` binds the exact
reference implementation and archive; `disconnected-dof.json` records the
numerical adjudication. This changes a diagnostic comparator setting, not the
native statistical policy. The harness verifies nonnesting before permitting
that setting for clustered comparisons.

The corrected small harness runs also cover zero regressors and saved outputs,
with explicit-dummy SVD checks and an independent native output audit.
`initial-harness/` preserves four successful application calls whose Python
adjudication initially failed on empty numeric dtypes and empty R CSV schemas.
Those are harness failures, not accepted scientific comparisons.
`corrected-harness/` contains the five successful final calls and their checks.

`weighted-harness/` adds ten native calls covering frequency IID, robust,
FE-clustered, two-way clustered and full-output requests. Each agrees with
weighted explicit dummies and a distinct literal replicated-row calculation of
coefficients, covariance, RSS, rank and effective N. Nine one-FE weighted calls
cover robust inference, two-way clustered saved FE and full outputs, including
a tight stock reference. Exactly one FE channel is exported for that control.
The zero-regressor independent output audit also passes.

## Scope and provenance

The archive contains 80 application calls: 36 initial graph calls, sixteen
tighter graph calls (four nonconverged references), four initial harness calls,
five corrected harness calls and nineteen weighted/one-FE calls. Failed
comparisons and attempts remain visible. Native artifacts, controller hashes,
reference hashes, input hashes and the contemporaneous harness accompany each
series; the separate sparse-direct receipts bind that implementation too.
The small oracle self-check source is copied for reproducibility. Raw licensed
logs and machine paths are excluded.

The graph/distribution/memory/one-FE harness extensions and the broad inventory
controller are development infrastructure. The inventory expands to 77 primary
cells and 111 native controls; its full screen and acceptance blocks have not
run at this checkpoint. No universal win, release qualification, installed
package change or remote-state change follows from this report.
