# Fresh final PPML replay and ordered sample-ledger validation

Local Mac arm64 development evidence, 2026-09-25. Not qualification.
[ADR 0195](../../../../docs/adr/0195-fresh-ppml-final-replay.md) retains
**finalreplay1** as a modest common verification improvement. Fresh medians improve
2 ms for the principal low-memory case and 1 ms for slopes; three medians tie.
The broad multi-FE <=0.90-times-stock objective remains unfinished. No new solver,
IRLS schedule, tolerance, reference setting, or output omission earns these gains.

## Source and computational change

Candidate source is `f97c9dbe` plus `ppml/family.rs` and `ppml/result.rs` under
`crates/fereg-core/src/`. All **558 captured inputs** match the candidate worktree.
The accepted baseline is canonicalcg3. At starting checkpoint f97c9dbe, 555 of
its 557 captured inputs still matched; two later diagnostic-script differences
are documented in ADR 0194. Candidate and baseline estimator changes are confined
to the two files above. Captured
source copies, checksums, build recipes and candidate patch are archived.

| Artifact | Plugin SHA-256 |
|---|---|
|canonicalcg3, baseline|dcf1672c99e05521f5869309a3756f862025794e6bc23d95e6518e3088755d98|
|finalreplay1, candidate|84db18f8e91f31eb64f26c854df98609b3722a3d49828edbad040118b3f188b3|
|finalreplayledger1, diagnostic baseline|e05e86667c4e103836f8fc6701ced7aa86cb5c3c1f8b64eb3639f81115d8861f|
|finalreplayledger2, diagnostic candidate|06de07a74e4b6abd4c1eba877d584174899af9c6ed456307c6f793b2d39f10e4|

Final fitted-objective replay and null likelihood share a fresh original-input
row pass and one log-Gamma evaluation per outcome. No prepared IRLS terms enter
this independent replay. Null-mean preparation uses separately scaled positive
weight/outcome sums with compensation; subnormal or nonfinite intermediate
arithmetic refuses to the existing log-space method. Each likelihood still sums
stable per-row terms, rather than subtracting large aggregate totals.

The sorted retained/dropped ledgers now advance together in linear time after
their existing independent order/range checks. Every row's membership, active
mask and reason must still agree. Seals, generation hashes, method certificates,
counts and request identity remain independently checked. No persistent cache,
observation bank, worker team or allocation is added.

## Attribution, separate from headline timings

The pending baseline diagnostic completes with four passing public states and two
passing native comparisons. It locates 4.355 ms in fitted/null likelihood work and
1.220 ms in separation-report validation on the large low-memory case. These are
one-repetition diagnostic spans, not stock measurements.

After the mixed public pilot, one closing two-rotation diagnostic compares the
same instrumented boundaries on development seed347. All eight public states and
four native comparisons pass. Values are inclusive median elapsed milliseconds;
scopes overlap, and worker sums must not be added to wall time.

| Phase | Low N61440/K6/T4: baseline / candidate | Narrow N24960/K2/T4: baseline / candidate |
|---|---:|---:|
|Complete result certification|8.101 / 5.728|3.346 / 2.491|
|Fitted + null replay, old separate calls / new combined call|4.368 / 2.618|1.812 / 1.110|
|Separation report|1.243 / 0.746|0.469 / 0.314|
|Canonical preparation|27.744 / 27.193|5.636 / 5.555|
|Whole instrumented plugin|103.476 / 99.677|31.875 / 31.010|

The measured certificate savings are 2.373 ms and 0.855 ms. The other phase
changes are not attributed to this implementation. On low mode, ordinary joint
work remains 23.836 ms, recurrence 5.784 ms, geometry preparation 1.485 ms and
coupling preparation 0.951 ms. This rejects an interpretation that rebuilding
group geometry alone explains the remaining whole-command gap. Canonical work is
still the larger target; a finer operation ledger must distinguish construction,
products, original-input reconstruction and checks before another redesign.

## Headline comparisons and all distributions

Pinned unmodified fixest 0.14.2, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
uses glm.tol=1e-10 and fixef.tol=1e-11. No strengthened reference setting is needed
in these blocks. OpenMP/Rayon match requested threads; BLAS and nested parallelism
remain bounded by the existing harness. Robust inference and fitted means are
included. Generation, import, process startup and warmup precede the clock;
public loading, estimation, certification, inference and output are inside it.
No build, profile, archive or competing fit overlaps headline timing. Every
process monitor passes, subject to its sampling rather than a guarantee against
all ambient operating-system activity.

The three-rotation development pilot uses seed347 and no tuning follows it.
Its mixed outcome remains visible; the phase savings motivate one source-frozen
fresh confirmation. Milliseconds, in declared repetition order:

| Pilot case | Baseline | Candidate | Stock | B/C/S medians |
|---|---|---|---|---|
|Low N61440/K6/T4|113, 112, 113|114, 112, 112|98, 94, 94|113/112/94|
|Narrow N24960/K2/T4|39, 40, 47|37, 41, 42|34, 32, 33|40/41/33|
|Two slopes N30720/K6/T8|79, 78, 77|79, 75, 82|78, 81, 81|78/79/81|

The pilot has four candidate/baseline wins, three losses and two ties. The low
median improves 1 ms, while narrow and slopes each lose 1 ms. No pilot clock is
pooled with the fresh block.

