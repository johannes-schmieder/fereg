# Frozen native comparator

`baseline.json` pins `legacy-comparator-v1`, the complete working source at
15a57ba285a3afede2e3337fe8598e72529a4090, including the vendored Rayon repair.
It records canonical artifact and metadata hashes after qualification. The
core/testkit directories move into active crates in M0.3; Git retains the full
frozen source. Legacy results are comparison evidence, not scientific authority.

Use `scripts/legacy_comparator.py fetch --platform PLATFORM --directory DIR`
to obtain a registered binary. It verifies hashes, never builds on cache miss,
and rejects incomplete or altered caches. Draft assets require authenticated
GitHub access; the existing licensing gate still controls public distribution.
An agent without access can run independent local checks but cannot claim the
binary comparison gate passed.

The one-time factory command is `build --platform PLATFORM --directory NEW_DIR`.
It requires the pinned toolchain and local immutable tag, archives only from
that revision, builds in isolation and runs the existing native audit/mock
checks. It does not register its result automatically. `register` refuses to
replace a canonical identity. A diagnostic rebuild is not a canonical substitute.

The manual comparator workflow builds Linux glibc 2.28 and Windows x86-64;
the licensed Mac supplies the universal macOS comparator. Qualified assets are
stored on the tag's draft release without clobbering existing files. Registration
is a reviewed repository change; freeze source and artifact identities together.

The Stata comparator is the frozen binary plus current shared ado files. Record
both identities and preserve the native wire protocol. CI replay bypasses ado
and exercises the actual plugin through the mock-SPI loader.
