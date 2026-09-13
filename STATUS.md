# fereg 0.5.0 beta status

**Last updated:** 2026-09-13
**Branch:** main
**Mode:** prototype/development
**Release state:** public OLS and PPML beta surfaces; no qualified release,
tag, or Windows scientific qualification.

This is the sole mutable progress/handover record. Historical receipts remain
unchanged under benchmarks/reports/. This compact handover supersedes the
completed optimization/calibration investigation chronology formerly here.

## P0/R1/P7 bounded LSMR batches: adopted local checkpoint

The [adoption report](benchmarks/reports/development/lsmr-batch-adoption-local-2026-09-13/README.md)
supersedes the held-prototype decision at `cd298d5` without altering its immutable
72-pair investigation. The private adapter windows independent RHSs only above
64 columns and 4× the installed team, sharing one weighted solver. Narrow batches
retain upstream transport. Shared categorical OLS/PPML memory estimates now charge
recurrence, dense construction, scatter, output overlap and Schwarz stack/pool
state; see [ADR 0049](docs/adr/0049-bounded-lsmr-batches-and-memory.md).

Repeated wide weak OLS native peak falls 403.31→351.32 MB (12.9%) with +1.5%
median paired command cost; wide weak PPML falls 182.18→169.43 MB (7.0%) with
+0.5% command cost. Adoption rests on memory savings, not speed. A prospective
ordinary-command follow-up adds 12 pairs / 24 calls: six identical-binary A/A
controls and six A/B comparisons, interleaved over three seeds in both orders.
A/B differences are −6/−8/−17/+3/−16/−23 ms, all within the original review
envelope; A/A ranges −19 to +18 ms. Earlier traced PPML increases of 21/36/38 ms
remain unexplained and preserved. No causal narrow speedup is claimed.

Full science and non-time solver/retry decisions are exact. The preceding 67
LSMR-filter and 77 memory-filter tests (134 distinct), three independent licensed
fixtures in both arms and nine source-bound allocation probes remain applicable.
The reviewed source differs only by a corrected test comment; its universal plugin
is byte-identical to the measured `c9db0dce...` candidate and static/native arm64 ABI
checks pass. The report binds all 152 final native inputs and seven changed files.
A discriminating 327680-byte admission test rejects the old over-budget LSMR
construction and selects certified CG. Three unconverged allocation probes remain
allocation evidence only. The ill-conditioned smooth 65-column oracle and earlier
low-FW/saved-AW weak certificate failures remain failures, not passing controls.

Next repair actual attempted/fallback peak reporting: public OLS selected-model
bytes still omit solver increments. Opaque factor/stack bounds and full construction
counters remain open; this is not universal memory-bound qualification. Continue
remaining #27 packages before the authorized two-agent investigations, upstream
CMG assessment and new small SCC campaign. Installed dist, the 97 unrelated Veneto
STATUS additions and all pre-existing local evidence remain intact. No SCC,
qualification or push occurred.

## P0/R1 borrowed LSMR transport: adopted local checkpoint

The borrowed categorical LSMR adapter now delays its overwritten candidate
allocation until the upstream solve ends, releases RHS transport before output
materialization, and releases demeaned/unrequested coefficient banks before
independent certification. The original input and requested recovery coefficients
retain their scientific lifetimes; see
[ADR 0048](docs/adr/0048-borrowed-lsmr-transport-lifetimes.md).

The [report](benchmarks/reports/development/lsmr-transport-local-2026-09-12/README.md)
binds seventeen successful pairs / 34 measured commands against `15bd21d`.
Three-seed automatic weak-graph native peaks fall 30.37→27.31 MB at N30720/K10
and 469.17→403.31 MB at N122880/K65 (10.1%/14.0%). Command medians are
0.060→0.060 and 0.997→1.013 seconds; the wide median paired overhead is 0.9%.
This is a measured memory improvement, not a speed claim. Full successful
science and non-time solver decisions remain exact. Generic explicit fast
LSMR, strong Schur and complete IID control peaks remain unchanged. The PPML
checkpoint control adds 10 ms/3.9%, with exact science and retry diagnostics.

Sixty-two focused Rust tests and both arms of two explicit-dummy Stata oracles
pass, including retained batches, saved coefficients, raw-scale certificates
and constrained routing. Two other weak requests fail identical r(459) guards
on both artifacts: low FW cluster is uncertified; saved AW coefficient recovery
is not certified/aligned. All seven timed failed calls and the separate
untimed duplicate-solver warm-up failure are preserved. Those failures remain
unresolved and do not count as passing oracles.

The [P0 audit](benchmarks/reports/development/lsmr-transport-local-2026-09-12/memory-audit.md)
proves the OLS model's missing per-RHS/coefficient-space recurrence accounting
with six independent allocation tests and four pinned-within batch probes.
Reporting separately omits topology solver increments. Row permutation,
scatter, batch output overlap and Schwarz dense/setup/apply lifetimes also need
reconciliation; inspect the existing more detailed PPML model before sharing
it, including its unproven nested-concurrency bound. Public model excesses persist:
27.31 versus 13.39 MB and 403.31 versus 268.55 MB on these primary cases.
The small weak control exposes about 55 MB versus 1.045 MB. P0 is not closed.

Adopted native inputs match the measured universal plugin; static/native arm64
ABI checks pass. Installed dist, 97 unrelated Veneto STATUS additions and all
pre-existing untracked paths remain preserved. No SCC, qualification or push.
Next correct and validate joint memory accounting/reporting, then address the
remaining #27 work and preserved certificate failures. Finish #27 before the
authorized independent agents, upstream CMG assessment and new small SCC campaign.

## P3/P7 cluster-boundary setup: adopted local checkpoint

OLS ordered clustering and PPML paired-independent reductions now share a bounded
search for each sorted cluster's end. It preserves every chunk/boundary, empty
chunk, row/reduction order and allocation; see
[ADR 0047](docs/adr/0047-bounded-cluster-boundaries.md). It removes repeated scans
through giant clusters without splitting their numerical reductions.

The [report](benchmarks/reports/development/cluster-boundaries-local-2026-09-12/README.md)
binds twenty pairs / forty measured commands against `fdf4624`. Repeated skewed
OLS command medians fall 0.430→0.401 seconds (6.5% paired, 27–37 ms savings).
PPML inference falls 0.06969→0.03532, but total command medians only fall
2.914→2.893 (1.1% paired, variable 7–47 ms savings). Adoption rests on OLS's
repeatable complete-command gain. Primary native/scratch peaks are unchanged.

All science, samples, saved outputs and non-time solver decisions are exact.
The initial PPML two-way control slows 43 ms/12.5%, localized to unchanged solve
phases; three fresh rechecks improve 19/53/17 ms. The original result is retained,
and the source of timing variation is not established. Other controls stay within
the review envelope. Weak OLS retains a pre-existing memory-model discrepancy.
Twenty-five focused Rust tests and both strengthened 65,536-row Stata/Mata oracle
arms pass; the first OLS oracle was enlarged after review because it did not cross
a chunk boundary. Adopted native inputs match the measured plugin exactly;
universal static/native arm64 ABI audits pass. Installed dist, unrelated Veneto
work and historical evidence remain intact; no SCC, qualification or push ran.

Next address P0's observed/model memory discrepancies, then remaining P2/P3/P4
passes and P5 centered-tile work, carrying winners through P6/P7. R1–R5 remain
open. Finish #27 before the authorized independent agents, upstream CMG review
and new small SCC campaign.

## P3/P6 interaction coding: adopted local checkpoint

Categorical FE/cluster interactions now use one shared exact dense tuple mapping
when its full u32 table fits the displaced physical-row ordering bytes. Sparse
or overflowing products retain comparison sorting without partial allocation.
Original validation, lexical codes, sample/weight/singleton semantics and numerical
operations remain; see [ADR 0046](docs/adr/0046-bounded-interaction-coding.md).

The [report](benchmarks/reports/development/interaction-codes-local-2026-09-12/README.md)
binds 27 pairs / 54 measured commands against `9c2bcc5`: nine primary, fifteen
protected, one pilot and two public profiles. Repeated OLS command medians fall
0.439→0.393 seconds for interacted clusters (10.5% paired), and 0.370→0.345 for
interacted FE/clusters (6.8%, with 24–26 ms savings). Separate traces locate those
gains in preparation. PPML falls 0.383→0.370 (3.4%), but its recorded preparation
time barely changes; that smaller gain is not wholly attributed to this timer.

Full science, samples, saved values and non-time solver diagnostics are exact.
Primary native peaks are unchanged within their models; the largest protected
command increase is unchanged IID +4 ms/0.7%. Graph controls retain pre-existing
memory-accounting gaps, including weak OLS 30.6→30.9 MB against a 13.6 MB model;
this does not close P0. Forty-one distinct focused Rust tests and both independent
Stata/Mata OLS/Poisson oracle arms pass. The final comment-reviewed universal
plugin is byte-identical to the measured candidate; static/native arm64 ABI checks
pass. The first comment-layout rebuild mismatch is retained in validation.

Installed dist, unrelated Veneto changes/evidence and historical receipts remain
intact. No SCC, qualification or push occurred. Next evaluate the duplicate
cluster-boundary scans under P3/P7, then remaining score reconstruction and the
P0/P2/P4–P7/R1–R5 work. Finish #27 before the authorized two-agent investigation,
upstream CMG review and new small SCC campaign.

## P3 shared subset score passes: adopted local checkpoint

Scratch-backed OLS covariance can now feed all subset accumulators from one
original-row traversal when the entire compensated bank, range metadata and
prefix scratch fit one displaced physical-row ordering budget. Planning is
all-or-none; larger products retain the accepted per-subset route. Group/merge
order, independent per-group bread, weights, PSD, rank and publication contracts
remain unchanged; see [ADR 0045](docs/adr/0045-batched-low-memory-cluster-scores.md).

The [report](benchmarks/reports/development/cluster-subset-batch-local-2026-09-12/README.md)
binds 26 pairs / 52 measured commands against `c37686b`, including original and
revised pilots, six original primary pairs, nine controls, six fresh many-way
pairs, a separate profile and two memory rechecks. The original two-/three-way
10% target was missed (0.7%/5.9% paired gains). A separately specified follow-up
repeats five-way gains of 20.4% at N30720/K4 and 17.4% at N122880/K10 with AW,
with command medians 0.054→0.043 and 0.441→0.365 seconds. Adoption rests on those
many-way results; ordinary two-way gain remains unconvincing.

All full scientific exports, saved values and non-time solver diagnostics are
exact. Primary native/scratch peaks are unchanged. Worst protected command
slowdown is one-way +5 ms / 2.0%. Weak controls still exceed their pre-existing
memory models. A two-way native-peak increase changes direction in rechecks;
the source of variation remains unestablished, and no model-bound pass or
universal RSS reduction is claimed. Bank bytes and fewer score traversals are
source-derived counts, not measured physical I/O.

Thirteen distinct focused Rust tests and both arms of an independent full-dummy/
Mata covariance oracle pass, covering unit/AW/FW/PW, full/deficient rank, all
31 five-way masks, exact admission bounds and fallible reservations. Initial
oracle failures (DoF rank, omitted PSD repair, eigenvector orientation) and the
slower initial prototype are preserved. The reviewed universal plugin is
byte-identical to the measured revised artifact; static/native arm64 ABI checks
pass. Installed dist, unrelated Veneto changes and historical receipts remain
intact. No SCC, qualification or push occurred.

