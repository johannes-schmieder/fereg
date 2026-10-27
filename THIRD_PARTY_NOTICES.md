# Third-party notices

## Stata plugin interface

`plugin/stplugin.c` and `plugin/stplugin.h` are the official Stata Plugin
Interface 3.0 sources published by StataCorp LP at the URL recorded in
`references.lock.toml`. Their original copyright notices are retained.
They are not relicensed by fereg.

## Pinned numerical dependencies

The MIT-licensed `within` and `schwarz-precond` runtime dependencies are pinned to
`297f32d9c4f83cc7d814ea778cd28c1f976e68d0`. The native LSMR adapters map fereg FE
views to upstream operations; fereg independently recomputes the final weighted
FE certificate. Preserve the upstream MIT notices with source distributions.
`fe/lsmr.rs` maps each native FE term to a `within` effect and copies back only
the demeaned observation vectors; recurrence convergence alone is never
accepted, because `fe/certificate.rs` recomputes `D'Wr` before an estimate
proceeds. No upstream source is copied or modified.

CMG is GPL-3.0-only, pinned to `f6a044c98ba00fddc82bee1800dccdb310b5a240` and
statically linked for graph-Laplacian/SDDM preconditioning and PCG. Preserve its
original license and notices. Exact runtime pins also appear in Cargo.lock and
references.lock.toml.

fereg is distributed under GPL-3.0-only. On 2026-10-13 Johannes confirmed
that the required native-distribution permissions had been obtained; see the
[licensing decision](docs/licensing.md). Third-party sources retain their own
copyrights and licenses. The matching source archive includes the pinned Cargo
dependency sources, their license files, and an offline Cargo configuration.

## Rayon

The local `vendor/rayon-core` fork and the upstream Rayon dependencies are
available under MIT OR Apache-2.0. Preserve both license files in source
packages; the local fork's notices are in `vendor/rayon-core/LICENSE-MIT` and
`vendor/rayon-core/LICENSE-APACHE`.

## Earlier fereg source notice

The following notice is retained for fereg code previously offered under MIT.
The current project distribution uses GPL-3.0-only; this does not revoke the
license grants made by earlier revisions.

```text
MIT License

Copyright (c) 2026 fereg contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

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
