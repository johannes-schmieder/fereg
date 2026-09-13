# OLS residual-derived output precision investigation

Accepted baseline: `f576bc5`, measured omission artifact `fe066c02`, verified
against all 176 native/build inputs. This investigation excludes the separate
saved-Schur/recovery optimization. No optimization adoption is implied by the
passing intermediate checks. These are local development probes, not release
qualification.

## Precision-only prototype

The first two-file candidate (`64491903`) tightens the existing final residual
projection to at most 1e-12 when FE coefficient retention is requested. CMG
receives a temporary per-call configuration without mutating its retained config.
A closed-form crossed-cell regression fails on accepted source and passes across
five solver families with unit/nonconstant weights. There are 96 passing core
tests, 48 independent saved fits, four weighted omission cases and two public CG
cap/rollback checks. The original rank-deficient and PW requests pass their
independent oracles; PW residual and FE errors fall to approximately 2.7e-12.

A twelve-pair cost protocol stops at three FE. Direct reduced-dummy SVD and full
sparse LSQR/LSMR establish baseline error and candidate accuracy at original
tolerances. The first-two-FE component contrasts are identified; arbitrary
individual FE coordinates are not. The resumed run then exposes an 8.875-second
automatic-CMG final retry versus a 0.068-second baseline. That capture was intended
as explicit CG, but its harness omitted the selector. Its actual command requests
auto and its initial solver is CMG; it is additional automatic-CMG evidence, not
a completed explicit-CG control. The independent two-FE oracle validates candidate
accuracy and identifies several baseline residual/FE errors. Both failures and
the incomplete original cost protocol are retained.

Decision: this prototype is superseded. Do not accept its expensive final graph
retry or count its timings as a speedup. A terminal residual correction should
release its hierarchy before admitting the existing certified CG correction.

## Terminal-CMG prototype

The second candidate (`344b2451`) consumes the terminal CMG session and drops its
graph before a checked CG correction when the final certificate fails. It passes
97 core tests, including exact/one-byte-short terminal admission, both original
rank/PW oracles, auto/explicit-CG oracles, 48 saved fits, four weighted omission
cases and the public CG caps. The functional automatic-CMG replay takes 0.084
seconds; that observation is outside paired performance aggregates. A test-only
usize/u128 error is corrected, with the original compiler output retained. The
subsequent full rebuild is byte-identical. A collector initially treated public
solver warning count as checkpoint retry count; its correction retains the
completed capture and adds only the previously unexecuted explicit-CG request.

A fresh thirteen-pair cost comparison stops at the low-memory FW control. The
independent weighted-dummy solve and literal-frequency-expanded full LSQR/LSMR
oracles reproduce finite rank, full normalized-intercept cluster covariance,
nesting and both DoF frames. Both artifacts fail pointwise output tolerances:
baseline fails 9 residual/9 first-FE rows; candidate fails 61 residual/24 first-FE/
240 second-FE rows. The initial oracle's N_full assertion is corrected to the
existing frequency-weighted count, preserving the failed check and all numerical
calculations. The public command times are 0.223/0.222 seconds; this is failed
science evidence, not an accuracy-matched speed comparison.

Source inspection identifies the gap: low memory intentionally disables initial
FE coefficient retention, so retention cannot represent requested output
precision. The final projection still uses the loose tolerance on that path.
This candidate is superseded by an independent final-residual precision setting
for requested residual, absorbed, combined-prediction and saved-FE outputs in
both memory modes. Initial projection and coefficient-storage policy remain
separate. The subsequent candidates and their complete checks are recorded below; none
is adopted yet.

See each stage's protocols, source/artifact manifests, complete numeric archives,
independent oracle code/results and terminal evidence. Raw licensed logs remain
local; committed receipts retain their hashes. Pre-existing memory reporting
gaps, initial rank/slope cost flags and all failed/superseded captures remain.

## Output precision independent of retention

The third candidate (`f96cb778`) separates requested output precision from FE
coefficient retention, covering fast and low execution. Its 97 core and 95 plugin
tests, original rank/PW/auto/explicit-CG/low-FW oracles and all four licensed
validation fixtures pass. A fresh cost comparison reaches the original tiny
weighted cut and stops: the internal 1e-12 target still leaves 28 residual rows
outside the unchanged public tolerance. Coefficients, full covariance and both
saved FE channels pass. The accepted baseline fails 448 residual/828 first-FE/
960 second-FE rows on this same input. The [output-precision probe](output-precision-probe/)
preserves every capture and the failed comparison.

The independent weighted-cut oracle uses 80-digit weighted group centering and
a grounded 63-by-63 reduced solve. Its normal-equation error is below 2e-79;
full original-design LSQR and LSMR agree with its residuals within 1.9e-14.
It validates full normalized-intercept b/V, rank and DoF. The candidate's maximum
residual error is 1.76e-9. The missing temporary mpmath dependency and its
installation are disclosed; no data, seed or comparison threshold changed.

## Strict final projection: science passes, admission unresolved

The fourth candidate, [strict probe](strict-probe/), is
`76858524a758932f19f28b9891247954f14ea6d2cc5669ffe6da560952ced2ff`.
It binds all 176 native/build inputs and changes three production files relative
to accepted `f576bc5`. Residual, absorbed, xbd and saved-FE requests use at most
1e-14 for the existing final projection, preserving tighter callers. This applies
independently of coefficient retention in projection-only, fast and low fits.
Initial projection, reference scales, rank and public tolerances remain. Terminal
CMG releases its hierarchy before the admitted existing CG correction.

