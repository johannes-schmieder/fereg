# Prepared direct-CG admission: adopted correctness repair

Adopt [ADR 0060](../../../../docs/adr/0060-prepared-direct-cg-memory-admission.md).
Prepared direct-CG adapters now check their supplied joint execution/CG budget
before working allocations. They reuse the existing checked CG estimate and
five retained coefficient-batch allowance, with actual RHS width and installed
workers. This is an admission repair, not a numerical change or speedup.

## Source and execution coverage

Accepted numerical baseline is `dadd956`, at evidence HEAD `de6988b`.
[Source identities](cg-built-identities.json) bind 176 inputs and the isolated
[two-file implementation](cg-implementation.patch):

- Baseline plugin: `e831a6574268402682aec9432a5275cfce1697cc94feb8472f1a95f2f834ae26`.
- Candidate plugin: `fdc3e141992fe4449064b688e995b472175bbd4966efdc10d65a9a25de51869d`.

Both compared sources exclude the unadopted saved-Schur/recovery prototype.
The only production change is `prepared_fit_view.rs`; `prepared_fit.rs` adds
its regression. No observation-sized state, new solver, SPI call, arithmetic,
weight/sample/rank/VCE/DoF policy or certificate change is introduced. Planning
uses fallible FE-scale level metadata. Missing budgets and empty/slope topology
retain their existing behavior; this is not a universal allocation proof.

The original adapters passed only `config.map` to CG and discarded the joint
budget. The automatic LSMR/CG planner's existing checks do not protect explicit
CG or every preselected direct-CG call. The new guard covers simple intercept
and categorical intercept adapters, with coefficient retention where supported.
It runs for initial, residual and bounded-block calls through those adapters.

## Independent and constrained validation

[Validation](validation.json) records 65 isolated prepared-fit tests, including a
closed-form residual test with 36 dispatch/weight/topology/worker combinations.
Each checks exact admission, one byte below and zero budget: simple/categorical,
unit/supplied weights, retained coefficients, direct/explicit/preselected-CG
entry and one/four installed workers. The unchanged residual oracle is ±1 at
1e-10. The test fails before the guard because CG succeeds below its model;
all expanded cases pass afterward.

The [public fixture](cg_cap.do) uses N4096, four FE levels, no finite controls
and an independent full-dummy OLS residual. Execution is 212,992 bytes and the
CG increment 98,504, giving 311,496 usable bytes. Four [checks](cg-cap-validation.json)
pass: accepted baseline uncapped and one byte below; candidate one byte below
returns r(909), preserves prior b/V/sample and row order, and publishes no
requested residual; candidate exactly at the bound succeeds with 296,259 native
peak bytes. The earlier baseline's actual peak also fits this fixture's cap;
the defect is missing conservative admission, not a demonstrated allocation
overrun on this particular small example.

All 48 independent saved-OLS dummy fits and rollback pass on the isolated
candidate: unit/AW/FW/PW, one/two FE and fast/low. The existing weighted-within
oracle, changed only to explicit CG, passes b/V/DoF/RSS/residual comparisons at
one/seven workers. Both architecture builds/static audits and native arm64 ABI
checks pass. The missing DenseMatrix constructor, omitted isolated test fixture,
initial absent build cwd and diagnostic collection correction are disclosed in
validation; no numerical threshold was loosened. No SCC/Windows or qualification
ran. Installed dist and separate Veneto work remain untouched.

## Complete-command cost

The [six-control protocol](cg-cost-protocol.md) and subsequent
[two-pair changed-dispatch supplement](explicit-cg-protocol.md) precede their
measurements. The protocol's `complete_unit` label denotes the captured
`complete_iid` request. All eight pairs use fixed existing data/warm-ups, fresh
local M2 Ultra Stata/MP 19 processes and rotated arm order. No builds, tests or
diagnostics overlap these timings. Native worker requests are distinct from
Stata's eight-processor license cap; macOS affinity is uncontrolled.

