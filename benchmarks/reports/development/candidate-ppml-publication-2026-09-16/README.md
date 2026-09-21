# PPML publication admission and terminal allocation observation

Uncommitted development checkpoint on main
`5240e589400c888690666265263ad96f9b694aa6`. **Implemented and focused-test
validated; whole-action coverage and candidate adoption remain incomplete.**
Historical receipts and the installed package are unchanged.

## Implementation

The [incremental patch](incremental-implementation.patch), relative to
`5240e58-lifetime-ledger6`, completes the capacity inventory for the certified
PPML result retained by the plugin. It includes nested coefficient/status maps,
original/candidate row ledgers, output and saved-channel vectors, covariance and
Wald metadata, FE/cluster names and identities, and separation witnesses. The
shared row-ledger allocation includes its Arc header once. Spare capacities
are counted. Reporting walks container descriptors; it does not allocate or
scan observation arrays.

Before allocating publication shadows, PPML now admits the simultaneous frame:

```
max(embedding reserve, surviving action metadata capacity)
+ certified result capacity
+ shadow matrices and declared-channel slice descriptors
```

The embedding reserve is deliberately not reclaimed even though some earlier
input owners have already dropped. Actual shadow capacities must fit before any
store. The existing scalar PPML publication reservation now also includes the
slice descriptor bank, including declared channels that are not saved. Exact
and one-byte-short budgets are tested. This post-certification boundary does
not prove the earlier result assembly or independent PSD/Wald certification
frame; the ledger retains its publication coverage gap.

Two avoidable validation allocations are removed:

- Final-sample validation compares the sorted retained rows directly with the
  retained entries of the original-row drop ledger. It no longer allocates an
  original-N boolean mask. Existing order, range and error checks stay intact.
- OLS and PPML capture the resolved worker count during action setup. Final
  validation reuses it instead of querying OS parallelism again. On Linux with
  default threads, the earlier query allocated during publication.

The opt-in allocator trace additionally emits a version-1
`FEREG_ACTION_MEMORY_TERMINAL` record at action return, after action frames have
dropped, on success or failure. A sequence guard prevents a pre-entry failure
from reusing an earlier action baseline. The existing model record still says
precommit; the terminal record does not invent a complete model or ratio.
Diagnostic stderr writing occurs outside the transaction, does not call SPI,
and cannot change the command's return code or turn COMPLETE into failure.
The original OLS ABI dispatch branch remains pinned and unchanged. Public
memory fields, protocol versions, numerical policies and thread selection are
unchanged.

## Focused validation

Snapshot `5240e58-publication-lifetime2`, job **7596013**, passes:

- 127 plugin unit/mock-SPI tests with allocation audit disabled, and the same
  127 tests with it enabled.
- 30 core certified-result tests, including 5,334 small final-row/drop-ledger
  combinations checked against an independent boolean-mask oracle.
- 45 core memory-planner tests, including no/partial/all saved-channel requests.

These are **202 distinct native tests**, not 329 independent cases. New tests
compare retained result/action capacity counts with independent allocator
releases, include excess nested capacities, check shadow growth against the
admitted frame, and verify zero allocations through final validation and stores.
Fixtures cover no FE, weighted clustered saved effects, exact-FE separation,
and low-memory publication. Failure and interruption leave no COMPLETE receipt;
a subsequent call succeeds. Existing per-store failure and allocation-failure
fixtures remain in the passing suite. Final-sample validation also passes an
allocation-free check at 100,000 original rows.

[Source verification](source-verification.json) binds the source snapshots and
[remote test-executable hashes](native/remote-verification.json). All 290 final
bound files match the working source. Core sources are identical between the
passing native snapshot and final snapshot `5240e58-publication-lifetime4`.
The only later native changes are the ABI diagnostic placement/sequence guard
and the private ledger's explicit post-certification publication frame.

All **38 Python source-contract/inventory checks** pass. The repository checker
and OLS v25/action v8 audits pass; historical manifests and F0 pins remain
untouched. [Check logs](checks/) preserve the initial failures and final pass.
These static checks establish source guard consistency, not complete memory
coverage.

## Preserved failures

Job **7595986** remains failed. Of its 125 plugin tests, the new zero-allocation
publication test found five allocations caused by the OS thread-count query;
that demonstrated defect was fixed. Its 29 result tests passed. One of 45 core
memory tests failed because the new fixture declared too few output columns
for its saved channels; the fixture was corrected without changing the contract.

The initial source checks also rejected diagnostic code placed inside the
frozen OLS dispatch boundary and stale manifests during editing. The diagnostic
was moved to a separate label helper, preserving the exact original branch;
the serialized final checks pass. A source transfer initially failed because
its remote parent directory did not exist; creating that task directory fixed
setup before submission. Canceled job 7596058 never started: its superseded
snapshot is preserved and supplies no accepted test or performance evidence.