Next: remaining P3 interaction construction/order reuse, skew scheduling and
score residualization; P0 memory/counter reconciliation and P2/P4–P7/R1–R5 remain
open. This completes the bounded low-memory subset-sharing experiment, not #27.
Finish #27 before the authorized independent agents, CMG review and new small
SCC campaign.

## P3 low-memory cluster scores: adopted local checkpoint

Low-memory OLS now reads eligible cluster score subsets sequentially, using
compensated group scores only when their complete table and presence flags fit
inside the displaced N-row ordering budget. Larger/unrepresentable Cartesian
products retain tuple sorting. Original within-group row order, lexicographic
group order, per-group bread and independent covariance arithmetic remain;
see [ADR 0044](docs/adr/0044-sequential-low-memory-cluster-scores.md).

The [report](benchmarks/reports/development/cluster-grouping-local-2026-09-12/README.md)
binds thirteen pairs against `2243b4d`: six primary comparisons, six controls and
one separate pilot. Complete-command paired gains are 16.4% for low FW one-way
clustering (0.307→0.259 s) and 48.2% for low PW two-way clustering (1.870→0.964 s).
Primary native/scratch peaks are unchanged; full scientific exports, saved
outputs and non-time solver diagnostics agree exactly. Every bounded control
improves, including weak, high-cardinality fallback, small, fast, saved and no-FE
cases. Single-pair control gains are not repeatability claims.

The weak control retains a substantial pre-existing observed/reported model
discrepancy: 29933828→29827588 native bytes versus 13460424 modeled. Primary and
other control bounds hold. PW sampled RSS rises 0.169→0.192 GiB; no general RSS
saving is claimed. A sample-hash-verified cache replay explains fewer repeated
tile requests, but is explicitly source-derived rather than measured disk I/O.

Nine distinct focused Rust tests and both arms of the full-dummy Stata b/V oracle
pass, covering unit/AW/FW/PW, full/deficient rank, precise table bounds, overflow
fallback and allocation failures. Universal static/native arm64 ABI audits pass;
the reviewed rebuild is byte-identical to the measured prototype. Installed dist
and unrelated Veneto work/evidence remain intact; no SCC or qualification ran.

A separate sequential-histogram prototype is not adopted: its ordering kernel
improves, but nine command pairs show under-2% noisy gains with changes in
unchanged input work. Both experiments and all attempts are archived. Existing
simple cluster codes and covariance/certificate order sharing were already in
place. Next P3 work is combining subset consumers within bounded score storage,
interaction construction/order reuse, skew and remaining score reconstruction.
P0 model/counter reconciliation, P2/P4–P7 and R1–R5 remain open. Continue #27
before the authorized two-agent investigation, CMG review and small SCC campaign.

## P3 inference ownership: adopted local checkpoint

Ordinary fast robust/clustered OLS now packs score values only up to 32 MiB,
then uses the existing retained strided route when the finite solve permits it.
This also selects existing stable raw moments and fused residual construction.
Shared weighted-linear/PPML policy, IID, low memory, rank, memory admission and
public contracts remain unchanged; see
[ADR 0043](docs/adr/0043-ols-inference-strided-cache-policy.md).

The [report](benchmarks/reports/development/ols-inference-ownership-local-2026-09-12/README.md)
binds 29 pairs / 58 measured calls against native baseline `f4cebe7`. The final
comment-reviewed source rebuild is byte-identical to the prototype plugin.
Three-seed command medians fall 1.312→0.803 seconds for robust missing,
0.368→0.253 for PW robust and 0.539→0.409 for FW clustering; paired gains are
38.8%, 31.4% and 24.1%. Primary native peaks fall about 25–26%, and every observed
action peak fits the unchanged model. These gains include reference/residual
passes; strided score certification costs more and covariance time stays similar.

Samples/status/DoF, FE/cluster tables and solver decisions agree exactly. Full
b/V differences are at most 3.34e-16 / 7.20e-21, with unchanged independent
certificates/tolerances. Newly eligible weak, no-FE, K65 and interacted two-way
cluster cases pass. No protected control exceeds max(10 ms, 5%); complete IID
is +12 ms / 2.1%, small/narrow +1 ms. QR profiles improve 4.0% while retaining
about 0.12 seconds of score reconstruction in both arms; that work is not removed.

[Validation](benchmarks/reports/development/ols-inference-ownership-local-2026-09-12/adoption/validation.json)
records eighteen distinct focused Rust tests and an explicit-dummy full-covariance
Stata oracle on baseline/candidate across unit/AW/FW/PW, robust/clustered and full/
deficient rank, with r(0) sentinels. Universal static/native arm64 ABI checks pass.
The initial pilot diagnostic-assertion failure remains disclosed in immutable
preliminary evidence. No full profile, SCC, Windows or qualification ran;
installed dist and unrelated Veneto work remain preserved.

P3's packed/strided lifetime comparison is adopted. Next audit actual repeated
cluster-state/order construction and score consumers, then evaluate useful tiling,
skew scheduling and low-memory duplicate work. P0/P2/P4–P7, R1–R5 and the later
agent/CMG/new-SCC phases of the active goal remain open.

## P1 candidate-run and mixed-source loading: adopted local checkpoint

Against `5411b9b4d37aab1a856b12431db84ecfce1c6f22`, shared OLS/PPML input loading
batches numeric peers of strings and long retained candidate runs at N≥16,384.
Runs under 64 rows stay scalar, chunks stay bounded at 65,536, and strings keep
original encounter order. PPML offsets/exposures use the same reader. No new
observation-sized storage, excluded-cell reads, classifier, memory-admission,
solver/inference or publication change is introduced; see
[ADR 0042](docs/adr/0042-candidate-run-numeric-loading.md).

The [report](benchmarks/reports/development/candidate-input-local-2026-09-12/README.md)
binds isolated plugins and 29 completed pairs / 58 measured calls. Three-seed
command medians fall 0.615→0.445 seconds for restricted AW OLS, 0.528→0.430 for
FW OLS with string clustering, and 1.643→1.539 for restricted PW PPML with offset.
Paired gains are 27.6%, 18.9% and 6.3%. OLS input medians fall 0.390→0.217 and
0.210→0.113 seconds with stable fit/inference. PPML has no separate input timer;
its unchanged finite work also varies, so the whole gain is not attributed to
loading. Complete-data adjustment-only pairs show no reliable command gain.

All scientific exports, samples/status/DoF, certificates, iterations, retries,
fallback decisions and saved outputs match exactly. All measured action peaks
fit unchanged models; primary peak medians are unchanged. One AW pair uses 0.7%
more native peak. Worst graph-control slowdown is weak robust +8 ms / 3.7%;
complete numeric, fragmented restrictions, small/narrow, low-memory and saved
controls remain inside the predeclared review envelope. No RSS gain is claimed.

[Validation](benchmarks/reports/development/candidate-input-local-2026-09-12/validation.json)
records 43 focused plugin tests, three candidate licensed fixtures and the new
explicit-dummy/Poisson/missing-code oracle on baseline, with r(0) sentinels.
Universal static and native arm64 ABI audits pass. A PPML control input-cache
name collision stopped setup before estimation; all four controls passed with
distinct identities afterward. Initial unit expectation and collector corrections
are disclosed. Installed dist, unrelated Veneto work and historical evidence
remain untouched. No SCC, push or qualification ran.

P1 loading is adopted; adjustment-only speedup remains unsupported. Continue P3
inference ownership/lifetime comparisons and the remaining P0/P2/P4–P7 work.
The full active goal (including later independent agents, CMG assessment and the
new small SCC campaign) remains open; P1 completion does not complete #27.

## P1 PPML ado preparation: adopted local checkpoint

Against `8be388e280eb64af5de9830204a63bee8de3b33d`, remove three nested PPML
sort-preservation scopes and the optional ado completeness marker/markout/count.
Candidate and weight-expression handling, authoritative native row/domain rules,
pre-admission classification and all publication/rollback checks remain intact.
[ADR 0041](docs/adr/0041-ppml-ado-preparation.md) records the decision.

The [report](benchmarks/reports/development/ppml-ado-preparation-local-2026-09-12/README.md)
binds the same native plugin in both arms and 25 pairs / 50 measured calls.
Primary complete-command paired medians improve 2.4% (complete one FE), 1.1%
(FW/exposure/missing) and 0.4% (PW/offset/two-way cluster). Outer Stata medians
fall 0.271→0.101, 0.092→0.052 and 0.074→0.042 seconds. Full native work varies;
these are modest command gains, not native-kernel gains. All full scientific
exports and solver/retry/fallback decisions are exact. Primary native peaks
are unchanged and all action peaks fit their unchanged models.

The low-memory control initially slows 957→1085 ms. Six retained follow-ups
plus that original yield a −1.0% median paired change, with variation mostly in
unchanged native finite/strict work; no low-memory speedup or precise external
cause is claimed. Other graph controls slow by at most 6 ms. Four distinct
candidate licensed fixtures and the baseline native-preparation fixture pass
with r(0) sentinels; 17 transaction and five registration tests pass. Tests cover
independent Poisson science, all missing codes, FW/PW, string clusters, singleton
ordering, saved outputs, original row/sort metadata and injected failure exits.
The [validation](benchmarks/reports/development/ppml-ado-preparation-local-2026-09-12/validation.json)
retains the corrected initial weighted-varlist draft failure. Installed/dist and
separate Veneto work remain untouched. No SCC or qualification ran for this step.

Continue #27's remaining preparation/loading, pass, inference, centered-tile,
output and scheduling investigations; the five weak-route allocation events also
remain open. The active user goal now explicitly authorizes completing #27,
then two independent agent investigations and their evidenced optimizations,
then assessment of upstream CMG small-component changes, followed by a new small
SCC benchmark report (about ten tasks, roughly 3–5 hours each, smaller datasets
across graph families and primarily fixest/pyfixest/reghdfe comparisons). This
is later work, not a restart of either stopped historical campaign. No push is
authorized. P0–P7 and R1–R5 completion must be evidenced, not inferred from the
completed checkpoints.

## P4 stable reference passes and scratch: adopted local checkpoint

Against `d32f05e45519aaee5c5d44bc87caddb4f68482f3`, two isolated changes are
adopted: reuse the retained identity bank for frozen-solve QR scratch, then
schedule current-weight reference norms/means by column for fast PPML with one
categorical FE at N≥65,536, eight stored columns and multiple installed workers.
Each column preserves original row order, scaled norms and compensated means;
there is no observation-sized cache, stale-weight reuse or solver/rank-policy
change. See [ADR 0040](docs/adr/0040-ppml-stable-reference-pass-scheduling.md).

The [report](benchmarks/reports/development/ppml-reference-passes-local-2026-09-12/README.md)
and [identities](benchmarks/reports/development/ppml-reference-passes-local-2026-09-12/identities.json)
bind three plugins and 36 paired comparisons / 72 measured calls. Scheduling
versus the allocation-only baseline improves public primary command medians
11.800→10.179 seconds (complete one-FE robust, N983040/K50/16 threads),
3.741→3.342 (FW/exposure/missing, N245760/K50/four threads), and 3.546→3.075
(PW/offset/two-way cluster, N245760/K50/16 threads). Median paired gains are
13.7%, 10.7% and 14.8%. Existing projection and strict/final timers locate the
savings. Preparation/inference remain near baseline; primary native peaks are
identical across arms at about 2.523 GB, 633.5 MB and 645.3 MB. All
observed native action/profile peaks remain within unchanged models. RSS is
reported separately, including an FW median increase; no RSS gain is claimed.

