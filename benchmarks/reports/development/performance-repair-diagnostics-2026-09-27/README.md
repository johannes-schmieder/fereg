# Performance repair: Schwarz admission and failure attribution

Development evidence, not qualification or headline timings. Baseline source is
`a05882933fb0f1fc668e2a09bed8ff768b7329be`; `baseline-source.json` binds all 210
computational files. `baseline-plan.json` freezes ten attempts using the unchanged
systematic generator and original seed 104729. Linux plugin SHA-256:
`259e27939df8288456a254bdebe150682dac6d30fff321e2bc32d0bdac04d909`.

SCC job **7759645** uses one isolated E5-2680v4 node, 28 slots, 3 GiB per slot
and a two-hour limit. Individual processes have nested CPU affinities and native
thread counts. Stata itself is limited to its four licensed processors. Logs and
traces are retained under development run `20260927-performance-repair-a0588293`,
`diagnostic01`. Instrumentation clocks must not be compared with stock headline
timings. Failed attempts remain in the inventory.

## Harness repairs checked locally

- Four new worker-diagnostic tests pass: warmup exceptions produce receipts;
  failure stage and application return code survive quiet logs, command failures,
  startup timeouts and extraction timeouts. Six existing campaign tests also pass.
- Two capacity-preflight tests pass. The bias benchmark controller checks the
  maximum requested Stata/Mata team across the entire frozen plan before any
  timed fit; it never silently clamps either arm. A real Stata 19 preflight at
  one processor passes; an intentionally impossible request returns r(198)
  before any timed fit. New supported plans require fresh frozen identities.
- A real licensed Stata error probe verifies that `capture noisily quietly`
  preserves the error text and return code while successful commands remain
  quiet. Previous `capture quietly` suppressed the error text.
- Broader local systematic discovery could not import two plotting-dependent
  test modules because matplotlib is absent in the bundled Python environment.
  This is an environment limitation, not a passing check. No dependencies were
  installed and no original receipts or comparator settings were modified.

Raw licensed logs stay outside Git. The changes add failure attribution and
early capacity rejection; scientific eligibility, timing gates and tolerances
are unchanged. The controller's source fingerprint now includes the failure
helper so later frozen harnesses cannot omit its identity.


## Native correction and scientific checks

[ADR 0228](../../../../docs/adr/0228-schwarz-worker-stack-memory-bound.md)
replaces the Schwarz scratch charge T*(1+R) with the proven T+R bound on
worker stack depths. The numerical implementation and all acceptance checks
are unchanged. The old model could reject LSMR at 28 threads even when it
admitted the same fit at fewer threads. The replacement fixes that avoidable
route reversal; it does not implement adaptive thread ceilings.

`candidate01-source.json` binds the computational snapshot (one changed Rust
file). Linux plugin: `e072ecfc7305d63e12833385ee44339423c8f0a429ffbbf1998ab31e988083fa`.
Mac universal plugin: `1b2bc3ccbaa237bf0cd67da8606804c046c711e8573b7e69282afb4f00d823a3`.
The base-commit field in the manifest identifies its parent, not an unmodified
checkout. Artifact identities are separate from Git identity.

SCC baseline diagnostic job **7759645** completed with eight passing fits, one
r(459), and one timeout; scheduler failed=0/exit_status=0 and the application
completion sentinel are present. Candidate job **7759702** passed all eight
nonzero-design attempts, with failed=0/exit_status=0 and its terminal sentinel.
Both ran on scc-wo4, E5-2680v4, 28 reserved slots. Candidate job wall time was
538 seconds including its preceding build; maxvmem was 8.265 GiB. That job-wide
maximum includes compilation and is not a fit-specific memory comparison.

Single diagnostic command clocks, seconds (not headline benchmark medians):

