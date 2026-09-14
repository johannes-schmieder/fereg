# P6 saved OLS final-target recovery: adopted local checkpoint

Adopt [ADR 0064](../../../../docs/adr/0064-saved-ols-final-target-recovery.md)
against accepted `bc5a6ae88f9db5427e577923a8b618af3e4e3c26`. Public one-shot OLS can
use the existing guarded Schur route with saved categorical effects, recovering
those effects from the certified final target. Only accepted probes omit initial
coefficient products; rejected probes retain the prior policy and route. PPML's
shared sessions default to the prior requirement. Explicit solver, width, sample,
weights, rank, DoF, covariance, component normalization and publication rules stay.
The separate 1e-14 final-output precision requirement remains authoritative.

The five-file change also admits each intercept-recovery attempt against the
existing joint budget. It covers retained-product normalization, CG coefficient
banks, SD/LSMR overlap and requested-channel/certificate storage, retaining the
cheaper certified MAP fallback when needed. Slopes/PPML keep existing recovery
policies; this does not close broader P0 accounting/reporting.

## Identity and scope

[Identities](built-identities.json) bind 178 inputs and isolated universal plugins:

- Baseline: `14b396c7f0d1829e24a8f4f92222a46e675d6aac2d17eec1d30b3af1bb73d0d2`.
- Candidate: `8bfa9fc20a19de01055ac8c551266d9dba4ec6581a2f5a55364d29537a252f69`.

The [patch](implementation.patch), [changed source archive](source-archive.json),
executed/archived script hashes and input/artifact hashes make the comparison
reviewable. Builds were isolated and forced recompilation of core/plugin on both
architectures; main and installed dist were never built. The installed plugin,
unrelated 97 STATUS additions and 358 original untracked paths were preserved.
These are local M2 Ultra/Stata-MP development measurements, not a fresh external
comparator, SCC, Windows or release qualification. No push occurred.

## Complete-command results

The [prospective protocol](protocol.md) and [cost protocol](cost-protocol.md)
retain the original requests/seeds, paired rotated order and all fallback work.
There are 23 pairs: nine primary pairs across three seeds, twelve original controls,
one unchanged weak-PW PPML control and one automatic-CMG saved-output control.
All 23 are accuracy-matched. No case exceeds both 5% and 10 ms added command cost,
so no extra timing repeats are required. Diagnostic/cap calls are excluded.

| Primary | N / finite K / workers | Command medians, seconds | Median paired reduction | Native peak medians, MB |
|---|---|---:|---:|---:|
| Unit, two saved effects | 245760 / 50 / 16 | 3.012 → 0.809 | 73.5% | 839.05 → 378.71 |
| AW robust, saved effects | 983040 / 50 / 4 | 83.397 → 68.675 | 17.7% | 3363.96 → 2316.04 |
| PW clustered, partial saved effects | 245760 / 50 / 16 | 3.641 → 2.676 | 26.8% | 841.84 → 581.41 |

Native peak reductions are 54.9%/31.2%/30.9%. Ratios of independently computed
medians need not equal median paired changes. Full per-case command, input,
native, initial/final setup/solve, probe, iterations, fallback, scratch and sampled
RSS evidence is in [summary.csv](summary.csv), [cost-results.json](cost-results.json)
and the content-addressed [captures](captures.json). Phase scopes can overlap.

The initial absorption explains the main time gains. Accepting unit seeds fall
from roughly 2.61–2.65 to 0.139 seconds initial solve. AW accepting seeds fall from
81.62/81.92 to 61.36/45.43 seconds, and PW accepting seeds from 2.91/2.94 to
1.39/1.41 seconds. Input time is nearly unchanged. Final projection, recovery and
publication are included in command totals; no target-kernel-only gain is claimed.

The original `8675309` unit/PW seeds reject the 0.01 contraction guard and cost
+54 ms/1.80% and +20 ms/0.55%. The AW `104729` seed also rejects, costs **+3.851
seconds/4.62%**, and retains its 3.364 GB peak. Its extra time lies mostly in
unchanged legacy CG; the cause is unestablished. It remains in the three-seed
median. Accepting AW/PW seeds still exhaust the 64-step Schur attempt before CG:
291 = 8 + 64 + 219 and 138 = 8 + 64 + 66 iterations, respectively. Accepting unit
seeds take 65 total steps with no continuation. These are full costs, not failed
attempts discarded from performance evidence. Earlier standalone admission +36/
+43 ms flags remain in the [preceding report](../saved-recovery-admission-local-2026-09-13/README.md);
this combined adoption does not claim the scalar planner explains them.