All scientific exports, sample/status/DoF, actual/reported solver iterations,
strict retries, fallback decisions and saved outputs are exact. Weak/dense/
bottleneck/cycle/disconnected/unbalanced, small/narrow, low-memory, restricted,
saved/separated and row-threshold controls pass. Worst scheduling slowdown is
55→62 ms on disconnected (+12.7%, +7 ms), below the declared review threshold.
An allocation-only low-memory flag (878→963 ms) did not reproduce in two
independent-seed repeats; the original remains archived.

The scratch fix removes the observed one allocation per frozen solve. Real
16-iteration audits change 16→0 events for fast PW and low-memory FW one-FE
cases, with passing whole-iteration allocation certificates. Subsequent FW/PW
scheduling profiles also record zero. The weak-graph audit changes 21→5 events
and **still fails** that operational certificate, despite stable live/peak
memory and exact science. The failed broad assertion and narrowed adoption
claim are explicitly recorded; the five residual events remain open.

[Validation](benchmarks/reports/development/ppml-reference-passes-local-2026-09-12/validation.json)
records 148 distinct focused Rust tests, three licensed allocation-only fixtures,
a new N65536/K8 full-dummy Poisson/Hessian/sandwich oracle against the final
plugin, five registration checks and universal static/native arm64 ABI audits.
No full scientific profile, SCC, Windows or qualification work ran. Installed
dist and unrelated Veneto status/evidence remain preserved; nothing was pushed.

Next: localize the five weak-route allocation events, then measure remaining
raw RHS/stride and within-norm passes or P3 inference ownership. Current-frame
norm reuse, skipping fresh-input checks, broader generated-column/low-memory
coverage and P5 centered tiles remain hypotheses. Scheduling does not itself
remove a mathematical reference-statistic pass.

## P4 PPML one-FE projection: adopted local development checkpoint

Against `0889721a7c781618dd9f132642a85d065e34afad`, fast PPML with one categorical
intercept effect now enters the existing reusable categorical projector and
fused changing-response/retained-design projection. It certifies after the first
group sweep, preserves zero-iteration certification of useful warm candidates,
and allocates no accelerator matrices. Current-weight raw reference norms/means,
rank/QR policy, final-Hessian synchronization, independent certificates and
strict RAII rollback remain unchanged. No automatic family region, memory
admission or separate reconstruction behavior changes; see
[ADR 0039](docs/adr/0039-ppml-one-fe-retained-projection.md).

The [report](benchmarks/reports/development/ppml-one-fe-local-2026-09-12/README.md)
and [identities](benchmarks/reports/development/ppml-one-fe-local-2026-09-12/identities.json)
bind 23 paired comparisons / 46 measured calls: nine public primary pairs,
ten protected pairs, three separate private allocation profiles and one pilot.
Three-seed command medians fall 14.027→11.724 seconds for complete one-FE robust
PPML at N983040/K50/16 threads, 4.422→3.708 for N245760/K50/four-thread FW with
exposure/missing data, and 4.223→3.572 for N245760/K50/16-thread PW with offset and
two-way clustering. Median paired gains are 16.3%, 16.0% and 15.7%. Projection
medians fall 3.555→2.001, 1.201→0.730 and 1.203→0.681 seconds; strict/final
medians fall 5.046→4.281, 1.292→1.102 and 1.243→1.078. Preparation and inference
are largely unchanged; overlapping timers and independent medians do not sum.

Reported outer/inner counts, step halvings, strict retries and fallback decisions
are unchanged. Actual MAP iteration totals fall 5→3 and 6→4 because certified
warm solves can require zero projections; the runtime count charges at least one
per solve. All samples/statuses/DoF/weight/cluster/separation/receipt identities
are exact; b/V differ at roundoff scale (max 5.99e-17 / 1.28e-20), and independent
certificates and saved outputs pass. Unaffected weak/dense/bottleneck/cycle/
disconnected/unbalanced and low-memory routes retain exact science. Worst
protected slowdown is cycle 57→64 ms (+12.3%, +7 ms), below the predeclared
max(10 ms, 5%) review threshold; small/narrow and requested-output controls improve.

Primary native peaks rise 3.4–3.5% (84.16 MB large, 21.04 MB weighted) because
the existing group/RHS workspace now serves one FE. All observed action and
profiled phase peaks remain inside unchanged models. Real allocation audits in
both arms record 16 events in 16 iterations, without live/peak growth: retained
capacities and fresh/warm equivalence pass, but the whole-iteration
allocation-free certificate remains false. Event sizes/call sites remain open;
neither mock-probe tests nor passing science imply that operational certificate.

[Validation](benchmarks/reports/development/ppml-one-fe-local-2026-09-12/validation.json)
records 140 focused Rust tests, five licensed fixtures with r(0) sentinels and
five profile-registration checks. New independent weighted group-mean and
full-dummy Poisson/Hessian/sandwich oracles cover unit/FW/PW and robust/clustered
inference. Universal static audits and native arm64 ABI tests pass; no complete
scientific profile, SCC, Windows or qualification run occurred. The installed
plugin and unrelated Veneto status/evidence remain preserved.

The subsequent reference-pass/scratch checkpoint above supersedes this
checkpoint's allocator and scheduling resume point. Its original
[cost audit](benchmarks/reports/development/ppml-one-fe-local-2026-09-12/cost-audit.md)
remains source-bound evidence; current-frame norm reuse and the broader pass
experiments remain open.

## P1 PPML numeric ownership: adopted local development checkpoint

Against `b19a1cfac038dcc6fbf015cfbfaf9f51022dafb4`, production PPML now constructs
the admitted numeric row ledger and weight reports without a throwaway compact
copy of outcome/model sources. It consumes offset/exposure storage through stable
compaction within the candidate-capacity bound. Raw original-axis sources and raw
pweights remain available for separation rebuilds. Allocation-free preflight and
exact shape agreement, original-row errors, canonical weight/exposure semantics,
solver/session/reconstruction code and existing memory admission remain unchanged.
[ADR 0038](docs/adr/0038-ppml-numeric-selection-ownership.md) records this boundary.

The [report](benchmarks/reports/development/ppml-owned-preparation-local-2026-09-12/README.md)
and [identities](benchmarks/reports/development/ppml-owned-preparation-local-2026-09-12/identities.json)
bind isolated universal plugins and 23 completed pairs / 46 measured calls:
nine public primary pairs, ten protected pairs, three private audit/profile pairs,
and one separately reported exploratory pilot. Three-seed command medians fall
15.395→14.408 seconds for N983040/K50/16-thread complete one-FE robust PPML,
4.683→4.534 for N245760/K50/four-thread FW with exposure/missing data, and
4.591→4.452 for N245760/K50/16-thread PW with offset/two-way clustering. Median
paired gains are 6.4%, 4.5% and 3.3%. Existing preparation-phase medians fall
1.615→0.880, 0.351→0.227 and 0.358→0.221 seconds; this phase also includes IRLS
initialization. Inference and all iteration/solver/fallback decisions are exact
apart from timing. Every b/V/sample/status/DoF/weight/cluster/separation/certificate
and requested saved-output comparison passes exactly.

The removed complete-case source-copy payload is 408,944,640 bytes, derived from
source shape rather than hardware traffic. Overall native peak stays about
2.438 GB for the large complete case; weighted peaks fall about 2.21 MB because
the loaded adjustment no longer remains live beside its canonical offset. Later
session initialization dominates the preparation phase peak. Private allocation
audits pass, with all observed action peaks within unchanged models. Sampled RSS
and all phase/peak/fallback results are retained; no 409 MB fit-peak reduction is
claimed. Weak/dense/bottleneck/cycle/disconnected/unbalanced, small/narrow,
restricted, low-memory and saved-separation controls retain exact science; worst
slowdown is 1 ms. Low-memory FW improves 0.925→0.843 seconds.

[Validation](benchmarks/reports/development/ppml-owned-preparation-local-2026-09-12/validation.json)
records 136 focused Rust tests, four licensed Stata fixtures with explicit r(0)
sentinels, analytic Poisson/FW-expansion/PW-scaling oracles, and universal static
audits plus arm64 ABI/self-tests. One weighted extraction attempt was excluded
after a shared benchmark do-file edit corrupted Stata's incremental reading;
per-run worker copies/hashes now prevent that race, and both arms were rerun.
The installed/dist plugin, separate Veneto STATUS additions and historical
evidence are preserved. No SCC, push or qualification occurred.

P1 numeric ownership is adopted. The source audit identifies PPML's triple
sort-preservation wrappers and input markout/count as removal candidates, but
does not adopt ado changes. Batched adjustment/restricted-source loading,
post-admission parallel classification, generation-copy reduction and P0 counters
remain open. Next P4 work should isolate fresh one-FE RHS/norm/mean construction,
warm projection and strict/final-Hessian passes while preserving reference scales
and weight-generation validity. The large candidate's projection/finite/strict
scopes remain substantial; these scopes overlap and must not be summed. P3 OLS
inference remains a separate measured opportunity after the earlier Schur gains.

## P2 weighted OLS Schur: adopted local development checkpoint

Against baseline `39d1a2a876b92dcc7f8032be792ee171cb85cfdd`, the OLS planner now
admits supplied weights through the existing three-RHS probe and weighted Schur
engine. The probe uses actual normalized weighted degrees, statistics and
operator applications. It preserves the 0.01 contraction guard, eight charged
probe steps, 64-step full attempt, shared continuation budget, raw reference
scales and independent final certificates. The 16--64 actual RHS / 16,384-row
fast-mode envelope, saved-FE exclusion and explicit solver semantics remain.
[ADR 0037](docs/adr/0037-weighted-ols-schur-admission.md) records the development
admission. PPML session/reconstruction files and separate Veneto work are intact.

The [report](benchmarks/reports/development/weighted-schur-local-2026-09-12/README.md)
and [source/artifact identities](benchmarks/reports/development/weighted-schur-local-2026-09-12/identities.json)
bind isolated universal plugins and 39 completed pairs / 78 measured commands.
Three-seed command medians fall 6.623→1.244 seconds for N983040/K50/four-thread
AW two FE, 0.780→0.237 for N245760/K50/16-thread FW, and 1.042→0.490 for PW
two-way clustering. Median paired gains are 80.8%, 69.6% and 53.0%. Separate
traces reduce initial absorption 5.407→0.466, 0.530→0.056 and 0.598→0.065 seconds,
including probes of 45.1, 8.0 and 6.8 ms. No natural full-solve fallback occurs.

The large AW target's native peak rises 12.4%, from 1.156 to 1.300 GB within
its unchanged 1.675 GB model. FW and PW native peaks fall; all per-case native,
modeled and sampled RSS results are retained. Unchanged low and small/narrow
controls have observed/model discrepancies of +10.0% and +27.9% in both arms;
these remain P0 modeling work, not qualified memory evidence.

