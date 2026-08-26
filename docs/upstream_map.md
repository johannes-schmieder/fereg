# Upstream source map

Exact commits are machine-readable in `references.lock.toml`.

| Reference | Role | Current 0.5.0 use |
|---|---|---|
| PyFixest | Model matrix, OLS/PPML orchestration, result semantics, tests | Behavioral map only; no copied code |
| within | Modified LSMR, FE operator, diagonal/Schwarz preconditioners | Exact git-revision dependency used by `fe/lsmr.rs`; fereg recomputes its own final FE certificate |
| reghdfe | Stata syntax, FE/singleton/DoF/VCE compatibility oracle | Exact pinned reference capture and differential qualification; never called at runtime |
| ppmlhdfe | Future PPML compatibility oracle | Frozen source reference only; PPML is outside 0.5.0 |
| Stata SPI 3.0 | Native ABI | Official `stplugin.c/h` compiled into plugin |

Later code adaptation must identify the exact upstream file/function, preserve
license notices, add a source-level mapping here, and add a regression test that
pins the borrowed behavior. A repository name alone is not sufficient
provenance.


## Active within adapter

`crates/fereg-core/src/fe/lsmr.rs` maps each native `EffectTerm` to
`within::Effect`, calls `within::solve_batch`, and copies only the returned
demeaned observation vectors back into fereg storage. The adapter uses the
exact commit in `references.lock.toml`. Numerical convergence is not accepted
from the recurrence alone: `fe/certificate.rs` recomputes `D'Wr` in the native
fereg representation before an estimate can proceed. No upstream source is
copied or modified.

## Native borrowed-intercept MAP path

`crates/fereg-core/src/fe/borrowed_map.rs` is native fereg code. It
operates directly on the prepared-data borrowed intercept design and does not
call or adapt `within`, PyFixest, `reghdfe`, or another numerical solver. The
independent explicit-dummy test oracle is used only in `fereg-testkit`.
