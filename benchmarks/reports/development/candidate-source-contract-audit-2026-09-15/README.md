# Candidate allocation-source inventory audit

The unchanged v17 structural guard fails on both exact baseline `9ecf47d`
(**80 distinct violations**) and native checkpoint `a765c33` (**87**).
The seven additional reported categories concern token or literal changes in
accelerated MAP, MAP, solver reports, topology finalization and slope MAP.
They reflect the implemented memory carrier and admission changes. Some further
changes fall within files that already violated the frozen inventory, so the
seven categories are not a complete diff of new code.

The guard, manifest and original F0 identities are unchanged. The failures are
preserved in [baseline.txt](baseline.txt) and [checkpoint.txt](checkpoint.txt);
[comparison.json](comparison.json) separates inherited and newly reported
categories. [Validation](validation.json) binds the checker and manifest.
The first baseline attempt used a build-only snapshot that lacked the manifest;
it was not a valid audit. One read-only retry used the exact Git source closure.

Before candidate freeze, review the allocation and one-call boundary changes,
verify the watcher covers the new memory helper modules, and create the next
version of the inventory with that review. Retain historical F0 bindings and
receipts. Merely regenerating hashes cannot demonstrate that allocation or
routing contracts are satisfied. The passing numerical/allocator fixtures do
not substitute for this static gate. No native behavior or guard policy changed
in this audit, and it is not formal qualification.
