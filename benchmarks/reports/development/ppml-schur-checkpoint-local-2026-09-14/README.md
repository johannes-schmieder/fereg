# P4 guarded PPML Schur checkpoints: preliminary evidence

The [final repaired adoption](../ppml-schur-checkpoint-adoption-local-2026-09-14/README.md)
supersedes this preliminary candidate. Its captured initial gate assessment was
superseded after the combined-fallback regression; all measurements remain intact.

Prototype for [ADR 0067](../../../../docs/adr/0067-guarded-ppml-schur-checkpoints.md)
against evidence HEAD `7996a5e`, accepted native `7739e8e`. Admitted two-FE PPML
checkpoint sessions reuse the existing weighted Schur engine and retained graph,
while ordinary auto/LSMR iterations continue to use joint LSMR. Each proposed
projection must pass the canonical certificate at its original requested tolerance.

[Source/artifact identities](built-identities.json) bind 178 inputs and isolated
universal plugins: baseline `2689faf0`, candidate `cfeed644`. Both core and plugin
were forcibly rebuilt for both architectures; static audits and actual arm64 ABI
checks pass. Main and installed dist were not built or replaced. The
[implementation patch](prototype-v2.patch) and [changed-source archive](source-archive.json)
are replayable. These are local macOS/Stata-MP development results, not release,
Windows, SCC or external-estimator speed claims.

## Complete-command results

The [original protocol](protocol.md) and [repeat/control protocol](followup-protocol.md)
precede their respective calls. The three original route-screen inputs remain
unchanged. Each target has three seeds, rotated A/B order, fresh processes and
an untimed small warm-up. Builds, tests, generation and numerical oracles do not
run during timing. Complete estimation, inference and publication are included.

| Target | N / finite K / workers | Command medians, seconds | Median paired reduction | Native peak medians, MB |
|---|---|---:|---:|---:|
| Dense two FE, unit robust | 122880 / 50 / 4 | 2.391 → 1.689 | 26.4% | 575.78 → 448.43 |
| Strong two FE, FW/exposure/missing, robust | 122880 / 50 / 4 | 3.414 → 1.956 | 42.3% | 621.37 → 475.44 |
| Dense two FE, PW/offset/two-way cluster | 122880 / 50 / 16 | 2.004 → 1.717 | 15.1% | 687.70 → 456.66 |

Percentages are medians of within-pair changes, not ratios of independently
computed medians. Every primary pair improves. Strict/final scope medians fall
0.953→0.581, 1.517→0.554 and 0.757→0.556 seconds. FE projection scopes also fall;
these nested phase scopes must not be summed. Preparation and inference remain
similar; some unchanged finite-solve work is slower. Full command gains are not
assigned exclusively to a single kernel. All phases, inner iterations and costs
remain in [summary.csv](summary.csv) and [the adoption summary](adoption-summary.json).

The best existing explicit route on strong FW was MAP. Across the same three
inputs accepted MAP takes 2.905/2.815/2.831 seconds, versus candidate auto
1.971/1.930/1.956: an additional 30.9–32.2% reduction. Existing CMG results remain
in the [prior route screen](../ppml-weighted-schur-screen-local-2026-09-14/README.md);
CMG was not repeated or adopted as a new default.

## Guard, fallback and scientific evidence

Eligibility requires an already admitted checkpoint-LSMR session, exactly two
categorical intercept effects, N>=16384, actual RHS width 16–64, and retained
Schur graph storage. An eight-step three-column probe uses the actual current
normalized weights and the existing 0.01 contraction guard. A passing probe
permits at most 64 full Schur iterations, bounded by the remaining configured
budget. PPML raw reference norms and canonical tolerances remain unchanged.

The candidate is a fallible temporary copy. The original inputs stay untouched;
a finite capped or uncertified attempt is discarded before original-input LSMR,
its existing tighter retry and MAP fallback. Allocation, input, internal and
interruption failures remain fatal. No numerical state is reused across changed
weights without rebuilding it. The narrow final residual checkpoint stays LSMR.

