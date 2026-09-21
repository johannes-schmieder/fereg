# Action lifetime ledger and retained FE parents

Uncommitted development checkpoint on main
`5240e589400c888690666265263ad96f9b694aa6`. **Implemented and unit-tested;
not adopted, packaged or qualified. Whole-action coverage remains incomplete.**
This extends the preceding construction/frame prototype without rerunning an
empirical estimation or the performance campaign.

## Implemented

The [patch](incremental-implementation.patch) introduces one fixed-size memory
ledger for both plugin actions. It sums simultaneous parent/workspace storage,
takes maxima across sequential frames, requires every applicable stage, and
keeps missing-owner bits through repeated or discarded attempts. An incomplete
model cannot supply a complete peak or ratio. Scalar ledger construction has
independent zero-allocation checks for both estimator adapters. The OLS
admission helper uses the same checked frame sum.

The opt-in `FEREG_ACTION_MEMORY_AUDIT` trace identifies itself as **precommit**.
It contains modeled publication storage but has not observed publication finish;
it is neither a COMPLETE receipt nor a whole-command allocation measurement.
Public base-memory fields and wire versions remain unchanged. The exhaustive
owner-family obligations and their source locations are recorded in
[allocation_inventory.md](../../../../docs/allocation_inventory.md#action-lifetime-ledger-and-proof-boundary).

Two retained-product boundaries now have explicit parent charges:

- Fast fits reserve the initial all-RHS FE product and the combined one-RHS
  product before creating a retained solver session. The reservation uses
  prepared levels/channels, includes nested layout arrays, and is checked for
  overflow. The residual session inherits that budget. Its reported peak is
  translated back to the original execution base with the same parent once.
- Uncached robust/clustered score reconstruction charges the actual capacity
  of the final FE product that remains live beside its next solver. Cached
  scores and retained scratch keep their existing admission.

These are conservative parent reservations, not measured byte savings or proof
that the old whole-action cap was exceeded. The initial-fit reservation can be
conservative while the initial solver constructs its own product. Tight
budgets can reject earlier. Unknown solver reports remain unknown. Floating-
point operations, samples, weight semantics and numerical policies are unchanged.

## Validation and identities

[Source verification](source-verification.json) and
[remote hashes](native/remote-verification.json) bind 290 files per snapshot,
including dependency/reference locks. Inputs are synthetic fixtures contained
in those frozen Rust sources.
Snapshot `5240e58-lifetime-ledger4` passes **121 plugin unit/mock-SPI tests**
with the trace disabled and the same **121 tests** with it enabled. They include
allocation failures, interruption, repeated calls, saved outputs and rollback.
The enabled pass emits [930 valid ledger records](ledger-trace-validation.json):
362 OLS and 568 PPML. All required stages are present, all parent/workspace
sums and sequential maxima agree, and every incomplete ratio is undefined.
These are test invocations, not 930 independent scientific/performance cases.

The final core snapshot `5240e58-lifetime-ledger6` passes **44 retained-state
checks plus two score-reconstruction checks**, with one overlapping test: 45
distinct core tests. The new fit check accepts the exact modeled joint budget,
rejects one byte below it and below the retained parent, and preserves exact
coefficients, residuals, retained products and workspace diagnostics. The
inference check rejects below its retained parent and preserves exact
coefficients, residuals and covariance when its modeled solver envelope fits.
The unit scalar model checks layout arithmetic and overflow. Existing retained
PPML/session/recovery controls also pass.

Only test bodies in `prepared_fit.rs` and `prepared_robust.rs` differ between
the passing plugin and final core snapshots; production tokens and literals
match. Current source matches the final snapshot. These are fresh isolated
Cargo targets, not reused installed artifacts. Native test executable SHA-256s:

- Plugin: `02b14d17f9ae20ba2dda81d8fb104667876271821254766655bedb61e0a205f2`.
- Core: `a36acfa3abebe136e4fa475817457b2ba7ffc76bb3e228d3291cd30e97c73263`.

The [38 Python checks](checks/python-final.txt) pass, including source mutation
and incomplete-coverage guards. OLS source inventory v24 and action inventory
v7 preserve historical manifests and F0 boundary pins. Repository and whitespace
checks are recorded separately. No licensed Stata, release-plugin ABI, Mac,
Rosetta or package-install run is claimed for this change.

## Preserved failures and job accounting

All [job receipts](job-results.json), [native logs](native/) and original
log hashes are retained. Source-root/toolchain paths in the review copies are
normalized; originals remain in the development run directory and local
collection, with hashes in [original-log-hashes.json](original-log-hashes.json).

| Job | Disposition |
| --- | --- |
| 7590631 | Compile failure: new helper accessed a private topology field; corrected to its accessor. |
| 7590639 | New fixture omitted its regressor-status entry; 42 other retained-state checks passed. |
| 7590645 | New tests selected a legacy CG path that does not retain products; plugin tests also exposed a missing test-field initializer and import. |
| 7590673 | Both 121-test plugin passes succeeded; the job remains failed because the new core fixture used the wrong exported preparation namespace. |
| 7590731 | Core fixtures assumed a one-FE shape in a two-/three-FE helper and a known CG report without supplying its required memory budget. These assumptions were corrected; unknown reporting was preserved. |
| 7590739 | Final core checks pass, `CORE_LIFETIME_NATIVE_PASS`, scheduler failed=0/exit=0, 73 seconds and 3.459G maxvmem including compilation. |

The initial Python run used system git blocked by the local Xcode-license
prerequisite, and overlapped edits/manifest generation. Its failures are retained;
the serialized source-stable rerun uses Homebrew git and passes. The initial
repository invocation hit the same system-git prerequisite. No science tolerance
or accepted fixture was weakened to make the new checks pass.

## Remaining gates

This change does **not** clear whole-action coverage. Outstanding proofs include
transport/compiler and string-dictionary construction, complete finite/PPML
parent and generation overlaps, dependency container capacities and cold/retiring
worker pools, saved-output cases and terminal publication observation. The new
ledger names these omissions; it does not erase them based on a passing sample.

No new command-time comparison was run, no speedup is claimed, and the preceding
saved-slope low-memory timing flag remains unresolved. Full action RSS/scratch,
discarded-attempt costs and the original German/three-FE reproductions are not
requalified here. The installed Mac artifact remains
`4187efdfbd6235e63c369d9b42dede6381b8a9e50b5360fd63ee237bd6c11b08`.
Mac linking is still blocked by the Xcode licence prerequisite. Freeze, Mac/Linux
review packaging and installation gates remain open. No commit, push, tag,
release or installed-package replacement was performed.
