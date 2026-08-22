# Remote CI

Rust compilation for `chatgpt/rust-completion` is performed by GitHub Actions in `johannes-schmieder/playground_stfixesf` because the current execution environment has no local Rust toolchain.

`StFixest` remains the authoritative repository. Each mirror checkpoint records the exact `StFixest` commit in `.ci/source.json` and runs the Rust matrix on Ubuntu, macOS, and Windows:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked`
- `cargo test --workspace --all-targets --locked --release`
- `cargo build --workspace --locked --release`

The mirror writes `.ci/latest.json`, with `.ci/**` excluded from push triggers. A result is accepted only when the source SHA, mirror SHA, workflow run, and all matrix outcomes agree.

GitHub Actions evidence covers Rust compilation and tests. It does not replace licensed-Stata parity or activation gates.