Nine separate [diagnostic calls](trace-results.json) preserve exact scientific
exports and discrete solver fields against their same-artifact untraced calls.
They show two successful full-width Schur checkpoints per primary fit. Probe/copy
setup is about 9–12 ms per checkpoint; full solve scopes are about 34–65 ms.
Weak and bottleneck controls accept their initial checkpoint but reject the later
current-weight probe. Cycle and tiny-weight bridge controls reject both probes;
these rejection costs are retained. Disconnected and unbalanced controls certify.
Warm calls are distinguished by row count. Traced times are excluded from speed
aggregates. No untraced primary fit uses an ordinary numerical fallback.

All 20 original/repeated/control pairs preserve exact sample, rank/status, DoF,
weight/cluster/separation structure and registered b/V/objective tolerances;
maximum absolute coefficient difference is 1.70e-16. Eleven protected controls
cover wide weak/bottleneck/cycle/disconnected/unbalanced and tiny-weight graphs,
rank deficiency, actual three FE, low FW, small/narrow and numeric slopes. Their
largest added command cost is 1 ms. The low-FW improvement is a single unchanged-
route observation, not an optimization claim for that route.

## Memory and output boundaries

The [ownership audit](cost-audit.md) proves that one transient N-by-R candidate
replaces larger serial LSMR transport/output banks already admitted beside the
retained canonical design and ordinary engine. The weighted probe reuses the
retained graph; no new observation-sized preparation state or retained design
is added. All observed untraced peaks fit unchanged selected models. Primary
sampled RSS medians fall 0.645→0.434, 0.722→0.491 and 0.787→0.454 GiB. Native
live allocation and sampled process RSS are distinct measures.

The [public supplement](public-boundaries/boundary-results.json) admits explicit
LSMR on both artifacts at 3,036,827,657 available bytes, exactly 2,429,462,126
usable after existing headroom. One byte below rejects both with r(909) and a
terminal worker sentinel. The two expected failures remain archived and are
excluded from speed comparisons. Native peaks at the exact bound are
575.78/448.43 MB. A separate saved-two-FE/mean control improves 2.410→1.799 seconds;
all 122880 original-row outputs agree within unchanged rtol1e-7/atol1e-9, with
maximum absolute difference 3.28e-10 and admitted peaks.

The public four engine rows retain their identities: successful Schur is the
existing MAP accelerator, while ordinary auto remains LSMR. Rejected candidate
work contributes retries, warnings and aggregate final-engine costs. Private
traces separate Schur attempts. The eight-step pilot field reports its budget,
not exact early-exit operator counts; P0's broader per-attempt/build/copy accounting
remains unfinished. No allocation-free checkpoint or universal memory claim is made.

## Validation and disposition

[Validation](validation-summary.json) records 67 distinct focused core tests and
95 serial plugin tests. New tests cover exact copy admission, changed weights,
1/4/16 workers, dirty/nonfinite scratch, weighted cuts, fixed capacities, forced
cap/certificate rejection with bitwise original-input LSMR results, and fatal
allocation/interruption recovery. Nine licensed native fits and three independent
full-dummy Poisson/Hessian/sandwich fits cover unit/FW/PW and auto/MAP/CMG, including
independently computed covariance corrections. Every required sentinel passes.

The [main archive](archive.json) retains 52 command captures; the
[public supplement](public-boundaries/archive.json) retains six more, including
two expected admission errors. Raw licensed logs, plugins and data are not committed.
The first nonexistent cargo-launcher failure, v1 source/tests and subsequent
scratch-reset correction are preserved in [execution notes](development-notes.md).

Final review added a combined capped-Schur/double-LSMR-rejection test. It exposed
stale scalar energy in the subsequent MAP continuation, so this artifact is not
adopted. Its measurements remain preliminary evidence; the repaired source must
pass the unchanged oracle and same-input validation before adoption. This report
does not resolve P4's weighted-Schur comparison. Other preparation/construction/state work, generated/slope/three-plus-FE
layouts, P7 scheduling and P0 accounting remain. R1–R5 stay excluded; independent
reviews, upstream CMG assessment and the bounded SCC campaign follow P0–P7.
