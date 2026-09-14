# P6 generated RHS tiles: standalone prototype rejected

Reject the standalone generated-column RHS tile change against accepted
`fb4cc5ba41aeb263662225161354cf0f1f42f116`. The candidate passes scientific checks,
but none of the three prescribed command screens meets the prospective gain
threshold of both 5% and 10 ms. Accepted source is restored. The candidate patch,
source, inputs, failures and twelve command captures remain in this report.
This is local M2 Ultra/Stata-MP development evidence, not qualification or a fresh
external-estimator comparison. Main and installed dist were not built or replaced.

## Implementation evaluated

The [three-file patch](implementation.patch) shares generated RHS construction
across ordinary and complex prepared layouts, including bounded low-memory
blocks. At least 4096 active rows and a categorical recipe enable tiles with
at most 128 rows and a roughly 64 KiB destination target. Factor codes remain
borrowed, including topology-backed codes; one fixed 128-byte interaction mask
replaces repeated per-cell component lookup. The existing installed team is used
above 65536 output cells. No extra observation-sized allocation is introduced.
Numeric-only builders, classification, sample/code rebasing, weight kinds, rank,
reference scales, solvers, independent certificates and publication stay intact.

Both [artifact identities](built-identities.json) bind 178 input files. Baseline
plugin is `f4cb7dee`; candidate is `6bbc5ace`. The isolated universal build visibly
recompiled core/plugin for both architectures and passed static and actual arm64
ABI/self-tests. The x86_64 slice was not executed.

## Complete-command screen and attribution

The [prospective protocol](protocol.md) fixed seed 104729, fresh processes,
untimed small warm-ups, rotated paired order and a 120-second command cap.
Original input hashes include missing factor/source/weight/cluster values as
applicable. Builds, tests, data generation and oracles completed before timing;
compression followed all measurements. Eight numeric controls accompany the
factor interactions. Complete estimation, inference and publication are timed.

| Target | N / factor levels / workers | Baseline → candidate, seconds | Change |
|---|---|---:|---:|
| Fast AW conventional OLS | 491520 / 8×8 / 16 | 1.118 → 1.129 | +0.98% |
| Low PW one-way clustered OLS | 122880 / 4×8 / 4 | 2.224 → 2.231 | +0.31% |
| FW/exposure robust one-FE PPML | 122880 / 4×8 / 4 | 1.522 → 1.481 | −2.69% |

All three pairs preserve exact full b/V, sample, rank/status, DoF, weights,
cluster/separation structure, certificates and non-time solver fields. Native
peaks are exactly unchanged at 411.335 / 33.217 / 221.157 MB, within unchanged
922.308 / 49.230 / 293.063 MB models. Sampled process RSS rises
0.520→0.539 / 0.0949→0.0957 / 0.1610→0.1628 GiB. Native allocation and sampled
RSS are distinct measures; there is no memory-saving claim.

Three separate diagnostic pairs preserve exact scientific exports against each
same-artifact untraced call. In fast AW OLS, full-data RHS construction falls
31.432→25.121 ms, saving 6.311 ms. The baseline RHS scope is only 2.81% of its
untraced command time, so even removing it entirely cannot meet the 5% target.
Other baseline fit scopes are raw moments 121.729 ms, initial absorption
130.577 ms, finite solve 142.769 ms and bread/raw residuals 90.528 ms. These
figures locate remaining work without attributing whole scopes to recipe lookup.
Warm scopes are kept separately in [phase attribution](phase-attribution.json).

PPML's untraced FE-projection scope falls 273.742→242.624 ms; finite work also
moves, while strict/final and preparation stay close. These overlapping scopes
must not be summed, and no direct PPML RHS subtotal was instrumented. Low score
retention traces are not RHS timers. Diagnostic command times are excluded from
the adoption decision. Full totals, phase fields, solver/retry/fallback reports,
models and peaks remain in [captures](captures.json) and [summary](summary.csv).

## Validation, failures and disposition

[Tests](test-results.json) pass 18 distinct core checks and 95 serial plugin
checks. The new raw-level oracle checks both prepared layouts, widths above 64,
vanished factors, syntax omissions, signed zero, final-sample code rebasing,
partial tiles, one/two/full-width blocks and 1/4/16 workers. Its largest sample
also exercises parallel narrow blocks. Wrong dimensions, invalid factor codes
and injected RHS reservation failure fail closed.

The [licensed independent fixture](generated_oracle.do) passes five fits per
artifact: fast AW robust, low PW clustered, actual three-FE FW conventional,
unit absorbed slopes, and generated FW PPML. OLS compares full finite b/V,
residuals, sample, DoF and row order against explicit-dummy regression. PPML
compares finite coefficients and means against Poisson and forms a separate
explicit-dummy weighted Hessian/sandwich with the registered frequency correction;
syntax-omitted coordinates are independently checked as zero. Existing numerical
tolerances remain unchanged. All return codes and terminal sentinels pass in
[oracle results](oracle-results.json).

Original development failures remain: an invalid test clone, an unsupported
factor-base fixture, an incorrect assumption that both layouts retain vanished
columns, and the initial PPML oracle's omitted-factor name mapping. The corrected
oracle also replaces direct reference-VCE reliance with the explicit mathematical
sandwich. The initial archive attempt used a Python without NumPy; collection
succeeded with the established numerical environment. Original source patches,
fixtures, failed results and log hashes are retained. No failed measurement is
reclassified as a passing command.

No additional seeds, broad graph holdout timing, private iteration allocation
audit or public exact memory-cap calls were run after screen rejection. Those
adoption gates are not claimed. This rejection resolves this particular RHS
prototype, not the broader P6 generated-column item. Next profile and improve
repeated generated-column raw-residual/predictor consumers while preserving
per-row FMA order and independent reconstruction. Seven P0–P7 items remain;
R1–R5 stay excluded, with reviews/upstream/SCC later in the authorized sequence.