| Fixed control | Baseline → candidate seconds | Change |
|---|---:|---:|
| Unit, saved both FE, automatic | 2.925 → 2.905 | −20 ms / −0.7% |
| PW, partial saved FE, automatic | 3.540 → 3.558 | +18 ms / +0.5% |
| Weak degree-three graph, automatic | 0.044 → 0.044 | 0 ms |
| Low FW, clustered, automatic | 0.203 → 0.207 | +4 ms / +2.0% |
| Small AW, automatic | 0.007 → 0.005 | −2 ms |
| Complete one FE IID, automatic | 0.567 → 0.571 | +4 ms / +0.7% |
| Unit, saved both FE, explicit CG | 2.921 → 2.933 | +12 ms / +0.4% |
| Low FW, clustered, explicit CG | 0.209 → 0.213 | +4 ms / +1.9% |

None crosses both 5% and 10 ms, so no extra repeats are triggered. The two
explicit-CG pairs address the fact that a result labeled CG can enter through
an already-budgeted automatic planner; they directly exercise changed adapters.
Full b/V/fit/certificate/sample/status/DoF/FE/cluster/subset/saved exports and
non-time solver fields are exact throughout. This does not claim every workload
has identical time or that the guard improves performance.

[Summary](summary.csv), [comparisons](cost-fix-results.json) and the
[checksummed archive](archive.json) preserve input/native/solver/fallback times,
iterations, scratch, native peaks, public base models and sampled process RSS.
Native peaks are exact except automatic low FW, down 4,608 bytes; that small
change is not localized or claimed as a saving. Public models and scratch remain
exact. Saved/low public base-model gaps persist. RSS moves both ways and includes
Stata/allocator retention; it is not equated with native live allocation.

## Separate saved-prototype diagnosis

This diagnostic is distinct from the adopted repair and the eight performance
pairs. It changes three files relative to unadopted saved/recovery candidate
`247f889a`, logging existing admission calls and directly timing its FE-scale
recovery planner. [Identities](diagnostic/built-identities.json),
[patch](diagnostic/diagnostic.patch), [protocol](diagnostic/protocol.md) and
[results](diagnostic/diagnostic-results.json) bind all three original requests.
All preserve exact scientific/solver exports against their previous captures.

| Original saved request | Public execution base | Initial joint admission check | Observed native peak | Recovery planner |
|---|---:|---:|---:|---:|
| Unit, both FE | 418.80 MB | 827.75 MB | 378.71 MB | 3.334 µs |
| PW, partial FE | 424.69 MB | 831.18 MB | 581.41 MB | 3.250 µs |

The PW continuation rechecks the same 831.18 MB bound after releasing Schur
workspace. The low-FW trace records 181.36 MB joint CMG construction checks and
16.85 MB retained checks, versus its 14.94 MB public base and 16.18 MB native
peak. Checked construction estimates are conservative reserves, not measured
allocations; their maxima are not added together across non-overlapping phases.
These traces establish the checks on these requests, not every solver path.

Full recovery takes 0.3847/0.6219 seconds in the unit/PW diagnostic. Its planner
alone takes microseconds and cannot directly explain the earlier 36–43 ms
command increases. This does **not** establish the cause of those binary-to-binary
costs or discard the flagged observations. Diagnostic command times are excluded
from ordinary performance aggregates. The initial collector searched `.log`
instead of `stderr.txt`; all original traces were collected afterward without
rerunning commands.

P0 public whole-action reporting/other-route accounting and P6 saved-Schur
adoption remain open. The direct-CG repair removes a concrete admission bypass;
it does not adopt the larger prototype. Next rebase that prototype on this
repair, verify its initial/final overlapping reserves, and compare its complete
saved-output cost against the repaired accepted baseline. The earlier incremental
cost signal remains a disclosed limitation to assess in that full comparison.

Replay scripts use named environment roots and the existing local comparison
harness; executed/archived hashes are in [script identities](script-identities.json).
The numeric ZIP deduplicates identical CSV content while retaining every run,
input hash and file mapping. Licensed logs, binaries, datasets and machine paths
remain outside Git. No agents, cluster work, dependency update or push occurred.
