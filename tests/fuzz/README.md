# Fuzz qualification for fereg 0.9.0

Use `cargo-fuzz` targets for raw ABI argument vectors, key/value protocol
parsing, model-spec table validation, numeric/string factorization, and
interaction tuples. Seed corpora are the files in `tests/fixtures`.

Record toolchain, target, duration, corpus hash, and outcome for every 0.9.0
qualification run. Where `cargo-fuzz` or a supported sanitizer is unavailable
on a host, record the gap and run the applicable Miri, property-test, and
adversarial protocol suites instead; do not silently claim the fuzz gate.
