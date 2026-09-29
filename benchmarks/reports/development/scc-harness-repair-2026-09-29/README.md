# SCC harness repair and pending-group continuation — 2026-09-29

This bounded follow-up repairs three operational defects, reruns the affected
validation stages once, and continues only the 27 untouched benchmark groups.
Production estimator code, candidate patches, scientific tolerances, samples,
reference pins and the original 77/16/111 definitions are unchanged. The jobs
use the frozen **c09eeab3… / 161b9491** package and **59d0341a…** Linux baseline;
they do not measure the newer bias-support/reconstruction changes on local main.
The separate empirical/Separations work is outside this follow-up.

## Original outcomes preserved

[original-outcomes.json](original-outcomes.json) binds terminal accounting,
deployments and logs for the five first diagnostic attempts, plus the selected
comparison, gtools and memory receipts. No historical receipt was rewritten.

- **7767016 / 7767319:** neither test executable built. The copied package
  inventory omitted `tests/fixtures/ppml/reconstruction.csv`, which the core's
  tests include at compile time. These were deployment failures, not numerical
  results for either candidate.
- **7766146:** all 12 PyFixest applications completed, and the four gtools checks
  passed, but the harness looked for `gtools_sample.log` instead of the actual
  `gtools.log`. Its failed scheduler outcome remains recorded.
- **7767737:** an unhandled 30-second `qacct` timeout stopped public validation
  before candidate acceptance gates.
- **7767269:** the memory diagnostic completed. The identical input and baseline
  plugin finish in 19.218 process seconds under a 448-GiB admission limit, using
  LSMR and 3.53 GiB sampled process-tree RSS. Under a 192-GiB limit, estimation
  reaches the 180-second process cap with 2.61 GiB sampled RSS. Limits restrict
  the available-memory frame; they are not observed consumption. This supports
  the conservative-admission explanation but is not a candidate timing result.

The first sweep's four tasks have clean accounting. Task 1 checkpointed at
39,127 controller seconds; tasks 2–4 completed their assigned groups. The frozen
controller audit finds **541/568 groups complete, 27 missing and no integrity
errors**. Every completed input receipt matches the original campaign exactly.
Eight application timeouts and 146 scientific discrepancies remain visible.

## Narrow changes and local checks

The two core deployments now stage the exact registered CSV and validate its
hash. A check of literal `include_str!`/`include_bytes!` paths in the core rejects
missing or external compile-time inputs before compilation. The source identities
include the fixture; the Schwarz identity also now includes its staged toolchain
file. Candidate production patches retain their original hashes.

The comparator validator reads the actual log, requires one standalone terminal
marker, rejects uncaught Stata return-code lines and checks all four exact
sample/count/coefficient/covariance rows plus the empty-sample boundary.
The public gate allows three accounting queries of at most 60 seconds, within
its existing controller deadline. Query timeouts/nonzero exits remain pending
rather than accepting a candidate; exhausted queries refuse the arm. Failed or
malformed accounting still fails closed. Attempt receipts retain timeout evidence.

[Six local repair tests](local-repair-tests.txt) pass, including missing/mutated
fixtures, external includes, missing/duplicate/error log markers, failed or absent
sample checks, timeout-then-success and exhausted/failed accounting, and rejection
of changed/repeated/lost continuation groups. The four existing CG parser tests
and six public-gate tests also pass. Python AST, shell syntax and exact deployed
helper bindings pass; [local-bindings.json](local-bindings.json) records the latter.

## Reruns

| Stage | Job | Resources | Result |
|---|---:|---|---|
| Nonretaining CG | 7778963 | 8 slots, 3G/core, 1 hour | Core pass |
| Schwarz admission | 7778964 | 8 slots, 3G/core, 30 minutes | 119/120 tests pass; gate refuses |
| Comparator state | 7778965 | 4 slots, 3G/core, 30 minutes | Diagnostic pass |
| Public candidate science | 7778967 | 16 slots, 3G/core, 45 minutes | Eight existing fixtures pass; new oracle fails |
| Pending benchmark groups | 7778971 | 28 E5-2680v4 cores, 3G/core, 3 hours | Submitted; completion collected separately |

The three initial diagnostics may run concurrently. Public validation is held
behind all three, and benchmark continuation behind all four, preventing overlap
between these builds/diagnostic fits and headline timing. Submission receipts,
new registrations and deployment hashes are under [deployments/](deployments/).
The new helpers are frozen under [harness/](harness/); unchanged assets retain
hash-bound copies in the original registration reports and SCC attempt roots.

[CG evidence](cg-results.json) records baseline failures on **14/72** known-within
cases, **72/72** candidate cases passing, and all ten controls passing on both
arms. This is untimed core evidence; it does not adopt the patch or establish a
public performance benefit.

[Schwarz evidence](schwarz-results.json) records the one failed exact control:
`initial_full_width_lsmr_preserves_canonical_multiway_rank_policy`. Its assertion
finds `fallback_used=false` while the reported solver is MAP. The other 119 tests
pass, including all three new memory-bound tests. The failure is not waived or
attributed to this patch without a baseline adjudication. This candidate remains
ineligible for public validation and production adoption.

[Comparator evidence](comparator-results.json) records 12 successful applications,
independent oracle completion, and the passing gtools fixture with exact samples
and unchanged b/V. Default-profile accuracy failures remain separate from
application success. Reconstructed current-weight covariance is diagnostic and
never substituted into stock timing results.

[Public evidence](public-results.json) records a successful candidate plugin
build, binary/ABI checks and all eight existing licensed fixtures passing. The
new near-within oracle returns Stata r(9), an assertion failure whose exact
loop/check is not identified by the untraced log. The unchanged baseline fails
earlier with r(459), reporting negative borrowed-CG Hestenes improvement potential.
No tolerances or assertions were changed, and no failed fixture was automatically
rerun. The CG patch therefore remains unadopted despite its core improvement.
The Schwarz arm is refused before building because its core evidence fails.
The accounting-query repair worked; the job reached its complete terminal marker
and exited two because neither arm passed all public gates. These are validation
outcomes, not a second accounting/deployment failure.

Diagnostic accounting: CG 130 s / 5.901G, Schwarz 101 s / 6.159G and comparator
151 s / 1.436G scheduler maxvmem. These are job resource observations, not solver
performance comparisons. The bounded retests are complete; further candidate
work must first localize the two remaining assertions without weakening them.

## Exact continuation

[Preflight](continuation-20260929-preflight.json) binds the original accounting,
541 completed marker hashes and original input identities. The
[continuation plan](production-continuation-20260929.json) contains exactly the
27 checkpointed groups / 81 applications. Every per-group field is copied
unchanged from the original submitted plan, including command caps, seeds,
threads and skips. No completed failure, discrepancy or timeout is rerun.
Only bundle membership and its wall limit change. The original forecast for
these groups is 1.183 node-hours; this is not a measured completion time.

[Submission](continuation-20260929-submission.json) binds plan SHA-256
`bf9177e7267dcae1752cdbdb4c4478fb46136f3f889c576bf799fbbea9c6029e`.
The unchanged measurement fingerprint is
`fb2ad5407afcea22d56155aa7858203bfcaea590f750ca5d06eeccce746cffc6`.
The existing frozen controller and stock libraries execute the continuation.
Final collection must verify clean accounting, the exact complete bundle,
unchanged earlier markers, all 568 original input identities and the complete
frozen-controller audit before reporting full distributions.
