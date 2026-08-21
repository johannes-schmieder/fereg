# Third-party notices

## Stata plugin interface

`plugin/stplugin.c` and `plugin/stplugin.h` are the official Stata Plugin
Interface 3.0 sources published by StataCorp LP at the URL recorded in
`references.lock.toml`. Their original copyright notices are retained.
They are not relicensed by StFixest.

## Frozen behavioral references

The repositories listed in `references.lock.toml` are pinned scientific and
behavioral references. PyFixest, reghdfe, and ppmlhdfe remain behavioral references only. The Rust
`within` crate and its transitive Rust dependencies are compiled statically from
the immutable revision recorded in `references.lock.toml`; `within` is MIT
licensed and its repository license is preserved by Cargo metadata and release
source distributions. No upstream source is copied into the StFixest tree. The
exact adapter mapping is documented in `docs/upstream_map.md`.

## Rust standard library

Release plugins statically include ordinary Rust crate code and use standard
operating-system libraries. The user is not required to install Rust, Cargo,
Python, Julia, BLAS, OpenMP, or a separate native side library.