The original weighted-cut functional replay passes all 30,720 rows: maximum
residual/first-FE/second-FE errors are 3.05e-13 / 3.11e-13 / 3.43e-14, full b/V
errors 4.93e-16 / 8.14e-18. All 97 focused core tests pass, including exact and
one-byte-short terminal-CG admission. Both architecture builds and native arm64
ABI checks pass. Licensed validation records 48 saved dummy fits, four weighted
omission cases and both existing public CG admission/rollback caps.

The initial 95-plugin-test run passed. A final source-binding rerun (after removing
an unrelated core formatting hunk) passes 94/95: the isolated PPML allocation
audit sees one warm allocation where zero is required. Ten fixed fresh-process
exact checks per accepted/candidate test binary all pass, followed by one full
serial 95-test suite per binary, both passing. The concurrent failure's cause
remains unresolved; it is retained with all follow-ups. No assertion or production
PPML code was changed. The first cost invocation stopped at this prerequisite
before any measurement. The prospective gate was then bound to both unchanged
serial suites and the focused core checks. This is not a claim that the failed
suite passed or that a PPML allocation issue was repaired.

The full cost protocol completes **19 pairs / 38 measured commands**: thirteen
original/supplemental cases and two reversed-order follow-ups for each of three
flags. Every candidate passes the original scientific checks. Existing direct
oracles cover all captured PW, rank-deficient, three-FE, explicit-CG, weak,
low-FW, weighted-cut and automatic-CMG saved outputs; full b/V is independently
checked where the oracle supplies the normalized constant. The weak oracle's
finite-only coordinates are not misrepresented as full public b/V. Three-FE
saved-g comparisons use the independently identified first-pair component
contrasts. Six original baseline cases miss saved-output accuracy and remain
outside accuracy-matched speed claims: PW, three FE, weak, low FW, weighted cut
and automatic CMG. Their failed parity and all timings remain evidence.

| Original request | Baseline / candidate command, seconds | Disposition |
|---|---:|---|
| Unit saved | 3.032 / 3.147 | +115 ms / 3.8%; scientific parity passes |
| PW partial saved | 3.657 / 3.750 | Baseline output errors; candidate oracle passes |
| Rank-deficient saved | 0.542 / 0.534 | Scientific parity and direct oracle pass |
| Absorbed slope saved | 0.355 / 0.382 | Repeated +27 ms median / 7.6%; larger final workspace |
| Three-FE partial saved | 0.107 / 0.112 | Baseline output errors; candidate identified contrasts pass |
| Explicit CG saved | 0.069 / 0.077 | Actual selector checked; direct oracle passes |
| Weak saved | 0.105 / 0.106 | Baseline residual error; candidate oracle passes |
| Low-memory FW saved | 0.228 / 0.235 | Baseline output errors; candidate oracle passes |
| Complete no-output IID | 0.588 / 0.596 | Scientific parity passes |
| Tiny weighted cut saved | 0.057 / 0.110 | Repeated +54 ms median; corrects baseline errors |
| Small AW saved | 0.006 / 0.006 | Scientific parity passes |
| Weak PW PPML | 0.033 / 0.034 | Unchanged scientific exports |
| Automatic CMG saved | 0.069 / 0.087 | Repeated +19 ms median; corrects baseline errors |

The weighted-cut final correction takes 32 CG iterations and about 30–31 ms;
automatic CMG takes six and about 6–7 ms. Both report the correction fallback
and one warning, with unchanged native peaks of 15.84 and 17.50 MB. These phase
times do not explain every millisecond of the total added output cost. Public
solver column 12 is warning count, not checkpoint retries; the zero-retry proof
belongs to the separate terminal-CMG core test.

The slope final projection changes from one MAP iteration to seven charged
LSMR/pilot iterations, with roughly 11 ms setup and 16.5 ms solve versus 5.4 ms
baseline solve. Its native peak rises **28.87 → 38.54 MB**, while the public
execution base stays 18.93 MB. Other observed/model gaps remain disclosed in
[full cost/memory review](strict-probe/cost-review.json); no general memory or
speed claim is made. Builds, tests, oracle solving and archive compression did
not overlap these command measurements.

## Generic admission prerequisite

The [accepted-source regression](generic-admission-red/) establishes a distinct
pre-existing gap. Slope adapters use an owned `FeDesign` and the generic
`absorb_fixed_effects` family. Those generic branches receive but do not enforce
`AbsorbConfig::solver_memory_budget`. Four closed-form intercept/slope tests
first prove unrestricted residual accuracy, then all fail the required rejection
at a one-byte joint budget: auto, MAP, CG and LSMR each returns a certified solve.
The initial test build lacked an existing PPML include fixture; exact accepted
fixture bytes were added before the four intended failures were obtained.

**Decision: the precision repair remains unadopted.** Its newly exercised final
LSMR workspace cannot be accepted before this generic admission gap is repaired.
Keep its complete successful science/cost evidence and the separate archived
saved-Schur/recovery optimization. Next establish checked owned-design and
solver/fallback reserves, validate the original slope and constrained boundaries,
then resume adoption against that independently repaired baseline. Do not avoid
the gap by dropping slope coverage, relaxing output precision or changing inputs.
P0 whole-action reporting and P6 final-target recovery remain open. The installed
plugin and separate Veneto work are preserved; no agents, cluster, qualification,
dependency update or push occurred in this step.