All numerical comparisons and original certificates pass, with maximum full
b/V differences 2.67e-15 / 3.55e-14. Unit-weight weak/dense/bottleneck/cycle/
disconnected/unbalanced controls retain exact science and solver decisions,
with at most 7 ms slowdown. Tiny weighted cuts reject Schur (0.0583 contraction)
and keep exact CMG science. Their repeats have median paired cost +3.8%; an
individual +16 ms / 6.1% case and the traced 2.7 ms rejected probe are disclosed.
The remaining traced cost lies mostly in unchanged legacy absorption; its
precise cause is unestablished. Small/narrow repeats are 14→13 ms medians after
an initial 11→15 ms observation. Saved outputs remain exact; width/low controls
preserve the existing eligibility boundaries.

[Validation](benchmarks/reports/development/weighted-schur-local-2026-09-12/validation.json)
records 60 prepared-fit and three additional Schur tests, four distinct licensed
Stata fixtures with a fifth invocation for added rank checks, binary audits and
actual arm64 ABI/self-tests. Weighted dummy, frequency-expansion, AW/PW rescaling,
finite/FE collinearity, 1/4/16-thread, memory-budget, forced-continuation and
transaction checks pass. Two builds from an incorrect working directory were
excluded before comparisons; correctly isolated artifacts were used throughout.
The installed/dist plugin is unchanged. No SCC, push or qualification occurred.

P2 weighted probe/admission, bounded continuation and width comparisons are
adopted. Simple weighted one-FE already uses direct projection; complex one-FE
pass reduction and separate constant-weight specialization remain open. Continue
with #27's P4 one-FE/pass work and remaining P1 PPML materialization, preserving
the allocation-free preflight and reconstruction repair; P3 inference remains
a measured bottleneck after the absorption gains. P0 modeling/counter gaps and
the weighted-cut routing cost remain explicit follow-up work.

## P0/P1 OLS preparation: adopted local development checkpoint

