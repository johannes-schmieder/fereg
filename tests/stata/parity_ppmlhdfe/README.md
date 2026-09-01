# PPML parity tests

The public 0.5.0 `feppml` command remains a fail-closed `r(498)` stub, while a
private PPML implementation is exercised end to end before activation. These
fixtures deliberately keep those two boundaries separate.

- `feppml_fail_closed.do` checks that the installed public stub returns before
  a substantive plugin call and preserves data, requested-output names, and
  prior estimation state.
- `estimate_ppml_hidden_transaction.do` exercises the guarded hidden ado
  transaction and the one live-SPI `estimate-ppml` call, including rollback
  and result publication checks.
- `estimate_ppml_hidden_allocation.do` drives the native fallible-allocation
  hook and requires `r(909)` plus exact populated and null-state rollback,
  followed by a complete successful action in the same Stata process.
- `estimate_ppml_runtime_validator.do` tests the ado-side PPML result validator
  against its complete 108-row runtime matrix and selected mutations without
  invoking estimation.
- `estimate_ppml_hidden_*.do` science wrappers set
  `FEREG_PPML_TEST_COMMAND` to `_fereg_ppml` and then run the shared
  `feppml_*.do` fixture bodies. Before activation, run the wrappers—not the
  bodies' default public-command route.
- The shared command, saved-slope, and time-series fixtures close public
  defaults and rejection classes, the required/forbidden `e()` namespace,
  partial and complete intercept/slope output channels, namespace conflicts,
  reconstruction invariance, and Stata panel-gap plus `if`/`in` semantics.

The canonical pre-activation profiles are `ppml-fail-closed` for the public
stub and `ppml-hidden` for the private implementation. The latter also runs
the allocation fixture and the licensed native-core/publication-boundary PPML
Break fixture. These profiles establish distinct
qualification inputs; their presence does not claim that the gates have
passed or that public activation is complete.
