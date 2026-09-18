# Retained OLS coefficient parent admission — development evidence

Code commit: `ded4f95b2d875605c96f2c496ff74bd084107385`, on `main`.
This is a focused memory-boundary change, not a production candidate or formal
qualification. CMG remains pinned to `1d39c113f0c397f29653d92672090e5a42f80921`;
components remain opt-in and solver-selection policy is unchanged. Admission
uses the corrected live-parent budget, so a previously understated tight budget
can now reject the fit or select an already supported certified fallback.

## Implemented boundary

Fast OLS now admits the initial and combined FE coefficient products before
constructing a retained solver. The initial product survives the finite solve
and residual projection; the combined product can overlap it. Sequential solver
reports include this parent once relative to the unchanged execution base.
Uncached robust/clustered score reconstruction separately admits the observed
capacity of the final retained product. This commits the relevant portion of
the preserved memory work together with its new capacity checks.

At initial return, the values and all three layout-vector capacities must fit
the reserved parent together with the future combined product. Combination
checks admission before requesting its bank and validates the actual returned
capacities. Spare capacity cannot silently consume a retained session's budget.
No numerical arithmetic, sample, weight, rank, tolerance or covariance policy
changed. The checks are outside numerical loops and allocate no reporting bank.

## Validation and identities

`identity.json` and `source.sha256` bind the clean and separately labelled
preserved-overlay source, build inputs and synthetic fixtures. All 163 clean
native/build/fixture files match the code commit. Tests were compiled before
commit with the base SHA in the build environment; this is snapshot-bound
native development evidence, not an exact-SHA package qualification.
`validation.json` records native test-binary hashes, log hashes, counts and
terminal markers; scheduler receipts record job outcomes and resources.

| Source scope | Test group | Passed | Failed |
|---|---|---:|---:|
| Clean snapshot | clean-fit | 78 | 0 |
| Clean snapshot | clean-plugin | 97 | 0 |
| Preserved overlay | overlay-fit | 79 | 0 |
| Preserved overlay | overlay-plugin | 137 | 0 |
| Clean, added inference test | robust | 12 | 0 |
| Clean, added inference test | clustered | 9 | 0 |

Jobs 7628936 and 7628956 both end with their explicit PASS sentinel and
scheduler `failed=0`, `exit_status=0`. Both reserve 28 slots on scc-zj4.
Job 7628936 takes 343 seconds with maxvmem 8.746G; the inference job takes
49 seconds with maxvmem 4.109G. These totals include compilation and
test subprocesses; they are not estimation memory or performance bounds.


The new tests cover exact and one-byte-short parent budgets, spare value/channel/
level capacities, checked estimate overflow, three injected combination
allocation failures, rejection before new allocation, repeated calls and exact
small coefficient arithmetic. The wider fit suite covers weighted fast/low
agreement, weak graphs, saved effects, reconstruction and strict rollback;
plugin tests cover public transaction, allocation and interruption failures.
There are no standalone tests in the score helper module: that filter returns
zero cases and supplies no additional coverage count.

Both clean and preserved-overlay structural inventories pass. Five focused
Python inventory tests and repository checks pass. Clean manifests are OLS v32
and whole-action v17; the preserved working overlay uses v33/v18. Exactly the
three reviewed Rust files change the clean whole-action inventory. Historical
manifests and receipts remain unchanged.

The first deployment, job 7628933, failed test compilation because it omitted
an existing `include_str!` CSV fixture. It never executed tests. Its original
snapshot identity, failure extract/hash and scheduler record are preserved.
One corrected deployment, job 7628936, includes the existing synthetic fixtures;
the Rust source did not change between attempts.

The dedicated inference test was then adopted from the preserved worktree.
Its first scoped extraction (7628954) omitted its `prepared_inputs` test helper
and failed compilation; the original log hash and scheduler failure are retained.
Job 7628956 includes that helper and passes both inference groups. The production
tokens are unchanged from the code commit. Its separate `score-test-*` identities
and `score-validation.json` bind this test-only addition. No production defect or
scientific threshold was changed to address either deployment failure.

## Limits and remaining work

The handoff check bounds products after the producing solver returns; it does
not prove that solver's internal construction allocations. Ordinary CMG iterator
ingestion, remaining fereg construction/transient frames, complete inference/
recovery/publication accounting and opaque dependency/runtime coverage retain
their separate obligations. No incomplete-coverage bit is cleared and the
whole-action ratio remains undefined. The accepted 32 MiB/pool, 2 MiB worker
heap and 8 MiB requested worker stack policy remains explicitly estimated.

No new performance comparison, licensed Stata run, Mac linked test or release
package was run. The earlier CMG PPML/phase timing flag remains unresolved; Rust
suite runtimes are not complete-command performance evidence. Mac compilation
remains blocked by the unaccepted Xcode license. The installed Mac plugin still
hashes to `4187efdfbd6235e63c369d9b42dede6381b8a9e50b5360fd63ee237bd6c11b08`.
No installed artifact, CMG source, issue #27, push, tag, release, unrelated job
or stopped campaign was changed. Unrelated memory work remains in the worktree.
