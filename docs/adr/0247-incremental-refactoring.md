# ADR 0247: Working-copy refactoring with a frozen native comparator

**Status:** Accepted by Johannes, 2026-10-02 (approved plan implementation).

## Decision

Abandon the unfinished clean-core rewrite. Move the complete working core and
testkit into active crates with git mv; retain the working host for next.
Initially preserve code, dependencies, routing, options and numerical behavior.
Next defaults both commands; a separate frozen legacy plugin remains bundled.
Next keeps its existing filename and __fereg_plugin handle; legacy receives a
separate name and handle. Engine selection never triggers cross-engine fallback.

Freeze revision 15a57ba285a3afede2e3337fe8598e72529a4090 under the immutable
legacy-comparator-v1 tag. Store qualified per-platform binaries and hashes as
canonical tag assets, initially authenticated draft assets pending G-LICENSE.
Cache by hash; never substitute a rebuilt binary for a missing canonical file.
The complete source/build closure includes the later vendored Rayon repair.

Legacy means frozen plugin plus current shared ado. Keep its protocol working.
Prefer next-only Rust scientific fixes; test any necessary next-specific
frontend adjustment against unchanged legacy command behavior. Legacy and
reghdfe are comparisons, not reasons to copy bugs or reject simplification.
Full-matrix OLS multiway covariance repair, including the constant, is approved.

## Scope

No automatic legacy retirement, new Schur default, one-mode migration or public
option cleanup in M0–M1. Preserve supported LSMR/CMG, both bias corrections and
existing CG rejection/CMG repair. Old rebuild-only callback APIs/pins are not
requirements. Keep the existing GPLv3 direction and native-distribution gate.
Scientific and transaction requirements remain; historical ADRs do not bind.
