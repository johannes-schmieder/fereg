# Clean-machine test plan

Start with licensed Stata and no Rust, compiler, Python, Julia, BLAS, or prior
fereg installation. Test only the staged production files and the platform
plugin from the audited 0.5.0 archive.

1. Verify `which fereg`, `which feppml`, and the platform plugin resolve only
   inside the isolated install.
2. Run `fereg, version`, `doctor threads(1)`, `doctor threads(32)`, and
   `selftest threads(32)`; require action protocol 1.5, OLS result wire 1.4,
   and model wire 2.1 for fereg 0.5.0.
3. Drop/redefine the plugin handle 100 times and rerun diagnostics to detect
   loader lifecycle defects.
4. Run public parser/error, OLS, weight, FE/slope/string, time-series,
   singleton, VCE/DoF, output, prediction, `test`, `estat summarize`,
   memory-mode, transaction, store-failure, panic, and Break fixtures.
5. Confirm `feppml` returns `r(498)` without changing data, sort order,
   estimates, or variables.
6. Confirm developer adapters, raw logs, reference PDFs/libraries, and build
   caches are absent from the installed archive.
7. Inspect exports, architecture, loaded modules, OS/runtime baseline, and
   dynamic dependencies. Only ordinary OS libraries may be required.

Linux qualification uses SCC licensed Stata 18 and 19 and the glibc 2.28+
x86-64 artifact. macOS qualification uses licensed Stata 18 natively on arm64
and exercises the x86_64 universal slice under Rosetta. Windows x64 has a
private, bounded licensed-Stata driver described in
`docs/windows_qualification.md`; it remains deferred until that driver has an
accepted exact-source controller receipt. Rosetta is not native Intel-host
certification, which is also deferred.

The private `ppml-hidden` profile runs from the source tree before activation;
it is not a clean-install package surface. A pre-activation archive must still
contain the public stub and exclude the private adapters and qualification
fixtures. After a separately approved activation change, replace the stub
check with the full public PPML installation/postestimation matrix and rerun
this plan on that exact archive rather than inferring success from hidden-path
evidence.

Every do-file prints `END_FIXTURE|<name>|rc=0` only after its final assertion.
Process exit, Stata code, fixture status, and the unique sentinel must agree.
