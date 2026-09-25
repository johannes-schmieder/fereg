# Disconnected PPML mean adjudication and corrected-accuracy comparison

Local Mac arm64 development evidence, 2026-09-25. No qualification or release.
Decision: [ADR 0194](../../../../docs/adr/0194-disconnected-ppml-mean-adjudication.md).
The two seed429 mean discrepancies from the
[iterative canonical product report](../ppml-iterative-canonical-products-local-2026-09-25/README.md)
are independently attributed to stock outer stopping. Fereg passes its existing
mean envelope. Original failed comparisons remain unranked. A new matched block
at verified reference accuracy gives **107/102 ms low** and **90/103 ms fast**
for accepted fereg/stock. Fast meets the 0.90 target; low remains a loss.

## Exact scope and independent enclosure

Both cases have identical N49920/K12 data, two categorical partitions with
8320/832 levels in four disconnected components, T8, robust inference and means.
The original inputs, samples, coefficients and means are bound by hashes. All
five repetitions of both baseline and candidate in each memory mode are checked.
No frozen validation input becomes a performance tuning input in this milestone.

A separate Python/SciPy explicit full-design Poisson Newton solver starts at the
global intercept and grounds one second-partition coordinate per component. It
solves full sparse Hessian systems, rather than using either estimator's
absorption recurrence. Eight Newton updates produce an approximate optimum.
Its last maximum predictor step is 7.30e-16. This convergence observation alone
is not the proof of accuracy.

The new general two-FE entry in `scripts/ppml_coefficient_accuracy.py` independently
reconstructs feasible FE potentials and projects controls. Dense reduced LAPACK
solves propose those potentials only. Sixty-digit outward Decimal arithmetic
bounds original-input FE loads, per-component spanning-tree flow energies,
control projection error, finite curvature, the full Newton decrement and
public-mean reconstruction error. Every component root must balance separately.
The mathematical bound is the existing ADR0029 proof applied to a forest; no
inverse-energy transfer across disconnected components is allowed.

The certified finite curvature is at least **378055.4425**. The Newton decrement
squared is at most **8.66739e-23**. The finite coefficient radius is at most
**3.02829e-14** and the anchor's log-mean distance to the optimum at most
**1.50227e-11**. Transforming this interval through outward exponentials proves
mean inclusion/exclusion against the unchanged `rtol=1e-7, atol=1e-9` envelope.

| Original output | Maximum mean-error ratio, upper bound | Proven outside |
|---|---:|---:|
|resgate1 low|0.000184|0|
|canonicalcg3 low|0.000184|0|
|resgate1 fast|0.002062|0|
|canonicalcg3 fast|0.002045|0|
|stock, either mode|1.543246|6|

These bounds hold in every repetition. Stock's maximum error ratio is also
bounded **below by 1.54294**. The six excluded observations are 4921–4926,
all in first-partition group 821. Every fereg mean is proven within its optimum
envelope. The enclosure diagnoses mean accuracy; it is not an optimum-covariance
exception or a new automatic-route admission. Existing public-state, sample,
coefficient, covariance and objective audits remain required.

## Reference stopping experiment

Pinned unmodified fixest 0.14.2 has archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
The diagnostic stock rerun at glm.tol=1e-10/fixef.tol=1e-11 reproduces the original
mean bytes exactly. Public response predictions equal the fit's stored means.
The pinned `R/estimation.R` stopping test uses relative deviance change; at
iteration 5, 8.511939e-6 / (0.1 + 141225.9) is below 1e-10. The remaining mean error
is outside the saved-output envelope even though the objective has stabilized.

| Diagnostic settings: outer / FE tolerance | IRLS iterations | Maximum absolute mean error vs oracle | Maximum envelope ratio |
|---|---:|---:|---:|
|1e-10 / 1e-11, original strict stock|5|6.84987e-7|1.54310|
|1e-12 / 1e-11, outer tightened|6|2.21512e-12|0.000001563|
|1e-10 / 3e-12, FE only tightened|5|6.84987e-7|1.54310|
|1e-12 / 3e-12, both tightened|6|2.49223e-12|0.000001765|

