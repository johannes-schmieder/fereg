# Close the local-runtime allocation inventory gap

The all-action structural guard now watches locally vendored Rust runtime source
and exact build inputs. The accepted estimator is unchanged: all **259** package
inputs still match `c09eeab3…` / `161b9491`. Parent checkpoint is `76ec6485`.
This repairs a guard omission; it does not repair an estimator allocation or
complete the outstanding memory review.

## Evidence and implementation

The old action inventory covered core/plugin Rust files and `Cargo.lock`.
That is insufficient for the newly local Rayon patch: modifying the vendored
sleep implementation without changing the lock left a freshly rendered inventory
green. Changing the root Cargo manifest likewise escaped this companion guard
(the separate OLS guard already bound root Cargo wiring).

[Two red mutation tests](red.txt) independently reproduce these omissions.
`scripts/action_allocation_inventory.py` now adds:

- production-token, literal, module-binding and allocation-syntax records for
  `vendor/*/src/**/*.rs`, including the actual patched Rayon sleep module;
- exact-byte records for root Cargo/config/toolchain files and top-level
  crate/vendor manifests, locks and build scripts;
- specific added/deleted/changed/duplicated build-input errors; and
- continued source-drift reporting when an old inventory lacks new schema fields.

The observed schema is version 2. Source directories contain **180 Rust records**,
including **27 local runtime records**. Build products under vendor `target/`
directories are outside these patterns. Out-of-line test modules inside `src/`
remain conservatively inventoried; their syntax counts are not production byte
charges. Exact file records are in [review.json](review.json).

[Eight tests ran](after-tests.txt): seven pass, including the two formerly red
mutation tests. The existing `test_current_source_matches` still fails because
the checked-in inventory is stale. That assertion remains unchanged. Controls
cover added/deleted helpers and build inputs, unchanged-lock mutations, duplicate
records, excluded generated build products, preservation of old source-drift
errors, PPML helpers/literals and refusal to claim complete byte coverage.

The [before action audit](action-before.txt) has **56 findings**. The
[after audit](action-after.txt) retains all 56 and adds four schema/coverage
findings, for **60**. The [independent OLS audit](ols-before.txt) has **86 findings**;
its script and v39 manifest were not changed. The action v24 manifest is also
unchanged and its hash is bound in the review. **No successor inventory was
generated or adopted.** This intentionally preserves the requested drift failure.

## Limits and next boundary

These are structural checks on the specified local paths, not Cargo dependency
resolution, reachability, allocation-byte accounting, stack measurement or an
external dependency memory proof. Alternate source layouts and external path
dependencies require explicit review; changing their Cargo wiring is detected
but does not inventory their contents automatically. Full source/artifact
identity remains separately captured by the development measurement harness.

The stale successor review still needs to cover the accepted solver/preparation/
inference changes since `12de1adb`. The new guard ensures that such a review also
binds the local runtime rather than mistaking a Cargo lock for its identity.
Physical stack/cap accounting, constrained whole fits and opaque dependency
coverage remain open. No compilation, fit, profiling, benchmark, SCC submission
or qualification was performed for this guard change. Active timing jobs and
their controller bytes were untouched.
