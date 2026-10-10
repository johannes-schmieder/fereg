# Third-party notices

## Stata plugin interface

`plugin/stplugin.c` and `plugin/stplugin.h` are the official Stata Plugin
Interface 3.0 sources published by StataCorp LP at the URL recorded in
`references.lock.toml`. Their original copyright notices are retained.
They are not relicensed by fereg.

## Pinned numerical dependencies

The MIT-licensed `within` and `schwarz-precond` runtime dependencies are pinned to
`803a4feacef526f55e1520f22be4409779b7d8ff`. The native LSMR adapters map fereg FE
views to upstream operations; fereg independently recomputes the final weighted
FE certificate. Preserve the upstream MIT notices with source distributions.
`fe/lsmr.rs` maps each native FE term to a `within` effect and copies back only
the demeaned observation vectors; recurrence convergence alone is never
accepted, because `fe/certificate.rs` recomputes `D'Wr` before an estimate
proceeds. No upstream source is copied or modified.

CMG is GPL-3.0-only, pinned to `1d39c113f0c397f29653d92672090e5a42f80921` and
statically linked for graph-Laplacian/SDDM preconditioning and PCG. Preserve its
original license and notices. Exact runtime pins also appear in Cargo.lock and
references.lock.toml.

Johannes approved GPLv3 as the distribution direction. Project license metadata
and corresponding-source packaging must be migrated together before publication.
The [licensing decision](docs/licensing.md) retains the unresolved Stata/SPI
native-linking gate. A change to fereg's license cannot grant exceptions for
third-party code; native release needs the necessary rights-holder permissions.

## Frozen behavioral references and provenance

`references.lock.toml` records exact source identities, relevant upstream
paths and licenses for PyFixest, reghdfe, ppmlhdfe and comparator dependencies.
PyFixest maps model-matrix, OLS/PPML and test behavior; reghdfe is the
Stata syntax, FE/singleton/DoF/VCE oracle; ppmlhdfe is the PPML sample,
separation and inference oracle. They are not called by fereg at runtime.
The independent explicit-dummy oracle is development-only.

Pinned offline reference sources and required files remain independently bound.
The digest-bound ppmlhdfe archive is combined outside the repository with the
exact pinned reghdfe, ftools and require trees, and qualification receipts must
bind both the candidate source and that comparator closure. A floating checkout
or preinstalled ado package does not establish identity. Independent Poisson
and recession-cone calculations supplement reference parity, since comparator
agreement alone is not scientific authority. Somaini and Wolak (2016), "An
Algorithm to Estimate the Two-Way Fixed Effects Model", *Journal of Econometric
Methods*, doi:10.1515/jem-2014-0008, is literature background only; no code is
adapted.

The `fe/borrowed_map.rs` path is native fereg code, not adapted within,
PyFixest or reghdfe source. Later source adaptation must identify the exact
upstream file/function, preserve its notice and add a regression for the
borrowed behavior. A repository-level attribution alone is insufficient.

## Rust standard library

Release plugins statically include ordinary Rust crate code and use standard
operating-system libraries. The user is not required to install Rust, Cargo,
Python, Julia, BLAS, OpenMP, or a separate native side library.
