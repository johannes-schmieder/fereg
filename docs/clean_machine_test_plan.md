# Clean-machine test plan

For each platform, start with a Stata 18+ installation and no Rust, compiler,
Python, Julia, BLAS, or StFixest files.

1. Copy only ado/help/package files and the one platform plugin.
2. Verify `which stfixest` and `findfile stfixest_<platform>.plugin` resolve to
   the isolated directory.
3. Run `stfixest, version`, `doctor threads(1)`, `doctor threads(32)`, and
   `selftest threads(32)`.
4. Drop/redefine the plugin handle 100 times and rerun selftest to detect loader
   lifecycle defects.
5. Verify `stfeols` and `stfepois` return 498 without changing data, sort order,
   current estimates, or creating variables.
6. Run parser fixtures with numeric/string FE sources, factor base choices,
   interactions, grouped absorbed slopes, cluster tuples, `if/in`, and missing
   values. Confirm no dummy variables or group variables are created.
7. Inspect process-loaded modules and the plugin dependency list. Only ordinary
   OS libraries may appear.
8. On macOS, run the same artifact natively on Intel and Apple Silicon. On
   Windows and Linux, test the oldest supported OS/runtime baseline.

Every do-file must print `END_FIXTURE|<name>|rc=0` only after its final
assertion. Treat a missing or nonzero sentinel as failure even when the Stata
process exits zero. Rosetta execution exercises the x86-64 slice and loader but
does not satisfy the native Intel-host requirement in step 8.
