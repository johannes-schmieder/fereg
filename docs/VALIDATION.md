# Validation status

The many-FE, two-FE structural-gate and ownership/deferred-cell improvements are
accepted, as are the many-FE geometry policy with one documented timing exception
and the PPML two-FE CMG reuse and lean protocol released in v0.5.0-beta.7.
The OLS method-reporting correction changes diagnostic labels and
fallback flags only; solver selection, iteration budgets, numerical calculations
and PPML reporting are unchanged.

## Accepted results

| Change | Evidence and scope |
| --- | --- |
| Many-FE Auto | [Final compact result](../bench/results/incremental-many-20261018-compact.json): Linux qualification and practical Mac recovery passed all 126 protected comparisons per platform; public activation and independent science passed. |
| Two-FE structural gates | [Linux](../bench/results/two-fe-policy-gates-qualification-linux-20261022.json) and [Mac](../bench/results/two-fe-policy-gates-qualification-mac-20261022.json): all 84 protected bounds per platform passed. [Activation](../bench/results/two-fe-policy-gates-activation-20261023.json): exact public/hidden science, worker, gate, hierarchy and resource checks passed after the documented transport-accounting amendment. |
| Ownership and deferred cells | [Linux](../bench/results/ownership-qualification-final-linux-20261024.json): 10,440 calls, 58 settings and 12 distinct physical hosts, all 116 protected bounds passed. [Practical Mac check](../bench/results/ownership-qualification-practical-macos-20261024.json): 870 calls and all 116 paired-median comparisons within max(5%, 1 ms). The Mac check is descriptive, not a confidence-bound claim. |
| Many-FE geometry Auto | [Decision record](../bench/results/many-geometry-20261025.json): Linux qualification passed 80 of 84 protected comparisons and public activation 122 of 126, on 12 distinct physical hosts each. Every miss is the near-confounded third FE, accepted by Johannes as a documented exception; public/hidden agreement was exact. |
| PPML two-FE CMG reuse and lean protocol | [Lean record](../bench/results/ppml-two-fe-lean-20261026.json): every two-FE cell faster than beta.6 at 4 and 28 workers on Linux (0.32-0.84), science within the §20 envelopes, 73/73 replay cases. Merged and released as v0.5.0-beta.7; the [beta.7 SCC comparison](fixest_benchmarks.md#october-26-2026-beta7-rerun-with-pyfixest-and-areg) found all 168 native fits passing and every OLS fit bit-identical to beta.6. |
| Two-FE Auto speed | [Qualification record](../bench/results/two-fe-auto-qualification-20261027.json): Linux 7,560 calls on 31 physical hosts, all 126 protected comparisons within max(5%, 1 ms) after Johannes waived the 12-distinct-host rule (84 cells had 11); practical Mac check 119 of 126 passed, the other 7 inconclusive with paired medians within the bound. [Veneto comparison](../bench/results/two-fe-auto-veneto-20261027.json): 1.24-2.08 times faster than beta.7 without saved FE and 1.06-2.18 with saved FE; faster than PyFixest's `within` in 9 of 10 cells. |
| Two-FE Auto round two | [Round-two record](../bench/results/two-fe-round-two-20261027.json): Linux 10,080 calls on 28 physical hosts, 123 of 126 protected comparisons within max(5%, 1 ms) after the 12-distinct-host waiver (every cell had 11) and 3 inconclusive under 1 ms; Mac check 115 of 126 passed, 11 inconclusive; Veneto candidate faster than beta.8 in all 20 cells and than PyFixest `within` in 9 of 10; PPML exact timing 12 of 12 within bound with identical science. |
| Public integration | [Final activation](../bench/results/ownership-public-activation-20261024.json): 174 actual hidden/public/public-repeat calls per platform, independent witnesses, fixed-worker exactness, cross-worker science, resource/gate/hierarchy checks, Mac/Linux ABI/load/panic checks and the full source suite passed. Six licensed Mac Stata fixtures and actual public OLS Break/rollback passed. |

Ownership/deferred cells activate only for OLS Auto/Auto with a global intercept
and exactly two categorical-intercept FE terms. Explicit injections and paths
outside the scope retain their contracts. The Break test interrupts at the
native pre-preparation checkpoint; it does not establish interruption inside a
cell constructor. Scientific behavior is specified in [SPEC](SPEC.md).

## Two-FE Auto speed

Public OLS Auto/Auto with exactly two categorical-intercept FEs applies its CMG
hierarchy on the worker team, runs the deferred cell operator in parallel,
builds the hierarchy without CMG's low-effective-degree correction and hands
off early under gate v2; Fast fits without saved outputs certify their within
residual by its derived bound. Specified in [SPEC](SPEC.md) and
[the gate notes](two_fe_auto_gates.md); the investigation behind it is in
[the PyFixest comparison](pyfixest_twoway_investigation.md).

The fixed-candidate qualification used beta.7 public Auto as original and
incumbent and the hidden `two-gates-v2` arm, with the 42 routing cells. Linux
passed every protected comparison; the weak and high-degree-weak cells ran
4-11% faster and the others within noise. Science, gate decisions, resources
and cross-worker agreement passed. The routing cases all request saved outputs,
so they do not exercise the derived residual; its witnesses are the dense
explicit-OLS oracle, the 68 licensed Mac Stata fixtures and the Veneto runs.

On one whole SCC node with the audited Veneto panels (seconds, medians of three):

| Panel, workers | Candidate | beta.7 | PyFixest `within` |
| --- | ---: | ---: | ---: |
| short, 1 | 2.01 | 2.49 | 2.08 |
| short, 4 | 1.03 | 1.85 | 1.08 |
| short, 16 | 0.78 | 1.61 | 0.96 |
| long, 1 | 10.53 | 13.32 | 26.94 |
| long, 4 | 5.28 | 8.54 | 9.84 |
| long, 16 | 3.62 | 7.08 | 6.23 |

Every fixed-worker repeat was exact and every cross-worker, beta.7 and
PyFixest comparison passed the campaign tolerances. The 73-case replay corpus is
exact against beta.7's runtime, and PPML's campaign two-sparse design ran 2%
faster from the hierarchy options with the other PPML designs unchanged.

## Two-FE Auto round two

Round two keeps beta.8's policy and changes three mechanisms, specified in
[SPEC](SPEC.md) and [the gate notes](two_fe_auto_gates.md): the (worker, firm)
cells are built on the worker team, bit for bit as the serial build, and before
the diagonal pilot, so the pilot's Schur products use the cell operator, with
block cancellation polls in the serial kernel; the pinned CMG (`f6a044c9`)
dispatches a level's row-operator matvec to the team when its entries times
lanes reach the parallel threshold, so dense coarse levels with few rows no
longer run serially; and PPML's joint recurrence merges helper products, forms
the finite cross block and the level masses in exact parallel form and reuses a
finished canonical probe. The CMG and PPML changes are arithmetic-preserving;
the cells change moves low bits of the pilot within the certified tolerances
and is bound by the replay scope
[`2026-10-27-two-fe-eager-cells-macos.json`](../tests/replay/scopes/2026-10-27-two-fe-eager-cells-macos.json)
(three two-FE cases; solver codes, iterations, stop reasons and every discrete
decision unchanged). The [round-two record](../bench/results/two-fe-round-two-20261027.json)
holds every number below.

The fixed-candidate Linux qualification (beta.7 public Auto as original, beta.8 as
incumbent, the hidden `two-gates-v2` arm, 42 routing cells, 3 shards by 12
replicas, 10,080 native calls on 28 physical hosts) reported `needs_discussion`
because every cell reached 11 of the required 12 distinct physical hosts;
Johannes waived that rule for this round as for the previous one. Under the
waiver 123 of 126 comparisons satisfy max(5%, 1 ms) and three are inconclusive
by under a millisecond (sixteen-fe-t4 and small-retained-t4 against the
incumbent, small-retained-t4 against the original, medians below zero). No
scientific, gate, resource or cross-worker check failed and no native call was
repeated. Against beta.8 the repeated-cells, disconnected and wide-rhs cells
ran 19-21% faster at one worker and the high-degree and weak cells 10-15%;
weighted-t1 ran 4% faster, so the eager build's cost on that no-aggregation
design needs no exception on Linux.

The practical Mac check (42 cells, blocks of 6, 1,638 native calls, beta.8 as
original and incumbent) passed 115 of 126 comparisons with 11 inconclusive and
no regression or scientific error; weighted-t1, which the first eager-cells
candidate had regressed by 9% because the serial kernel polled once per cell,
closed to a paired ratio of 1.021 after the block-poll change. The licensed
Mac Stata development profile passed its 68 fixtures on a fresh build. On the
synthetic Veneto-shaped AKM panels (profiling builds, no competing process),
native time relative to beta.8 at 1/2/4/8 workers was 0.944/0.931/0.937/0.886 on the short panel
and 0.949/0.887/0.884/0.914 on the long panel. Attribution on the same
panels: eager cells cut the pilot's operator phase (long panel at two workers
0.72 s to 0.41 s) and the entries-gated matvec cut the short panel's recurrence
by 14/32/40% at 2/4/8 workers (its 3,043-row level is newly parallel; the long
panel's planned levels all exceed 4,096 rows and its dispatch is unchanged).

On one whole SCC node with the audited Veneto panels, three rotated
repetitions, every fixed-worker repeat exact and every cross-worker, beta.8 and
PyFixest comparison eligible (seconds, medians):

| Panel, workers | Candidate | beta.8 | PyFixest `within` | vs beta.8 | vs `within` |
| --- | ---: | ---: | ---: | ---: | ---: |
| short, 1 | 1.81 | 2.01 | 2.07 | 0.90 | 0.88 |
| short, 2 | 1.39 | 1.42 | 1.34 | 0.97 | 1.03 |
| short, 4 | 1.03 | 1.06 | 1.08 | 0.97 | 0.95 |
| short, 8 | 0.78 | 0.82 | 0.94 | 0.95 | 0.82 |
| short, 16 | 0.73 | 0.78 | 0.96 | 0.94 | 0.77 |
| long, 1 | 9.64 | 10.53 | 26.95 | 0.92 | 0.36 |
| long, 2 | 6.96 | 7.38 | 15.42 | 0.94 | 0.45 |
| long, 4 | 4.83 | 5.29 | 9.84 | 0.91 | 0.49 |
| long, 8 | 3.54 | 3.99 | 7.04 | 0.89 | 0.50 |
| long, 16 | 3.22 | 3.63 | 6.19 | 0.89 | 0.52 |

With saved fixed effects (PyFixest's saved effects exceed the command limit):

| Panel, workers | Candidate | beta.8 | vs beta.8 |
| --- | ---: | ---: | ---: |
| short, 1 | 2.83 | 3.27 | 0.86 |
| short, 2 | 2.11 | 2.14 | 0.98 |
| short, 4 | 1.49 | 1.51 | 0.99 |
| short, 8 | 1.11 | 1.15 | 0.97 |
| short, 16 | 1.03 | 1.13 | 0.91 |
| long, 1 | 13.19 | 14.61 | 0.90 |
| long, 2 | 9.68 | 10.25 | 0.94 |
| long, 4 | 6.61 | 7.15 | 0.93 |
| long, 8 | 4.78 | 5.35 | 0.89 |
| long, 16 | 4.31 | 4.97 | 0.87 |

PPML exact timing (campaign-DGP cases through `bench/ppml_cases.py` and
`bench/time_exact.py`, one whole node, five alternating repetitions, science
identical between beta.8 and the candidate in every run; candidate over
beta.8 native-time ratio, candidate median in parentheses):

| Design, rows | 4 workers | 28 workers |
| --- | ---: | ---: |
| two-sparse, 1,000,000 | 0.991 (5.66 s) | 0.984 (4.01 s) |
| two-sparse, 4,000,000 | 0.990 (25.42 s) | 0.967 (17.23 s) |
| akm-baseline, 1,000,000 | 0.983 (4.08 s) | 0.990 (2.74 s) |
| akm-baseline, 4,000,000 | 0.985 (18.32 s) | 0.995 (12.15 s) |
| two-dense, 1,000,000 | 0.996 (3.83 s) | 1.005 (2.48 s) |
| two-dense, 4,000,000 | 0.996 (14.76 s) | 0.999 (10.07 s) |

All twelve PPML cells pass the nonregression bound. The team application of
CMG to PPML's joint frames above 350,000 finest edges was built, measured at
0.98 on the long Poisson panel at 8 workers and removed; separation detection
is 42-61% of PPML native time on Veneto-scale Poisson panels and is the
subject of a separate workstream.

## Many-FE geometry policy

Public OLS Auto/Auto for at least three categorical-intercept FEs uses the
geometry policy specified in [SPEC](SPEC.md#qualified-auto-routing-and-developer-comparisons).
Johannes accepted it on 2026-10-25 with one documented exception, below. The
`many-geometry-v1` arm and `routing_many_geometry_v1` token select the same
policy for comparisons; hidden `routing_incremental_many` keeps the previous one.

Linux evidence used the candidate plugin built at `99f0134d` with the release
toolchain and the beta.5 plugin (`1ac7125b`) as original and incumbent. Each of
12 replicas ran on its own whole generic 28-core node, all arms in the same
allocation; accounting recorded `failed=0` and exit 0 for every replica. Ratios
are candidate/beta.5 median native-call times; bounds are one-sided .025
bounds on the excess over max(5%, 1 ms) across physical hosts.

| Cell, 1 / 4 workers | Qualification | Public activation |
| --- | ---: | ---: |
| Four FEs | 0.68 / 0.64 | 0.68 / 0.64 |
| Eight FEs | 0.60 / 0.61 | 0.60 / 0.62 |
| Sixteen FEs | 0.62 / 0.67 | 0.61 / 0.67 |
| Small third FE | 0.65 / 0.63 | 0.65 / 0.61 |
| Near-confounded third FE | 1.125 / 1.066 | 1.122 / 1.063 |
| All other cells | 1.00-1.05 | 0.98-1.02 |

Qualification (5,040 native calls) passed 80 of 84 protected comparisons.
Public activation (7,560 calls) compared public Auto with the selected hidden
arm and passed 122 of 126, with exact scientific agreement and solver
signatures between public and hidden calls, fixed-worker repeatability and
cross-worker agreement. In both runs the misses were the near-confounded third
FE against both references: a one-worker regression (excess +16.1 and +15.4 ms,
lower bounds +10.7 and +11.2 ms) and an inconclusive four-worker result
(+1.9 and +1.8 ms, bounds spanning zero). This design needs the additive
preconditioner from the start; the Jacobi trial spends a few iterations before
escalating and finishes in 19 rather than 17 LSMR iterations. Johannes accepted
this cell as a documented exception rather than requiring a successor. A
`within` change sharing one prepared design between the trial and additive
solvers would remove roughly half of that overhead.

Two harness and wiring amendments, approved by Johannes before measurement,
made the contracts match the policy: the scientific harness checks the geometry
arm at its requested certificate and admits its MAP pilot, rather than the
unseeded portfolio's private hundredth; and hidden `routing_many_geometry_v1`
keeps public two-FE ownership/deferred-cell activation so public/hidden
agreement is exact on every design. Operational failures are recorded in the
decision record: an earlier run blocked on the old certificate contract before
any timing, one bench unit test lost a timer race and passed on its single
retry, and two activation replicas failed to open the shared placement lock
before any native call and were resubmitted.

Before qualification, `./scripts/run_tests.sh --source-only` passed (268 Python tests, Rust unit and
independent numerical integration tests, Clippy, FFI allocation audits,
portable checks, mock SPI and the complexity ratchet). New witnesses compare the
public policy with dense explicit-dummy QR on a mixing panel that finishes on
the Jacobi trial and a weak AKM panel that escalates, with exact fixed-worker
repeats and exact public/hidden agreement. The plugin replay against main's
plugin was exact for 72 of 73 cases. In `ols-30`, main rejected a three-FE
LSMR solve at the private hundredth of the request and fell back to 28 MAP
sweeps with a warning; the candidate accepts a 12-iteration solve at the
request. Its 502 numerical leaves stay within the
[declared scope](../tests/replay/scopes/2026-10-25-many-fe-geometry-linux.json)
and science passes. The used-solver, fallback and warning fields are the
intended route change. Four-worker repeats and cross-worker science passed for
every case. The suite and replay were rerun after the recurrence margin below
and after merging main.

On Apple Silicon, the source-only suite passed at `7b641ceb` (319 Python tests).
The candidate's universal Mac plugin passed its binary audit and actual
ABI/load/panic checks. Its replay against main's Mac plugin was exact for 72 of
73 cases. `ols-30` takes the same route change as on Linux, with 510 numerical
leaves inside a separate
[macOS scope](../tests/replay/scopes/2026-10-25-many-fe-geometry-macos.json),
because the macOS replay input has its own hash. Science, four-worker repeats
and cross-worker checks passed for every case. Fourteen focused Stata 19/MP
fixtures with three or more absorbed effects passed, including dense-dummy,
reghdfe parity, saved-output, memory-mode and LSMR-reuse checks, and so did
public OLS Break/rollback. Two development fixtures had asserted main's route
and were updated without changing any numerical threshold:
`ols_three_fe_map_oracle.do` now expects the geometry pilot to certify the
final-residual projection with MAP, and `ols_mixed_solver_reports.do` expects
the accepted LSMR solve where main fell back to MAP. Main's plugin still passes
both original fixtures; the candidate agrees with the dense references within
4e-14 in both.

Local diagnostic timing on a shared cloud VM, four workers, alternating
candidate/main replays, medians of five runs (ten for the last two rows),
`NATIVE_SECONDS`. These are descriptive engineering checks, not qualification
evidence; repeated runs varied by several percent.

| Design | Rows | Candidate/main |
| --- | ---: | ---: |
| Three dense FEs, unit weights | 100,000 / 1,000,000 / 4,000,000 | 0.55 / 0.87 / 0.61 |
| Three dense FEs, analytic weights | 100,000 / 1,000,000 / 4,000,000 | 0.96 / 1.01 / 0.83 |
| AKM-like three sparse FEs | 1,000,000 | 0.38 |
| Mixing designs with 3, 4, 8 and 16 FEs | 100,000 | 0.52-0.62 |
| Near-confounded third FE | 100,000 | 1.02 |
| Campaign-shaped three sparse FEs | 1,000,000 | 1.04 (1.03-1.10 across runs) |

Stopping the recurrence a decade below the request instead of a hundredth cut
one to three LSMR iterations from every LSMR-routed solve above (for example
17 to 15 on the AKM-like design and 26 to 23 on the campaign-shaped one). No
case needed the correction. Against the candidate without this margin, ten
alternating runs on a slower replacement VM gave 0.95 on both one-million-row
sparse designs; the 100,000-row ratios (0.97-1.06) were within that host's
noise.

The last two rows need the additive preconditioner from the start; the Linux
near-confounded result above is the corresponding accepted exception. Weighted
dense designs do not reach the unit-weight pilot.

A one-pass shifted projection route from the research plan failed two existing
witnesses (CG energy roundoff and tiled-bank cross-thread identity), so MAP
projections keep the scaled two-pass route. A bit-identical cache-blocked
row-streaming kernel for dense group statistics and certificate norms was
faster on the development Mac but slower on Linux at 4, 8 and 28 workers.
Complete four-million-row dense three-FE fits took 5% longer at four workers
and 21% longer at 28, so it was not adopted
([record](../bench/results/r4-row-streaming-20261026.json)).

## PPML two-FE CMG reuse and lean protocol

PPML Auto with exactly two intercept FEs reuses one CMG hierarchy across IRLS
frames and canonical products, and publication fits enter the strict
checkpoint once the predictor settles, as specified in
[SPEC](SPEC.md#qualified-auto-routing-and-developer-comparisons) and §13.
Johannes accepted the result; it ships in v0.5.0-beta.7. The
[lean record](../bench/results/ppml-two-fe-lean-20261026.json) retains every
timing of the final qualification and the
[CMG record](../bench/results/ppml-two-fe-cmg-20261026.json) the earlier
CMG-reuse round.

On one whole node (`scc-wk1`, Xeon E5-2680 v4), campaign-generator PPML inputs
with 16 regressors ran five rounds per cell with rotated arm order: shipped
beta.6, the CMG-reuse head `7db43e41` and the lean head `277c5d11`. Times are
medians; ratios are medians of paired rounds.

| Design | Rows | Workers | beta.6 s | `7db43e41` s | Lean s | Lean/beta.6 | Lean/`7db43e41` |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Two sparse FEs | 1,000,000 | 4 | 17.75 | 11.05 | 5.82 | 0.324 | 0.528 |
| Two sparse FEs | 1,000,000 | 28 | 10.86 | 7.56 | 4.13 | 0.380 | 0.543 |
| Two sparse FEs | 4,000,000 | 4 | 81.30 | 45.28 | 26.37 | 0.327 | 0.582 |
| Two sparse FEs | 4,000,000 | 28 | 48.36 | 32.92 | 18.04 | 0.374 | 0.549 |
| AKM | 1,000,000 | 4 | 5.07 | 5.02 | 4.20 | 0.833 | 0.850 |
| AKM | 1,000,000 | 28 | 3.36 | 3.34 | 2.76 | 0.820 | 0.826 |
| AKM | 4,000,000 | 4 | 22.94 | 22.65 | 18.66 | 0.811 | 0.828 |
| AKM | 4,000,000 | 28 | 14.71 | 14.55 | 12.29 | 0.835 | 0.845 |
| Two dense FEs | 1,000,000 | 4 | 4.70 | 4.54 | 3.88 | 0.825 | 0.852 |
| Two dense FEs | 1,000,000 | 28 | 3.01 | 2.94 | 2.49 | 0.823 | 0.845 |
| Two dense FEs | 4,000,000 | 4 | 20.30 | 19.93 | 14.90 | 0.734 | 0.755 |
| Two dense FEs | 4,000,000 | 28 | 12.42 | 12.17 | 10.15 | 0.819 | 0.831 |

Against beta.6, coefficients and covariance stay within 2e-5 and 7e-10 of the
§20 envelopes (largest on the sparse design, whose published state is one
Newton step earlier), and sample, status, DoF, drop, separation and FE
fields are exact in every cell. Every arm repeats bit for bit across rounds;
4- and 28-worker lean results agree within 1e-8 and 2e-10 of the envelopes
with exact discrete fields. IRLS iteration counts are unchanged except the
1M-row sparse design (8 instead of 9). The sparse design defers its strict
checkpoint once (1M) or twice (4M) and passes the first one. The earlier
CMG-reuse round had left AKM four-worker cells at 1.048-1.062; all cells are
now faster than both baselines.

The 73-case replay corpus passes its independent dense science targets in
every case with no discrete, repeat or thread differences. All OLS cases are
bit-identical to `7db43e41`; seven PPML cases differ by at most 1.2e-14
relative in coefficients and covariance. New witnesses cover the settling
rule, small and shared-CMG implicit components, the implicit assembly choice,
team-independent joint checks and Gram, refreshes and the three-attempt
bound. The full source-only suite passed at `277c5d11`. A parallel CMG
application, a row-tiled certificate Gram and a data-based IRLS start were
measured and rejected (slower, or no fewer iterations).

### PPML two-FE parity work (development, Mac)

PPML exact-timing protocol: campaign-DGP PPML cases are written by
[`bench/ppml_cases.py`](../bench/ppml_cases.py) (maintained generator, policy
seed, 16 regressors, robust VCE, `separation(fe simplex ir)`, tolerance 1e-8,
iterate 10000) and timed with [`bench/time_exact.py`](../bench/time_exact.py),
which guards exact science between the plugins it compares. The beta.7 lean
record's case generator was never committed; this tool reproduces its protocol
for arithmetic-preserving PPML candidates. Result-changing PPML candidates
still need an extension of `bench/qualify.py`, which refuses PPML today.

On an M2 Ultra Mac (development measurement, three repetitions per cell,
medians of repetitions 2-3, arms interleaved, no competing build/test/Stata
process observed), the exact parallel joint sections and the reuse of a
finished canonical probe changed no bit of `e(b)` or `e(V)` in any of 17
cells (campaign-generator AKM, sparse and dense designs at 1M rows with 16
regressors at 1, 4 and 8 workers; synthetic Veneto-shaped Poisson panels of
1.39M and 5.63M rows at 1, 2, 4 and 8 workers), and the 73-case replay was
exact against beta.8. Native time was 0.975-0.986 of beta.8 on the campaign
designs at 4 and 8 workers and 0.994-1.009 on the Veneto panels. Separation
IR is 42-61% of PPML native time on the Veneto panels at 4 and 8 workers;
the joint frames' scalar CMG cycle is 5-8% on the long panel. A team
application of the PPML hierarchy above CMG's 350,000-edge single-RHS
minimum (row-operator plan only) left results bit-identical but changed the
long panel's native time by 0.980-1.013 (1-8 workers) against the same build
without it, short of the 5% acceptance margin; it was not retained.

### PPML separation routing (development, Mac; activated for beta.10)

The iterated rectifier was 42-66% of PPML native time on the Veneto-shaped
Poisson panels. On both panels the separation chain's IR phase is one graph
proposal followed by a single rectifier projection that certifies no further
separation at iteration 1: a weighted finite-plus-FE projection of 3 (short) or
12 (long) columns with zero-outcome weights near 1e-21, solved by
retained-product CG (60-61 iterations, no preconditioner hierarchy). The
hidden `inject=ppml_separation_auto` arm solves admitted two-intercept detector
projections with the OLS two-FE Auto route instead; the detector's weights,
tolerances, iteration limit and decision rules are unchanged and the public
default is not changed. Measured on an M2 Ultra (three repetitions per cell,
medians of repetitions 2-3, arms interleaved, no competing process; the
[record](../bench/results/ppml-separation-routing-2026-10-27.json) keeps every
cell):

| Panel | Workers | Public native s | Hidden native s | Ratio | Public IR s | Hidden IR s | Ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Short (1.39M) | 1 | 14.03 | 6.25 | 0.446 | 9.02 | 1.24 | 0.137 |
| Short (1.39M) | 4 | 6.43 | 4.29 | 0.666 | 3.23 | 1.04 | 0.321 |
| Short (1.39M) | 8 | 5.01 | 3.68 | 0.735 | 2.11 | 0.79 | 0.374 |
| Long (5.63M) | 1 | 112.08 | 46.17 | 0.412 | 74.09 | 8.70 | 0.117 |
| Long (5.63M) | 4 | 43.48 | 23.91 | 0.550 | 26.50 | 7.10 | 0.268 |
| Long (5.63M) | 8 | 29.64 | 18.92 | 0.638 | 15.60 | 5.00 | 0.320 |

On both panels at 1, 2, 4 and 8 workers, on four campaign-generator
separation designs (dense, AKM strong and weak mobility, probability weights)
and on the replay corpus, `e(b)`, `e(V)`, the separated rows, drop counts and
`e(feppml_separation_sequence)` are identical between the arms, and
`e(feppml_separation)` differs only in the rectifier's interior certificate
magnitude; the separation trace digest in the receipt hashes that magnitude
and therefore also differs on `ppml-separation-ir`. Each arm repeats bit for
bit. The public replay is exact (73 of 73 cases) with the arm compiled in.

The witnesses do not hold on every case. A seeded census
(`crates/fereg-oracle/tests/ppml_separation_routing.rs`: 18 small chain,
sparse-AKM and dense designs, some with induced finite-regressor separation,
three separation configurations, public tolerances) is identical in 32 of 54
cells. Where both arms succeed the separated set never differs, but in 9 cells
the hidden arm certifies the terminal rectifier at iteration 1 where CG needs
2, which changes RSEP and RSEQ iteration counts. In 9 cells retained CG fails
a projection certificate and the public fit errors while the hidden arm
completes; in 2 cells the Auto route exhausts its iteration budget where the
public fit succeeds; 2 cells fail in both arms with different errors. Under
`fe simplex ir` the divergences are 5 iteration-count cells and 3 public
failures. The hidden arm is deterministic at fixed workers and its decisions
agree across 1, 2, 4 and 8 workers wherever it succeeds. In an isolated unit
test, a Break at every host poll inside a rectifier projection returns
`Interrupted` on both arms.

The public failures were a detector defect, now fixed (SPEC §14, the
[record](../bench/results/ppml-separation-routing-2026-10-27.json)'s
`post_fix_census`). Retained CG solves the rectifier projection on the
weighted normal equations, which square the zero-row weight
`tolerance^2 / zeros` (about 1e-18): on six cells the proposed direction left
the model span (independent least-squares span distance 0.14-0.19 on seeds 11
and 17, 2e-8 on seed 5) and failed the deletion certificate, r(430). On three
`fe simplex ir` cells the span check of an exact simplex direction lying in
the FE span failed its finite score certificate, normalized by FE-solver
noise, r(459). A detector step that fails such a certificate now runs once
more with LSMR under the same rules and certificates. All 9 cells complete
and retain exactly the sample of an independent HiGHS linear program
(maximal recession-cone separation alternated with singleton pruning), which
every completing arm of all 54 cells also matches. Every other cell, in both
arms, is unchanged bit for bit (full separation report, `e(b)`, `e(V)` and
error messages). Seed 3 (2 cells) still fails in both arms: its CG direction
has the right rows but span distance 1.8e-10 over the 1e-10 tolerance, and
LSMR ends 15% over its FE certificate. The replay corpus is exact against the
published beta.9 plugin (73 cases). Through Stata, the four campaign
separation designs (4 and 8 workers) and both Veneto-shaped Poisson panels
(1, 2, 4 and 8 workers) give identical `e(b)`, `e(V)`, separation matrices,
drop counts and dropped rows with no re-execution; run through Stata, the
seven fixed census cells that return r(430) or r(459) under beta.9 complete
with the LP sample (seed 3 stays r(430)). Both arms repeat
bit for bit at fixed workers and agree across 1, 2, 4 and 8 workers.

After the fix the census is identical in 35 of 54 cells and differs in 15
iteration-count cells (equal sample and fit), 2 hidden-route failures and the
2 seed-3 cells; under `fe simplex ir` only 5 iteration-count cells differ.
The two hidden failures (seed 9 without simplex) occur in the first rectifier
projection, not through the rectifier's iteration limit: the Auto route's
two-FE PCG accepts the final residual absorption at iteration 0 on its
recurrence proxy, the FE certificate (the same as CG's) rejects it, final
residual absorptions have no PCG correction, and the MAP fail-safe exhausts
its 16,000 iterations; retained CG certifies the same residual in 8
iterations.

The residual was accurate; the certificate's arithmetic was not. The
categorical FE certificate (shared by two-FE PCG, retained CG and MAP) forms
each level mean as the first member's value plus a compensated sum of
weighted differences. When that member is a zero-outcome row with a residual
of order one, the positive rows' residuals (about 1e-18) vanish in their
differences and the mean carries one ulp of the shift, which against the
rectifier's weighted residual norm (9.1e-9) is of the order of the 1e-8
tolerance: exact relative moments at most 2.7e-10, computed 1.06e-8, and MAP
corrections below one ulp leave every bit unchanged for 16,000 sweeps; retained
CG's later iterates certified (8.5e-21). A stop scaled like the certificate
would not remove that floor. Since 2026-10-28 an Auto-routed detector step
that fails a numerical certificate takes no MAP continuation and runs once
more as retained CG under the same budget and rules, then continues exactly
as a retained-CG step. Both seed-9 cells complete with the LP-witness sample
and the seed-3 cells report the retained-CG error in both arms; every other
census arm-cell is bit-identical at 1 and 4 workers (full report with
certificate magnitudes, `e(b)`, `e(V)`, errors). The census is then 39 same
and 15 iteration-count cells.

#### Activation (beta.10 candidate, 2026-10-28)

Johannes authorized activating the Auto route for beta.10 with a declared
scope for the iteration-count class (SPEC §14): the separated set, sample,
`e(b)` and `e(V)` are unchanged on every witness; `RSEP` iteration counts and
certificate magnitudes, `RSEQ` iterations and the receipt's separation trace
digest may change. `inject=ppml_separation_cg` keeps the retained-CG path.
Witnesses, all on the activated arm64 profiling candidate against the
published beta.9 plugin with the same ado (the [record](../bench/results/ppml-separation-routing-2026-10-27.json)'s
`activation`):

- Census (`ppml_separation_routing.rs`, CG arm vs default): all 54 cells
  agree in sample and fit (39 identical, 15 iteration-count cells, no
  failure class); every completing arm retains the LP-witness sample;
  fixed-worker repeats and cross-worker decisions hold at 1, 2, 4 and 8
  workers.
- Stata: the four campaign separation designs (4 and 8 workers) and both
  Poisson panels (1, 2, 4 and 8 workers) give `e(b)`, `e(V)`,
  `e(feppml_separation_sequence)`, drop counts, `e(feppml_fit)` and the
  dropped rows identical to beta.9 at every worker count (1,794, 430, 1,078,
  430, 7,498 and 1,536 dropped rows); `e(feppml_separation)` differs only in
  the rectifier row's interior certificate magnitude and the receipt only in
  the separation trace digest. Each worker count repeats bit for bit; dropped
  rows, drop counts, the separation sequence and the discrete separation
  columns agree across worker counts (coefficients within 2.1e-12 relative).
  No detector re-execution occurred. The CG arm reproduces beta.9 exactly,
  separation matrices and receipt included, on all six designs.
- Replay (`scripts/run_plugin_replay.sh`, 73 cases): 72 exact; only
  `ppml-separation-ir` differs, in `RSEP` zero-based 22 (the rectifier row's
  interior certificate, 2.69e-20 to 2.39e-20) and `RREC` 28-29. The replay
  harness cannot declare that change: an affected scope accepts only
  numerical envelope leaves (RFIT, RCERT, RINF, RSOLVER) and rejects `RSEP`
  and `RREC` leaves as nonnumerical transaction fields, while its discrete
  view protects the whole `RREC` table. The harness was not changed; the
  scope file `tests/replay/scopes/2026-10-28-ppml-separation-activation-macos.json`
  is not written. Declaring this class needs Johannes's decision on a typed
  separation-trace scope in `scripts/replay_plugins.py`.
- Break: an isolated unit test cancels at every host poll inside a rectifier
  projection of the activated policy (Auto, no MAP continuation, retained-CG
  re-execution) and of retained CG; every stop returns `Interrupted` and is
  not re-executed.

Timing (frozen rule: before each cell, no other Stata, cargo, rustc or Python process
and no other agent's timing driver for 60 s, a 10-s monitor during the cell, discard and
repeat once on a competitor; one cell, short at 1 worker on the candidate, was discarded
for a rustc process and repeated cleanly). arm64 profiling builds of the activated tree and
of v0.5.0-beta.9, Stata 19/MP on the M2 Ultra, three repetitions per Stata process, medians
of repetitions 2-3, arms alternating first per cell, `FEREG_PHASE_TIMING=1`:

| Panel | Workers | beta.9 native s | Activated native s | Ratio | beta.9 IR s | Activated IR s | Ratio |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Short (1.39M) | 1 | 14.72 | 6.93 | 0.471 | 9.32 | 1.38 | 0.148 |
| Short (1.39M) | 4 | 6.50 | 4.38 | 0.674 | 3.27 | 1.05 | 0.322 |
| Short (1.39M) | 8 | 5.03 | 3.67 | 0.729 | 2.13 | 0.77 | 0.363 |
| Long (5.63M) | 1 | 87.88 | 38.11 | 0.434 | 57.01 | 7.29 | 0.128 |
| Long (5.63M) | 4 | 34.19 | 20.55 | 0.601 | 19.81 | 6.23 | 0.314 |
| Long (5.63M) | 8 | 23.81 | 16.23 | 0.682 | 12.03 | 4.35 | 0.361 |

The rectifier's share of native time falls from 42-65% to 19-30%.

Seed 3 (cells (3,1) and (3,2)) remains a documented limitation. With a
60-digit reference projection the rectifier's exact direction flags the 20
LP-witness rows and passes every deletion certificate (span 6e-19). Retained
CG's fitted values are wrong by up to 2.3e-10 on zero-outcome rows only
(4.9e-17 on positive rows; `u - r` formation is exact), which the weighted
FE certificate cannot see at weight 1e-18 but the unit-weight span check
(1e-10) rejects (1.82e-10). LSMR's and Auto's directions are accurate (span
3.3e-17 and 1.7e-12, same 20 rows) but their residuals fail the FE
certificate's first-member-shift floor (computed 1.149e-8 and 4.25e-8, exact
6.3e-23 and 1.2e-18; independent of summation order). Options for Johannes,
none implemented because each changes a certificate or tolerance: shift each
certificate level by its largest-weight member (reproduces the exact moments
in emulation; shared with OLS, so it needs its own scope) or re-certify that
way only after a first-member failure; certify the span check's projection of
the direction when only the span check fails; a detector-scaled FE
tolerance; or keep the limitation.

## Package checks

### Beta.9 two-FE Auto round-two packaging

Beta.9 ships the round-two changes described above. The qualified Linux binary
is the `d6975ec2` SCC build (job 8305425), Windows was built at `8b2f38a4`
(run `win-20261027T235540Z-45d74d45`), and universal macOS was built offline
from the corresponding-source archive at `8b2f38a4`. Runtime production inputs
are unchanged across these revisions (`scripts/check_complexity.py
--compare-production`). The [package metadata](../dist/fereg-native.json)
records each actual build commit, binary checksum, runtime identity and
corresponding-source hash.

| Platform | Beta.9 checks |
| --- | --- |
| macOS | Rust 1.85.1 universal build from the vendored source archive with an empty Cargo home and offline mode; binary privacy/export audit and actual ABI/load/panic checks passed. All 67 fixtures of the full Stata 19/MP profile, the 73-case replay against the beta.8 plugin under the eager-cells scope, public OLS/PPML Break/rollback and eleven staged-package fixtures passed (the five remaining installation fixtures need the intentionally excluded development adapters or a Windows host). |
| Windows x86-64 | Run `win-20261027T235540Z-45d74d45` at `8b2f38a4` passed the Rust 1.85.1/MSVC static-CRT build, exact-FMA and fused-kernel tests, C numeric guards, PE audit, native loader checks, checksum, Stata smoke and single-runtime stages; the controller stopped the machine, deleted transient objects and released its lock. The collected plugin's SHA-256 matches the receipt. |
| Linux x86-64 | The qualified binary from build job 8305425 retains its SHA-256. Job 8308581 passed the ELF audit (glibc 2.28 maximum), actual ABI/load/panic checks and all 67 fixtures of the full Stata 19/MP profile; job 8308686 passed the same eleven staged-package fixtures from the exact binary archive (job 8308667 stopped before any fixture because the runner needs an absolute Stata executable path). |

The dependency audit found no advisories. The strict default
`./scripts/run_tests.sh` passed with the refreshed package. The source archive
SHA-256 is `7f1ab939328f94c338ceb126cb404185439f0b1bbe0857a508963c9e7547decd`;
the binary archive SHA-256 is `ae409717f30440191e1ce633695d14079714f8e74f02e6d834f2dc2de5ae7fac`. Native Intel-host Stata testing
remains outstanding; both macOS slices are supplied. Two tooling mistakes
preceded the accepted runs and changed no artifact: the first source-archive
attempt used an exported snapshot without Git metadata and the first staging
attempt lacked the plugin checksum files.

After publication, fresh Stata 19/MP sessions installed beta.9 from the public
[beta.9 URL](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.9)
and from updated `main` into isolated PLUS directories. All 35 installed files
matched the staged package bytes on both routes, the manifest bound the runtime
to `8b2f38a4`, native selftest passed, and `fereg`/`feppml` on the `auto` data
agreed with Stata's `regress` and `poisson` dummy regressions to rounding
(coefficient relative differences below 1e-13). GitHub's recorded SHA-256 and
size matched every one of the 50 published assets.

### Beta.8 two-FE Auto packaging

Beta.8 ships the two-FE Auto changes described above. The qualified Linux
binary is the `18b47675` build, Windows was built at `37f1ab89`, and universal
macOS was built offline from the corresponding-source archive at `6b97e504`.
Runtime production inputs are unchanged across these revisions. The
[package metadata](../dist/fereg-native.json) records each actual build commit,
binary checksum, runtime identity and corresponding-source hash.

The strict default `./scripts/run_tests.sh` passed with the refreshed package:
319 Python tests, Rust unit and independent numerical integration tests,
Clippy, FFI allocation audits, portable checks, mock SPI transactions,
the complexity ratchet and the root-package identity check. An independent
high-effort source review found no blocking defects in the parallel operators,
gate v2, derived-residual certification or publication validation; it did not
replace the executed checks.

| Platform | Beta.8 checks |
| --- | --- |
| macOS | Rust 1.85.1 universal build from the vendored source archive, with an empty Cargo home and offline mode; binary privacy/export audit and actual ABI/load/panic checks passed. All 44 mapped Stata 19/MP fixtures, 73 exact replay cases against the qualified Mac candidate, public OLS/PPML Break/rollback, and ten staged-package fixtures passed. |
| Windows x86-64 | Run `win-20261027T153054Z-e4e3b51f` at `37f1ab89` passed the Rust 1.85.1/MSVC static-CRT build, FMA/hardware tests, C numeric guards, PE audit and actual ABI/load/panic checks. A fresh local `net install` verified plugin bytes and passed installation smoke, single-runtime repeatability and four OLS/PPML fixtures. The controller stopped the machine, deleted transient cloud objects and released its lock. |
| Linux x86-64 | The qualified binary from build job 8298656 retains its SHA-256. Job 8299903 passed ELF audit (glibc 2.28 maximum), actual ABI/load/panic checks and 42 Stata fixtures. Two development fixtures required the intentionally excluded `_fereg_estimate_ols_test` adapter; after restoring it only in the test workspace, job 8300025 passed both. No runtime or assertion changed. Job 8300111 passed ten staged-package fixtures and a fresh local `net install`, verifying all 34 versioned files and independent OLS/PPML comparisons. |

A fresh local Mac installation of the root package verified all 35 installed
files, native selftest, exact OLS/PPML repetition and rollback, and independent
full-dummy OLS and Poisson/covariance comparisons. The dependency audit found
no advisories. The initial Mac build without the documented path remaps failed
the personal-path audit; the remapped offline rebuild is the distributed binary.
An initial local source-test invocation selected Python 3.9 and failed on missing
Python modules; the required strict suite passed in the corrected environment.
No performance observations were repeated during release preparation.

The source archive SHA-256 is
`8e89db60ba8b9111b000c68ea81d71e0bea48b5f6421f421a00e205592546c62`;
the binary archive SHA-256 is
`aafc8ce74dbe0df876a07c4375f6e34f0c860fbcd44f76d29bb7ec67a7ebd843`.
Native Intel-host Stata testing remains outstanding; both macOS slices are supplied.

After publication, fresh Stata sessions installed from the public
[beta.8 URL](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.8)
and through `github install johannes-schmieder/fereg` from updated `main`.
All 34 versioned files and all 35 main-package files matched their expected
bytes; native selftest, repeatability/rollback and the independent full-dummy
OLS and Poisson/covariance checks passed on both routes. GitHub's recorded
SHA-256 and size matched every one of the 50 published assets.

### Beta.7 PPML packaging

The beta.7 package shipped the lean PPML two-FE protocol and CMG reuse.
Its [versioned release](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.7)
records each binary's build revision, checksum and corresponding-source identity. The Linux plugin is the qualified
`277c5d11` build measured above. Windows was built at `7a33fc20`, and the
universal macOS plugin was built offline from the corresponding-source archive
packaged at `7a33fc20` with a passing dependency audit (no advisories). The
runtime identity is unchanged between `277c5d11` and `7a33fc20`.

The default `./scripts/run_tests.sh` passed at `8506a65a`: 319 Python tests,
Rust unit and independent numerical integration tests, Clippy, FFI allocation
audits, portable checks, mock SPI transactions, the complexity ratchet and the
root-package identity check.

| Platform | Beta.7 checks |
| --- | --- |
| macOS | The offline archive build passed its binary audit and actual ABI/load/panic checks. 40 Stata 19/MP fixtures passed: the 18 beta.6 release fixtures plus 22 PPML route, canonical, joint, separation, inference and parity fixtures. Public PPML and OLS Break/rollback passed, and ten installed-package fixtures passed from the staged archive. |
| Windows x86-64 | Run `win-20261026T181228Z-1bcb165d` at `7a33fc20`: Rust 1.85.1/MSVC static-CRT build, exact-FMA and hardware-kernel tests, C numeric guards, PE audit and actual ABI/load/panic checks passed. A fresh `net install` verified the installed plugin bytes, then passed the installation smoke, single-runtime and OLS/PPML fixtures in Stata 19/MP. The machine was stopped afterwards. |
| Linux x86-64 | Qualification job 8287434 built the plugin offline with Rust 1.85.1 and GCC 12.2.0; ELF audit and actual ABI/load/panic checks passed. Job 8289216 repeated the audit (glibc 2.28 maximum) and load checks and passed the same 40 Stata 19/MP fixtures; job 8289358 passed ten installed-package fixtures from the staged archive. |

The root package passed a fresh local Mac `net install`: all 35 installed files
matched, and selftest, OLS/PPML examples, a three-FE OLS comparison and a
two-FE PPML comparison with explicit dummies passed. After publication, a fresh
Stata 19/MP `net install` from the
[beta.7 release](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.7)
matched all 34 versioned package files and passed the same checks. All 50
release assets matched their local SHA-256 checksums. After the merge,
`github install johannes-schmieder/fereg` from `main` matched all 35 installed
files and passed the same checks in a fresh session.

### Beta.6 many-FE geometry packaging

The beta.6 package shipped the many-FE geometry policy. The Linux and macOS
plugins were the qualified `99f0134d` builds; Windows was built at `07d4da6d`, and the runtime
identity is unchanged between them. The corresponding source was packaged from
`07d4da6d` with a passing dependency audit (no advisories). An offline Cargo
build from that archive produced a universal Mac plugin that passed its binary
audit and ABI/load/panic checks.

The default `./scripts/run_tests.sh` passed at `78dfeca0`: 319 Python tests,
Rust unit and independent numerical integration tests, Clippy, FFI allocation
audits, portable checks, mock SPI transactions, the complexity ratchet and the
root-package identity check.

| Platform | Beta.6 checks |
| --- | --- |
| macOS | Binary audit and actual ABI/load/panic passed. The 14 focused Stata 19/MP fixtures above passed, public OLS Break/rollback passed again at `07d4da6d`, and eight installed-package fixtures passed from the staged archive. |
| Windows x86-64 | Run `win-20261026T003403Z-d67fa2b4` at `07d4da6d`: Rust 1.85.1/MSVC static-CRT build, exact-FMA and hardware-kernel tests, C numeric guards, PE audit and actual ABI/load/panic checks passed. A fresh `net install` verified the installed plugin bytes, then passed the installation smoke, single-runtime and four OLS/PPML fixtures in Stata 19/MP. The machine was stopped afterwards. |
| Linux x86-64 | SCC job 8277741 built the plugin offline with Rust 1.85.1 and GCC 12.2.0. ELF audit (glibc 2.28 maximum) and actual ABI/load/panic checks passed. Job 8280233 passed 18 Stata 19/MP fixtures, including the 14 focused many-FE fixtures; job 8280278 passed eight installed-package fixtures from the staged archive. |

The root package passed a fresh local Mac `net install`: all 35 installed files
matched, and selftest, OLS/PPML examples and a three-FE comparison with
explicit dummies passed. After publication, a fresh Stata 19/MP `net install`
from the [beta.6 release](https://github.com/johannes-schmieder/fereg/releases/tag/v0.5.0-beta.6)
matched all 34 versioned package files and passed the same checks. All 50
release assets matched their local SHA-256 checksums. After the merge,
`github install johannes-schmieder/fereg` from `main` matched all 35 installed
files and passed the same checks in a fresh session.

### Beta.5 platform packaging

The beta.5 packages added Linux and Windows binaries with the method-reporting
correction; their runtime production code was unchanged by the packaging work.
The default `./scripts/run_tests.sh` passed for the reporting correction: 316
Python tests, Rust unit and independent numerical integration tests, Clippy,
FFI allocation audits, portable checks, mock SPI transactions and the complexity
ratchet. Packaging changes reuse that runtime evidence and add focused tooling,
native and installed-package checks. No new speed claim is attached to them.

| Platform | Checks |
| --- | --- |
| macOS | Universal plugin built offline with Rust 1.85.1; binary audit and actual ABI/load/panic containment passed. All 12 mapped Stata 19/MP fixtures passed on Apple Silicon, plus public OLS Break/rollback at the pre-preparation checkpoint. |
| Windows x86-64 | Rust 1.85.1/MSVC static-CRT build; exact-FMA and eight hardware-kernel tests, C numeric guards, PE audit and actual ABI/load/panic checks passed. A fresh `net install` verified the installed plugin bytes, then passed the installation smoke, single-runtime and four OLS/PPML fixtures in Stata 19/MP. |
| Linux x86-64 | Offline Rust 1.85.1 build; ELF audit (glibc 2.28 maximum), actual ABI/load/panic and C numeric guards passed. Six Stata 19/MP fixtures passed, followed by a fresh local installation, selftest and four OLS/PPML fixtures. All 30 installed files matched. |

The scientific fixtures cover CG/CMG reporting, zero-step certificates, mixed-block
MAP fallback, independent OLS dummy-variable comparisons and PPML full-dummy
Poisson. The broader Mac set also covers weighted/wide OLS and explicit CMG-Schur.

Linux build job 8269530 produced the plugin and passed its audit/load checks,
then failed because the default compiler's sanitizer library was missing.
Its dependent installation job 8269613 stopped before testing. Job 8269823 reused
the same plugin with GCC 12.2.0: C guards and all Stata checks passed, but the file
checker rejected Stata's lowercase ancillary filenames. Exact-byte verification
with case-normalized names passed in job 8269843 (`failed=0`, `exit_status=0`).
The focused verification reused the existing installed files and test results.
These environment/checker failures required no numerical changes or estimation
retries. The failed jobs remain failed.

The Windows build harness required two corrections: it assumed exactly three
hardware-kernel tests although the suite now has eight, and its installed-file
shell call used Stata compound quotes instead of OS quotes. Native compilation,
exact-FMA/hardware checks and ABI/load/panic containment passed before the latter
failure. Short inert-file probes isolated argument quoting; the corrected probe
verified installation and exact bytes without building or loading native code.
A later cold build exceeded the controller’s 15-minute limit during the C bridge
build. The fereg build/test profile now has a fixed 30-minute allowance; other
profiles and cleanup safeguards are unchanged. Failed build/install attempts
remain failed; these were tooling repairs. Final run
`win-20261024T170546Z-5b24267c` passed at source `5b381a2c` in 884.843 seconds.
Its actual build revision and plugin checksum are recorded in the native
package metadata and release manifest. Transient objects were deleted, the lock released and the
Windows machine stopped.

The assembled three-platform main package passed a fresh local Mac installation,
selftest and four OLS/PPML fixtures; all 35 installed files matched exactly.
After publication, actual `net install` from beta.5 and `github install
johannes-schmieder/fereg` passed in fresh Stata 19/MP sessions. All 34 versioned
package files and 35 main-package files matched, including Linux and Windows
binaries; selftest and the four OLS/PPML fixtures passed in both installations.
All 50 GitHub release assets matched their local SHA-256 checksums and sizes.

## Limits

- Linux performance evidence applies to Linux. The practical Mac ownership/cell
  check reports paired medians, not confidence bounds or a universal speed claim.
- Many-FE designs that need the additive preconditioner from the start can be
  slower than beta.5: the near-confounded third FE is about 12% slower at one
  worker on Linux. This is an accepted exception, not a passed comparison.
- The Mac plugin contains Apple Silicon and Intel slices. Licensed installation
  testing is on Apple Silicon; native Intel-host Stata testing is outstanding.
- PPML CMG reuse mainly helps weakly connected two-FE designs; AKM and dense
  two-FE PPML gain 16-27% from the leaner passes and settled strict entry.
  The PPML joint frames still apply their CMG hierarchy with one scalar
  cycle per step, and separation IR dominates PPML time on Veneto-shaped
  panels; the October 27 parity work measured these on the Mac only. In
  the beta.7 SCC comparison fereg was faster than default fixest in every
  eligible PPML configuration, but PyFixest's `within` and MAP demeaners were
  faster on dense two- and three-FE PPML.
- PyFixest's `within` default was also faster than fereg on dense three-FE OLS.
  After the two-FE Auto speed changes it remains 3% faster on the short Veneto
  panel at two workers; fereg is faster in the other nine Veneto cells measured.
- The two-FE Auto speed qualification accepted 11 instead of 12 distinct
  physical hosts per Linux cell (waived by Johannes); its Mac check is
  descriptive.
- Explicit `solver(cmg)` was about 50 times slower than `solver(cmgschur)` on the
  real Veneto panels, and explicit `solver(lsmr)` returned `r(909)` on the long
  panel in a 64 GB allocation where Auto completed. Auto does not use either
  route there.
- Package comparisons are descriptive: one seed, three repetitions, and
  same-model SCC nodes that differed by up to 2.4 times in absolute speed.
- GitHub Actions remain disabled; checks run locally or on the authorized SCC
  and private Windows hosts.
