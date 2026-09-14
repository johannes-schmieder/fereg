# P4 weighted Schur: existing-route screen and checkpoint attribution

The existing PPML MAP route already uses the weighted Schur recurrence shared
with OLS. This screen compares its complete-command cost with current automatic
joint LSMR and explicit prepared CMG before choosing a new Schur application.
No production code or automatic routing changes here. The next experiment is a
guarded checkpoint application that retains joint LSMR for ordinary iterations.

## Identity, oracle and workloads

[Identities](baseline-identities.json) bind evidence HEAD `09f0f2f`, accepted
native `7739e8e`, 178 unchanged inputs and the existing isolated plugin
`2689faf0418896ac66f0cb7fcaa9552b339a1928ab38409b43710dab6f63faeb`.
No new candidate or main/dist build was made. The original artifact's build
evidence is in the [predictor adoption](../ppml-predictor-tiles-local-2026-09-14/README.md).

The [independent oracle](two_fe_oracle.do) uses 16 finite controls and two
intercept effects at N16384. Nine public fits cover unit/FW/PW under auto/MAP/CMG.
Three separate Stata full-dummy Poisson fits supply coefficients and means;
an independently formed full Hessian and sandwich supplies slope covariance.
The oracle independently computes N-effective/(N-effective-1) or 97/96
corrections, including literal FW multiplicity and PW squared scores.
All nine native fits pass, with explicit r(0) and terminal sentinels.

The [protocol](protocol.md) precedes three newly frozen local inputs at N122880,
K50: dense unit robust/t4, strong degree-three FW exposure/missing/t4, and dense
PW offset/two-way cluster/t16. The existing generator and PPML preparation
recipes use seeds 104729/full and 104730/warm. Original data/column hashes,
requests, executable harness hashes and worker identities are retained.
No generation, oracle calculation, build, test or compression overlaps timing.

## Complete-command route screen

Each cell has one call per route in rotated order, a small untimed warm-up and a
fresh local M2 Ultra Stata/MP process. This is a single-seed diagnostic screen,
not a repeated speed estimate, release qualification or external comparison.

| Target | Auto / MAP / CMG seconds | Auto / MAP / CMG native peak, MB | Auto / MAP / CMG model, MB |
|---|---:|---:|---:|
| Dense unit, robust | 2.342 / 2.978 / 4.142 | 575.78 / 343.26 / 685.36 | 2429.46 / 693.65 / 2967.76 |
| Strong FW, exposure/missing | 3.289 / 2.878 / 7.203 | 621.37 / 365.54 / 732.20 | 4480.62 / 694.93 / 4830.40 |
| Dense PW, offset/two-way cluster | 2.263 / 2.451 / 3.109 | 686.49 / 351.49 / 777.55 | 5885.07 / 708.88 / 6698.14 |

MAP is 12.5% faster than auto on the strong FW screen but 27.2%/8.3% slower on
the dense unit/PW screens. Its native peaks are lower in all three. This does
not justify a blanket MAP replacement. CMG loses these selected well-connected
screens; its difficult-graph usefulness is not evaluated or rejected here.
Sampled RSS (auto/MAP/CMG) is 0.6543/0.3324/0.8046, 0.7462/0.3301/0.8824 and
0.7926/0.3448/0.8902 GiB. Scratch is zero. All native action peaks are admitted.

| Target | Auto → MAP FE-projection scope, s | Auto → MAP strict/final scope, s |
|---|---:|---:|
| Dense unit | 0.5984 → 1.5230 | 0.9328 → 0.5943 |
| Strong FW | 0.7620 → 1.4161 | 1.6097 → 0.5487 |
| Dense PW | 0.5394 → 1.0627 | 0.9854 → 0.4828 |

These scopes motivate retaining ordinary joint LSMR while evaluating checkpoint
Schur work. They contain subordinate operations and must not be added as
independent components. Preparation is close across routes; finite work and
inference/publication remain included in command totals.

All nine calls converge in seven outer iterations with no step halvings or
ordinary route fallbacks. Inner counts differ and are retained. Samples,
rank/status, DoF, weights, cluster/subset/separation metadata and correction/PSD
states match exactly. Coefficient differences between routes are at most
1.80e-16; full covariance/objective comparisons pass the unchanged registered
tolerances and every independent runtime certificate passes. See
[route details](baseline-route-summary.json) and [validation](validation-summary.json).

## Schur use and checkpoint retries

The [source audit](source-audit.md) identifies `TwoFePcgWorkspace` and
`absorb_two_fe_pcg_reusing` as shared machinery. PPML MAP enters it after a
four-sweep pilot and retains its workspace and MAP continuation policy. OLS
instead has a current-weight three-column probe, a capped attempt and admitted
CG continuation. They share a recurrence, not identical entry/fallback policies.

Six separate existing phase-trace calls preserve exact science and solver
decisions against each untraced route. MAP emits 17/16/16 Schur records across
full fits plus warm-ups. These lines lack row labels, so they are not presented
as full-fit invocation counts. The first tracing pass did not enable the
separate checkpoint trace flag; [the correction](trace-correction.md) preserves
those six calls. Three additional auto calls activate that flag through a local
post-sanitization environment wrapper, leaving source, threads and inputs intact.

Full-fit checkpoint traces separate compact rows from warm-ups and reveal:

- Dense unit: three LSMR projection attempts, RHS widths 52/51/1, all passing.
- Strong FW: widths 52/51/51/1. The first strict 51-RHS attempt fails its
  independent 1e-12 certificate at 1.1663e-12; the existing 1e-14 recurrence
  retry passes at 3.1819e-15.
- Dense PW: widths 52/51/51/1. The first strict attempt fails at 1.1577e-12;
  the existing tighter retry passes at 9.8004e-15.

All retry work is included in the untraced totals. The solver matrix reports
two LSMR checkpoint observations and one MAP observation per auto fit; those
aggregated observations do not equal every underlying projection attempt.
The [direct attempt records](checkpoint-attempt-summary.json) retain all widths,
setup/solve/certificate times and both rejected first certificates. No tolerance
is weakened. Instrumented timings are excluded from the route screen.

## Next implementation boundary

Test a bounded current-weight Schur checkpoint application using existing
storage and recurrence, with full probe/fallback accounting and unchanged
canonical rank/reconstruction/final-Hessian authority. Preserve original-input
LSMR fallback, reference norms, fatal-error propagation, memory preflight and
weak-graph alternatives. The existing weighted checkpoint retries are a measured
cost to compare, not permission to relax their certificates.

This screen does not complete P4's Schur-application item or broader P0 counters.
There is no new automatic eligibility region or accepted optimization. Repeated
candidate comparisons and protected weak/weighted-cut/narrow/low/generated/slope/
three-FE cases follow only after a concrete prototype. All [18 captures](captures.json)
and [phase/memory results](summary.csv) remain archived. Installed dist, unrelated
work and historical receipts are intact; no push, agents, cluster, upstream or
R1–R5 work occurred.
