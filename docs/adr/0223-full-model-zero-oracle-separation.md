# ADR 0223: Full-model separation in the bounded zero-count oracle

Date: 2026-09-26. Status: accepted harness correction.

The first lighter-campaign smoke oracle removed all-zero FE groups and singleton
cascades. That was insufficient for sparse zero-count designs: combinations of
FE and finite regressors can certify additional separated rows even though no
remaining FE level is all zero. Checking fitted coefficients alone cannot expose
this difference; retained sample and N-dependent covariance can still differ.

The independent oracle now solves the complete explicit recession cone with
SciPy HiGHS, outside fereg's detector implementation. Direction coefficients are
bounded to [-1,1], positive-outcome directions equal zero, and zero-outcome
directions are nonpositive. Minimizing their sum finds a nonzero recession
direction when one exists. The oracle independently checks the finite model-space
reconstruction and sign margins before deleting any row, then repeats singleton,
zero-group and full-cone passes to a fixed point. Failed LP, sign certificate or
unresolved negative objective fails closed. Existing regression, covariance and
objective tolerances are unchanged. Explicit Poisson/Hessian/sandwich inference
is then formed on that independently established sample.

An analytic test covers finite-regressor separation with both FE groups still
containing positives, a subsequent singleton cascade, and opposing zero rows
that rule out separation. The corrected 21-test controller suite passes. A
separate SCC oracle-only adjudication reuses the exact captured input bytes and
native outputs; no estimator is rerun and original smoke receipts are unchanged.
The two sparse zero designs retain 938 and 943 rows, matching feppml's exact
sample hashes, coefficients, full covariance and objective. The stock-reference
sample differences remain scientifically incomparable. The other two zero
designs also pass unchanged.

This fixes the benchmark oracle, not the estimator. The original frozen smoke
remains rejected, including the separate unresolved low-memory OLS slope
initialization failure. It cannot authorize the pilot or production array. A
new controller identity and appropriate focused validation are necessary before
resuming. Do not overwrite the old controller, promote its rejected receipt, or
retry a scientific failure merely to obtain a passing run.
