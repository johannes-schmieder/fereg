# Validated-artifact reuse: complete local input inventory

This harness repair makes future Linux artifact reuse reject incomplete package
inventories. It changes no estimator, scientific policy, frozen controller or
captured result. The live sweep and queued diagnostics remain on their existing
deployments; no build, fit or SCC job was added.

The earlier reuse setup checked every caller-listed file but did not establish
that the list covered the local package. It also compared a declared snapshot ID
to the validation receipt without recomputing that ID from the file map. The
current 259-file inventory is internally consistent and all its files still
match. However, it omits eleven local build/package inputs, including the C
bridge, linker exports, Cargo configuration, toolchain, reference lock and Linux
build script. `verification.json` preserves their names and the exact checks.

`artifact_source.py` now discovers package inputs, rejects omissions/additions,
checks the compact sorted file-map digest, and verifies every registered file in
both the origin and extracted candidate trees. Mandatory package entry points
must exist. Vendored inputs and optional explicitly registered fixtures are
binding. Generated build products and Git/Finder metadata are excluded; symlinks
in package trees, unreadable directories and external registered paths fail
closed. New frozen controllers bind this verifier's own bytes.

Seven focused tests pass, covering omitted bridge/build/vendor files, modified
content, a stale snapshot ID, added/missing source, optional registered inputs,
generated products, path boundaries and post-freeze verifier drift. Shell syntax
and diff checks pass. Tests run locally without scientific runtime dependencies.
No remote setup integration run or compiler/environment closure is claimed.

The new guard deliberately refuses the historical subset inventory. Do not edit
old receipts or append hashes while retaining an old validation snapshot. A new
reuse request needs a complete inventory captured before its build and bound by
validation, or separately reviewed original build evidence. The existing campaign
also binds its entire candidate archive and exact binary, which this finding
does not invalidate. Those independent bindings remain necessary: source equality
alone cannot prove what a compiler built.

All 259 accepted estimator inputs remain unchanged. Static allocation drift,
physical-memory accounting and the pending numerical/performance candidates
remain open. This is a provenance repair, not a performance improvement or
qualification result.
