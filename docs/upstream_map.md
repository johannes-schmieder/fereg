# Upstream source map

Exact commits are machine-readable in `references.lock.toml`.

| Reference | Intended later use | Current package use |
|---|---|---|
| PyFixest | Model matrix, OLS/PPML orchestration, result semantics, tests | Behavioral map only; no copied code |
| within | Modified LSMR, FE operator, diagonal/Schwarz preconditioners | Exact git-revision dependency used by `fe/lsmr.rs`; StFixest recomputes its own final FE certificate |
| reghdfe | Stata syntax, FE/singleton/DoF/VCE compatibility oracle | Baseline scripts and parser cases only |
| ppmlhdfe | PPML IRLS, separation, prediction compatibility oracle | Baseline scripts and test manifests only |
| Stata SPI 3.0 | Native ABI | Official `stplugin.c/h` compiled into plugin |

Later code adaptation must identify the exact upstream file/function, preserve
license notices, add a source-level mapping here, and add a regression test that
pins the borrowed behavior. A repository name alone is not sufficient
provenance.


## Active within adapter

`crates/stfixest-core/src/fe/lsmr.rs` maps each native `EffectTerm` to
`within::Effect`, calls `within::solve_batch`, and copies only the returned
demeaned observation vectors back into StFixest storage. The adapter uses the
exact commit in `references.lock.toml`. Numerical convergence is not accepted
from the recurrence alone: `fe/certificate.rs` recomputes `D'Wr` in the native
StFixest representation before an estimate can proceed. No upstream source is
copied or modified.