This isolates outer stopping on these inputs; it does not show a general fixest
accuracy defect. No instrumented reference binary is used or timed.

The first diagnostic script ran with OpenMP 1 and then requested unsupported
fixef.tol=1e-13. It completed two fits and failed on the third; logs and partial
means are retained. The corrected script requests/checks T8 and uses supported
3e-12 for the FE-only/both diagnostics. No clock from either diagnostic probe is
ranked. The complete four-setting probe has an explicit terminal sentinel.

**Thread-description correction:** the actual headline harness sets
`OMP_NUM_THREADS` and `RAYON_NUM_THREADS` to the requested thread count. BLAS,
MKL, NumExpr and vecLib are one; nested OpenMP is disabled. The prior report's
statement that OpenMP was also one was incorrect. Its archived controller and
requested/effective T8 receipts are correct and remain immutable. This is a
prose correction, not a reinterpretation of measurements with mismatched threads.

## Prospective matched timings

The private surface now accepts explicit `fixest_glm_tol` only with strict
comparison enabled, and only at a positive finite value <=1e-10. Omission retains
the original 1e-10 default. FE tolerance remains 1e-11. Frozen 77/16/111 definitions
and existing comparison bounds are unchanged.

Predeclare five rotating baseline/candidate/stock repetitions on the same seed429
inputs, using glm.tol=1e-12 for stock. Generated full `.dta` and `.bin` inputs are
verified byte-identical to the original cases 0/3. Fereg's artifact, options,
threads, robust inference and saved means are unchanged. This is corrected-
accuracy remeasurement on frozen data, not an additional fresh holdout.

No build, profile, archive or competing fit ran alongside headline measurements.
All process monitors pass without sampled overlap or monitor errors; that does
not exclude all ambient operating-system load. All 30 public states, 20 baseline
pairs and 10 direct candidate/stock pairs pass. No timeout or application failure
occurs in this block. Generation/import/startup and warm fits are outside the
clock; full public loading, estimation, certification, inference and output are
inside. In milliseconds, repetitions 0–4:

| Mode | resgate1 | canonicalcg3 | Stock, outer 1e-12 | B/C/S medians | C/S | C/B wins/losses/ties |
|---|---|---|---|---|---:|---|
|low|136,136,140,136,136|110,106,107,108,107|100,95,102,102,105|136/107/102|1.049|5/0/0|
|fast|113,111,110,113,108|89,91,90,93,90|103,102,104,104,100|111/90/103|0.874|5/0/0|

Candidate wins every baseline pair. Fast wins every stock pair; low loses every
stock pair. Low still needs **15.2 ms** off 107 ms to reach 91.8 ms, the 0.90 stock
target. The original 93/95-ms stock medians remain failed-accuracy clocks; do not
relabel them or combine them with this new block. The other seed429 cases retain
their original source-bound results, including the low T4 and narrow misses.

## Separate accepted-code cost ledger

**canonicalframeledger1** instruments committed 30d1c23d in an isolated source
copy. It passes actual-plugin ABI/self-test and panic containment. Two rotations
per case compare its outputs with canonicalcg3, using development seed347.
All 12 public states and six native pairs pass. These 12 fits are diagnostic and
are not stock timing observations. Each value below is a median total within
the final full plugin invocation; scopes are inclusive and overlap. Worker-time
sums, including parallel geometry preparation, are deliberately excluded.

| Phase, ms | Disconnected low N61440/K6/T4 | Narrow fast N24960/K2/T4 | Two slopes fast N30720/K6/T8 |
|---|---:|---:|---:|
|Whole plugin|102.596|33.025|71.921|
|Loading|2.943|1.635|2.199|
|Core preparation|3.633|1.171|2.145|
|Initial state|5.793|2.477|5.014|
|Canonical preparation|27.406|5.844|19.364|
|Finite rank inside canonical|4.201|0.694|2.794|
|Canonical residual projection|1.916|0.803|1.374|
|Ordinary joint boundary|23.981|—|16.680|
|Ordinary streamed/retained inner solve|11.840|5.657|9.080|
|Ordinary recurrence|5.743|3.131|3.817|
|Original response/residual norms|1.356|—|0.685|
|Original finite scores|3.285|—|1.089|
|Joint reconstruction/report|1.811|—|0.901|
|Fresh outer scores|3.608|1.420|2.377|
|Result certification|7.785|3.145|3.837|
|Inference|1.212|0.345|0.594|
|Publication|1.732|0.746|0.941|

