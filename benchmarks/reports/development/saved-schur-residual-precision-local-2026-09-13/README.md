# Saved Schur final-residual precision investigation

Local development evidence against accepted `f576bc5`, without adoption of the
five-file saved-Schur/recovery prototype. Source and artifact manifests bind
both stages; original inputs, failed comparisons and numerical thresholds are
preserved. No SCC, qualification, dependency update or push is included.

## Failed parent

The rebased prototype (`be5fc20a`) passes the original slope and three-FE saved
pairs, but its original deficient-rank pair fails saved residual parity. An
independent grounded weighted-dummy projection, LAPACK SVD and separately solved
full sparse LSQR/LSMR system adjudicate the discrepancy: the baseline passes,
while the candidate fails 13 residual, 48 first-FE and 160 second-FE rows. Its
final residual projection performs zero iterations, leaving a small FE component
that contaminates the subsequent saved target. The 0.496 to 0.406 second result
is failed-parity evidence, not a passing performance gain. The original explicit-
CG pair and primary verification pairs were not executed after this failure.

## Polished probe

The next isolated prototype (`7059496c`) tightens the existing final one-RHS
projection to the saved-recovery precision only after Schur has deferred FE
recovery. It preserves stricter requested tolerances. A closed-form crossed-FE
regression fails before this change with a remaining 2e-9 FE component and passes
afterward. The exact original rank-deficient request now passes the independent
oracle on every row: residual error is below 1.7e-13 and FE errors below 2.8e-12.
Focused saved/recovery tests, 48 independent saved-output fits, four weighted
omission/output cases, initial/recovery cap checks and native arm64 ABI checks
pass. These do not establish complete adoption.

The revised full protocol stops on its third pair. Unit seed 104729 passes
(2.896 to 0.755 seconds); the same-seed AW operator rejects Schur and retains
exact science (82.960 to 82.757 seconds). The original PW primary returns success
in both arms but fails saved residual/FE parity. No remaining primaries or
protected comparisons are run. Its timing and memory differences remain outside
passing performance aggregates.

The independent PW calculation reconstructs the exact 244274-row final sample,
rank, nesting, DoF, normalized intercept and full two-way CGM covariance. Direct
weighted dummy/SVD and full sparse LSQR/LSMR residuals agree within 7e-14. The
polished candidate passes every saved-output row. The retained baseline fails
121138 residual rows (maximum 2.618e-7) and 242668 first-FE rows (2.694e-6).
Both arms pass independent full coefficient and covariance checks. This is an
existing baseline precision defect, distinct from the deficient-rank prototype
regression. Original tolerances and samples are unchanged.

Decision: preserve both investigations and repair the accepted retained baseline
independently before using it as the optimization's accuracy-matched comparator.
The saved-Schur optimization remains unadopted. See the stage adjudications,
protocols, manifests, complete numeric archives and terminal evidence for details.

## Build incident and restoration

A mistaken build invocation from the main checkout removed the protected plugin
and native link intermediates. The owned process was stopped; no comparison used
that interrupted artifact. Original static libraries were intact. Recompiling the
C boundary from the verified original 484-input source snapshot and relinking
those original libraries at their original paths reproduced the exact protected
plugin SHA-256 `4187efdfbd6235e63c369d9b42dede6381b8a9e50b5360fd63ee237bd6c11b08`.
That exact binary and original native intermediates were restored; the original
checksum file was unchanged. An earlier relink using renamed library paths had a
different hash and was not installed. Sanitized restoration identities and raw
log hashes are retained. The subsequent valid isolated build explicitly checks
its source cwd, input manifest and protected plugin before and after compilation.
