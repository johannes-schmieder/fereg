# Bounded low-memory joint PPML slopes: local development

Accept `lowjoint4`, based on `7d665c37`, with all **537 computational inputs**
matching the final source. Plugin SHA-256:
`853bd995ff3783f7223e5e29c18b343aa5bd4d05da42ca9bc0452629809837f2`.
Baseline is accepted `framemom1`, plugin
`12fd878cf359681240c23c43f46cdc3f541ff41c81bd980106510502db5c7509`.
This is a bounded implementation milestone toward whole public PPML at
**<=0.90 times comparable stock fixest per case**, not completion of that goal.

## Adopted organization

[ADR 0134](../../../../docs/adr/0134-bounded-low-memory-joint-ppml-slopes.md)
extends the existing joint finite/FE Schur solver to scratch-backed low mode.
A single current-weight scan prepares stable finite-column norms. Bounded tiles
reuse the admitted workers' observation buffers; each tile projects its columns
and streams finite products. No additional resident N-by-K matrix is created.
Both storage preparations call the same block-preconditioned Krylov recurrence.
Fresh reconstruction, FE projection and original-input finite-score acceptance
remain independent of that recurrence.

The low owner also serves the existing strict Newton witness, followed by one
independent canonical final rank/bread/Hessian construction on the accepted
weight frame. Current-weight, objective, score, model-space, rank, reconstruction
and final-Hessian requirements remain unchanged. No altered tolerances, samples,
weights, DoF, separation or failure publication. Budget refusal retains the prior
certified path. Categorical, one-effect, non-MAP, mu-separation and public OLS
routing are excluded from this extension.

## Whole public command

Each main block has three alternating baseline/candidate/pinned-stock rotations.
Values below are milliseconds, **median [minimum, maximum]**. Timers include
public estimation, robust inference, fitted means and publication. Generation,
import, process startup, warmup and exports are outside the timer. No builds,
profiles, archives or competing fits accompanied headline measurements; every
contention/monitor check passes. Small oracle pilots are not timing rankings.

Stock is unmodified pinned fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with `glm.tol=1e-10`, `fixef.tol=1e-11`, matched application threads, serial BLAS
and the established comparable covariance normalization. Every current public
stock comparison passes. Historical/default-reference discrepancies are separate.
Low native versus ordinary stock is **not a physically memory-matched comparison**.

Final development screen, seed333:

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Low two slope partitions N30720 K6 T4 | 195 [184,223] | 177 [176,182] | 116 [113,122] | 1.526 |
| Low unbalanced three-FE slopes N30720 K6 T4 | 188 [187,189] | 180 [170,198] | 121 [119,122] | 1.488 |
| Low larger unbalanced three-FE slopes N61440 K10 T8 | 474 [469,476] | 415 [413,426] | 226 [226,229] | 1.836 |
| Fast two slope partitions N30720 K6 T4 | 129 [129,130] | 131 [129,132] | 120 [119,120] | 1.092 |
| Fast larger unbalanced three-FE slopes N61440 K10 T8 | 301 [299,305] | 302 [302,303] | 227 [226,228] | 1.330 |

Low median gains are **9.2%, 4.3%, 12.4%**. The middle case retains one **4.8%
paired loss**. Fast controls retain **2-ms and 1-ms median losses** with overlapping
ranges. Earlier intermediate artifacts cannot supply the final artifact's claims.

Predeclared fresh seed334, all low mode:

| Case | Baseline | Candidate | Stock | Candidate/stock |
| --- | ---: | ---: | ---: | ---: |
| Two slope partitions N38400 K8 T1 | 355 [349,360] | 340 [339,346] | 473 [468,480] | 0.719 |
| Wider unbalanced three-FE slopes N46080 K16 T8 | 565 [565,567] | 475 [467,499] | 1290 [1288,1293] | 0.368 |
| Multiple slopes N23040 K6 T4 | 173 [173,179] | 157 [156,158] | 132 [131,132] | 1.189 |
| Weak barbell, two slope partitions N7680 K4 T4 | 41 [41,41] | 38 [38,39] | 266 [263,266] | 0.143 |
| Disconnected, two slope partitions N15360 K6 T4 | 126 [125,126] | 109 [108,109] | 80 [79,81] | 1.363 |

Fresh native gains are **4.2%, 15.9%, 9.2%, 7.3%, 13.5%**. Every individual native
pair improves. Three cells clear the stock target, including its full observed
range; they were already stock wins in the baseline. The multiple-slope and
disconnected cells still need **38.2 and 37.0 ms** to reach 0.90 times their stock
medians. These cases do not establish family-wide dominance.

## Frozen comparison and rejected organizations

