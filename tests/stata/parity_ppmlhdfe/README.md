# PPML parity tests

The public 0.5.0 `feppml` command and its guarded private transaction share the
same scientific fixture bodies. Separate wrappers retain direct coverage of
the private transaction boundary.

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
  `feppml_*.do` fixture bodies.
- The shared command, saved-slope, and time-series fixtures close public
  defaults and rejection classes, the required/forbidden `e()` namespace,
  partial and complete intercept/slope output channels, namespace conflicts,
  reconstruction invariance, and Stata panel-gap plus `if`/`in` semantics.

The `ppml-scientific` profile exercises the public command. `ppml-hidden`
retains direct private-transaction, allocation, and licensed
native-core/publication-boundary Break coverage. Profile membership does not
claim that qualification has passed.
