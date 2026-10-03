# ADR 0248: Scientific acceptance, exact-change checks and bounded work

**Status:** Accepted by Johannes, 2026-10-02 (approved plan implementation).

## Decision

Use independent oracles to decide correctness, and comparator binaries to find
changes. The initial one-thread deterministic copy must be bit-identical on
the committed replay corpus. No-numerical-change commits preserve scientific
bits and available route signatures versus their parent under equal build
settings. Numerical fixes declare affected cases/fields before changing them;
unaffected outputs remain exact. Exclude environment/build diagnostics only.
Intentional legacy/reference deviations have named executable expectations.

Cross-thread bit identity is not required. Preserve fixed-thread deterministic
repeatability and seeds, scientific tolerances and stable discrete decisions.
Keep stronger named scientific regressions and nonunique-FE identified checks.

Build real OLS/PPML binary replay in M0, runnable in Linux CI without Stata.
Commit a bounded feature/test matrix before fixes; add tests for gaps or actual
regressions. Local checks follow task risk: runtime changes require the full
entrypoint and mapped science; docs use static checks; build tools use focused
checks plus native ABI qualification. Full push CI remains mandatory. Reuse
rollback, allocation and Break tests. Coverage has no percentage goal.

A failed/unverified prerequisite blocks dependent work. After three unsuccessful
substantive repairs or one working day on a blocker, ask Johannes with evidence.
Keep one ledger: fewer than 300 lines, each at most 200 characters.

Ratchet production lines per runtime crate, functions over 150 lines, files
over 1,500 and N/thread/weight/memory routing conditions. Growth requires a
narrow checked-in reason/scope; decreases lower the baseline. Do not waive by
silent refresh, minification or moving code outside counted paths. Inherited
violations do not require unrelated rewrites; aggregate line ceilings are gone.

## Consequence

Exact checks establish the change's scope, not legacy's scientific authority.
Bit differences are localized rather than automatically accepted by a tolerance
pass. Missing evidence never becomes a passed gate; independent work may proceed.
