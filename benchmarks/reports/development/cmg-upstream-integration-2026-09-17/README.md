# Published CMG integration: development evidence

CMG is pinned to `1d39c113f0c397f29653d92672090e5a42f80921` with
`parallel` and `experimental-components`. Every production configuration keeps
components disabled. Automatic routing, numerical tolerances, weight semantics
and certificates are unchanged. This is a scoped integration, not release
qualification or complete whole-action memory coverage.

## Source and ownership

Work started on main at `5240e589400c888690666265263ad96f9b694aa6`.
The earlier dirty memory work was preserved in a separate source snapshot.
Its 293 source hashes exactly match the previously tested
`5240e58-generation-capacity4` snapshot. The installed Mac plugin remains
`4187efdfbd6235e63c369d9b42dede6381b8a9e50b5360fd63ee237bd6c11b08`.
CMG itself was not modified. No installed binary, remote issue, push, tag,
release or stopped empirical campaign was changed.

Local commits:

- `5f2ff07a`: dependency, reference pin and admitted CMG ownership integration.
- `de2f8192`: use public dimensions to admit a plan before the routing query.
- `cae41abc`: preserve OLS phase traces, strengthen coefficient reconstruction
  checks and bind reviewed structural inventories.
- `9374a169`: bind allocation tests to their current reviewed manifest and add
  the bounded paired runner. No numerical code changes after `cae41abc`.
- `68b3b6c5`: one bounded same-binary PPML timing diagnostic.

The clean committed code and dirty overlay have separate source/artifact
identities. Never treat a result for one as an exact-SHA result for the other.
The committed inventories are OLS v30/action v15; the working memory overlay
uses OLS v31/action v16. These inventories detect structural changes; they do
not prove capacity coverage.

## Integration

Each solver owns one executor and its returned runtime estimate. Before startup,
fereg reserves 32 MiB pool heap, 2 MiB heap per worker, and 8 MiB requested stack
per worker, plus known handles. At 16 workers this is 192 MiB plus handles.
After including the executor in the storage union, only the heap and stack
allowances are added. Joined shutdown retains the charge through retirement.
This explicit policy remains an engineering estimate; strict runtime coverage
is unavailable and the whole-action ratio remains undefined when incomplete.

Ordinary and opt-in component hierarchy/factor construction use their matching
executor-aware admitted APIs. Prepared PPML topology, scratch, numeric assembly,
solver plans and fixed PCG banks use their matching checked APIs. Caller-owned
RHS/solution banks replace legacy owned results and lazy workspace growth.
OLS retains its batch widths and nuisance correction solves; prepared PPML keeps
its current-frame nuisance potential/seed reuse.

A single capacity union deduplicates shared graph/prepared Arcs and includes
nested banks. PPML subtracts the already charged prepared-parent union only
after measuring it together with the live frame. Covered private-container and
principal-array supplements are removed from production accounting. The old
untracked helper is preserved on disk but no longer compiled.

The upstream plan query can allocate. Its pinned public eligibility dimensions
are checked before routing, including a possible one-RHS tail. The upstream
component count is private, so some disconnected graphs conservatively admit a
small plan that routing will not use. No extra component discovery is allocated.

## Validation

[Native receipts](native-jobs.json) bind the final clean source `cae41abc` to
job 7626076: 40 CMG tests, six prepared-CMG tests, 35 PPML IRLS tests and
97 plugin tests all pass (178 test executions). Linux build/export and actual
plugin-loader checks pass. Both licensed `scc-dev-shared` fixtures pass with
`END_FIXTURE|fereg_smoke|rc=0` and `END_FIXTURE|feppml_smoke|rc=0`.
The final fixture summary binds the plugin artifact. Scheduler failed=0/exit=0,
28 slots on scc-wd2, 875 seconds, job maxvmem 3.521G. Build time is included;
this scheduler figure is not an estimation-memory bound.

The preserved memory overlay separately passes 40 CMG and 137 plugin tests,
build/export and the actual-loader allocation audit in job 7626099. Its 18
commands have explicit sentinels and 36 successful isolation checks. The clean
allocation mutation suite passes 37 tests; the overlay passes 39, including two
pre-existing added tests. The inspected source/lock diff changes only CMG's
revision, with no unrelated dependency updates.

The focused tests cover ordinary/components, weighted weak bridges, disconnected
graphs, retained coefficient reconstruction against certified CG, prepared
changing weights and lineage, fixed-bank reuse, one/four-worker execution,
serial/planned/concurrent-RHS routing, exact/one-byte-short admission, overflow,
old/new bank overlap, handle deduplication, fallback and repeated retirement.
Plugin fixtures exercise allocation failure, interruption and publication
rollback. Upstream's separately bound worker/TLS and allocation-injection
results remain upstream evidence, not additional downstream test executions.

