# Third-party notices

## Stata plugin interface

`plugin/stplugin.c` and `plugin/stplugin.h` are the official Stata Plugin
Interface 3.0 sources published by StataCorp LP at the URL recorded in
`references.lock.toml`. Their original copyright notices are retained.
They are not relicensed by StFixest.

## Frozen behavioral references

The repositories listed in `references.lock.toml` are pinned scientific and
behavioral references. No source from PyFixest, within, reghdfe, or ppmlhdfe is
copied into this Phase 0–1 package. Later adaptations must retain the applicable
MIT notices and document the exact source-to-module mapping.

## Rust standard library

Release plugins statically include ordinary Rust crate code and use standard
operating-system libraries. The user is not required to install Rust, Cargo,
Python, Julia, BLAS, OpenMP, or a separate native side library.
