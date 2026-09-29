# Third-party notices

## Stata plugin interface

`plugin/stplugin.c` and `plugin/stplugin.h` are the official Stata Plugin
Interface 3.0 sources published by StataCorp LP at the URL recorded in
`references.lock.toml`. Their original copyright notices are retained.
They are not relicensed by fereg.

## Pinned numerical dependencies

The Rust `within` dependency is MIT licensed, pinned to
`1ac3522a251c2622dbeab451415924f9ebe117b9` in
`references.lock.toml`. It supplies modified LSMR and Schwarz preconditioning,
not another public solver family. The legacy adapter in
`crates/fereg-core/src/fe/lsmr.rs` maps native `EffectTerm` values to
`within::Effect`, calls `within::solve_batch`, and copies returned demeaned
vectors into fereg storage. Prepared adapters reuse native categorical views.
`fe/certificate.rs` independently recomputes the weighted FE certificate;
the upstream recurrence alone is not scientific acceptance. No within source
is copied or modified in this tree. Its original MIT notice must accompany
the corresponding source distribution.

CMG is GPL-3.0-only, pinned to
`1d39c113f0c397f29653d92672090e5a42f80921`. It is statically linked by the
legacy `fe/cmg.rs` adapter for graph-Laplacian/SDDM preconditioning and PCG;
that adapter uses prepared Laplacian topology, hierarchy/workspace construction
and caller-buffer `solve_pcg_batch_*` interfaces. PPML's
`ppml/categorical_fwl_cmg.rs` shares this graph adapter and refreshes numerical
state with changing weights. Preserve CMG's original license and notices.
The clean engine retains both pinned dependencies through narrow adapters.

Johannes approved GPLv3 as the project distribution direction. Project license
metadata and corresponding-source packaging are migrated together at M7.3.
This notice does not assert that the Stata/SPI native-linking questions are
resolved: [rebuild ADR 0008](docs/cleanup/rebuild-adr/0008-dependencies-licensing.md)
records the unpassed G-LICENSE gate. Changing fereg's own license cannot grant
an exception for dependency or StataCorp code. Native-plugin release requires
the documented disposition and any necessary rights-holder permissions.

## Frozen behavioral references and provenance

`references.lock.toml` records exact source identities, relevant upstream
paths and licenses for PyFixest, reghdfe, ppmlhdfe and comparator dependencies.
PyFixest maps model-matrix, OLS/PPML and test behavior; reghdfe is the
Stata syntax, FE/singleton/DoF/VCE oracle; ppmlhdfe is the PPML sample,
separation and inference oracle. They are not called by fereg at runtime.
The independent explicit-dummy oracle is development-only.

`scripts/ppml_reference_closure.py` verifies the pinned ppmlhdfe source archive
and the reghdfe/ftools/require closure before staging an isolated offline
reference runtime outside the repository. A floating checkout or preinstalled
ado package does not establish reference identity. Reference comparisons bind
both candidate and comparator closure and are supplemented by independent
Poisson and recession-cone calculations.

The legacy `fe/borrowed_map.rs` path is native fereg code, not adapted within,
PyFixest or reghdfe source. Later source adaptation must identify the exact
upstream file/function, preserve its notice and add a regression for the
borrowed behavior. A repository-level attribution alone is insufficient.
These facts consolidate `docs/upstream_map.md`; that source remains during
the coverage migration.

## Rust standard library

Release plugins statically include ordinary Rust crate code and use standard
operating-system libraries. The user is not required to install Rust, Cargo,
Python, Julia, BLAS, OpenMP, or a separate native side library.