Local Mac linked validation remains blocked by the unaccepted Xcode license;
it was neither accepted nor bypassed. Cargo fetched the exact published pin.
Dependency-closure Python checks pass (10); working-tree allocation mutation
checks pass (39). Scoped Rust formatting, shell syntax and repository checks
pass. No full-format, Mac linked or Windows result is claimed.

## Performance

Three cases were frozen before measuring: narrow two-FE OLS (7,680 rows, K=2),
frequency-weighted clustered two-FE OLS (65,536 rows, K=8), and robust two-FE
PPML (16,384 rows, K=2). Each requests CMG, four native threads, fast memory and
saved residuals. Three alternating fresh-process pairs per case give 18 commands.
The screen requires exact scientific matrices, sample/order, saved outputs and
non-time solver fields. Regression investigation requires both >5% and >10 ms.

The baseline uses the preserved generation4 artifact, independently rehashed as
`558b13af49aed33f0a22ca0f275431d970da3467e7d1ffc0fd281b121a03c05f`.
The candidate includes the same preserved prior memory work plus this CMG
integration. No dirty overlay is presented as a clean release candidate.

[Original paired results](pairs/summary.json) and [phase/memory costs](pairs/phase-costs.json):

| Case | Baseline command s | Candidate command s | Change | Native heap peak, baseline → candidate bytes |
|---|---:|---:|---:|---:|
| Narrow OLS | .080 | .082 | +2 ms, +2.5% | 2,457,859 → 2,482,051 |
| Weighted clustered OLS | .195 | .202 | +7 ms, +3.6% | 19,754,554 → 19,866,538 |
| PPML | .254 | .277 | +23 ms, +9.1% **flagged** | 10,008,396 → 10,029,082 |

Median sampled process RSS is 72.69 → 73.63 MiB, 82.91 → 83.55 MiB and
72.11 → 71.84 MiB respectively. RSS is sampled every 10 ms and is distinct
from native requested-heap reporting and the estimated stack/runtime allowance.
Both bounded jobs have failed=0/exit=0 terminal accounting in
[pair/diagnostic job receipts](pair-jobs.json): 290 seconds and 3.406G maxvmem
for the build-and-pair job; 28 seconds and 1.407G for the same-binary diagnostic.
All cases report zero scratch. PPML has unchanged 23 inner iterations, zero
step halvings and zero fallbacks. Thus discarded fallback cost is zero in this
screen; this is not a complete-command measurement of a failed attempt.
Focused native tests and separately bound upstream evidence cover failure and
fallback behavior, with no additional downstream timing claim.

The PPML original flag is concentrated in FE projection (.06503 → .10219 s).
Other comparable phases and both OLS complete commands stay below the threshold.
The full per-arm observations remain in [partial results](pairs/partial-results.json);
independently computed phase medians need not sum to the command median.

One [bounded diagnostic](diagnostic/summary.json), job 7626155, reused the exact
same binaries and input on another node. Three untraced alternating pairs measured
.610 → .582 s; FE projection .19932 → .18896 s. One additional, separately
labelled traced pair measured .590 → .585 s. All eight diagnostic commands
matched their original arm's scientific results, sample and saved residuals.
No original result was replaced. The diagnostic's lower candidate times do not
establish that the first flag was noise or a universal no-regression result.

The traced pair had four CMG setups per arm. Hierarchy totals were .101388 s
baseline and .101151 s candidate; solver/workspace provisioning increased from
.000209 to .000330 s. This trace does not identify an intrinsic setup regression
large enough to explain the original 37 ms projection difference. Timing and
node sensitivity remain uncertain. No arithmetic or routing change is justified
by this evidence, and no further campaign was launched. The original performance
gate remains false; this integration is locally implemented and numerically
validated, but not performance-qualified or release-approved.

## Failed evidence and limitations

- The initial `5f2ff07a` Linux attempt (job 7626036) failed compilation because
  the upstream component accessor is private. No estimation ran. Public,
  conservative eligibility dimensions replaced it in `de2f8192`.
- An early static check saw stale structural manifests. Reviewed new manifests
  were generated separately for clean code and the preserved overlay.
- The first allocation Python run used a stale hard-coded manifest and failed
  (39 tests: two failures, 28 errors). Tests now use the reviewed default. The
  preserved failed output has not been rewritten.
- Local linked compilation is blocked by the Xcode license prerequisite.

Runtime heap/stack allowances are not measured bounds or OOM guarantees. Ordinary
iterator edge ingestion, allocator/native headroom, small infallible Arc
allocations and upstream's modeled Arc layout retain their limits. Fereg-owned
construction, inference, recovery and publication scopes retain independent
coverage requirements. Components are not promoted to automatic use. These
small local-development checks do not establish universal performance or close
beta packaging/platform/release gates.
