# P4 guarded PPML Schur checkpoints: adopted local checkpoint

Adopt [ADR 0067](../../../../docs/adr/0067-guarded-ppml-schur-checkpoints.md)
against evidence HEAD `7996a5e`, accepted native `7739e8e`/plugin `2689faf0`.
The final candidate is `f4cb7dee`; [identities](built-identities.json) bind all
178 inputs, isolated universal artifacts and the [two-file patch](implementation.patch).
Both architectures rebuilt core/plugin; static and actual arm64 ABI checks pass.
Main and installed dist were not built or replaced. This is local macOS/Stata-MP
development evidence, not release, Windows, SCC or external-estimator qualification.

## Complete-command evidence

The same original inputs, seeds, commands and tolerances are retained. Nine
primary pairs use three seeds and rotated order in fresh processes with untimed
small warm-ups. Builds, tests, generation and numerical oracles do not overlap
command timing. Estimation, inference and publication are included.

| Target | N / finite K / workers | Command medians, seconds | Median paired reduction | Native peak medians, MB |
|---|---|---:|---:|---:|
| Dense two FE, unit robust | 122880 / 50 / 4 | 2.312 → 1.679 | 27.9% | 575.78 → 448.43 |
| Strong two FE, FW/exposure/missing, robust | 122880 / 50 / 4 | 3.186 → 1.910 | 40.1% | 621.37 → 475.44 |
| Dense two FE, PW/offset/two-way cluster | 122880 / 50 / 16 | 2.007 → 1.716 | 14.3% | 648.85 → 456.66 |

Percentages are medians of within-pair changes, not ratios of independent medians.
Every primary pair improves. Strict/final scope medians fall 0.940→0.537,
1.537→0.533 and 0.750→0.569 seconds. FE projection scopes fall 0.576→0.329,
0.746→0.438 and 0.522→0.377; these nested scopes must not be summed. Preparation
and inference stay similar, while some unchanged finite work is slower. All
phases and failed/retried work remain in the complete totals and
[summary.csv](summary.csv), with [paired statistics](adoption-summary.json).

Accepted explicit MAP, previously the best existing strong-FW route, takes
2.931/2.878/2.832 seconds on the same three inputs. Candidate auto takes
1.910/1.852/1.918, saving another 32.3–35.6%. The
[earlier route screen](../ppml-weighted-schur-screen-local-2026-09-14/README.md)
compares all three existing auto/MAP/CMG implementations; CMG was not repeated or
admitted as a new default.

## Implementation and independent checks

Already admitted checkpoint-LSMR sessions with two categorical intercept effects,
N>=16384, actual RHS width 16–64 and existing Schur storage may reuse the weighted
operator probe. Its eight-step, three-column contraction guard remains 0.01.
A passing probe permits at most 64 full Schur iterations within the remaining
configured budget. Ordinary joint LSMR, narrow final residual checkpoints, one-FE,
three-FE, slope and low-memory selection remain unchanged.

One fallible transient copy preserves authoritative inputs. The candidate replaces
larger serial LSMR output/transport banks already in the admitted model; the probe
reuses the retained graph. Every result must pass the canonical certificate with
PPML's original raw norms and requested tolerance. Finite capped/uncertified
attempts are discarded before original-input LSMR, its existing tighter retry and
MAP fallback. Fatal errors propagate. Rank, current-weight Hessian, covariance,
reconstruction, saved-output and transactional publication policies remain.

Final review of the [preliminary candidate](../ppml-schur-checkpoint-local-2026-09-14/README.md)
added combined capped-Schur and double-LSMR-certificate rejection. It exposed stale
residual energy in the later MAP recurrence, which previously resized scalar banks
without clearing same-length contents. The final implementation resets those banks
at the shared recurrence entry, and merges Schur warnings/costs after forming the
LSMR-to-MAP message. The original failing test/source are preserved in
[the repair record](reporting-repair-protocol.md), [failed result](before-reset-test-results.json)
and [patch](before-reset-implementation.patch). The unchanged 2e-10 independent
projection check now passes with both warnings and the correct retry count.
All benchmarks and oracles were rerun on the final artifact; preliminary timings
are not pooled into its claims.

All 20 primary/control pairs preserve exact sample, rank/status, DoF, weights,
cluster/separation structure and registered b/V/objective tolerances; maximum
absolute coefficient difference is 1.70e-16. Eleven protected controls cover wide
weak/bottleneck/cycle/disconnected/unbalanced and tiny-weight graphs, rank deficiency,
actual three FE, low FW, small/narrow and numeric slopes. Largest added command
cost is 6 ms on three FE, below the predeclared both-5%-and-10-ms gate. Single-pair
improvements on unchanged routes are not optimization claims for those routes.

Nine separate [diagnostic calls](trace-results.json) exactly match each same-artifact
untraced scientific export and discrete solver field. Each primary fit has two
successful full-width Schur checkpoints. Probe/copy setup costs about 10–14 ms per
checkpoint; full solve scopes are 34–74 ms. Weak/bottleneck initial probes accept,
but their later current-weight probes reject. Cycle and tiny-weight bridges reject
both; disconnected/unbalanced certify. Probe rejection costs remain included.
Warm calls are separated by row count; traced times are excluded from speed claims.

## Memory, outputs and validation

All observed untraced peaks fit unchanged selected models. Primary native peak
medians fall about 22–30%; baseline PW peak varies across seeds, with each retained
in the archive. Sampled RSS medians fall 0.663→0.436, 0.747→0.491 and 0.772→0.455 GiB.
Native allocation and sampled process RSS are different measures. The
[ownership/cost audit](cost-audit.md) states the transient overlap proof and limitations.

The [public supplement](public-boundaries/boundary-results.json) admits explicit
LSMR on both artifacts at 3,036,827,657 available bytes, exactly 2,429,462,126 usable
under existing headroom. One byte below rejects both with r(909) and a terminal
worker sentinel. The expected failures stay archived and are excluded from speed
comparisons. Exact-bound native peaks are 575.78/448.43 MB. The saved-two-FE/mean
control improves 2.472→1.860 seconds, with all 122880 original-row saved values
within unchanged rtol1e-7/atol1e-9; maximum absolute difference is 3.28e-10.

[Validation](validation-summary.json) records 67 distinct focused core and 95 serial
plugin tests, nine licensed native fits, and three independent full-dummy
Poisson/Hessian/sandwich fits across unit/FW/PW and auto/MAP/CMG. Tests include
1/4/16 workers, changing weights, dirty/nonfinite scratch, exact copy admission,
fixed capacity, capped/certificate rejection, full MAP fallback, fatal-error reuse
and strict rollback. All final return codes and required sentinels pass.
The [main archive](archive.json) has 52 command captures and the
[supplement](public-boundaries/archive.json) six more, including two expected
admission errors. Raw licensed logs, plugins and data are not committed.

The public engine rows retain existing identities: successful Schur is the MAP
accelerator, while ordinary auto stays LSMR. Costs/retries/warnings aggregate on
the final engine row; private traces separate attempts. Pilot fields report a
bounded probe budget, not exact early-exit operator counts. This does not close
P0's full attempt/build/copy accounting or claim allocation-free checkpoints.

This resolves P4's weighted-Schur comparison through bounded adoption. Seven
P0–P7 items remain, including construction/state work, generated/slope/three-plus-FE
layouts, scheduling and complete accounting. R1–R5 remain excluded; independent
reviews, upstream CMG assessment and the bounded SCC campaign follow P0–P7.
