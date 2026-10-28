# Third-party notices

## Stata plugin interface

`plugin/stplugin.c` and `plugin/stplugin.h` are the official Stata Plugin
Interface 3.0 sources published by StataCorp LP at the URL recorded in
`references.lock.toml`. Their original copyright notices are retained.
They are not relicensed by fereg.

## Pinned numerical dependencies

The MIT-licensed `within` and `schwarz-precond` runtime dependencies come from
[within](https://github.com/py-econometrics/within), developed by Alexander
Fischer, Kristof Schröder, and contributors at the appliedAI Institute for
Europe and in the PyFixest community. fereg uses version 0.4.0 from its
[adapted fork at `efe35ee`](https://github.com/johannes-schmieder/within/tree/efe35ee386ad989b043a5076d01d879ca410ff19).
The upstream library supplies LSMR, pairwise Schwarz preconditioning, local
approximate factorizations, and an adaptive preconditioner strategy. The fork
adapts borrowed data access, resumable iterations, bounded allocation,
reproducible reductions, and cancellation. fereg's adapters supply the model
data and independently check the returned weighted fixed-effect residuals
before accepting an estimate. The numerical methods and their sources are
acknowledged in the [README](README.md#selected-references).

The upstream [MIT license](https://github.com/johannes-schmieder/within/blob/efe35ee386ad989b043a5076d01d879ca410ff19/LICENSE)
retains copyright notices for within contributors and the appliedAI Institute
for Europe gGmbH. Preserve those notices with source distributions.

The GPL-3.0-only [CMG Rust port](https://github.com/johannes-schmieder/CMG) is
pinned to `f6a044c98ba00fddc82bee1800dccdb310b5a240` and statically linked for
graph-Laplacian/SDDM preconditioning and PCG. It derives from Koutis and Miller's
[original MATLAB/C implementation](https://github.com/ikoutis/cmg-solver/tree/19752fc102f8cae8e34f66457bfaccb1aaa60375)
of the method by
[Koutis, Miller, and Tolliver (2011)](https://doi.org/10.1016/j.cviu.2011.05.013).
Preserve its original license and notices. Exact current runtime pins are
recorded in [Cargo.lock](Cargo.lock) and
[the core crate manifest](crates/fereg-core/Cargo.toml).

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
Methods*, doi:10.1515/jem-2014-0008, is cited for the two-way block-elimination
method; no source code is adapted from that paper.

The `fe/borrowed_map.rs` path is native fereg code, not adapted within,
PyFixest or reghdfe source. Later source adaptation must identify the exact
upstream file/function, preserve its notice and add a regression for the
borrowed behavior. A repository-level attribution alone is insufficient.

## Rust standard library

Release plugins statically include ordinary Rust crate code and use standard
operating-system libraries. The user is not required to install Rust, Cargo,
Python, Julia, BLAS, OpenMP, or a separate native side library.