| N983040 case | Threads | Baseline | Candidate | Candidate/baseline |
|---|---:|---:|---:|---:|
| PPML, sparse three FE, K10 | 8 | 33.778 | 34.347 | 1.017 |
| PPML, sparse three FE, K10 | 28 | 155.619 | 32.768 | 0.211 |
| OLS, strong degree-2 two-FE graph, K10 | 4 | 5.247 | 5.112 | 0.974 |
| OLS, strong degree-2 two-FE graph, K10 | 28 | 12.713 | 3.924 | 0.309 |
| OLS, sparse three FE, K50 | 4 | 24.571 | 24.520 | 0.998 |
| OLS, sparse three FE, K50 | 28 | 71.817 | 15.402 | 0.214 |
| PPML, sparse three FE, K50 | 4 | 128.461 | 132.847 | 1.034 |
| PPML, sparse three FE, K50 | 28 | 643.931 | 60.551 | 0.094 |

`diagnostic-summary.json` retains command and process clocks separately, phase
measurements, all losses/failures, input hashes and raw-file hashes. The paired
higher-core diagnostics now improve on their lower-core counterparts. The two
PPML low-thread slowdowns of 1.7% and 3.4% remain visible; one diagnostic draw
cannot establish either a regression or a robust gain. No current fixest
ranking or full-suite monotonic-scaling claim is made from this block.

Validation in `local-validation.json` and `local-stata.json`: 111 existing
memory tests, two new memory-bound tests and 29 joint-LSMR tests pass (overlapping
counts, not a sum of distinct tests). Three licensed fixtures pass: explicit
three-FE OLS; categorical PPML with independent Poisson/final Hessian/sandwich;
and multipartition slopes with independent Poisson. They cover fast/low,
weights, robust/clustered inference, saved predictions and transaction controls.
Mac and Linux binary audits and actual-plugin ABI tests also pass. No full
qualification, static allocation-inventory successor or constrained-CG repair
is claimed.

The correction reduces conservative modeled admission, not actual allocation.
It can choose a solver with greater actual memory than the former MAP fallback.
Per-fit RSS and matched constrained-memory confirmation remain required.

## Paired timing attempts and launch repair

The frozen block has 23 configurations and 126 fits: affected original cases,
fresh seeds 202609271/202609272, categorical controls and four bounded independent
OLS/Poisson oracle cases. Alternating orders are baseline/candidate/stock and
stock/candidate/baseline; no compilation or profiling overlaps headline fits.
Original work, full covariance and science thresholds remain matched.

- **7760118 / rotation01:** stopped after discovering strict-reference command
  limits inherited from native diagnostics. Known reference runs exceeded those
  limits. Partial native outputs and the interrupted reference stay preserved;
  this is not accepted paired evidence.
- **7760156 / rotation02:** exit_status=1 after four seconds, before fits. Its
  copied launcher retained the old output path `rotations01`; exclusive output
  creation correctly rejected it and preserved earlier evidence.
- A common `launch_rotation.py` now derives the plan, roots, harness and output
  from one attempt identifier, rejects any existing attempt, and checks source,
  plugin and prerequisite identities before writes. Three focused tests pass.
  A read-only SCC preflight passes using the pinned Python environment. The
  system Python was too old for that preflight; it ran no fits or writes.
  Another submission is pending explicit approval under the cluster retry rule.

`rotation02-registration.json` uses stock defaults for ordinary cases and stock
strict PPML only for the two bounded Poisson oracles. Time limits are frozen
from earlier reference maxima with headroom. Scientific discrepancies remain
unranked; failed default comparisons are not silently replaced by strict runs.
The full approved 140-configuration sweep has not started.

## Sparse-zero failure localization

The baseline two-FE sparse-zero N61440/K10/T4 attempt returns r(459), with
"borrowed MAP exhausted its iteration budget without a passing final
certificate" before publication IRLS. Three-FE reaches its 180-second cap.
These failures remain open and were excluded from the memory-only candidate.

The six-row analytic probe in `bridge-probe/experiment.patch` has an exact
FE-only fit, but its boundary weights form a nearly disconnected positive-row
graph. MAP exhausts 128 sweeps, a direct LSMR replacement fails the finite-score
certificate, and CG obtains residuals below 5.4e-13. `bridge-probe/result.txt`
preserves the diagnostic output. This is not a production repair or a passing
regression test. Any replacement needs current-weight model-span certificates,
proper memory admission and the full public failure cases; blindly swapping the
solver is not justified.