Original log hashes, normalized review copies and scheduler accounting are
preserved in [job-results.json](job-results.json), [native/](native/) and
[original-log-hashes.json](original-log-hashes.json). Scheduler accounting
reports failed=0/exit=0 for the passing job, with 149 seconds and 3.293G maxvmem
including compilation. That scheduler maximum is not a command-memory bound.

## Complete-command cost screen

Final snapshot `5240e58-publication-lifetime4`, job **7596067**, passes final
127-test plugin validation, the Linux build/export audit, actual-plugin loading,
and the audit-enabled actual-loader harness. The scheduler reports failed=0,
exit=0, 242 seconds and 2.489G maxvmem including compilation. After the requested
change to 28 cores it ran from 14:14:36 to 14:18:38 local cluster time. Its monitor
was deleted after terminal accounting; no validation jobs from this checkpoint
remain active.

The candidate plugin SHA-256 is
`c3ada630de1278c771bb7c128591ff4e9104dae3abc8e0584475598966116e83`.
The baseline is the preserved, unadopted `5240e58-action-coverage3` binary,
`e87a902c1a94a187513a6369312d8cd8879a0ce1601be78ab762e1cd2a49c3a0`.
This is a cumulative unpublished memory-admission cost screen, not a comparison
to committed main or an isolated speed claim for the two allocation removals.

All **nine alternating pairs / 18 fresh Stata commands** pass, with exact
scientific matrices, samples/order, solver non-time fields and saved outputs.
All worker return codes and terminal sentinels are verified, with no Stata
error code in the original logs. The fixed three cases, hashes and scientific
comparisons are in [cost/](cost/). Thirty-six scheduler snapshots establish
exclusive scheduled occupancy of the 28-core node before and after every
command; execution remained capped at four physical cores. The source, cases
and execution cap did not change when the queued reservation changed from 32
to 28 cores; the [resource-change receipt](resource-change-28-cores.json) retains
that history.

| Case | Baseline median | Candidate median | Change |
| --- | ---: | ---: | ---: |
| Saved-slope OLS, low memory | 0.303 s | 0.299 s | -4 ms (-1.32%) |
| Narrow robust PPML | 0.123 s | 0.122 s | -1 ms (-0.81%) |
| Weighted/clustered PPML, saved effects | 1.014 s | 1.020 s | +6 ms (+0.59%) |

All clear the predeclared slowdown threshold of both 5% and 10 ms. The
saved-slope flag does not reproduce, so this fixed checkpoint's command-cost
gate passes. Earlier failed/ambiguous timings remain preserved; this does not
claim a universal speedup or formally qualify performance.

[Phase and memory records](cost/phase-memory-summary.json) and the
[additional OLS phase summary](cost/extra-phase-summary.json) retain all three
observations per arm. Saved-slope native medians are 0.2130/0.2090 seconds;
fit/inference medians are 0.20744/0.20707. Native allocation peak is unchanged
at 3,301,792 bytes and scratch at 1,573,056 bytes. Narrow PPML native peaks are
unchanged at 2,429,937 bytes; weighted saved PPML peak medians are
65,661,663/64,687,388 bytes. These cumulative observations do not isolate the
cause of that difference. PPML scratch and fallback counts are zero in both
arms, and inner iterations/step halvings match exactly (6/0 and 49/0). No new
retry or discarded-attempt cost appears in the exact solver diagnostics.
RSS medians (baseline/candidate GiB) are 0.0730/0.0749 for slopes,
0.0709/0.0736 for narrow PPML and 0.1376/0.1346 for saved PPML. No reported
phase median exceeds both timing thresholds; these phase medians need not sum.
Allocation figures and the old public base ratios do not establish complete
whole-action coverage or a cap breach.

The actual-loader audit additionally produces [287 arithmetically valid terminal
records](terminal-trace-validation.json): three OLS successes and 284 failure
observations, including one PPML syntax error. This loader does not exercise a
successful PPML terminal trace; successful PPML publication is covered by the
separate mock-SPI and licensed fixtures. The expected injected panic remains
contained and the loader finishes with its PASS sentinel. No incomplete ledger
ratio is promoted to a complete model.

## Remaining boundary

This change closes the plugin frame after PPML core certification. Construction,
complete numerical/current-generation overlaps, dependency container capacities
and cold/retiring worker pools still need proofs. The core PSD/Wald certification
frame also remains separate. Whole-action coverage and its ratio stay undefined.
The fixed saved-slope timing gate above passes. The local Xcode
license prerequisite, candidate freeze, universal Mac/Rosetta checks, Mac/Linux
review packaging and clean installation remain open. No German estimation,
campaign restart, installed-package replacement, commit, push, tag or release
was performed.
