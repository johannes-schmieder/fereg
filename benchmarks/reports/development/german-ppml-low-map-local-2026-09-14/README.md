# German PPML and bounded categorical MAP in low memory

Local development evidence, 2026-09-14. Parent native source is `1e9581e`;
parent universal plugin is `bc1e8909`, candidate `9d920101`. Installed plugin
`4187efdf` is unchanged. [Inputs](inputs.json) bind all 177 compiled files;
[source.patch](source.patch) includes the added Stata fixture. No release,
cross-platform, or universal performance claim is made.

## Adopted implementation

PPML low memory now uses the existing certified categorical MAP projector for
one through three intercept effects. It projects bounded scratch column blocks
and the response separately; response/design fusion remains confined to dense
storage. It reuses existing current-weight group reductions and guarded
acceleration, without adding another solver. Public solver families, automatic
fast-memory LSMR selection, scientific tolerances, original reference scales,
rank/status, scratch identity, sample ordering and publication contracts remain.
Generic OLS/slope sessions retain their projector. Effective low RHS capacity is
capped by model width before allocation. The planner explicitly charges low
parallel group/dot banks and two-FE level-PCG or multiway observation-space
recurrences by that bounded width. Preparation classification remains allocation
free before admission.

## German subset results

The outcome is the user-selected `exp(logrwage2)`, with the original ten scaled
age-polynomial/education terms, year factors, and worker/establishment effects.
All comparisons use robust covariance, eight Stata processors/native threads,
ordinary recursive singleton removal and requested absorbed output. References
are pinned ppmlhdfe `b85665d8`, reghdfe `29b2b203`, ftools `d6e24d66`, and require
`f4760b86`. [Summary](summary.json) contains numerical, phase and resource fields.

| Worker cap | Input / retained rows | ppmlhdfe s | Parent fast s | Candidate low s |
|---|---:|---:|---:|---:|
| 5,010,000 | 7,759 / 6,654 | 0.440 | 1.155 | 0.931 |
| 5,100,000 | 75,350 / 65,503 | 4.866 | 3.471 | 9.824 |
| 5,200,000 | 151,029 / 132,691 | 23.095 | 7.915 | 24.409 |
| 5,500,000 | 378,124 / 338,054 | 82.414 | 17.914 | 86.049 |

Parent low completed the smallest case in 1.439 s, versus 0.931 s after the
change. Parent low on 75,350 rows was interrupted after 257.68 process seconds,
while iterating in generic weighted FE projection; it never published a fit.
One bounded stack sample is retained. That incomplete run supports no exact
completed-command speedup ratio. Another brief Stata process appeared near its
end and may overlap the first fast timing. All timings are single diagnostic
runs, with solver tracing enabled for the candidate. They are not paired
benchmark medians. Large low-memory fits remain slower than fast, and the two
largest low fits are slightly slower than ppmlhdfe.

All completed native samples match reference masks and order exactly. All 18
slope differences are below 1.1e-13, and slope covariance differences below
1.3e-12, inside registered envelopes. Native published means replay objectives
and pass independent score and coefficient/model-space checks. An independent
SciPy weighted FWL sandwich on 65,503 rows agrees within 5.8e-17; see
[independent covariance](independent-covariance.json). Native low and fast agree
on rank, DoF and coefficient stripes. Native reports the normalization constant
in `rank` and removes it from `df_a`; ppmlhdfe assigns that direction to absorbed
DoF instead. Their identified slope rank, total model rank and residual DoF agree.
This existing reporting convention is unchanged.

Pinned ppmlhdfe's reconstructed means fail the model-space diagnostic on these
subsets; the largest also fails stationarity. They do not closely replay its
reported objective. This is recorded under ADR 0026, not used as an additional
objective witness or asserted to prove infeasibility. Its reported objectives
agree with the independently certified native results. The largest input wage,
8.37 billion, is removed by ordinary singleton pruning in both estimators; its
retained maximum is 2,252. No trimming or external pruning was performed. These
subsets therefore do not test the full sample's remaining extreme-outcome risk.

For the largest subset, low native allocation peak is 599.05 MB versus fast
1,183.40 MB; low scratch is 48.68 MB. Process peak RSS is 391.69 MB versus
1,116.50 MB. Selected native plans are 897.79 MB and 13,236.86 MB respectively;
the latter includes conservative specialized LSMR construction admission.
Low's smaller peak comes with more inner work: 5,063 reported inner iterations
versus 966 in fast, with nine outer iterations and one step halving in both.
Every completed native fit reports zero solver fallbacks. These counts do not
summarize every physical projection operation. The smallest low allocation
peak rises 12.58→13.20 MB, within its 19.01 MB admitted plan.

## Validation and limits

[Validation](validation.json) records 347 PPML-related core tests, 213 OLS tests,
97 plugin tests, universal binary audit and actual ARM64 ABI/self-test. The
Stata fixtures contain 26 native fits: weak 0.001-weight mobility links, one to
three effects, pools 1/2/10000, unit/FW/PW, clustering/missingness, interactions,
initial and separation-triggered singleton cascades, and independent dummy
Poisson oracles. A 557,370,912-byte cap rejects fast before workspace allocation
with r(909) and no requested output, while low retains the expected 6,654 rows.

The initial weak-link oracle stopped too loosely: 62 means failed its assertion.
With the same candidate plugin, tightening dummy Poisson's three optimizer
controls to 1e-12 resolves it; candidate acceptance thresholds are unchanged.
The largest independent projection initially refused its 512 MiB audit budget;
its explicit 1 GiB rerun passes with unchanged tolerances. These failed receipts,
the interrupted baseline, and runtime-cache isolation details remain preserved.
Raw licensed logs/data/estimates stay in the local `fereg-german-ppml-20260914`
bundle, with [hashes](local-evidence-hashes.json); they are not committed.

Full German PPML has not yet run. Next, inspect full-sample memory admission and
choose a fitting bounded pool before estimation and pinned-reference comparison.
No SCC work, installed-plugin replacement, or push occurred. Unrelated work and
historical receipts are preserved.
