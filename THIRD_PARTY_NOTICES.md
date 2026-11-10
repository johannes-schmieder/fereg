# Third-party notices

## Stata plugin interface

`plugin/stplugin.c` and `plugin/stplugin.h` are the official Stata Plugin
Interface 3.0 sources published by StataCorp LP at the URL recorded in
`references.lock.toml`. Their original copyright notices are retained.
They are not relicensed by fereg.

## Modified LSMR engine (moved MIT source)

fedemean's `lsmr` module (`crates/fedemean/src/lsmr.rs` and
`crates/fedemean/src/lsmr/`) is the modified LSMR engine of the MIT-licensed
`schwarz-precond` crate from
[within](https://github.com/py-econometrics/within), developed by Alexander
Fischer, Kristof Schröder, and contributors at the appliedAI Institute for
Europe and in the PyFixest community. It was moved into fedemean on
2026-11-09 from version 0.5.0 at the
[adapted fork's revision `8d401d82`](https://github.com/johannes-schmieder/within/tree/8d401d8278ac783b2f258c459e057f30cd80d595),
with its unit tests and without the crate's Schwarz preconditioner; the
lockstep multi-column solve (`mlsmr_columns`) was developed on that fork
afterwards, never published, and moved in with it. The fork adapted
resumable iterations, bounded allocation, reproducible reductions and
cancellation. The engine runs fedemean's full-system LSMR and fereg-core's
PPML joint LSMR; fereg's adapters supply the model data and independently
check the returned weighted fixed-effect residuals before accepting an
estimate. The numerical methods and their sources are acknowledged in the
[README](README.md#selected-references). Neither `schwarz-precond` nor the
repository's `within` crate is a fereg dependency, and no locked package
resolves from that repository.

The moved files keep the upstream copyright notices. The following MIT grant
covers them; preserve it with source and binary distributions.

```text
MIT License

Copyright (c) 2026 within contributors
Copyright (c) 2026 appliedAI Institute for Europe gGmbH

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

## Pinned numerical dependencies

The MIT-licensed `approx-chol` crate, version 0.6.0 from crates.io, supplies
the approximate Cholesky factorizations of fedemean's Schwarz local solves.
Its license and copyright notices travel with the vendored dependency sources
of source distributions.

The MIT-licensed [CMG Rust port](https://github.com/johannes-schmieder/CMG) is
pinned to its published [v0.3.0 release](https://github.com/johannes-schmieder/CMG/releases/tag/v0.3.0)
(`368eceba765db91ff818815698da35889ce42ed0`) and statically linked for
graph-Laplacian/SDDM preconditioning and PCG. It derives from Koutis and Miller's
[original MATLAB/C implementation](https://github.com/ikoutis/cmg-solver/tree/19752fc102f8cae8e34f66457bfaccb1aaa60375)
of the method by
[Koutis, Miller, and Tolliver (2011)](https://doi.org/10.1016/j.cviu.2011.05.013).
Preserve its copyright and license notices, including the MIT grant below. Exact current runtime pins are
recorded in [Cargo.lock](Cargo.lock) and
[the core crate manifest](crates/fereg-core/Cargo.toml).

fereg is distributed under MIT. On 2026-10-13 Johannes confirmed
that the required native-distribution permissions had been obtained; see the
[licensing decision](docs/licensing.md). Third-party sources retain their own
copyrights and licenses. The matching source archive includes the pinned Cargo
dependency sources, their license files, and an offline Cargo configuration.

## Rayon

The local `vendor/rayon-core` fork and the upstream Rayon dependencies are
available under MIT OR Apache-2.0. Preserve both license files in source
packages; the local fork's notices are in `vendor/rayon-core/LICENSE-MIT` and
`vendor/rayon-core/LICENSE-APACHE`.

## CMG MIT permission and notice

On 2026-11-03 Johannes Schmieder confirmed that CMG's developers had granted
official permission, communicated through Ioannis Koutis, to release CMG under
the MIT license. Johannes selected MIT for CMG, fevc, and fereg on that basis.

The following MIT grant covers the pinned CMG Rust source identified above,
including the CMG code incorporated into fereg.

```text
MIT License

Copyright (c) 2008-2020 Ioannis Koutis and Gary Miller
Copyright (c) 2026 Johannes Schmieder and contributors

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