Two isolated prototypes used identical captured weight-generation-two frames:
N2400/K6 and N30720/K6 two-partition slopes, N30720/K6 unbalanced three-FE slopes,
and N61440/K10 larger unbalanced slopes. These are FE-equivalent warm
representatives from the earlier joint-slope experiment, not untouched raw
regressors or fresh seeds. Compare response-plus-controls blocked absorption,
width-one absorption, dense joint solving and streamed joint solving, including
finite recovery and fresh residual/FE certification. Each prototype has **480
native kernel attempts** across T1/T4/T8, two tolerances and five rotations; all
runtime certificates pass. Two analytic oracle/boundary tests pass per prototype.

The first streamed prototype used one worker. The second used the already
admitted worker buffers for independent columns. Representative strict 1e-12
kernel medians from the second prototype (ms):

| Frozen frame | Blocked absorption | Dense joint | Streamed joint | Width one | Stock |
| --- | ---: | ---: | ---: | ---: | ---: |
| Two partitions N30720/K6/T4 | 8.809 | 6.094 | 6.303 | 35.817 | 22 |
| Unbalanced three FE N30720/K6/T4 | 15.836 | 7.615 | 7.913 | 31.848 | 25 |
| Larger three FE N61440/K10/T8 | 42.426 | 18.672 | 19.528 | 112.646 | 50 |

Stock frozen calls use the pinned library's `demean` at **1e-11**, followed by
finite recovery, on exactly those same inputs. Import and constructors are outside
these clocks. They are diagnostic kernels, not public-command timings; there was
no whole-machine contention monitor. No assistant-launched build/fit/archive ran
alongside them. The native blocked constructor clock accidentally covers both
blocked owners; do not attribute it solely to the ordinary blocked owner.

Independent explicit dense SVD checks the small frame; explicit sparse LSMR at
1e-14 checks the others. All **20 strict exported states per prototype** pass
coefficient and residual envelopes, including stock. Exports are T1/first-repeat;
T4/T8 runtime certificates are not independent exported oracle checks. The ordinary
1e-7 inner solves do **not** satisfy final-output envelopes: all 16 residual
comparisons and four coefficient comparisons fail per prototype. Those flags
remain visible. They do not rank ordinary-tolerance solves against strict stock
as final scientifically equivalent estimators. Public candidate acceptance uses
its mandatory strict/final procedure, and all public oracles pass.

Selected buffer payloads for N61440/K10/T8 are 16,801,248 bytes blocked,
17,904,368 dense joint and 17,404,168 streamed joint. These are selected owner
buffers, not whole-action memory. The prototype's frozen in-memory inputs omit
production scratch I/O, rank construction and publication costs.

That distinction mattered. Production `lowjoint2` passed science but regressed
the larger low public fit **488 -> 511 ms**. Its per-column preparation scanned
scratch 3*K times. `lowjoint3` replaced those scans with bounded complete-row
tiles but retained the old low finalization; medians were **186/179**, **190/185**
and **468/462 ms** for the three low cells. Neither adapter is separately adopted.
`lowjoint4` completes the shared strict/final path. The code-derived preparation
scan count is **1 + 2*ceil(K/W)** for W admitted workers, plus reconstruction and
independent checks. The recurrence, tolerances and memory payload do not change
between these preparers. No benchmark-name or seed guard was introduced.

## Attribution and memory

On the final larger development case, FE-projection telemetry falls
**190.754 -> 168.832 ms**, finite solve **48.610 -> 24.624 ms**, strict/final
**95.952 -> 94.326 ms**, and inference **33.543 -> 28.143 ms**. On fresh K16,
projection falls **283.972 -> 223.498 ms**, finite solve **62.857 -> 24.501 ms**,
while strict/final rises **106.713 -> 113.898 ms**. These are public phase medians,
some nested; they do not identify additive removable costs or isolate each
algorithmic component. Scratch inference varies substantially even in excluded
controls. Do not infer a numerical change from that timing variation.

Tracked and modeled peak increments agree exactly on each changed cell:
**272,088 / 308,184 / 611,064 bytes** on the three development low cases,
and **345,448 / 512,936 / 204,560 / 65,224 / 173,784 bytes** on fresh cases.
The larger development peak is **57,443,213 tracked / 69,232,515 modeled bytes**;
the fresh K16 peak is **47,895,891 / 56,927,699**. Excluded fast peak payloads
are unchanged. No new observation bank is hidden in these increments. Pool1/2
science and allocation checks, and public K16 exceeding default pool10, exercise
the independent finite-width admission. Physical-cap and cross-platform checks
remain deferred; static inventory drift remains an intentional unresolved failure.

## Science, controls and visible failures

