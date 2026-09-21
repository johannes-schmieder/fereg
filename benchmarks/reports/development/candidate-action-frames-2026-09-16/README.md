# OLS construction and numerical-frame admission

Uncommitted development checkpoint on main `5240e589400c888690666265263ad96f9b694aa6`.
This extends the preceding unadopted request/slope prototype. **Whole-action
coverage and the cost gate are still incomplete; this is not an adopted or
qualified production candidate.**

## Implemented changes

The [incremental patch](incremental-implementation.patch) adds checked scalar
reservations before numeric OLS preparation. They include categorical dictionary
growth, candidate/final topology and membership overlap, singleton workspaces,
cluster codes and construction metadata. Slope-channel metadata is a product of
channel count and categorical label/source length; a constant multiple of request
bytes alone would miss that repetition. No observation-sized reporting state or
extra SPI pass is introduced.

A separate execution envelope charges finite QR/Cholesky and inference frames
beside actual retained prepared/request owners. Fast/low row buffers, bounded
scratch I/O banks, moment partials, covariance certificates and cluster reductions
are included. The dense cluster reducer can independently admit up to at least
64 MiB and coexist with parallel partials; that allowance was absent from the
base-only budget. Sequential fit/inference frames use a maximum; simultaneous
parents and reducers are summed. FE solver storage stays separately admitted.
Public base-memory fields and numerical policies are unchanged.

These are additional conservative admission envelopes, **not a proof that every
construction or numerical lifetime is covered**. The exclusions below prevent
that claim. The implementation does not turn an unknown action peak into zero
or publish a whole-action ratio.

## Validation and identity

The fixed [nine-case list](final/fixed-cases.json) precedes measurement.
Snapshot `5240e58-action-frames3` binds 289 files in [identity.json](final/identity.json).
The production Rust/Stata/build inputs match the tested snapshot; the subsequent
change to `estimate.rs` tightens only a test's VCE-specific allowance. Two
inventory defaults advanced separately. [Source verification](source-verification.json)
records every byte-level difference. No installed artifact was replaced.

- Baseline `5240e58-action-coverage3`, Linux artifact
  `e87a902c1a94a187513a6369312d8cd8879a0ce1601be78ab762e1cd2a49c3a0`.
- Candidate `5240e58-action-frames3`, Linux artifact
  `21ffd5cf6b368610a40cab095d7619d8a88d5bd32b5e8f946b4fdb8de4c208e7`.
- Installed Mac artifact remains
  `4187efdfbd6235e63c369d9b42dede6381b8a9e50b5360fd63ee237bd6c11b08`.

Job 7590352 passes 113 plugin tests on the first snapshot. Job 7590380 uses a
fresh Cargo target, passes **114 plugin tests**, and passes the Linux binary
export and actual-plugin ABI/mock-SPI harness. Its terminal sentinel is
`OLS_PARENT_PAIRED_COMPLETE`; scheduler failed=0/exit=0, wall time 439 seconds,
maxvmem 2.476G including compilation. Native-test checks cover independent
allocation peaks, exact/one-byte-short preparation and execution budgets,
failure/interrupt/repeated-call controls, numeric dictionary growth through
65,537 rows, and simple/slope topology in fast/low conventional/robust/cluster
fixtures. Workers are primed in the isolated allocation tests; these do not
establish cold runtime-pool coverage. Loaded inputs already exist when the
construction measurement starts.

The final review tightened the allocation test so IID and robust checks use
zero clustered dimensions in their frame model. Job 7590422 runs that exact
single test from a separate snapshot/fresh target and passes, with explicit
`OLS_FRAME_ROUTE_RECHECK_PASS`. Production token/literal identity matches the
measured snapshot; this is a test-only correction.

The 35 source-contract and three action-inventory Python tests pass. The first
source-contract run used the stale v22 test manifest with a v23 renderer and
failed; the manifest reference was corrected. The first repository check hit
the unaccepted local Xcode license through system git; the check passes using
Homebrew git. Both original failures are retained under [checks](checks/).
The Mac toolchain prerequisite remains unresolved. No Mac/Linux release or
installation qualification is inferred.

## Paired results

All **27 alternating pairs / 54 fresh Stata commands** have exact coefficients,
covariance, rank/status, samples/order, DoF, certificates, non-time solver fields
and saved outputs. The harness checks all scheduled occupants before and after
each command: [108 isolation receipts](final/isolation.jsonl) establish that this
job holds every scheduled slot on its 28-core node. Four distinct physical cores
on one NUMA node execute each four-thread application; compilation finishes first.
This does not eliminate OS activity, CPU-frequency variation or application scheduling.

