# Final two-way OLS reference-accuracy adjudication

This separate diagnostic covers all 28 distinct primary/fresh-holdout cells
with a default fixest output or scientific discrepancy. It requests fit and
FE recovery tolerance 1e-11 from the same pinned stock reference. One pair per
cell is explicitly separate from the repeated headline default timings.

All **28 native calls pass** and preserve their headline scientific bundles
bitwise; every requested native output vector is also identical. Eighteen
tighter reference calls pass the unchanged comparisons: ten saved/full-output
cases and eight degree-two/sparse-mobility cases. The independent grounded
direct oracle passes all 26 applicable successful graph calls (eighteen native
and eight reference).

The remaining **ten tighter references do not converge** within the reference's
10,000-iteration limit: unit/AW, K=6/50 bottlenecks and cycles, plus the two
fresh-seed weighted narrow cases. These failures are retained. Their times
are not matched-accuracy wins and never replace headline default-reference
times. Independent native accuracy and default reference errors remain recorded
in the primary/holdout oracle receipts. No tolerance, statistical policy,
sample or requested output was relaxed.

## Preserved startup failure

There are 57 attempted application calls: 28 successful native calls, eighteen
successful references, ten nonconverged references and one native application
startup crash. The first diagnostic cell passed in the original directory;
the next Stata process exited with SIGTRAP before target invocation, with no
result metadata or terminal sentinel. The macOS crash trace identifies
`__chk_fail_overflow` and `__strcat_chk` in Stata startup, before estimation.
The precise internal buffer was not identified. A sanitized record and raw
crash-report hash are retained in `precision-startup-failure.json`.

A fresh directory with shorter root/labels repeated that failed cell and ran
the remaining 26 cells successfully at the application level, apart from the
explicit reference nonconvergence above. The successful original cell was not
rerun. Inputs, workers, binaries and command semantics were unchanged. The
original failed attempt remains in `measurements.json`; `continuation/`
contains the subsequent calls. No estimator fix or native source change was
made in response to a pre-estimation application crash.

## Provenance

`accuracy-audit.json` maps every cell to its original primary/holdout result,
input and scientific hashes. `graph-oracle.json` records the independent
successful graph checks. Both actual controllers and dispatch identities,
immutable selection plans, all attempted calls, reference fingerprints and
driver/audit sources are retained. Raw licensed logs and machine-specific
paths are excluded. This is local scientific adjudication for the development
milestone, not release qualification.