Final-source **353 distinct Rust tests**, actual-plugin ABI/panic checks and
**eight licensed runs of five fixtures** pass. They include analytic weighted
projection, changing-frame/rollback/rank boundaries, forced strict fallback,
explicit-design Poisson/Hessian oracles, unit/FW/PW weights, offsets,
robust/cluster inference, saved mu/eta/xb/d, T1/T4 and pool1/2, plus OLS memory
modes. Ordinary fixest weights are not used as a Stata FW inference oracle.

Final production blocks contain **153 passing applications**, **127 PPML public
states**, **46 native and 35 stock pairs**, **15 independent Poisson/Hessian
checks**, **13 exact OLS pairs** and **11 exact excluded PPML pairs**. Including
intermediate public experiments: **255 applications**, **229 states**, **80 native
and 69 stock pairs**, **27 independent checks**. No public application failure,
timeout, scientific discrepancy or detected contention occurs in these blocks.

Excluded low one-FE PPML initially loses **101 [101,101] / 118 [117,119] ms**,
with inference **15.418 / 30.157 ms**. Five predeclared follow-up pairs on exactly
the same imported columns give **107 [99,120] / 107 [99,121] ms**. Preserve the
initial loss and the wide follow-up distribution; this is not proof of universal
non-regression. Categorical fast controls are **136.5/137** and **191/191.5 ms**.
OLS controls are slopes **67/68**, low three categorical FE **63/64.5**, one FE
with full outputs **17/17**, and low generated factors **70/67 ms**. The low
three-FE five-pair follow-up is **64 [63,72] / 63 [62,67]** on identical inputs.
Small residual losses are retained rather than tuned away.

The real plugin's warmed 16-iteration audits pass exactly zero allocation events
and zero current growth at fixed pool1/2 and T1/T4 in fresh processes, with stable
capacities and warm/cold equivalence. **The mixed-thread sequence still fails**:
`lowjoint4`, after a T4 pool was retired, records **events=0, growth=-4376** in
pool2/T1, with stable capacities and equivalent solves. Baseline `framemom1`
passed that sequence. An isolated intrusive build locates frees of **2072+2304**
bytes in Crossbeam epoch collection of Rayon queues. The audit rule was not
relaxed; the new fixture runs one externally selected configuration per process.
This runtime-accounting limitation remains open. It is a developer audit failure,
not a successful zero-growth mixed-sequence claim or a public science failure.

Keep other failures too: the first frozen compile omitted test fixtures; a
partial-formatting script later broke a session test, which was restored as a
complete function before final-source testing. `lowjoint1` contains that bad
*test source* and is not adopted. A trace-enabled allocation probe itself added
32 events and is not a valid allocation witness. The intrusive deallocation
backtrace run returns r(3499) and is attribution-only. Corrected builds, exact
identities, failure receipts and recipes are archived separately.

## Next work and remaining budget

The larger low fit still needs **211.6 ms (51.0%)** to reach 203.4 ms; the smaller
low cells need **72.6 and 71.1 ms**. Fast controls still need **23.0 and 97.7 ms**.
Do not treat those gaps as unavoidable verification overhead. The inner joint
kernel is already faster than stock on the matched strict frozen frames; the
whole estimator still constructs, transfers and certifies much more state.

First investigate scratch-backed inference as a bounded reusable-row consumer:
its current adapter issues a positional read for each row, unlike the existing
scan-local reader used in finite solves. Measure read counts and shared consumer
work before changing the interface. This is relevant across low PPML, not one
favorable slope case. Preserve row order, parallel ownership and independent
score/Hessian validation. Do not reopen this solver's tile-width tuning.

Then continue the established plan: joint ordinary directions for sparse
categorical PPML, whole-command one-FE concentration/loading/publication, and
remaining fast-slope setup/warm-start costs. The one-FE and sparse categorical
budgets in the prior source-bound report remain historical baselines requiring
matched remeasurement. Preserve existing wins and extend coverage adaptively.
Seed334 is consumed; seed335 is unused/reserved. Frozen 77/16/111 definitions,
weighted constrained-fit and nonretaining-CG issues, historical comparator
failures and SCC/platform deferrals remain unchanged. No push, release,
qualification, SCC retry or stopped-campaign restart occurred.

## Immutable receipts

The content-addressed archive contains **11,852 logical receipts**, **2,495 unique
payloads**, **48,929,573 bytes**. SHA-256:
`9ecddafb244db5f6835c10ff0d8a2f836c9883daad904274364ae168832873f1`.
It preserves eight source snapshots, identities, frozen numeric inputs/exports,
controllers, declarations, full distributions, science audits and extracted
allocation diagnostics. Raw licensed logs, plugins and binary public datasets
are excluded; omitted numerical files retain hashes. Machine paths are sanitized
with original/archive digests retained. `restore_receipts.py` verifies the
container and every logical file into a fresh directory; that restore passed.
