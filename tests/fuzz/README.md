# Fuzz targets planned for Phase 0-1 handoff execution

The local Codex agent should add `cargo-fuzz` targets for raw ABI argument
vectors, key/value protocol parsing, model-spec table validation, numeric/string
factorization, and interaction tuples. Seed corpora are the files in
`tests/fixtures`. Fuzzing is a release gate once Rust is available; the
construction sandbox could not install cargo-fuzz.