Roadmap [#27](https://github.com/johannes-schmieder/fereg/issues/27) supersedes
#26 and #16--25 for optimization priorities. The baseline for this work is
`e93fe437f1d8855e1c3aa47ad03026eb23a9942d`, including the committed PPML
reconstruction repair. Its separate evidence and the Veneto status edits below
are preserved; no cluster job or installed/dist plugin was changed.

All six OLS preparation branches now consume loaded source buffers through
one ownership-aware boundary: unweighted/AW/FW/PW; IID/robust/one- and multiway
cluster inference; simple, interacted, slope and saved absorbed terms. The
existing row classifier, original-row domain precedence, raw weight kinds and
values, cluster missingness before singleton deletion, true categorical
singleton fixed point, final coding and candidate-capacity bounds are retained.
Wide preparation reuses the requested worker team after SPI loading. PPML
preflight/materialization and all solver/inference/transaction rules remain
unchanged. [ADR 0036](docs/adr/0036-native-ols-input-preparation.md) and the
[coverage/cost report](benchmarks/reports/development/ols-owned-preparation-local-2026-09-12/README.md)
record this extension.

[Source/artifact identities](benchmarks/reports/development/ols-owned-preparation-local-2026-09-12/identities.json)
bind isolated universal baseline/candidate plugins. The
[41 paired comparisons](benchmarks/reports/development/ols-owned-preparation-local-2026-09-12/comparisons.json)
include 82 successful measured local calls. Three-seed command medians fall
from 1.870 to 1.294 seconds for unweighted robust sparse-missing N983040/K50,
1.197 to 0.641 for AW IID missing, 0.652 to 0.527 for FW clustering at N245760,
0.490 to 0.352 for complete PW robust, and 0.638 to 0.510 for an interacted FE
with two-way clustering. Separate traces reduce AW preparation from 0.598 to
0.042 seconds while fit/inference remains about 0.204 seconds.

All pairs have exact full b/V, sample, status, DoF, fit, cluster and certificate
results and solver decisions apart from timings. Native peaks agree in 35/41
pairs, including every primary pair; restricted preparation falls from 129.1
to 118.3 MB. Weak/dense/bottleneck/cycle/disconnected/unbalanced controls retain
science, with at most 7 ms slowdown. Small/narrow repeats remain 7 versus 9 ms;
this disclosed regression is within the predeclared 10 ms review envelope.
The original 1.973-second AW candidate input-loading outlier remains in the
primary data; three separate rechecks are 0.614--0.623 versus 1.172--1.194.

[Validation](benchmarks/reports/development/ols-owned-preparation-local-2026-09-12/validation.json):
160 core tests, 40 plugin tests, 29 independent Rust oracle/preparation tests,
six distinct licensed Stata fixtures plus a constrained-memory rerun, universal
binary audit and actual arm64 ABI/self-test pass. The 543870912-byte constraint
rejects fast with r(909) and admits low/auto with matching science. The Python
boundary screen remains 25/26 on both baseline and candidate: an unchanged
prepared-view dynamic-dispatch assertion fails in `ols/prepared_fit.rs`.
No test is weakened or reported as passing when it failed.

P1 OLS ownership/weights/clustering/complex terms/team reuse is adopted for local
development. P0's source-derived copy costs and representative routes are
recorded; hardware traffic and full setup/build counts remain unmeasured.
PPML post-admission ownership, adjustment loading/ado work and restricted/mixed
SPI batching remain open. Next priorities remain P2 weighted absorption and
P4 one-FE PPML passes, then measured inference ownership and stable centered
tiles under #27. No new automatic solver region, SCC, Windows, qualification,
push or release is implied.

## PPML reconstruction fix: completed development validation

The requested Veneto repair changes the retained weighted-linear session's
reconstruction check to an operation-count rounding bound and constructs
canonical PPML predictors directly from the working response and residual.
The original response-relative diagnostic and final publication tolerance
remain unchanged, as do all other scientific checks and the one-shot OLS path.
[Numerical contract](docs/ppml_reconstruction.md) records the bound and
independent rational oracle, including corrupted and nonfinite negative cases.

[Local evidence](benchmarks/reports/development/veneto-ppml-reconstruction-2026-09-12/local-tests.json) records
94 passing Rust tests. The rebuilt universal macOS plugin passes binary audit,
actual arm64 ABI/self-test, and four licensed fixtures: OLS and PPML smoke,
PPML scientific contract, and weighted inference. Strict Clippy remains blocked
by six pre-existing lints in unchanged OLS/FE files, with none reported in the
changed files.

The [repair report](benchmarks/reports/development/veneto-ppml-reconstruction-2026-09-12/README.md) records
successful full Veneto fits on all 3,159,720 observations at four/fourteen
threads: 133.324/101.878 seconds, both with 15 outer and 589 inner iterations.
Slopes differ from the preserved ppmlhdfe reference by at most 4.59e-12;
SE and full slope covariance comparisons pass. Both final score/Hessian checks
pass, with identical samples and slightly better reported objectives. Native
allocation peaks are 5.958/7.374 GiB; these single runs are diagnostic, not a
general scaling claim.

SCC job **7536374** completed with scheduler `failed=0`, `exit_status=0` and
application/job sentinels. It reused the Linux plugin built and smoke-tested
in job **7536367**, whose later controller path typo stopped before full-panel
estimation. Both attempts remain documented. The
[validation](benchmarks/reports/development/veneto-ppml-reconstruction-2026-09-12/scc-validation.json) and
[source manifest](benchmarks/reports/development/veneto-ppml-reconstruction-2026-09-12/source-manifest.json)
bind the results to snapshot
`659924a603a3b750a7478cad608d95022166b5a83956e066d9b9e29698420b96`;
all 484 source/fixture files match the final working tree. The source and
portable validation evidence form the requested development checkpoint. No
qualification or broad benchmark restart occurred.

## Incomplete OLS compaction: preceding implementation

The user-requested development checkpoint collects the adopted OLS sources,
fixtures, benchmark tooling and publishable numerical evidence. The
[checkpoint inventory](benchmarks/reports/development/adopted-ols-checkpoint-2026-09-12.json)
verifies that production and fixture bytes match the retained local validation.
It records the path-bearing captures preserved outside Git and the configurable
SCC directory settings. No estimator benchmark or qualification was rerun for
this packaging step. Historical source identities and receipts are unchanged.

Unweighted IID preparation now consumes the authoritative row classifier's
drop mask and compacts owned columns independently. Wide inputs classify
16,384-row blocks on the installed requested team, preserving original-row
error order. Buffer reuse stays within the prior candidate-axis capacity
bound. Complete inputs retain their shortcut; weighted, complex categorical,
robust/clustered and PPML preparation keep their existing paths. No solver,
sample, tolerance, inference or transaction rule changes.

The [compaction report](benchmarks/reports/development/fixest-compaction-local-2026-09-12/README.md)
records 53 completed local invocations: 51 successful estimates and two
explained all-singleton negative controls. For 983,040 rows, 50 controls and
about 0.1% missing, three-seed previous/current medians are 1.461/0.926 seconds
at four threads and 1.230/0.613 at sixteen, improvements of 36.6% and 50.2%.
The original four-thread input-time outlier remains in the summaries; an
extra pair does not reproduce it. Complete-data one FE remains essentially
unchanged at 0.585/0.580 seconds versus fixest's 0.472, leaving a 22.9% gap.
Weak, dense, bottleneck, cycle, disconnected and unbalanced holdouts differ
by at most nine milliseconds and preserve all numerical results.

All 20 final candidate/previous pairs have byte-identical full b/V exports,
exact samples/statuses/DoF/fit/certificate tables and solver diagnostics
except timings. All seven fixest comparisons pass. Native allocation peaks
agree in 18 pairs; the two heavily filtered retained-pair holdouts fall from
220,448,053 to 120,419,044 bytes. Both versions return Stata code 2001 when
the initial filtering recipe leaves only singletons; those original failure
receipts remain intact.

[Validation](benchmarks/reports/development/fixest-compaction-local-2026-09-12/validation.json)
records 37 core preparation tests, five sample tests, 40 plugin action tests,
eight licensed Stata fixtures, universal binary audit and actual arm64
ABI/self-test. New coverage includes cross-block error order, retained value
bits/capacity, singleton cascades, and independent sparse/dense/restricted
wide dummy regressions at 1/4/16 threads.
[Replayable source identities](benchmarks/reports/development/fixest-compaction-local-2026-09-12/identities.json)
bind the retained plugin SHA-256
`43377269d0bbfd7dc1bc2a964e117d9f282e123350820252289e3f6b76a92fae`.

Remaining complete-data one-FE component medians are 0.303 seconds input,
0.210 native and 0.069 outer Stata work. Further parity work must address
loading or native estimation passes. Ownership/column compaction for weighted
and robust/clustered paths is a separate next experiment; small/narrow gaps
also remain unresolved. No SCC, remote job, qualification, commit, push or
release action was taken.

## Native OLS input preparation: preceding implementation

The local follow-up moves full-candidate numeric missing detection from ado
into Rust, removes the unused OLS sort-preservation wrapper, and installs
wide IID preparation on the requested worker team for independent source
completeness checks. The C bridge only transports SPI values and the SDK
missing boundary. Restricted/string loading, authoritative sample rules,
solver routes, tolerances, all numerical formulas and transaction guarantees
remain unchanged. [ADR 0036](docs/adr/0036-native-ols-input-preparation.md)
records the development extension to ADR 0013's loading procedure.

The [input report](benchmarks/reports/development/fixest-input-local-2026-09-12/README.md)
records 134 completed local measurements, including 54 rotated primary calls
at three seeds. At 983,040 rows and 50 controls, previous/current/fixest
medians are 0.916/0.776/0.806 seconds for one FE at four native threads,
0.690/0.582/0.454 for one FE at sixteen, 0.878/0.763/0.841 for degree three
at sixteen, and 0.836/0.712/0.695 for degree six at sixteen. Current fereg
narrowly beats fixest on four-thread one FE, leads degree three by 9.3% at
sixteen threads, and trails degree six by 2.4%. One FE at sixteen threads
still trails by 28.2%. Small/narrow one-FE cases also retain gaps.

All 39 final candidate/previous pairs have byte-identical full b/V exports,
exact samples/statuses/DoF/fit/certificate tables and solver diagnostics
except timings. All 25 final fixest comparisons pass. Weak, bottleneck,
cycle, disconnected, unbalanced and dense holdouts retain their performance
in these bounded local checks. Modeled memory budgets agree; native peak
allocation is unchanged except an unexplained 819,240-byte decrease in one
narrow weak holdout, which is not claimed as a memory improvement.

[Validation](benchmarks/reports/development/fixest-input-local-2026-09-12/validation.json)
records 35 core preparation tests, 40 plugin action tests, eight licensed
Stata fixtures, universal binary audit and actual arm64 ABI/self-test. The
new fixture checks all 27 Stata missing codes in five storage types, weights,
clusters/strings, exclusions, singleton deletion, row/sort-state preservation
on direct failure exits, and independent wide IID dummy regressions at
1/4/16 threads. Existing TS, transaction, saved-output, memory-mode,
weighted/robust oracle and PPML smoke fixtures also pass.
[Source identities and replayable patches](benchmarks/reports/development/fixest-input-local-2026-09-12/identities.json)
bind the retained plugin SHA-256
`93412e7c7654deac201f5f8c1bdc5af62aece7ebf4347180df0acebe3d227470`.

An 8,192-row loading chunk gave no consistent benefit and is rejected;
65,536 rows remain. Current sixteen-thread one-FE component medians are
0.301 seconds input, 0.207 native and 0.073 outer Stata work. Further
one-FE gains need fewer native data/layout passes or a demonstrably useful
loading overlap, not just more workers for the now-small completeness check.
A separate three-seed sparse-missing probe improved from 2.846 to 1.174
seconds after removing the input traversal cliff, but still exposes costly
row-wise native classification/copying when a small fraction of rows drops.
That report identified owned column compaction as the next experiment;
the implementation and new evidence are recorded above.
No SCC, Linux/Windows science, qualification, commit, push or release action
was taken. Historical receipts remain unchanged.

## Wide-OLS scheduling: preceding implementation

The local follow-up retains three OLS scheduling changes: disjoint Schur
recurrence updates above seven workers, parallel three-column probe setup,
and fixed group partials for wide one-FE/Schur certificates. The shared
PPML recurrence crossover, existing weak-graph solver routes, all acceptance
maxima, tolerances, iteration/fallback rules and memory budgets are unchanged.
The diagnostic two-norm changes summation order; it does not control
acceptance on these routes. No new workspace is allocated.

The [scheduling report](benchmarks/reports/development/fixest-scheduling-local-2026-09-12/README.md)
records 140 completed local measured calls, including 54 rotated primary
comparisons at three seeds. At 983,040 rows and 50 controls, sixteen-thread
previous/current/fixest medians are 0.951/0.881/0.838 seconds on degree three
and 0.875/0.835/0.704 on degree six: 7.4% and 4.6% improvements, leaving gaps
of 5.1% and 18.6%. At four threads current fereg remains 32% and 24% faster
than fixest on these designs. One-FE medians are 0.947/0.687 seconds at 4/16
threads; the small four-thread slowdown did not recur consistently in three
additional paired checks. All weak/bottleneck/cycle/disconnected/dense
holdouts pass and retain their performance within the bounded local evidence.
The separate eight-thread crossover checks improve and beat fixest.

All 35 final candidate/prior pairs have byte-identical full b/V exports,
samples, statuses, DoF, fit/certificate tables, acceptance maxima, routing,
iterations and fallback. The largest diagnostic norm relative change is
7.3e-14. All 24 final fixest comparisons pass. Primary peak allocated bytes
are exactly unchanged; one unchanged narrow weak holdout has an unexplained
3.8 MB lower reported peak, which is not claimed as a memory improvement.
[Validation](benchmarks/reports/development/fixest-scheduling-local-2026-09-12/validation.json)
records 34 focused Rust FE tests, four licensed Stata fixtures (including the
weighted/robust explicit-dummy oracle at 1/4/16 threads), universal binary
audit and actual arm64 ABI/self-test. The retained plugin SHA-256 is
`20289553c62d5280ece81a07dc39abeecaa573430c34a280cfb3c7cc1b2df763`.
[Source identities and replayable patches](benchmarks/reports/development/fixest-scheduling-local-2026-09-12/identities.json)
bind the uncommitted source; this is development evidence, not qualification.

Independent one/four-variable Schur jobs and RHS-parallel scalar reductions
were slower and are removed. Retain the batched graph operator unless a
better layout wins a controlled comparison. The remaining roughly 0.45–0.46
seconds of input/outer Stata work matters especially for one FE; further
native data/normalization passes and staging/input work are the next local
targets. No SCC campaign was submitted or resumed, and no commit, push or
release action was taken. Historical receipts remain unchanged.

## Numeric passes: preceding implementation

The local follow-up retains tiled numeric RHS construction and independent
raw-residual updates, parallel initialization of already reserved storage,
and parallel wide-source finite checks within an installed worker pool.
It changes four OLS source files; the preceding Schur route, C bridge and
Stata transaction are unchanged. General validation before pool installation
stays serial. No tolerances, sample/weight rules, rank, DoF or covariance
formulas change, and no additional observation-scale allocation is introduced.

The [numeric-pass report](benchmarks/reports/development/fixest-passes-local-2026-09-12/README.md)
records 139 measured calls, including 54 rotated primary comparisons at three
seeds and 44 final holdout/profile/repeat calls. Relative to the retained
Schur baseline, complete-command medians fall another 5–10%. At 983,040 rows
and 50 controls, current/fixest medians are 0.951/0.798, 1.326/1.927 and
1.222/1.578 seconds for one FE and degree-three/six graphs at four threads.
At sixteen threads they are 0.704/0.470, 0.960/0.835 and 0.878/0.692.
The tested wide weak-graph holdouts retain their performance. A 6 ms narrow
holdout difference did not recur across three additional paired seeds.

All 35 final current/prior pairs have identical exported full b/V files,
fit/certificate tables and solver diagnostics except timings. All 22 final
candidate/fixest comparisons pass. The final source passes 7 RHS tests,
60 prepared-fit tests and four licensed Stata fixtures, including the
50-control weighted/robust explicit-dummy oracle, saved outputs, memory
modes and PPML smoke. The universal macOS build passes binary audit and the
actual arm64 ABI/self-test. This is development evidence, not qualification.
[Source identities, patches and validation](benchmarks/reports/development/fixest-passes-local-2026-09-12/identities.json)
bind the retained plugin SHA-256
`26aca86c83c1cc7426a8de6601b1d8ee632acc43e3e5aacc9f0c3ea3a3ed0a2b`.

The report retains a 1.337-second one-FE outlier, largely located in unchanged
input loading; it did not recur in two paired checks, but its cause is not
established. Two external RSS samples miss the native peak and are flagged;
no new RSS ranking is claimed. Native peak allocated bytes and modeled fast
budgets are exactly unchanged across the 18 primary pairs.

The main remaining cost is about 0.45 seconds of input/outer Stata work at
sixteen threads. Callback hoisting and C-side tiled copying did not establish
a useful improvement and were rejected. Next local investigations are smaller
wide-model scheduling, remaining native passes, and reuse of private Stata
staging storage without weakening sampling or transaction guarantees. No
SCC campaign was submitted or resumed, and no commit, push or release action
was taken. Historical reports remain unchanged.

## Guarded OLS Schur route: preceding implementation

Johannes approved ambitious local work to approach fixest while preserving
weak-graph strengths. [ADR 0035](docs/adr/0035-guarded-wide-ols-schur.md) admits a
guarded Schur route for fast, unit-weight, two-FE OLS at 16–64 actual RHSs and
at least 16,384 observations. An eight-step three-column probe selects the
full-width solve; a 64-step cap and ordinary certified CG continuation share
the iteration budget. Both workspaces must fit the joint memory budget.
Other route classes retain the previous planner. The one-FE initial projection
now reaches its exact direct path, and fresh certified norms move into OLS.
Parallel group preparation/reconstruction and deterministic OLS norm reductions
remove repeated work without weakening scientific checks.

The [September 12 report](benchmarks/reports/development/fixest-gap-local-2026-09-12/README.md)
preserves 148 completed measured invocations, including 54 rotated final
primary calls at three seeds, 25 final holdout/narrow calls and six final
profiles. At 983,040 rows and 50 controls, four-thread medians are 1.404/1.297
seconds for degree-three/six versus fixest's 1.916/1.548 and baseline fereg's
7.630/6.962. Sixteen-thread medians are 1.009/0.936 versus fixest's 0.835/0.686.
One-FE time falls to 1.034/0.769 at 4/16 threads; fixest remains faster at
0.795/0.447. The weak and dense holdouts improve; the difficult cycle rejects
Schur and incurs about 3 ms of probe work. Narrow controls retain exact full
b/V parity. These are local development measurements, not universal graph
parity or release evidence.

Independent full dummy-model solves adjudicate native fixest discrepancies
on bottleneck and disconnected graphs. The final candidate matches the
independent coefficients/RSS/DoF. A native fixest stopping discrepancy and a
separate connected-component DoF convention difference remain explicit
failed native-parity comparisons; neither is turned into an equal-accuracy
speed claim. The earlier [September 11 report](benchmarks/reports/development/fixest-gap-local-2026-09-11/README.md)
remains unchanged, including the prototype weakness explained by the corrected
recurrence/certificate scaling.

[Validation](benchmarks/reports/development/fixest-gap-local-2026-09-12/validation.json)
records 33 borrowed-FE and 58 prepared-OLS tests, seven licensed Stata fixtures,
and the normal universal macOS build with binary audit and actual arm64
ABI/self-test. That report's measured plugin SHA-256 is
`ffa5537a0da609888de55ad122973140f86f48700a3368fb2f3098a785d072ac`.
[Source manifests and replayable patches](benchmarks/reports/development/fixest-gap-local-2026-09-12/identities.json)
bind the uncommitted source snapshot to base `d44e36bfa5ad7`; the base SHA
alone does not identify these changes. The [local harness](benchmarks/local_fixest_gap/README.md)
contains the follow-up reproduction entry points. No SCC job was submitted,
resumed or changed for this investigation; the stopped systematic campaign
stays stopped. No commit, push or release action was taken.

The numeric-pass follow-up above addresses the next RHS and raw-residual
work. Coefficient-space Irons–Tuck and batch missing detection were slower
and were not retained in this preceding investigation.

## Systematic benchmark campaign: stopped and reported

Johannes stopped this campaign on September 11. Array **7522398** was cancelled
and its monitor deleted. The [final cancellation receipt](benchmarks/reports/development/systematic-production-2026-09-11/cancellation-confirmed.json)
confirms the array is absent from the scheduler: tasks 1--78 completed, task 79
was interrupted, and tasks 80--237 never started. No continuation or checkpoint
resubmission is authorized.

The [discussion report](benchmarks/reports/systematic-stopped-2026-09-11/discussion.pdf)
and [figure atlas](benchmarks/reports/systematic-stopped-2026-09-11/figure-atlas.pdf)
analyze the collected partial evidence. The atlas retains individual vector
panels assembled with LaTeX `subcaption`. The [coverage receipt](benchmarks/reports/systematic-stopped-2026-09-11/stopped-coverage.json)
records 826 accepted registered comparison groups, plus two off-grid pilot
groups; 100 OLS configurations have six valid warm repetitions. PPML has five
accepted pilot groups and no complete repetition grid. Finished calls from
cancelled task 79 remain in the raw observations but are excluded from strict
aggregates. The [evidence archive](benchmarks/reports/systematic-stopped-2026-09-11/evidence.zip)
contains numerical receipts, provenance, accounting and analysis CSVs. Collected
raw logs are retained separately in the ignored production receipt `work/`
directory and on SCC. The [delivery receipt](benchmarks/reports/development/systematic-production-2026-09-11/report-delivery.json)
binds the eight-page discussion, 45-page atlas and checked evidence archive;
all pages were visually reviewed and all 132 panel PDFs remain vector figures.

Fereg's clearest advantage is difficult sparse FE versus reghdfe. Its main
weaknesses are the gap to fixest on well-connected designs, limited gains beyond
8--14 threads in several designs, and high RSS on sparse three-FE and the largest
degree-three case. In the latter case the automatic CMG-to-LSMR/Schwarz switch
coincides with setup increasing from 0.21 to 12.01 seconds; this is an observed
profiling lead, not a causal comparison of forced solvers. Tiny coefficient
discrepancies on the largest degree-two design remain unresolved and excluded.
Production consumed 96.3 node-hours (2,695 reserved core-hours); setup, smoke and
pilot work are additional. The grid is incomplete and ordered by model/design,
so it supports no balanced overall ranking. Estimator code and frozen scientific
thresholds are unchanged.

The requested OLS/PPML comparison harness is implemented under
[`benchmarks/systematic/`](benchmarks/systematic/README.md), with 304 unique
configurations, six warm repetitions and one cold diagnostic. It compares
the requested Stata estimators and pinned R/Python fixest versions on eight
designs, up to 3,932,160 observations. Its figures are individual vector PDF
panels assembled with LaTeX `subcaption`, including time, RSS and paired ratios.
The candidate remains clean source revision `d44e36b`; estimator code is unchanged.

The [development checkpoint](benchmarks/reports/development/systematic-2026-09-10/checkpoint.json)
and adjacent compact application/scientific/accounting receipts record the SCC
work in run `20260910-systematic-d44e36b`. Setup job 7520628 passed on a 28-core
node. Probe 7520678 passed its application checks. Smoke job 7520690 completed
all eight OLS designs with passing independent coefficient/SE checks; PPML
passed those checks for one FE and dense two FE, then its independent oracle
failed to converge on other designs. Focused retest 7521074 did not resolve the
oracle failure. The unrelated R metadata-rounding correction passed its OLS
retest. Scientific differences in native comparator output remain recorded.

Johannes authorized further investigation on September 11 before production.
Diagnostic job 7521509 completed successfully and traced three
failing cases with and without column scaling;
the [submission receipt](benchmarks/reports/development/systematic-oracle-investigation-2026-09-11-submission.json)
binds its inputs, controller and 28-core/30-minute resource request.
the [diagnosis and traces](benchmarks/reports/development/systematic-oracle-2026-09-11/diagnosis.json)
show an inner LSMR normwise accuracy floor from unequal column norms. Column
equilibration reaches the unchanged outer score threshold in six/seven iterations
for all three cases. The independent oracle now applies that correction; estimator
settings, inputs and acceptance tolerances are unchanged. The diagnostic heartbeat
was deleted. Array 7522234 (tasks 1--8) validated the final harness on both models,
all eight designs, two seeds and four/28 requested cores, with four groups packed
per task and a one-hour limit. All 32 candidate fits passed independent coefficient
and SE checks and pinned reghdfe/ppmlhdfe comparisons; all eight tasks have accepted
accounting. The [final smoke acceptance](benchmarks/reports/development/systematic-final-smoke-2026-09-11/acceptance.json)
preserves native comparator discrepancies and three gglm initialization errors.
These errors do not establish failure of the larger target fits. Its isolated
controller is `controller-final` and its output is `validation-final`.

The production controller adds same-node/task and partial-group recovery guards;
all 32 comparisons replay with unchanged outcomes, and 14 local evidence/figure
tests pass. Numerical workers, data generation and oracle are unchanged from the
accepted smoke. The runtime is frozen with fingerprint `04b5e8cf00ae6947…` under
`controller-production`. [Pilot array 7522274](benchmarks/reports/development/systematic-pilot-2026-09-11/submission.json)
was accepted with tasks 1--5, two groups per task, 28 E5-2680v4 cores,
8 GiB/core and a 12-hour limit. All ten groups and 55 arm receipts are complete,
with accepted scheduler accounting and application completion markers. The
[pilot acceptance](benchmarks/reports/development/systematic-pilot-2026-09-11/acceptance.json)
records ten successful candidate fits, nine passing pinned-reference comparisons
and one ppmlhdfe command timeout. Native pyfixest iteration failures, gglm
initialization failures/timeouts and accuracy discrepancies remain recorded.
Peak estimator RSS was 6.87 GiB; scheduler maxvmem was 13.004 GiB. The longest
task took 5,054 seconds. The pilot heartbeat was deleted after terminal accounting.

[Production array 7522398](benchmarks/reports/development/systematic-production-2026-09-11/submission-1.json)
was submitted with tasks 1--237, at most 12 concurrent tasks, 28 E5-2680v4 cores,
3 GiB/core and a 12-hour cap. It packs 2,120 remaining comparison groups; eight
registered pilot groups are reused under the same frozen runtime identity.
Measured packing targets eight hours and checkpoints unstarted groups before
the walltime cap. Native failures remain terminal benchmark outcomes and do not
trigger estimator-option changes. That continuation plan was superseded by the
user's stop instruction. Only 28-core SCC nodes were used and task walltime was
capped at 12 hours. Local checks exercise evidence acceptance and figure
assembly, never estimator benchmarks.

The [first 78 terminal tasks](benchmarks/reports/development/systematic-production-2026-09-11/first-78-validation.json)
have accepted scheduler accounting and 818 validated complete comparison groups.
Task 51 checkpointed after 20,626 seconds: ten groups completed and six never
started. Its [preserved checkpoint](benchmarks/reports/development/systematic-production-2026-09-11/checkpoint-51.json)
and [prepared two-task plan](benchmarks/reports/development/systematic-production-2026-09-11/prepared-checkpoint-51-plan.json)
are preserved as historical evidence; the prepared plan was never submitted and
will not be resumed. The frozen controller still matches its inventory. Final
accounting contains 79 started tasks, with nonzero cancellation status on task
79. Never-started tasks have no accounting record.

## OLS performance and LSMR correctness

This checkpoint collects the within-norm and wide raw-moment scan optimizations,
guarded wide finite solve, low-memory LSMR preparation reuse, certified mixed
LSMR/CG reporting, and the published parallel-repeatability repair below.
Johannes authorized committing the accumulated work on main and pushing it.
The production and controller file hashes match the tested integration
snapshot; the immutable receipts retain their original source identities.
Prior sections describe the development state when each receipt was recorded.

The approved patch is published in the within fork as
[`1ac3522`](https://github.com/johannes-schmieder/within/commit/1ac3522a251c2622dbeab451415924f9ebe117b9),
a fast-forward of its existing `fix/weighted-categorical-assembly` branch.
Both fereg runtime dependencies, Cargo.lock and the reference lock now use
that exact commit under [ADR 0034](docs/adr/0034-lsmr-parallel-reduction-order.md).
The [published-pin integration receipt](benchmarks/reports/development/ols-lsmr-repeatability-pin-local-2026-09-10.json)
binds the actual Cargo checkout, source snapshot and rebuilt plugin.
Earlier work and evidence are preserved in this checkpoint. The installed
plugin has not been replaced.

The upstream implementation changes only two indexed LSMR norm/dot sums:
one existing 4096-element chunk per reduction leaf fixes their arithmetic
tree independently of worker stealing. Thresholds, per-chunk arithmetic,
public APIs, allocations, routing, iteration budgets and independent
certificates are unchanged. No local dependency override or copied solver
internals remain in fereg. The reviewed patch and prepublication receipt are
preserved under `benchmarks/patches/` and `benchmarks/reports/development/`.

The published-pin build passes all 11 selected captures with exact science
against the reviewed patch, unchanged exact self-repetition and complete
pinned reghdfe checks. This includes the four original N100,000 seven-thread
failures: balanced two FE, balanced three FE and mixed three FE in fast
memory, plus mixed three FE in low memory. The serial low-memory case and
six K29/pool-size/weight/one-FE controls also pass. Weighted automatic and
explicit-Schwarz saved-output fixtures and transaction rollback pass.

Focused validation of the published pin passes: 40 upstream LSMR tests,
57 prepared OLS tests, 31 shared PPML tests, 11 independent prepared-fit oracle
tests, 31 allocation-contract tests and the exact dependency-pin test.
Both macOS slices build and pass binary/native ABI audits; clippy, formatting
and diff checks pass. Allocation inventory v17 changes only the reviewed
Cargo.lock and core dependency wiring; all source allocation entries and
hard boundaries match v16. No unrelated dependency versions changed.

The prepublication receipt records the intentional last-bit changes against
the old dependency and the source-extracted kernel benchmark. Seven-thread
median paired ratios were 0.986 for the norm and 0.992 for the dot; serial
ratios were 1.011 and 1.013. Those component measurements were not rerun for
pin integration and are not command-speed evidence. Failed baseline captures
remain failed evidence. The frozen F0 exact-extraction policy is unchanged.
No RSS, F0, SCC, Windows or release qualification was run.

This integration is complete. The separate explicit-diagonal LSMR certificate
failure and remaining performance milestones stay open.

## Prior certified LSMR/CG report aggregation

The next tracker #26 correctness issue is fixed under
[ADR 0033](docs/adr/0033-certified-lsmr-cg-batch-reporting.md), with the
[source/artifact-bound receipt](benchmarks/reports/development/ols-mixed-report-local-2026-09-10.json).
The baseline is the September 9 retained-LSMR candidate below. Only
`ols/prepared_fit.rs` changes in production relative to that baseline; all
work remains uncommitted on main.

Low-memory automatic fits can combine a certified LSMR batch and a certified
CG continuation when their requested/effective LSMR preconditioners agree
and the CG report carries its fallback flag. The aggregate reports requested
`auto`, used `cg`, and fallback true in either batch order. Maximum iterations
and certificate norms, summed timing/retry fields and ordered warnings retain
their prior conventions. Failed certificates, incompatible requests or
preconditioners, unflagged CG and other mixed engines remain rejected.
Solver choice, numerical work, tolerances, budgets, memory lifetimes and the
public result wire are unchanged.

The original weighted automatic fixture now passes against Stata explicit
dummies, including covariance, fitted values, residuals, saved-FE normalization
and reconstruction at pool sizes 1/5/10. Its data, commands and numerical
assertions are unchanged; only the expected aggregate solver metadata and
terminal name were updated. The same baseline still fails with `r(459)`.
The N100,000 three-FE low-memory serial reproduction now passes complete pinned
reghdfe comparison and exact self-repetition; maximum coefficient difference
from the reference is below 2.3e-15. The seven-thread reproduction gets past
the report error but fails exact repetition on coefficients at about 2.5e-17.
That capture remains failed evidence, separate from the fixed report error.

All six previously successful controls retain exact scientific equality,
including low/fast memory, pool sizes, one/seven threads and probability
weights. Focused checks pass: 57 prepared OLS tests, 31 shared PPML tests,
11 independent prepared-fit oracle tests, 31 allocation-contract tests,
clippy, formatting and licensed transaction rollback. The new report test
fails before the fix and passes after it for all six batch orders; rejection
tests preserve invalid-input atomicity. Both macOS plugin slices build with
Rust 1.85.1 and pass binary/native ABI audits. Inventory v16 changes only the
reviewed production tokens; allocation sites and hard boundaries match v15.

This is a correctness fix with no performance claim. Peak RSS, F0, SCC,
Windows and release qualification were not run; nothing was pushed or installed.
The next correctness work is parallel exact-repeat behavior and the separate
explicit-diagonal LSMR certificate failures. Broader M1a setup lifetimes and
other performance milestones remain open.

## Prior low-memory LSMR preparation reuse

The bounded categorical projection portion of tracker #26 M1a is implemented
under [ADR 0032](docs/adr/0032-low-ols-lsmr-preparation-reuse.md), with the
[source/artifact-bound receipt](benchmarks/reports/development/ols-lsmr-reuse-local-2026-09-09.json).
The baseline is the prior wide raw-moment candidate below. Three production
files change relative to that baseline: the borrowed FE adapter, its private
module export and prepared OLS integration. All work remains uncommitted on main.

Eligible low-memory categorical fits lazily prepare one LSMR operator and
preconditioner for the response and all regressor projection batches. The
sample, finalized FE design, weights and solver configuration remain fixed.
Weighted preparation uses one fallibly reserved owned weight copy. Every batch
retains its original RHS scale, iteration budget and independent certificate.
The operator is released before QR/VCE/output recovery and before any CG
continuation; after a continuation, subsequent batches use the existing
one-shot route. Fast/generic/sloped/output paths and report-merging policy are
unchanged. This does not complete all of M1a.

Five alternating untraced pairs per target, N=30,000, three categorical FEs,
29 controls, one native thread, low memory:

| Pool size | Baseline median seconds | Candidate median seconds | Median paired reduction |
|---|---:|---:|---:|
| 10 | 0.459 | 0.445 | 3.6% |
| 2 | 0.519 | 0.460 | 11.5% |

Reductions use paired ratios, not ratios of displayed medians. Every pair
matches scientific fields exactly and passes the complete pinned reghdfe
comparison. Five-pair fast controls have median candidate/baseline ratios
1.043 for the same three-FE design and 1.025 for one FE; individual timings are
noisy. Separate traces show projection setup falling from 18.0 to 6.25 ms at
pool 10 and from 63.5 to 6.44 ms at pool 2. Successful operator constructions
fall from four/sixteen to one per projection; the separate final-residual
LSMR call remains one-shot. Independent operator-application counts and RSS
were not measured.

All nine diagnostic cases pass exact baseline/candidate and self-repetition
checks plus complete pinned references, including analytic/probability
weights, seven threads, K10 and pool-size controls. Focused checks pass: 30
borrowed FE tests, 55 prepared OLS tests, 31 shared PPML tests, 22 independent
Rust oracle tests, 31 allocation-contract tests, nine harness tests, clippy
and formatting. The explicit Schwarz-LSMR licensed fixture checks weighted
coefficients, covariance, residuals, saved FE normalization and reconstruction
against Stata explicit dummies at pool sizes 1/5/10; transaction rollback also
passes. Both macOS plugin slices build with Rust 1.85.1 and pass binary/native
ABI audits. Inventory v15 records the reviewed allocations and preserves the
hard one-shot/transaction boundaries.

The five older N100,000 multi-FE failures reproduce on both the source-bound
baseline and final candidate. Three seven-thread fast cases fail exact
repetition; two low-memory cases fail with inconsistent solver reports. A
fresh trace localizes the low-mode failure: the response certifies with LSMR,
a regressor batch fails the tighter LSMR certificate and continues to certified
CG, and the existing report merger rejects the mixed engines. No tolerance or
report-policy change is retained. The initial constant-tail hypothesis did
not explain that failure, and its speculative report-merging edit was removed
before the final rebuild and measurements.

The weighted output fixture's automatic and explicit-diagonal variants also
fail on both baseline and candidate; the tighter explicit-diagonal attempt
fails too. These are preserved as failed evidence, separate from the passing
Schwarz fixture. Report aggregation is addressed by the current work above; the parallel
exact-repeat and explicit-diagonal certificate failures remain open.
Remaining performance work includes fast/generic/output setup lifetimes,
redundant RHS transport and measured serial VCE work. No F0, SCC, Windows,
release qualification, push or installed-plugin replacement was performed.

## Prior wide raw-moment scans

Tracker #26's next selected optimization is implemented under
[ADR 0031](docs/adr/0031-wide-raw-moment-column-blocks.md), with the
[source/artifact-bound receipt](benchmarks/reports/development/ols-raw-moments-local-2026-09-09.json).
The baseline is the guarded wide-solve candidate below, including the earlier
within-norm optimization. Production changes only `ols/prepared_fit.rs` relative
to that baseline; the work remains uncommitted on main.

Wide raw-moment caches scan eight adjacent columns per block, sharing matrix
loads and weight reads. Each column retains the prior arithmetic and row order
for maxima, scaled norms and compensated means. Blocking requires enough
blocks for the worker team; raw matrices at most 64 columns retain their
existing fused algorithm. There is no new heap allocation. The five fixed-size
accumulator arrays use 320 bytes per active block; inventory v14 preserves
allocation sites and the hard estimation/transaction boundaries.

Five alternating pairs per target, N=100,000, one FE, fast memory:

| Controls | Native threads | Baseline median seconds | Candidate median seconds | Median paired reduction |
|---|---:|---:|---:|---:|
| 64 | 1 | 0.437 | 0.352 | 18.8% |
| 64 | 7 | 0.186 | 0.178 | 4.3% |
| 65 | 1 | 0.464 | 0.371 | 20.4% |
| 65 | 7 | 0.228 | 0.185 | 12.6% |

Reductions use the median of paired ratios, not the ratio of the displayed
medians. Every pair matches all scientific fields exactly and passes its
complete pinned reference comparison. Five-pair unchanged K50 controls have
median ratios 1.000 and 1.00694 at one/seven threads. Separate K65 traces put
raw moments at 0.157 to 0.0643 seconds with one thread and 0.0297 to 0.00967
seconds with seven threads. Their phase timings are not paired speed claims.

All 16 additional diagnostic scopes reproduce the source-bound baseline
exactly and pass complete pinned reghdfe comparisons, including low memory,
K128, no FE, analytic/frequency/probability weights, exact collinearity and
absorbed slopes. The independent K65 group-elimination/SVD oracle passes;
coefficient error is below 1.2e-15. Licensed fixtures pass wide weighted saved
outputs against explicit dummies, PPML against Stata Poisson, fast/low parity
and transaction rollback. Focused checks pass: 54 prepared-OLS tests, 335 PPML
tests, 11 independent prepared-fit oracle tests, 31 allocation-contract tests,
seven diagnostic-harness tests, clippy and formatting. Both macOS plugin
slices build with Rust 1.85.1 and pass the binary/native ABI audits.

This is focused local macOS development. Peak RSS and independent operator
counts were not measured; no F0 matrix, SCC, Windows or release qualification,
push or installed-plugin replacement was performed. Earlier multi-FE repeat
failures and the separately recorded repository-checker failures remain open.
Retained LSMR setup reuse against this improved baseline is addressed by the
current work above.

## Prior guarded wide OLS solve

Johannes approved extending the guarded finite solve above 64 candidates.
[ADR 0030](docs/adr/0030-wide-guarded-ols-finite-solve.md) and the
[source/artifact-bound development receipt](benchmarks/reports/development/ols-wide-cholesky-local-2026-09-09.json)
record the implemented change. The baseline is the prior norm optimization
below; its plugin is unchanged from the earlier width receipt.

Wide fast fits now use disjoint Gram-row partitions and attempt the existing
guarded Cholesky solve. Weak/rank-deficient pivots and failed inverse
certificates retain QR; fresh score, FE, bread and reconstruction checks keep
their existing bounds. Wide moment construction needs no partial bank. Fast
preflight charges an additional `32*K^2 + 128*K` bytes for the live Cholesky
workspace, independent of thread count. At-most-64 arithmetic and low-memory
QR remain unchanged.

Five alternating warm pairs per target, N=100,000, one FE, K=65, fast memory:

| Native threads | Baseline median seconds | Candidate median seconds | Median paired reduction |
|---|---:|---:|---:|
| 1 | 6.965 | 0.429 | 93.8% |
| 7 | 2.447 | 0.207 | 91.6% |

Reductions use paired ratios. Five-pair K64 controls show no material
regression and exact science. A separate one-thread trace puts the finite
solve at 6.708 seconds before and 0.0575 seconds after. FE/pilot iterations,
solver choices and fallback reporting are unchanged in every accepted
comparison; independent operator-call counts were not collected.

All 16 diagnostic scopes have complete pinned reghdfe comparisons. They cover
K64/K65 at N100,000 plus K128, no FE, low memory, analytic/frequency/probability
weights, exact collinearity and absorbed slopes at N10,000. K64 and low-mode
science remain exact. K65 paired coefficient differences are below 2.8e-15;
all changed numerical results pass the registered OLS envelopes and unchanged
absolute certificates. Six independent group-elimination/SVD oracles pass.
The development comparator records covariance determinant and pilot
contraction as numerical diagnostics while keeping policy decisions exact;
the original F0 evaluator and its `extraction_exact` policy are unchanged.

Final-source checks: 52 prepared-OLS, 21 finite-kernel, 49 memory, 31 PPML,
40 plugin-estimation and 22 independent Rust oracle tests pass, as do 31
allocation-contract tests, seven base-harness tests and seven final wide
comparator tests. Wide unit cases include weighted full rank, near deficiency,
exact deficiency, thread determinism, overflow and memory preflight. Licensed
fixtures verify wide weighted saved outputs against explicit-dummy regression,
wide PPML against Stata Poisson and fast/low results, and transaction rollback.
Both macOS plugin slices build with Rust 1.85.1; binary and native ABI audits
pass. Inventory v13 records three reviewed production files with unchanged
allocation-site inventories and hard transaction/one-shot boundaries.

Limits: local Apple Silicon/Stata 19 development only; no F0 screen, SCC,
qualification, Windows execution, release, push or installed-plugin
replacement. Peak RSS was not measured. The K128/weighted timings are unpaired
diagnostics. The next measured bottleneck is the existing wide raw-moment
scan: about 0.153 seconds of a 0.319-second prepared fit at one thread/K65.
That bottleneck is addressed by the current work above. Earlier multi-FE
exact-repeat failures remain open.

The broad repository checker still fails on five obsolete STATUS headings
already absent at `c975bc5` and an ignored captured source snapshot counted as
a second status document. [The bounded check](benchmarks/reports/development/ols-wide-checker-preexisting-2026-09-09.json)
records these pre-existing findings; the checker and historical snapshot were
left unchanged. Focused scientific, build and allocation checks pass.

## Prior arithmetic-preserving OLS performance

Local development result from clean `main` at
`c975bc5c6e2aa5ff4be159225447d31f0fe20de6`:
[source/artifact-bound receipt](benchmarks/reports/development/ols-arithmetic-local-2026-09-08.json).
The retained source overlay changes only `ols/prepared_fit.rs` in production.

Wide parallel within-norm scans now process eight adjacent columns with
worker-local stack accumulators. Each column retains the scalar scaled-sum
arithmetic and row order. Blocking requires enough blocks for the existing
worker team; serial and narrow parallel paths retain their prior scans.
There is no new heap allocation, solver policy, tolerance, protocol or public
syntax change. The one-FE wide norm phase fell from about 10.2 ms to 3.7 ms
in separate traced runs.

Five alternating baseline/candidate warm pairs per target, N=100,000, K=50,
fast memory and seven native threads:

| Target | Baseline median seconds | Candidate median seconds | Median paired improvement |
|---|---:|---:|---:|
| No FE | 0.167 | 0.157 | 6.5% |
| One FE | 0.141 | 0.128 | 9.2% |

The improvement column uses the median of paired ratios, not the ratio of
the two displayed medians. All target pairs match science exactly and have
complete pinned reghdfe comparisons plus an independent group-elimination/SVD
oracle. Coefficient differences from that oracle are below 5e-16. Five-pair
narrow guards and three serial controls show no median regression above 5%.
Earlier width experiments, regressing narrow dispatch, and builds with invalid
source/artifact binding are preserved as excluded or rejected evidence.

The separate 38-scope diagnostic has 26 accepted baseline scopes and 25
accepted candidate scopes; all 28 complete baseline/candidate captures match
exactly. Several multi-FE scopes fail exact self-repetition, including an
additional candidate scope with a prior baseline trace failure. Four original
reference attempts lack complete accepted records. These remain unavailable
evidence. A supplemental wide one-FE guard matches saved FE and residual
outputs exactly and has a complete pinned comparator; it does not replace the
original unavailable weak-two-FE saved-output comparison.

Final-source checks pass: 51 prepared-OLS tests, 22 independent oracle tests,
40 plugin estimation tests, 31 shared PPML tests, 31 allocation-contract tests,
six diagnostic-harness tests, formatting and clippy. Five licensed Stata
fixtures cover transaction rollback, fast/low memory, constrained preflight,
PPML auto/explicit and saved-output behavior. Both universal-plugin slices
were built in an isolated target directory; binary and actual ABI checks pass.
Allocation-source inventory v12 changes only the reviewed production tokens
and error literal in `prepared_fit.rs`; allocation sites and hard F0 seams are
unchanged. No scientific baseline/candidate differences were accepted.

Supplemental K=64/65 checks are recorded in
[the width receipt](benchmarks/reports/development/ols-width-local-2026-09-08.json).
The current production files and both plugin hashes match the original
source-bound artifacts. All six one-FE scope pairs (one/seven threads, fast
K64/K65 and low K65) match science exactly and pass the complete pinned
reference comparison. Three independent SVD oracles pass, with coefficient
differences below 2.5e-15; seven harness tests pass. The diagnostic runner now
accepts a separate manifest and generates its requested control width without
altering the original manifest or receipt.

The earlier width receipt established the old K64/K65 discontinuity and exact
science for the norm optimization. Its timings are sequential diagnostic
captures. The subsequent approved wide-solve implementation and new evidence
are recorded above; neither historical receipt was rewritten.

## PPML auto admitted for development

ADR 0029 now admits the implemented automatic policy:

- Exactly two or three categorical intercept FE dimensions select LSMR when
  fast execution and the complete ordinary/checkpoint workspace fit the
  generation-zero memory plan.
- Other designs, absorbed slopes, low-memory execution, allocation audit, or
  modeled specialized-workspace veto select MAP. Auto never selects CMG.
- A modeled veto occurs before solver construction and is not a fallback.
  Actual allocation/interruption/data/internal errors remain fatal.
- Requested, selected, concrete, effective, checkpoint, retry and fallback
  reporting is preserved. Auto uses the same certified engine as explicit LSMR.

Admission: benchmarks/reports/development/ppml-auto-development-admission-2026-09-08.json.
This is development acceptance, not formal qualification, a successful
reclassification of the failed calibration campaigns, or a claim that every
scope was rerun on the repaired source.

## Scientific adjudication

All21 hard actual-auto scopes on9 original datasets have independent
coefficient binding, model-space, objective, finite/per-FE score, rank/DoF,
covariance and default/tighter stability evidence.

- 18 scopes pass the unchanged numerical comparisons using complete pinned
 reference records. Earlier complete rows in failed campaigns are prospective
 scientific witnesses only, not accepted timings or campaign passes.
- Task17 has a fresh optimum-accuracy proof on the repaired Linux capture:
 coefficient radius4.882e-12 and covariance radius1.762e-12. All5 complete
 reference repetitions have17 coefficients and50 covariance cells proven
 outside their existing envelopes; other compared science passes.
- Tasks7/8 share the many-level N75000/K50 dataset with no complete reference
 row. Both pass independent optimum inclusion. The task7 coefficient and
 covariance bounds are6.345e-12 and1.793e-12. Reference error is not claimed.

The apparent four additional VCE discrepancies were truncated-record artifacts.
The new strict reference parser rejects incomplete matrices/records, invalid
stripes, unsuccessful fits and invalid objective pairs. It does not treat
missing evidence as a numerical mismatch. ADR0026 finite/domain/Poisson-identity
and no-worse objective rules apply without weakening coefficients
(rtol1e-7/atol1e-10) or VCE (rtol1e-6/atol1e-10).

All9 preserved SCC dataset hashes were reverified read-only; the reference
closure matches original setup. Original input/source and audit bindings are
in ppml-auto-adjudication-completion-2026-09-08/work/aggregate-bindings.json.
No projected reference state is used as an additional objective witness.

## Common checkpoint-cycle repair

The Linux K0 N75000 many-level two-FE tighter fit cycled: a certified warm
checkpoint could reconstruct the older canonical predictor, moving eta by
1.599e-10 against the requested1e-10 profile gate despite a passing score.

A fit-owned checkpoint target now tightens100x after a certified unchanged-basis
profile rejection, bounded below by1e-14 without loosening an already-tighter
target. The refined target also governs final-Hessian synchronization. The
next actual checkpoint counts the extra work as a retry. Existing RAII,
warm-state invalidation, two consecutive deviance-plus-score convergence,
canonical rank and exact final q_i*mu_i certificates remain mandatory.

The formerly300-second timeout now takes1.873seconds at both default and
tighter outer tolerance. Strict/final eta movements are8.42e-12/7.29e-16.
Default b/V/objective/rank/DoF are unchanged. Repaired difficult2FE and task17
default results are identical to baseline; sparse3FE b/V differences are
1.05e-16/3.69e-19, with unchanged objective/rank/DoF and passing certificates.
Different hosts prohibit matched performance claims for those guard timings.

## Source, jobs and artifacts

Baseline main commit:7c1fc09207864b41f851c0cb58c2dcbe80b1e8be.
Frozen dirty snapshot:da29adb7f6061a933dccbedbae923d96957361f3aa495f75780be8a05755362e.
The sole subsequent compiled-production overlay is
crates/fereg-core/src/ppml/prepared_irls.rs SHA
977522f1b878c2f96b5147f85fd927e739cb65b5111a9062f25d5f3ff50d3c93.
Documentation, adjudicator and fixture changes are separately identified.

- 7490248: actual-auto Linux smoke, task17, pass.
- 7490294/7490295: baseline actual-auto arrays;16 successful scope receipts.
- 7490361: independent direct-ring reaudits repair auditor failures4/7/8;
 original failures remain recorded.
- 7490499: trace-only diagnosis, no numerical policy change.
- 7490653: repaired K0 task3, successful fits and independent audit.
- 7490705: repaired guard scopes1/11/17; all scheduler/application/audit passes.
- 7490776.1–4: strict reference attempts all timeout1200seconds,
 failed0/exit1, application124, no fit sentinel. None accepted.

Every substantive job reserved28cores. No job is currently pending for this
goal and the completed campaign monitors were deleted.

Baseline Linux plugin:12f17d5873a0092e2ac6baee6b3ced2732504550c2983e5ada4a2d1a07f3450a.
Repaired Linux plugin:01af450c8fa6d841584661608fa45db41b0f9afb14cd7750eeaa9a3f51bc5124.
Repaired universal Mac plugin:47e578679f81ba9f0ffbb087c94864019427fa15c1f1ecac7ecc3bac5791caee.
The isolated Mac build used installed target libraries; only unavailable rustup
provisioning was omitted. Standard compilation, binary audit and actual ABI
loader tests passed. Formal exact-source qualification remains separate.

## Validation and handoff

- 745 repaired-core tests;18 focused strict-checkpoint tests;2 constrained-memory
 tests; formatting and core all-target Clippy passed.
- 71 focused auto Python tests,10 accuracy tests,8 optimum-adjudication tests,
 16 PPML help-contract tests,31 allocation-source-contract tests and10 build-
 closure tests passed. Existing independent oracle/memory evidence is retained.
- 12 licensed Mac auto-versus-explicit route pairs pass with exact b/V/objective/
 rank/DoF/sample/mu agreement. Low memory selects MAP; failed auto restores
 prior results. An additional3FE pair exactly matches saved effects and eta/d.
- Terminal Stata sentinel and explicit application returncode0 were checked.
- Git diff whitespace validation passes.

The full evidence chain is listed in the development admission receipt.
The admitted universal artifact is installed in dist and passes the same
fixture in a fresh Stata process, including its explicit terminal sentinel.
Activation receipt: ppml-auto-local-activation-2026-09-08.json.
Final whitespace-only cleanup of the independent coefficient-proof script
preserves its Python AST and nonwhitespace bytes exactly;10 accuracy tests were
rerun. The original executed source is preserved and historical proof hashes
are not rebound to the cleaned file.
Historical optimization evidence remains in its named immutable reports;
nothing is promoted across source revisions. The coherent development commit
includes this handover and the source-bound admission/activation records.

## Remaining performance work (not part of this admission)

- Sparse/mixed3FE ordinary joint LSMR conditioning and operator work remain
 high-leverage; do not restore the rejected retained all-pairs Schwarz policy.
- CMG keeps its bounded nuisance envelope and fresh certificates. Guarded
 weak-edge grouping, graph batches and certified warm starts are retained;
 unconditional grouping and ineffective response-start experiments are not.
- Explicit MAP is still slow on poorly connected graphs. Auto now avoids it
 within its admitted two/three-effect envelope.
- Easy one-FE wide-K weighted-design passes remain a useful optimization target.
- No further upstream API expansion, auto-policy refit or new campaign is implied.

## Release boundary

Development acceptance does not close the beta's formal exact-source
scientific, memory, performance or cross-platform release gates. Windows and
the formal aggregate remain deferred. No push, tag, release or broader
qualification is authorized by this completed development admission.
