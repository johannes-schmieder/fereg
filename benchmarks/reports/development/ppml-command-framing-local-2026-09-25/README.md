# Whole-command attribution and bounded request framing — 2026-09-25

This local development milestone retains a small shared OLS/PPML overhead
improvement. It does **not** close the multi-FE <=0.90-times-stock objective or
adopt a new numerical solver. [ADR 0182](../../../../docs/adr/0182-bounded-request-framing.md)
records the implementation and error-lifetime boundary.

## Source and comparator

The accepted parent is `6712d5de`; its 554 runtime inputs match `familyrows2`
before editing. Its native computational checkpoint remains `711802bf`.
Candidate `frameado3` changes only `stata/_fereg_estimate_ols.ado` and
`stata/_fereg_estimate_ppml.ado`. All 554 final input hashes match the worktree
before this report; the other 552 files are unchanged. The candidate borrows
the verified parent plugin rather than rebuilding identical Rust/C sources:
SHA-256 `4a326b8d3b210084620478dfd857ffdca672db5aacfc7acbd82fbc3194b98af8`.
Each arm has its own captured ado tree and identity. No native timing result is
transferred from the diagnostic binary.

The comparator is pinned unmodified fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with glm.tol=1e-10, fixef.tol=1e-11, matched threads, BLAS1, robust inference
and saved means. Timings cover the public command after an untimed warm fit
in a fresh process. Host data loading precedes the command clock; native
loading, estimation, checking and output remain inside it. Cold first-command
and other-platform performance are not measured here. Process isolation passes
for every recorded attempt. No build, profile, audit, archive or competing fit
accompanied headline measurements.

## Same-command attribution

Five existing development designs use seed202609347. `commandcost2` adds only
six disjoint Stata timers around the unchanged plugin. `commandnative1` adds
aggregate native timers, emitting after the call instead of printing each
nested event. Both run separately from headline rotations, twice per case.
The earlier `commandcost1` fits pass, but captured Stata output suppresses its
phase messages; that unsuccessful output mechanism is retained.

The narrow N21120/K2/T4 command takes 39 ms in both ado-only diagnostics:
metadata preparation 2 ms, shadows/request 3 ms, plugin 27–28 ms, bounded result
validation 3 ms, observation-output validation 1–2 ms, publication 1–2 ms.
Timer resolution and entry/exit work explain imperfect sums; these are not
subtractions between unrelated instrumented and stock runs.

The aggregate diagnostic medians below are milliseconds. Native/canonical/joint
spans are **inclusive and nested**; never add them as independent phase totals.

| Diagnostic family | Plugin | Canonical preparation | Ordinary joint solves | Interrupt round trips | Native publication |
|---|---:|---:|---:|---:|---:|
| Two FE categorical N21120/K2/T4 |26.987|6.354|4.481|0.293 / 33 calls|0.639|
| Disconnected categorical N42240/K10/T8 |76.789|24.551|14.236|0.465 / 33|1.259|
| Three FE slopes N61440/K10/T8 fast |208.897|73.129|67.899|0.452 / 38|1.772|
| Same, low |225.368|89.607|67.317|0.454 / 38|1.778|
| Two slope partitions N30720/K6/T8 |93.543|22.838|38.126|0.599 / 39|0.952|

Interrupt exchanges and native publication cannot explain the principal gap.
The Stata request phase contains a repeated interpreted gettoken/string-growth
loop over roughly 80 fields; replacing that work has a small public benefit.
Canonical construction and ordinary joint work remain much larger targets,
especially in slopes. This attribution does not identify every included pass as
avoidable or claim that certificates alone account for the remaining loss.

## Adopted change and corrected prototypes

The ado control plane uses Mata only to frame its bounded metadata list. An
explicitly owned input string backs the tokenizer. Semicolons delimit internal
key=value fields; spaces within values and UTF-8 byte lengths are preserved.
Vectorized length-prefix assembly produces the existing count/length wire format.
The 4,000-byte inline/local switch, 16 MiB native cap, decoder, one plugin call
and all scientific computations are unchanged. No observations or generated
design columns enter Mata.

`frameado1` passed numerical checks and showed overhead gains. The successor
explicitly owns the tokenizer input and releases the raw Stata metadata after
framing. A separate probe then demonstrated that tempname does not clean Mata
externals after an aborted ado call. It also showed that dropping a list with
an absent name does not provide complete partial-construction cleanup. The final
source defers Break over this bounded metadata section, captures any Mata error,
independently drops each temporary, then propagates the original error. Injected
errors at six construction boundaries and a successful path leave no external
objects. These are injected interpreter errors, not physical out-of-memory tests.
No allocation-sized failure or error is converted into a successful fit.

The original encoder is an independent byte oracle for spaces, punctuation,
embedded quotes, UTF-8 and empty input. Early diagnostic script syntax/output
failures are preserved rather than counted as successful validation. The final
extracted production block passes all those byte comparisons. No permanent Mata
function, public option, native solver bank or observation-sized storage is added.
Temporary metadata storage scales with request bytes/fields and is released before
native entry. This is not an RSS, physical-cap or complete allocation qualification;
existing memory-inventory drift remains open.

## Final fresh public PPML distributions

Seed202609415, frozen final `frameado3`, five alternating baseline/candidate/stock
rotations. All cells are scientifically rankable. Times are median [min,max] ms;
B and C belong to the same block. Different seeds/sizes in prior reports must
not be presented as this change's speedup.

