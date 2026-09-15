# Review-only packaging implementation

Seventeen focused Python tests pass. A Mac/Linux subset is accepted only in
explicit review mode and carries a schema-3 manifest, Windows missing, and
publication eligibility false. Default production staging still requires all
three binaries. Tests check repeated deterministic archive bytes, extracted
inventory/checksums, unsafe members, tampering, dirty-source rejection and
review-boundary rejection. Plugins in these tests are synthetic fixtures.
No real candidate package or licensed clean-install result is claimed here.
Exact implementation hashes are in `validation.json`; usage is documented in
[release_process.md](../../../../docs/release_process.md).
