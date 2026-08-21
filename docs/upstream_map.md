# Upstream source map

Exact commits are machine-readable in `references.lock.toml`.

| Reference | Intended later use | Current package use |
|---|---|---|
| PyFixest | Model matrix, OLS/PPML orchestration, result semantics, tests | Behavioral map only; no copied code |
| within | Potential Rust FE operator/LSMR/preconditioner adapter | Solver research pointer only; no dependency |
| reghdfe | Stata syntax, FE/singleton/DoF/VCE compatibility oracle | Baseline scripts and parser cases only |
| ppmlhdfe | PPML IRLS, separation, prediction compatibility oracle | Baseline scripts and test manifests only |
| Stata SPI 3.0 | Native ABI | Official `stplugin.c/h` compiled into plugin |

Later code adaptation must identify the exact upstream file/function, preserve
license notices, add a source-level mapping here, and add a regression test that
pins the borrowed behavior. A repository name alone is not sufficient
provenance.