Fresh seed **202609430**, declared before generation, five rotations on each of
five unchanged family definitions. No further tuning or favorable rerun follows:

| Fresh case; N/K/T | Baseline | Candidate | Stock | B/C/S medians | C/stock | C/B wins/losses/ties |
|---|---|---|---|---|---:|---|
|Disconnected categorical, low; 61440/6/4|114, 114, 115, 117, 113|110, 114, 111, 112, 113|98, 94, 94, 94, 94|114/112/94|1.191|3/0/2|
|Disconnected categorical, low; 38400/10/1|145, 144, 142, 143, 144|144, 144, 138, 141, 146|241, 244, 246, 243, 237|144/144/243|0.593|3/1/1|
|Narrow categorical, fast; 24960/2/4|39, 39, 39, 41, 40|40, 41, 39, 39, 38|35, 35, 31, 34, 33|39/39/34|1.147|2/2/1|
|Wide categorical, low; 23040/36/8|88, 86, 85, 86, 86|86, 86, 87, 86, 90|129, 128, 128, 126, 129|86/86/128|0.672|1/2/2|
|Two slope partitions, fast; 38400/8/4|104, 106, 109, 105, 109|105, 106, 104, 104, 108|181, 176, 177, 181, 184|106/105/181|0.580|3/1/1|

Fresh totals: **12 wins, six losses, seven ties** against baseline. Two medians
improve, three tie; serial, narrow and wide retain individual losing pairs. The
wide candidate loses two pairs by 2 and 4 ms. These are modest gains, not evidence
that every fit becomes faster. Retention rests on reduced shared verification
work, a 2-ms gain with no losing pairs on the principal low target, no new storage
or solver state, and unchanged science. It does not warrant another tuning cycle.

Three of five fresh medians meet the 0.90-stock target. Low still needs **27.4 ms**
off 112 to reach 84.6 ms; narrow needs **8.4 ms** off 39 to reach 30.6 ms. These
ceilings belong to this block, not the older source/seed timing blocks.

## Science, controls and memory

All 114 public states, 74 baseline-anchored comparisons and 34 direct stock pairs
across headline/diagnostic blocks pass. There is no timeout, application failure
or scientific discrepancy in these blocks. Coefficients, covariance and fitted
mean bytes are identical for all 34 production candidate/baseline PPML pairs.
Outer/inner counts and fallback counts are unchanged. Null statistics may change
rounding because their mean is no longer evaluated through log/exp in ordinary
arithmetic. An independent original-sample `math.fsum`/SciPy log-Gamma audit checks
68 baseline/candidate null-statistic states. Maximum absolute discrepancies are
1.24345e-14 in mean, 2.61935e-10 in null likelihood and 1.11023e-15 in pseudo-R2.
Existing comparison envelopes remain unchanged.

Fifty-two distinct Rust tests pass: 21 family and 31 result tests. New independent
checks use integer factorials/Gamma(3/2), literal frequency expansion and an
algebraic extreme-weight mean whose normalized product underflows. Subnormal mean,
invalid-domain/nonrepresentable arithmetic, overlapping sorted ledgers and
corrupt masks fail closed or follow the intended fallback. Existing objective-
drift, separation-seal, generation, metadata, rank/VCE and threaded family tests
remain green. The first new mean assertion incorrectly required bit equality to
a rational value; it observed one-ulp error. It now allows two machine epsilons.
That failed draft is retained; no existing scientific tolerance changes.

Six licensed fixtures pass with explicit terminal sentinels and zero application
return codes: iterative canonical, multi-partition slopes, three-partition slopes,
public scientific contract, weights/inference, and separation outputs. They include
independent Poisson/Hessian/sandwich checks, weight replication/rescaling,
postestimation, sample policy and failure atomicity. All plugin builds pass the
actual-plugin ABI/self-test and intentional-panic containment checks.

The unchanged OLS controls have nine byte-identical scientific export pairs.
Baseline/candidate medians are **11/10, 59/58 and 108/108 ms** for small categorical,
three categorical, and slopes. Four pairs win, four lose and one ties. No OLS
algorithm change or OLS speed claim follows these noisy controls.

There is no new production heap allocation or observation-sized storage. Every
pilot/fresh pair has zero tracked peak and scratch delta. Selected-model telemetry
varies from -48 to +48 bytes; it is not a measured payload increase. No new warmed
allocation campaign or physical-cap proof is claimed. Existing allocation-inventory
drift, constrained-fit/nonretaining-CG issues, baseline checkpoint-count assertion,
older three-FE coefficient discrepancy and deferred platforms remain open. Frozen
77/16/111 definitions are unchanged; no qualification, push or release is performed.

## Immutable receipts

`receipts-content-addressed.zip`: 6,129 logical receipts, 1,458 payloads, 11,481,657 bytes; SHA-256:
`096a769d0e906d30148081889d319be0baaf3ac5812477ec2ce21a6bcbc4090b`. All receipts have been restored and hash/privacy checked.

Run `python restore_receipts.py DESTINATION` to restore source identities, captured
sources, recipes, declarations, monitors, complete timing vectors, audits, test
results and diagnostic traces. Full datasets/plugin binaries are omitted with
hashes and reproducible generation/build recipes. Raw licensed logs are excluded;
fixture receipts and protocol-only trace excerpts remain. `summary.json`,
`verification.json` and `null-statistics-audit.json` expose the checks summarized
here. Seed430 is consumed/frozen and cannot become a tuning seed.
