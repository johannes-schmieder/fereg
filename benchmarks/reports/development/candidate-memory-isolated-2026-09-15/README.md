# Isolated native memory checkpoint

The source-bound accounting prototype passes **880 core, 96 plugin and 3 testkit
library checks** in SCC job 7586305 (failed=0, exit=0). The terminal sentinel,
compiler-reported test artifacts and their SHA-256 identities are checked.
This run uses its own Cargo target directory; no cached artifact from another
snapshot is selected. Earlier failed and shared-target receipts remain intact.

The snapshot adds PPML saved-effect recovery/normalization/output overlap to the
existing phase inventory, including actual worker count, all declared channels,
component normalization and slope workspaces. Focused checks cover exact and
one-byte-short budgets, allocated capacities, recovery failure, interruption,
repeated calls and the saved-output allocator test. Parent storage is charged
beside transient recovery storage; mutually exclusive stages use maxima.

This is a development checkpoint. Whole-action coverage, versioned public
reporting, original memory discrepancy reproduction and paired command costs
remain open. The source patch and compact carrier are archived here while the
larger prototype remains unadopted. It is not an exact-source candidate freeze,
Mac/Linux review package, performance qualification or release receipt.