Low ordinary boundary 23.981 ms contains 11.840 ms inner solve, of which 5.743 ms
is recurrence. Its separate norms, score setup, original scores and reporting
sum to 7.121 ms. Geometry and other setup still occupy part of the gap. Canonical
preparation 27.406 ms and result certification 7.785 ms remain substantial. Narrow
recurrence 3.131 ms is still insufficient to close its whole-command target.
These observations motivate shared current-frame preparation and independent
checking organization, rather than another standalone recurrence experiment.
They do not establish that all checking cost is removable.

## Source, validation and memory limits

No Rust/ado estimator source changes. Runtime remains canonicalcg3, accepted at
30d1c23d. Two of its 557 captured inputs now differ: the offline proof script and
its tests. The other 555 match. Do not claim exact equality of all captured
computation/test inputs after this diagnostic change.

| Artifact | Plugin SHA-256 |
|---|---|
|resgate1|9103c3092e83291e2941634ac07de06d445bfec85c39ca372a3b96f833f166d9|
|canonicalcg3|dcf1672c99e05521f5869309a3756f862025794e6bc23d95e6518e3088755d98|
|canonicalframeledger1|d5cf1b8948d3eaffdec4f415c71e967f438bf67c4b568664bc76fbb014e17f09|

Twenty-two focused Python tests pass: six coefficient/forest proofs, six covariance
controls, eight optimum-adjudication controls and two comparator-setting tests.
Exact-rational flow checks, independent disconnected nonzero-residual truth,
coefficient perturbation, memory refusal, old connected-ring behavior and the
existing unsupported-arithmetic checks remain. The final proof source and final
receipts match; the earlier proof used in the timing declaration is preserved
at its original hash. Their numerical bounds are identical; the final proof
adds conservative dense workspace accounting.

The general proposal adds five Q-squared double-matrix payloads to the diagnostic
workspace estimate; the frozen case estimates 168,663,040 bytes. This is not
Python/SciPy physical memory qualification. No production allocation allowance,
cache lifetime, certificate schedule or weight authority changes. No Rust/Stata
fixture campaign is rerun for Python-only diagnostic/harness changes. Known
baseline count failure, static allocation drift, constrained-fit/CG issues and
deferred platforms remain open. The older three-FE coefficient discrepancy is
not adjudicated here.

## Immutable receipts

`receipts-content-addressed.zip` contains 3,006 logical receipts, 952 deduplicated
payloads, 23,545,704 bytes. SHA-256:
`7df0a8c3455653fcc6da86e3c9fb85957fa4f985f260ea8c09114cbdf3b79d21`.
Restore to a new directory with `python restore_receipts.py DESTINATION`; the
container and every logical payload are hash checked. Closeout restores all
receipts and checks local-home/license-header privacy. Historical archives are
unchanged.

The restored `ppml-mean-adjudication-20260925/` directory includes declarations,
controllers, process monitors, science audits, complete distributions, reference
probes (including the failed initial probe), initial/final optimum enclosures,
explicit Newton proposals, source identities and phase traces. `binary-witnesses.json`
retains the anchor and original/reference means as base64 with hashes. Captured
estimator sources and current proof/test/harness recipes are included. Full input
datasets and plugin binaries are omitted with hashes; frozen generator recipes
and seed identify the inputs. Stata logs retain only protocol/trace lines.
Sanitized entries record original and archived hashes. A failed first archive
assembly used an incorrect helper filename and created no container; it is
recorded separately from application results.
