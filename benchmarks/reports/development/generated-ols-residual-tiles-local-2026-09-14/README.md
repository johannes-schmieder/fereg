# P6 generated OLS residual tiles: standalone prototype rejected

Keep accepted source `25bf610` / plugin `e6978061`. Candidate `e97dee05` is
**not adopted**: clustered PW misses the predeclared repeated-primary gain gate,
and a three-FE control exposes an unchanged native-memory modeling gap. All
original measurements remain; no extra repeats or public cap checks follow
rejection. The [restoration record](restoration.json) verifies all 178 accepted
inputs and installed dist. The rejected [patch](implementation.patch),
[source archive](candidate-source.json) and [artifact identities](built-identities.json)
remain available for a later, separately specified experiment.

## Experiment and command results

The [initial audit](protocol.md) reuses the preceding generated-RHS experiment's
original fast-AW and low-PW inputs. Separate baseline diagnostics locate
82.546 ms in fast raw-residual accumulation (7.61% of its 1.085-second untraced
command), versus only 4.205 ms in low memory (0.20% of 2.156 seconds).

The prototype extracts the accepted generated-predictor recipe evaluator and
uses it for fast independent raw residuals in both prepared layouts. It borrows
ordinary/topology codes into a fixed 128-value stack bank under the existing
team. Ordinary OLS retains skipped absent-indicator FMAs; complex preparation
retains all-entry FMAs. Numeric precedence, low-memory residual construction,
original coefficient order, normalization, rank and independent certificates
remain. No new N-sized allocation, matrix, worker pool or SPI call is added.

The [screen](screen-protocol.md) passes its initial AW gate:
1.120→1.062 seconds, 58 ms/5.18%. Protected low PW changes 2.174→2.222
(+48 ms/2.21%), and shared generated PPML changes 1.442→1.431. The
[follow-up](followup-protocol.md) fixes three seeds per primary and fourteen
additional controls before measurement. Every call uses its original full/warm
inputs, a fresh process, rotated arm order and a 120-second cap. Builds, tests,
oracles, generation and compression do not overlap timing.

| Primary, N491520/K8 plus 8×8 factors | Workers | Command medians, seconds | Median paired reduction | Gain gate |
|---|---:|---:|---:|---|
| AW conventional, missing | 16 | 1.095 → 1.039 | 5.18% | Pass |
| Unit conventional, missing | 4 | 1.252 → 1.195 | 5.25% | Pass |
| PW two-way cluster, missing | 16 | 1.553 → 1.484 | 4.44% | **Fail** |

The repeated-primary gate requires both 5% and 10 ms savings. PW's three paired
reductions are 5.13%/3.54%/4.44%; its median misses the percentage gate. Medians
of paired changes need not equal ratios of independently computed command
medians. The original screen remains included. This is a rejected broad
adoption decision, not a claim that the AW/unit gains disappeared.

Native time medians fall 0.606→0.551 / 0.681→0.624 / 1.062→0.994 seconds.
Input time remains about 43–50 ms, and outer Stata time about 441/527/441 ms.
The complete [summary](summary.csv) includes setup, solve, iteration, fallback,
scratch, modeled/live memory and sampled RSS costs for all 25 pairs.

## Direct attribution and protected outcomes

The separate [baseline](diagnostic-results.json) and
[candidate](candidate-diagnostic-results.json) diagnostics match the respective
untraced scientific exports and non-time solver fields exactly. On the original
490026-row fast-AW sample, accumulation falls 82.546→4.568 ms, saving 77.978 ms.
Initialization remains about 0.3 ms. Low accumulation is 4.205→4.186 ms.
Warm scopes remain separate; instrumented command times are excluded from gains.
Differences in unchanged work remain visible rather than assigning the entire
command change to the kernel.

All 25 untraced pairs preserve exact scientific exports, samples, statuses,
DoF, b/V, certificates, non-time solver fields and requested saved outputs.
Controls include weak/bottleneck/cycle/disconnected/unbalanced and tiny-weight
graphs, actual three FE, slopes, saved outputs, topology-backed factors with an
exact duplicate, small/numeric/width guards, low OLS and shared PPML. The OLS
topology/duplicate case succeeds in both arms; the previously reported PPML
final-basis failure is a distinct estimator case and is not reclassified here.

The largest added time is the original low-PW screen's 48 ms/2.21%; small numeric
adds 1 ms/12.5%. Neither crosses the joint 5%-and-10-ms protected review gate.
Single control improvements are not repeated speed claims.

## Memory admission failure

All 25 paired native peaks are exact. Primary peaks are 411.335/407.702/422.837
MB, within unchanged 922.308/922.308/934.105 MB selected models. Sampled RSS
medians are 0.5266→0.5228 / 0.5199→0.5216 / 0.5635→0.6186 GiB; the PW RSS
increase is retained. Native live allocation and sampled process RSS differ.

**The actual three-FE FW conventional control is not admitted by its reported
model:** both artifacts peak at **50,265,168 bytes**, versus a selected model of
**22,983,168 bytes**. The [decision record](decision.json) retains both violations.
The first summary stopped at that failed check; the revised summary records the
failure rather than claiming all budgets passed. Other measured cases fit their
reported models. This is a baseline P0 gap, not an introduced allocation or a
successful memory boundary. Investigate it on the original input before broader
performance work; do not raise a multiplier merely to cover this observation.

## Validation and disposition

Sixteen distinct focused core checks and 95 serial plugin tests pass. The raw-
level/FMA oracle now compares both original scalar residual implementations with
the candidate at N4095/4096/4099 and workers1/4/16, including generated widths
above64, topology/rebased codes, vanished levels, omitted coefficients and signed
zero. Existing reconstruction and strict rollback checks remain green.

Eight independent licensed fits per artifact cover conventional/robust AW OLS,
low PW clustered OLS, three-FE FW OLS, slope OLS and unit/FW/PW generated PPML.
Explicit-dummy regressions/Poisson means and independent Hessian/sandwich matrices
retain original tolerances, weight corrections, sample and row-order checks.
Universal builds force core/plugin compilation for both architectures; static
and actual arm64 ABI/self-tests pass. Diagnostic builds are separate.

The [archive](archive.json) binds 56 command captures: two baseline audit calls,
25 untraced pairs and four separately instrumented calls. Raw licensed logs,
data and plugins stay outside Git; their hashes and source identities remain.
A Python bytecode-cache permission failure is recorded in
[collector notes](collector-notes.md); cache-free syntax checking passed before
execution. The memory-summary failure and its original script/log identity remain.

No thresholds, inputs, seeds, scientific policies or model budgets were changed
to pass this prototype. Accepted `25bf610` is restored, with unrelated STATUS
additions, original untracked paths and installed dist preserved. Seven P0–P7
items remain. Next prioritize the three-FE memory-accounting gap, then finish
generated/shared-layout, PPML state and scheduling/cost work. R1–R5 remain
excluded; independent reviews, upstream CMG assessment and the bounded SCC
campaign remain later steps. No agents, cluster work or push occurred.