## Protected graphs and the memory tradeoff

Samples, status, DoF, topology/cluster tables, full b/V, original independent
certificates and requested outputs pass throughout, including weak graphs, tiny
weighted cuts, disconnected/unbalanced designs, slope, three FE, rank deficiency,
small/narrow models, low FW, complete IID and explicit CG. The weighted cut rejects
the actual weighted-operator probe (0.1160), keeps CMG, and costs +10 ms/10.1% with
unchanged native peak. Small AW adds 2 ms, low FW 4 ms and slope 2 ms. Rank-deficient
saved OLS improves 87 ms and falls 89.07→47.61 MB. PPML science is byte-identical.

Three controls switch from CMG to Schur and trade memory for command time:

| Control | Command, seconds | Native peak, MB | Candidate peak under 35.15 MB usable cap |
|---|---:|---:|---:|
| Disconnected blocks | 0.080 → 0.051 | 19.97 → 36.54 | 15.99 MB |
| Unbalanced | 0.082 → 0.057 | 19.93 → 26.06 | 15.98 MB |
| Automatic-CMG saved | 0.084 → 0.051 | 17.50 → 32.87 | 15.24 MB |

The [isolated allocator diagnostic](memory-review-results.json) reproduces exact
candidate science and each unrestricted whole peak. All these peaks occur in
**final LSMR residual absorption**, after the initial Schur solve. Recovery peaks
near 8.05 MB. Weak-d3 also peaks in final LSMR (19.96 MB), below its prior 34.30 MB.
The diagnostic patch/artifact and stage map are archived separately and excluded
from timing aggregates. No diagnostic hooks enter adopted source.

At **35,148,872 usable bytes**, all four original controls select the existing
admitted final CG continuation. Weak-d3 peaks at 17.26 MB; the other peaks are in
the table. Full b/V and saved effects/residuals pass both unrestricted-candidate
parity and the independent full oracles; maximum cap-versus-unrestricted output
difference is 2.39e-12. See [cap results](control-cap-results.json) and
[independent checks](control-cap-independent.json). Accept the unrestricted
6–17 MB increases because the complete commands improve and the bounded
alternative works; no graph-label or seed-specific routing rule is introduced.

[Primary admission replay](primary-admission-replay.json) uses all 51 generated
coordinates plus outcome, installed workers and actual finalized levels. It
cross-checks the exercised unit/PW exact/short boundaries and admits all primary
peaks. [Control replay](control-admission-replay.json) covers initial admission
only: disconnected's later whole peak exceeds its initial bound, which prompted
the final-projection trace and cap checks rather than a false admission claim.
The public execution-base scalar remains smaller than whole-action reservations.
It is not a universal model bound, and broader P0 reporting remains open.

## Independent science, failure evidence and validation

The first control run stopped after both disconnected model calls: the archived
oracle supplied 16 finite coordinates, while public b/V includes the normalized
intercept as coordinate 17. All completed captures were preserved and reused by
verified request/artifact identity. The [harness correction](harness-correction.json)
and original script are archived. The [independent extension](extend_full_oracle.py)
forms full b/V from grounded dummy projection/SVD and the full response influence
map, checks the old finite sandwich block, and verifies two response directions
through independently projected SVD fits. Separate full-design LSQR/LSMR agree
within 1e-10. Candidate values do not construct the oracle. No data/seed/tolerance
was replaced; timing stopped during oracle mathematics.

[Validation](validation-summary.json) records 124 focused core checks (68 prepared
fit, 56 memory), 95 full serial plugin tests, 48 independent saved dummy fits with
actual Schur assertion on eligible cases, and four weighted omission cases. Public
recovery/CG/generic exact and one-byte-short boundaries pass, including rollback.
Original unit/PW initial exact bounds of 827,753,800/831,179,336 usable bytes succeed
with admitted peaks; one byte below returns r(909). Four additional graph caps
and their independent checks are described above. Isolated universal/static and
actual arm64 ABI checks pass. Explicit fixture sentinels accompany process codes.

The original patch-context check failed before mutation because accepted final
precision added an adjacent field. The rebase kept that field and final precision
independent of recovery permission. A first scalar replay assumed the wrong RHS/
chunk dimensions and failed its exact-cap cross-check before reporting; corrected
values and that diagnostic error are recorded. No failing scientific fit was
replaced or promoted. Earlier rejected prototypes and accepted correctness repairs
remain immutable evidence.

This closes P6's final-target saved-FE recovery evaluation/adoption. It does not
close low-memory duplicate score residualization, generated/sloped layout work,
PPML Schur/state experiments, whole-action memory reporting or R1–R5.
