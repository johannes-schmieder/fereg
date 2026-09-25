# Compiled bounded PPML transport: local development checkpoint

Date: 2026-09-25. Decision: retain **controlplane3** under
[ADR 0201](../../../../docs/adr/0201-compiled-ppml-transport.md).
The broad multi-FE <=0.90-times-stock objective remains unfinished.
This checkpoint removes repeated control-plane interpretation while preserving
all scientific acceptance and transactional boundaries. It does not introduce a
new solver or claim that wrapper optimization closes the remaining gap.

## Source and execution identity

Baseline is **e3ef1757 / slopetopology1**, as captured before checkpoint
**19ab41ce**. Final candidate `controlplane3` contains **564 captured computational
inputs**, all verified against the production tree. Exact source maps and every
per-call fingerprint are in the archive. Native plugin SHA-256 is unchanged:
`1c598ce6ecbdea8e1a6e0ffcc9dfcd655c89165750a8d2b02d8189ec15b9ae1e`.
The new helper source SHA-256 is
`6a4dc62d7986374f6cbd2cd8132fbd2491ff43bc558e793f1de04b2170d3fd8c`.
Its three compiled objects and hashes are in the package source-binding receipt.

Pinned, unmodified stock fixest is 0.14.2; source archive SHA-256:
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`.
Comparable calls use `glm.tol=1e-10`, `fixef.tol=1e-11`, robust inference and fitted
means. Threads and requested work match. Default-reference discrepancies remain
separate historical evidence, not alternative headline clocks.

Final warm public fits run under Stata 19 on Apple Silicon. Each measured fit
follows a warm-up, with process startup and import outside the clock. Parsing,
estimation, validation, inference and requested output publication stay inside.
The separate cold experiment removes that estimator warm-up; it still excludes
process startup/import. These are public-command clocks, not end-to-end session
launch timings. Instrumented ado clones supply attribution only. No compilation,
profiling, audit, archive or competing fit accompanies headline calls. All sampled
process monitors and applications pass; sampling does not establish a perfectly
quiet operating system.

## Implementation and attribution

The old result validator interprets hundreds of predicates and repeatedly reads
bounded result fields through Stata expressions. The candidate copies sixteen
bounded matrices into local Mata variables, checks layouts before indexing, and
executes the same ordered predicates in compiled code. Scalar publication uses
the same e() names/values and absence rules in a second function. Exact wire,
sample, rank, DoF, covariance, solver, memory and separation evidence remains
checked. The original response-level output checks, stripes, held-estimate
transaction and protected commit remain. No observation design or estimation
loop moves out of Rust; there is still one substantive plugin call.

Paired warm instrumented clones (`ado`, two rotations) measure bounded validation
at **3/3 ms baseline versus 1/0 ms candidate** on disconnected low categorical,
and **3/4 versus 0/0 ms** on narrow categorical. Publication is about 2 ms in both
arms, with one 3-ms baseline and one 1-ms candidate. Values have one-ms timer
resolution and instrumentation overhead. Native code is identical; apparent
native clock changes are not a solver speedup. This supports the interpretation
savings without attributing every public millisecond to a particular instruction.

The source-compiling delivery (`controlplane1/2`) was useful experimentally but
is rejected for production. It added **183/187 ms** to the two cold medians.
The final correction ships three `.mo` assets built with actual Stata 18 and
loaded automatically. `scripts/build_ppml_mata.py` checks application completion,
compiler version and sentinel; a manifest binds source and object hashes.
Staging rejects stale source, altered objects or a newer compiler claim. A second
build reproduced every byte. There is no source-compilation fallback.
[Stata's object documentation](https://www.stata.com/manuals/m-3matamosave.pdf)
and [StataCorp's compatibility explanation](https://www.stata.com/statalist/archive/2009-02/msg00649.html)
explain the loading and oldest-compiler choice. Local objects execute under both
Stata 18 and 19; other platforms are unqualified.

## Final fresh comparison

Seed **202609434** was predeclared after source freeze and is consumed/frozen.
Five alternating B/C/stock rotations; milliseconds below. Do not pool these data
with seed433, seed432 or the pilot.

| Case; N/K/T; mode | Baseline / candidate / stock medians ms | C/stock | Paired W/L/T |
|---|---:|---:|---:|
|Disconnected categorical; 61440/6/4; low|112 / 109 / 97|1.124|5/0/0|
|Serial categorical; 38400/10/1; low|141 / 136 / 242|0.562|5/0/0|
|Narrow categorical; 24960/2/4; fast|41 / 38 / 30|1.267|5/0/0|
|Wide categorical; 23040/36/8; low|97 / 93 / 128|0.727|5/0/0|
|Two slope partitions; 30720/6/8; low|77 / 74 / 79|0.937|5/0/0|
|Two slope partitions; 30720/6/8; fast|74 / 71 / 79|0.899|5/0/0|
|Unbalanced three-FE slopes; 61440/10/8; low|195 / 194 / 233|0.833|2/2/1|
|Three slopes on two partitions; 38400/8/4; low|139 / 138 / 303|0.455|3/2/0|

**35 paired wins, four losses, one tie; all eight medians improve 1–5 ms.**
The first six cases win every pair. Five of eight candidate medians meet 0.90
stock, versus four baseline medians. The new fast-T8 crossing has only 0.1-ms
median headroom and fails the target in two of five individual stock pairs;
that family remains **provisional**. Ratios do not justify a universal win claim.

The remaining median savings needed are **21.7 ms** disconnected low categorical,
**11.0 ms** narrow categorical and **2.9 ms** low T8 slopes. The three-FE and
multiple-slope cases retain individual baseline losses despite faster medians.
Full distributions, in repetition order:

| Case index | Baseline ms | Candidate ms | Stock ms |
|---|---|---|---|
|0|115, 112, 112, 110, 112|113, 109, 109, 108, 108|97, 98, 100, 96, 97|
|1|140, 144, 140, 141, 141|136, 137, 136, 136, 133|242, 242, 250, 241, 241|
|2|40, 40, 41, 42, 41|39, 38, 40, 38, 38|30, 31, 30, 31, 29|
|3|97, 95, 98, 96, 98|92, 93, 93, 93, 93|128, 130, 126, 128, 128|
|4|78, 77, 76, 76, 87|73, 73, 74, 74, 74|79, 79, 79, 80, 80|
|5|76, 73, 75, 74, 73|70, 71, 72, 72, 71|79, 80, 81, 79, 77|
|6|200, 195, 194, 192, 199|194, 196, 194, 193, 192|228, 233, 234, 235, 232|
|7|140, 139, 135, 140, 138|142, 134, 138, 139, 136|304, 303, 314, 303, 301|

Every final public state passes independent score, model-space and objective
replay; direct candidate/stock coefficient, covariance and mean comparisons pass.
Scientific discrepancies would remain unranked rather than be removed from the
performance denominator.

## Earlier blocks, cold losses and excluded paths

All earlier blocks are preserved separately in `summary.json`, declarations,
progress and audit files. The six-cell `controlplane1` pilot, seed347 with three
rotations, has B/C/stock medians **109/106/100, 39/37/34, 84/85/126, 74/70/79,
74/72/81 and 181/179/234 ms**. Its wide case loses one ms. `controlplane2` fresh
seed433, five rotations in the final table's order, has **107/105/99, 141/139/246,
39/37/34, 96/94/129, 75/74/79, 71/70/78, 184/180/231 and 137/134/272 ms**.
It is not the final production delivery and does not replace seed434.

First-fit three-pair data, seed347, preserve every outlier:

| Delivery/case | Baseline ms | Candidate ms | Median B/C ms |
|---|---|---|---:|
|Source compilation; low categorical|125,125,125|308,307,308|125/308|
|Source compilation; narrow|54,55,55|241,242,242|55/242|
|Precompiled; low categorical|129,130,131|459,132,130|130/132|
|Precompiled; narrow|58,59,55|62,60,58|58/60|

The precompiled 459-ms call reports only 98.7 ms in the substantive native action;
most of its delay occurs elsewhere. Separate cold instrumented clones show
low-case public B/C vectors **408,133 / 367,141 ms**, with first-use pre-plugin
shadow/request/loading phase **278/229 ms**, then **4/5 ms**. Both arms exhibit
large first-use clone delays. This is evidence about those diagnostic calls,
not proof of the original outlier's cause. Cold validator/publication spans and
normal first-fit comparisons still show small candidate losses. No cold
non-regression claim is made, and the outlier is not discarded.

Final one-FE PPML controls (seed434, three pairs) improve **24→21 ms categorical**
and **80→77 ms slope**; every pair improves. The earlier seed433 block improves
22→19 and 77→74 ms. These are shared control-plane effects on excluded native
solver families, not new one-FE solver evidence.

OLS uses unchanged called source and the identical plugin. Its timing controls
remain visible:

| Block; small / three-FE / slopes | B/C medians ms |
|---|---|
|Initial controlplane2, three pairs|8/10; 61/58; 117/118|
|One predeclared confirmation, five pairs|11/11; 58/58; 109/108|
|Final controlplane3, three pairs|8/7; 60/61; 108/111|

Final vectors are small **8,8,9 / 7,7,8**, three-FE **60,61,60 / 61,61,61** and
slopes **108,110,108 / 111,112,109 ms**. All three final slope pairs lose.
The confirmation does not erase earlier or final losses. Source identity makes
this poor evidence of a causal OLS change, but it also does not establish the
cause or resolve ADR0199's earlier six-ms slope concern. No general
non-regression claim or further timing retry follows this bounded milestone.

## Science, failure and memory evidence

- **28 focused Python tests pass**, covering transaction predicates, package
  inventory and source/object/compiler-binding rejection. No full suite claim.
- Stata 18 passes runtime-validator/reload, weights/inference and scalar
  equivalence fixtures. Stata 19 additionally passes the old/new validator
  equivalence/reload fixture. **3,496** single-cell bounded mutations agree,
  including missing, extended missing, negative, fractional, tiny and huge values.
  The mutation set is a regression probe, not an exhaustive proof.
- Scalar fixtures compare every e(scalar) name/value for full, missing and signed
  bounded tables. Independent final scientific fixtures pass: iterative canonical,
  multipartition slopes, three-partition slopes, public scientific contract,
  weights/inference and separation/outputs. The unchanged plugin remains the
  native computation source.
- Current-wire transaction replay and the licensed PTY interruption fixture pass.
  An isolated package missing either validator or publisher object returns r3499
  while preserving prior e(b), e(V), e(sample), data signature and output absence.
  Restoring the publisher makes the same fit succeed. Automatic reload after
  `mata: mata clear` passes.
- Across all nine PPML blocks, **358 public states, 228 baseline-anchored pairs
  and 98 direct stock pairs pass**. All **130 native PPML pairs** have byte-identical
  coefficient, covariance and mean exports, with unchanged outer/inner/fallback
  counts. All **33 OLS pairs** pass and have exact coefficient/covariance exports.
  Counts include diagnostic/cold blocks; they are not independent timing replicates.

Three objects occupy **129,124 file bytes**. Their in-memory bytecode cost is not
measured. Local validator matrices have O(K^2) scalar storage and are released
at return; the covariance copy lives through validation rather than being
reloaded repeatedly. Numeric payloads calculated from captured tables are
3,976 bytes narrow, 4,456 low categorical and 16,216 wide, excluding Mata object
headers, runtime and temporary expressions. This is bounded control-plane memory,
not an N-by-K retained bank. Native tracked peak and scratch deltas are zero in
all 130 pairs; selected-memory telemetry spans -144 to +144 bytes. These are not
Mata RSS or constrained-memory measurements. No new Rust test/build, warmed
allocation audit, physical cap, static inventory qualification or cross-platform
campaign is claimed. Known inventory drift remains a failure.

Preserved development failures include the initial no-op ado autoload attempt,
incorrect `mata clear` fixture syntax, reload placed before a global-scratch
sentinel, a relative source path in the first Stata18 compilation, and an
unsupported `--ado-root` CLI invocation. Each corrected boundary has explicit
successful receipts. The old hidden transaction fixture has a pre-existing
minor2 assertion against actual wire3; a captured diagnostic changes only that
assertion and its sentinel. It is not silently rewritten. The broad release-test
invocation was interrupted when it began source archives; focused checks replace
it, not an all-suite success claim. Default Python3.9 lacked tomllib once; the
configured newer interpreter passed. No failure overlapped headline fits.

## Retention and next work

Retain a simpler shared validation implementation and warm gains. The accepted
algorithmic slope engine and current-weight science remain untouched. This is a
useful preparatory checkpoint, not completion of the redesign/performance goal.
Close this packaging experiment; do not chase its remaining cold milliseconds.

Use the existing nonoverlapping native lifecycle ledger for the next coherent
change. Narrow still needs 11 ms and low categorical 21.7 ms. Identify duplicate
original-column preparation, same-state canonical products and accepted-state
reconstruction across owners; transfer valid current-state products rather than
rebuild sessions. Changing weights still require fresh numeric geometry and
independent acceptance. A shared structural saving should cover the small T8
slope deficits naturally. If the measured duplication cannot meet the budget,
the declared next alternative is one exact-weight sparse FE factorization
experiment covering all actual initial/ordinary/strict frames, setup and checks;
no favorable-frame-only comparison or revived incidence/recurrence tuning.

Preserve OLS timing concerns, the unranked older three-categorical-FE discrepancy,
weighted constrained-fit/nonretaining-CG instability, static-inventory drift and
deferred Linux/thread/physical-cap confirmation. Frozen 77/16/111 definitions
remain unchanged. No push, release, SCC retry or stopped campaign occurred.

## Immutable evidence

`receipts-content-addressed.zip` contains **17,355 logical receipts** with original
and sanitized hashes. All were restored and hash/privacy verified. The bundle
hash and verification records accompany this report. The first privacy scan
matched its own regex and two synthetic fixture-path allowlists in the archived
scanner source. The corrected verifier exempts only those exact source literals;
the archive is unchanged and actual-home/licensed-header checks still pass.
Reconstruct with:

```bash
python3 restore_receipts.py /a/new/receipt-directory
```

Restored files live below `ppml-control-plane-20260925/`: `summary.json`, twelve
screen directories, declarations, controllers, audits, source-bound identities,
eight artifact source snapshots, compiler/fixture receipts, failed recipes and
all final public vectors. `ado-phases.txt` is explicitly preserved. Data, native
plugins and `.mo` binaries are omitted with hashes; package-owned final objects
are committed beside their reviewable source. Reference pins and generation,
measurement, cold-controller, audit and archive recipes are included. Licensed
logs are represented by invocation/status records and filtered sentinels, not
raw headers. Sanitized path substitutions are explicit in the manifest.
