# Fuzz qualification for fereg 0.9.0

The `fuzz/` package provides five bounded `cargo-fuzz` targets:
`raw_abi_args`, `key_value_protocol`, `model_spec_tables`, `factorization`,
and `interaction_tuples`. Seed corpora are the files in `tests/fixtures`.

Run each target with a finite qualification budget and record the exact target
name, Rust/cargo-fuzz versions, duration or run count, corpus hash, and outcome.
The `raw_abi_args` target begins at the safe action dispatcher immediately
after the already unit-tested C argv conversion and panic boundary; it excludes
the deliberately crashing internal `panic-test` action.

Record toolchain, target, duration, corpus hash, and outcome for every 0.9.0
qualification run. Where `cargo-fuzz` or a supported sanitizer is unavailable
on a host, record the gap and run the applicable Miri, property-test, and
adversarial protocol suites instead; do not silently claim the fuzz gate.
