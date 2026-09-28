# Requested cores and selected native teams

The new read-only [thread inventory](../../../performance/thread_inventory.py)
fills a reporting gap without changing the frozen measurement controller. It
joins that controller's eligibility ledger to the actual native receipts and
separates the requested cores, selected native team, and Stata processors.
It never upgrades an NR comparison or establishes a causal scaling claim.

Seven focused evidence tests pass: missing/invalid thread metadata, capped
requests, cross-node observations, missing or failed repetitions, input/sample
differences, repeated seeds, and complete collector identity/coverage checks.
The collector preserves every input file and binds each consumed receipt.
[tests.txt](tests.txt) records this proportionate local validation. No estimator
test, fit, compilation, profiling or timing campaign was run for this change.

## Validation against real captures

[validation.json](validation.json) binds the analyzer, frozen reports and
consumed native receipts. The full read-only inventories remain at the recorded
SCC evidence root; their hashes are included. All analysis ran on existing files.

The original completed campaign has **568 native attempts: 536 successes,
28 command timeouts and four worker failures**. All 536 successful fits have
valid requested/selected metadata, and all selected their requested team.
Stata used four processors for native requests of 4/8/28 and one for T1.
The collector finds 72 adjacent-request transitions across otherwise identical
registered configurations. Of these, 68 have all four scientifically eligible,
same-input/sample observations. **262 of their 272 repetition pairs cross
nodes or tasks**, so they require controlled confirmation before causal claims.

These seven transitions lose on every original repetition. The table is
**historical source 58284f76**, fingerprint **041bf479…**; it is not evidence of
current-source regressions or a new adjudication of their causes.

| Model / design | N / K | Requested and selected native teams | Median higher/lower time |
|---|---:|---:|---:|
| OLS / degree two | 983,040 / 10 | 4 → 28 | 3.669 |
| PPML / degree two | 983,040 / 10 | 4 → 28 | 1.191 |
| OLS / sparse three FE | 983,040 / 10 | 8 → 28 | 1.031 |
| PPML / sparse three FE | 983,040 / 10 | 8 → 28 | 7.189 |
| OLS / sparse three FE, wide | 983,040 / 50 | 4 → 28 | 3.170 |
| PPML / sparse three FE, wide | 983,040 / 50 | 4 → 28 | 6.265 |
| PPML / weak degree three | 983,040 / 10 | 4 → 28 | 2.377 |

At **2026-09-28 14:56 UTC**, an isolated partial report produced by the unchanged
current controller has 20 completed native applications, all with valid thread
metadata. Nineteen selected the requested team; one T28 request selected T8.
The partial inventory has **no complete eligible scaling distribution**. Running
task accounting and missing repetitions are not assumed successful. Its
fingerprint is **fb2ad540…**, accepted estimator package **c09eeab3…**. The live
sweep, its source, plans, reference settings and scientific checks are untouched.

## Interpretation and use

The output retains every registered cold/warm attempt and all missing/failure
states. Transition distributions require every registered warm repetition and
unchanged scientific eligibility, with matching input/sample identities. Each
repetition retains its clock, locations and selected teams. A capped larger
request is labeled `same_native_team`, so request overhead cannot be described
as the effect of adding native cores. A reported selected team does not imply
that every phase uses all those workers.

Use the supplement after collecting the complete current-source sweep with its
original controller, following [LIGHT.md](../../../systematic/LIGHT.md). Any
suspected slowdown requires alternating same-node confirmation before routing
changes. Even a same-node/task cross-case observation is not an alternating
experiment. The universal 1.20 ceiling, 0.90 aspiration, and no-repeatable-slowdown
objectives remain open; this milestone improves the evidence used to pursue them.