| Case | N/K/T; memory | Baseline | Candidate | Stock | C/B | C/stock | Losses / ties |
|---|---|---:|---:|---:|---:|---:|---:|
|Two categorical FE|26880/2/4; fast|46 [44,46]|44 [43,46]|34 [33,36]|0.957|1.294|0/5; 3/5|
|Disconnected categorical|48000/12/8; fast|101 [101,104]|100 [99,103]|89 [88,93]|0.990|1.124|0/5; 1/5|
|Three FE slopes, unbalanced|72960/10/8; low|284 [282,287]|282 [281,285]|279 [278,281]|0.993|1.011|1/5; 0/5|
|Three FE slopes, unbalanced|72960/10/8; fast|262 [262,264]|260 [260,265]|281 [279,288]|0.992|0.925|2/5; 0/5|
|Two slope partitions|38400/6/8; fast|126 [126,131]|126 [125,128]|89 [86,90]|1.000|1.416|1/5; 1/5|
|Two categorical FE, serial|25920/3/1; fast|66 [64,68]|67 [62,68]|65 [64,65]|1.015|1.031|2/5; 1/5|
|Four categorical FE|32160/10/4; low|237 [231,240]|231 [230,237]|558 [558,561]|0.975|0.414|0/5; 2/5|

There are 21 wins, six losses and eight ties among the 35 final PPML pairs.
Five medians improve, one ties, and serial categorical loses 1 ms. The narrow
median improves 4.3%, with two improving pairs and three ties. The disconnected
median improves 1%, with four improving pairs and one tie. This supports a small
shared overhead change, not a uniform 10% speed advantage.

Earlier complete blocks remain separate evidence: development uses `frameado1`
and seed202609347, followed by five rotations at fresh seed202609412. The owned-
input `frameado2` uses five rotations at seed202609413. Its large low case loses
four of five pairs, 288→291 ms; its numerical checks pass. The final source's
correction addresses error cleanup, not a performance threshold selected from
that loss. All earlier distributions and paired losses are in the archived
`summary-final.json`, alongside the final block. No frozen seed is used to tune
routing or numerical requirements.

## OLS protection and validation

Five OLS controls use seed202609414 and three B/C rotations: small serial two FE,
threaded two/three FE, low-memory three FE and two absorbed-slope partitions.
Their medians are 55→55, 36→35, 54→52, 146→145 and 90→91 ms. The slope control
loses two of three pairs. A separately declared five-pair confirmation of the
same small/threaded/slope cases gives 55→52, 36→35 and 94→92 ms, with one, zero
and two losses respectively. All 30 OLS pairs have exact scientific CSV exports;
there are nine losses and three ties across the two OLS timing blocks. No claim
of zero timing regression follows. This is a focused control, not a rerun of the
frozen OLS qualification definitions.

- All 360 headline PPML states and 30 diagnostic PPML states pass independent
  original-design score, model-space and objective replay.
- All 240 baseline PPML comparisons and 120 direct candidate/stock comparisons
  pass. Scientific CSV exports and saved-mean bytes agree exactly between native
  baseline and candidates in all 120 headline PPML pairs.
- All 60 OLS applications and 30 B/C comparisons pass. No timed application,
  timeout, isolation failure or scientific discrepancy is omitted.
- The final source passes three licensed fixture executions: long request
  transport with independent weighted OLS and Poisson/sandwich oracles, canonical
  PPML products with generated/weighted cases, and three-FE OLS with explicit
  design checks. Nine fixture executions across all source versions pass; the
  final three are the acceptance evidence.
- The focused static screen has 25 passes and two failures. Both failures are
  reproduced individually on the unchanged parent: an obsolete PPML result-wire
  assertion and an old fixed-width string-loading source expectation. The first
  broad parent replay additionally lacks three documentation files in its partial
  artifact; those errors are retained and are not counted as numerical failures.
- No Rust numerical test suite is rerun: every Rust/C input and the actual native
  binary are unchanged. Prior native validation is not relabeled as a new run.

## Remaining work and preserved evidence

On this final block, narrow categorical still needs 13.4 ms (30.5%) savings to
reach 0.90 stock; two-partition K6/T8 slopes need 45.9 ms (36.4%). Large low
slopes need 30.9 ms (11.0%); large fast slopes need 7.1 ms (2.7%). These budgets
apply only to their declared cases. Historical larger losses remain evidence
requiring their own remeasurement; a favorable fresh seed does not repair them.

Move to a larger canonical-construction intervention, using the measured 73/90 ms
fast/low preparation boundary and 68 ms ordinary joint boundary. Keep weighted
geometry, mathematical witnesses and final inference in the same cost comparison.
Do not pursue interrupt round-trip tuning or another metadata micro-experiment.
The objective remains active. Frozen 77/16/111 definitions, constrained-fit and
nonretaining-CG failures, older comparator discrepancies, allocation-inventory
failure, SCC authentication and deferred platform gates remain unchanged.

The verified archive preserves **16,896 logical receipts / 3,259 payloads / 29,851,575 bytes**, SHA-256
`cbe4175e801f449a205c852a602ed0212ed05bda1a208c8667f6894f3ea49186`. It includes captured sources/identities for all arms, declarations, phase counters, numerical exports, audits, boundary probes, corrected and failed experiments, and recipes. Generated data/native binaries and licensed logs retain hashes; raw licensed logs are excluded. `restore_receipts.py` verifies and restores the sanitized logical files. Work root: target/ppml-command-cost-20260925.