| Case | Baseline seconds | Candidate seconds | Declared command gate |
| --- | ---: | ---: | --- |
| Small/narrow | .037 | .037 | Pass |
| Tiny weighted bridge | .463 | .456 | Pass |
| Three FE, FW/generated | .783 | .768 | Pass |
| Absorbed slopes | .329 | .376 | **Flag: +47 ms, +14.3%** |
| Singleton cascade, PW/cluster/low | .397 | .343 | Pass |
| Saved slopes, fast | .402 | .403 | Pass |
| Saved slopes, low | .533 | .575 | **Flag: +42 ms, +7.9%** |
| Wide one FE | 1.139 | .902 | Pass |
| Weak wide | 4.351 | 4.346 | Pass |

[Per-command receipts](paired-results.json) bind input values, artifacts,
reference lock, output hashes, solver tables, RSS and scratch. [Cost summaries](cost-summary.json)
include independently computed command/phase/memory medians; their medians need
not sum. All selected base-byte and scratch values match. Native allocation
peaks match except weak-wide medians 351,019,681/351,347,361 bytes (+327,680,
0.093%); this difference has not been attributed. Retries and discarded work are
included in total elapsed/peak observations, not separately isolated by this
harness. Solver non-time identities match. These are local development findings,
not speed claims over committed main or a frozen performance qualification.

## Timing attribution and disposition

Job 7590338 first rechecks the *previous* two artifacts with three AB pairs and
three identical-artifact AA controls. Command medians are .301/.302 seconds;
plugin pre-store medians .210088/.208833. The original weak-graph aggregate native
flag does not reproduce. One initial-absorption subphase still shifts +14.15 ms;
AA controls themselves shift +10.84 ms in aggregate plugin time. It would be
incorrect to infer that every earlier subphase difference has a proven cause.
All exports match and `WEAK_GRAPH_PHASE_DIAGNOSTIC_PASS` is present.

The full-node screen above clears the current weak-graph command and reported
native-phase medians but flags slopes. One bounded follow-up, job 7590402,
uses the unchanged artifacts, full-node isolation, phase traces, perf task-clock
and child resource counters. All 24 commands preserve exact science and end with
`SLOPE_COST_DIAGNOSTIC_PASS`; scheduler failed=0/exit=0, 59 seconds.

- Absorbed-slope AB command medians reverse to .377/.320; fit phases
  .187763/.158634. The AA command medians are .357/.404, and AA fit phases
  .143749/.197419: the same binary generates a +53.67 ms fit-phase shift.
- Saved-slope low AB command medians remain flagged at .509/.568
  (+59 ms, +11.6%). AA medians are .609/.557 (-52 ms). Whole-process child CPU
  medians are 2.675/2.775 seconds for AB and 2.909/2.780 for AA. These counters
  include startup/import/export and cannot localize the command discrepancy.
- The perf environment reports user-mode task-clock. Its zero context-switch
  counts do not mean no switching; child `getrusage` records hundreds of switches.
  CPU counters are diagnostic and do not prove a scheduler-only explanation.

[Raw scalar diagnostics](slope-diagnostic.json) and [phase medians](phase-summary.json)
retain every run. No failed receipt is reclassified as accepted. The saved-slope
low cost flag remains unresolved, so adoption is blocked. There is no automatic
retry, campaign expansion or expensive empirical rerun scheduled.

A queued `exclusive=true` request (7590370) never started: SCC advertises the
resource definition but no host offers it. It was canceled, then replaced by
the verified full-node request. It supplied no numerical or timing evidence.

## Remaining proof obligations

[Dependency review](dependency-review.json) pins the inspected source. At CMG
`90e1fe0`, `CmgMemoryEstimate`/`CmgMemoryReport` explicitly count **principal**
heap storage. In particular:

- `Laplacian::retained_bytes` excludes its two `Arc<Vec<_>>` owner/control blocks.
- `CmgHierarchy::retained_bytes` omits the `Vec<HierarchyLevel>` allocation.
- `ParallelPcgWorkspace::byte_len` sums inner payloads but omits its owning vector.
- `CmgWorkspace::byte_len` omits level/centering descriptor banks and dimensions;
  component-size reports use lengths although construction grows a vector.
- `ParallelExecutor` owns an additional Rayon pool beside fereg's team. Pool
  construction, lazy queues and retiring workers require their own lifetime proof.

Thus calling those reports “exact” must be qualified by their principal-array
scope. Existing fereg admission may conservatively cover some bytes, but neither
that possibility nor a passing sampled peak proves the missing owners. No
upstream dependency pin, kernel or numerical policy was changed in this checkpoint.

Other remaining obligations are fixed-width string dictionaries during SPI
loading, decoder/compiler allocations before the new admission boundary,
retained FE coefficient banks across nested finite/inference stages, and the
complete PPML parent/current-generation/runtime overlap proof. The current
construction and frame envelopes do not certify those scopes. Whole-action
coverage/ratio, candidate freeze, Mac prerequisite, package/install validation,
Windows artifact and release qualification remain open. No commit, push, tag,
release, installed-package replacement or unrelated cluster-job mutation occurred.
