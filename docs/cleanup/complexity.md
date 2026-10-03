# Runtime complexity inventory

## Version 1

`scripts/check_complexity.py` follows Rust module declarations from core and
plugin `src/lib.rs`, retaining every non-test platform/feature variant. The
existing allocation guard's lexer removes comments and provably test-only
items. Unreachable test files, testkit/oracle, generated and vendored code are
outside this runtime inventory; moving runtime work there is forbidden.
Unsupported module syntax fails rather than silently omitting code.

- Production lines are physical lines containing retained tokens. Multiline
  literals count their occupied lines. Blank/comment/test lines do not count.
- Function size is physical lines from `fn` through its closing brace, including
  intervening comments/blank lines. Duplicate names use occurrence numbers.
- File limits use production lines. New functions <=150 and files <=1,500;
  inherited larger items are individually pinned and may shrink.
- Dispatch candidates are lexical `if`/`match` conditions containing N, row,
  column, width, thread, worker, weight, memory, budget, capacity, length, size,
  fast or low identifiers (the script defines exact underscore-delimited terms).
  This conservative inventory includes validation branches; it is not a claim
  that every candidate selects an algorithm. Conditions mentioning chunk, tile,
  partition or grain identifiers form a separate kernel-candidate count. Both
  counts ratchet; review still decides whether a proposed branch is a route.
  Dataflow/aliases are not inferred. Renaming inputs, minifying or recategorizing
  conditions to evade the guard is forbidden.

The checked-in JSON contains reproducible measurements and source locations.
There is no percentage, aggregate line ceiling or automatic rewrite target.
Counts describe maintainability debt; they do not judge scientific correctness.

## Updating the ratchet

Run `python3 scripts/check_complexity.py --write-inventory /tmp/inventory.json`
to inspect changes. Deliberately update `inventory` in `complexity.json` to the
reviewed measurement. Reductions lower the baseline; stale baselines fail.
For growth, `growth` must list each increased metric's exact `metric`, `before`,
`after` and nonempty `reason`, naming the bounded change and why it is needed.
New size violations cannot be grandfathered this way. Entries describe this
transition only; replace them on the next inventory change, never accumulate
permanent allowances. Before committing, check with `--base HEAD`; CI compares
with `HEAD^` (checkout depth >=2). Growth is never approved by inventory output.
Changing definitions requires a reviewed policy decision and a new version.

Coverage uses a separate LLVM-instrumented build through
`scripts/coverage_baseline.py`. Detailed export/HTML remains an artifact under
`target/coverage-baseline`; compact summaries belong in `bench/history`.
Instrumentation must never provide timing evidence or replace release replay.
