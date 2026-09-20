# Restore left means during Schur reconstruction

Local development measurements on 2026-09-20 on M2 Ultra, Stata/MP 19.0
(8 licensed processors) and pinned stock fixest, with 16 requested native/R
threads unless specified. The embedded build date is September 19. These
focused measurements do not close the broad milestone or qualify a release.

Candidate `fusedreconstruction` is base `830ca43c` plus the recorded core
overlay; all 516 captured source files match. Plugin SHA-256:
`ec0a780f7b2714dea64339b7f2e581f57e490625b43efec170f8541a7d3ac29e`.
Parent is accepted `weightedstatistics`. Its actual-plugin ABI/panic check
passes. No installed package or remote state changed.

Once the OLS edge cache is dead, its left-RHS bank is restored while traversing
the same left groups for reconstruction. Original values and weights are still
immutable; the original group weight totals remain valid. Original-value and
right-solution sums retain their separate row-order arithmetic and divisions.
Both existing banks are overwritten before packed cache values can enter
arithmetic. No new bank, solver guard or certificate is introduced; uncached
and shared/PPML behavior remains unchanged.
[ADR 0085](../../../../docs/adr/0085-ols-schur-restoration-during-reconstruction.md)
records the scope and safeguards.

## Timing

Milliseconds are separate medians of three paired runs. All individual calls,
pair orders, ratios and dispersion remain in the receipts.

| Cell | Parent | Candidate | Default fixest |
|---|---:|---:|---:|
| Shuffled unit K=3 IID | 226 | 225 | 235 |
| Shuffled unit K=5 IID | 257 | 255 | 248 |
| Shuffled unit K=6 IID | 254 | 246 | 256 |
| Shuffled AW K=6 IID | 295 | 285 | 303 |
| Shuffled AW K=6 full outputs | 439 | 444 | 573 |
| Ordered AW K=6 IID | 272 | 269 | 487 |
| Ordered AW K=6 full outputs | 428 | 417 | 704 |
| Ordered AW K=50 IID | 736 | 735 | 1940 |
| Ordered AW K=50 full outputs | 923 | 908 | 2143 |

Benefits are modest and uneven. Shuffled K=5 still trails fixest. Shuffled AW
full outputs are 1.1% slower in this block; the discrepancy is retained.
No control cell's median paired native ratio exceeds 1.05. The maximum is
1.03943 in the single-pair FW robust check; group-concentrated outputs are
1.02136. PW/FW robust, crossed cluster and two-cluster/saved, unequal weights,
tiny weighted bridges, low memory, four threads, weighted one-FE and protected
unit K=50 controls pass. Three-pair degree-six AW is 75 ms in both arms.
Ineligible-path timing differences are not attributed to this change.

Two separate trace pairs show the initial reconstruction phase falling
7.897 -> 5.684 ms for shuffled unit K=5 and 5.679 -> 4.724 ms for shuffled
AW K=6. All four use 35 initial Schur iterations and pass certification.
Those phase gains support removing the duplicate traversal; total application
differences also include run noise and are not all attributed to reconstruction.
Trace-enabled application timings are separate from the headline medians.

## Science, memory and evidence

All 125 application calls succeed with explicit result/sentinel checks.
There are 47 production native pairs and two diagnostic pairs. Every pair
preserves all six scientific files bitwise (b, V, status, DoF, certificate and
fit) and raises no native allocation peak. All thirteen saved-output pairs are
bitwise exact and pass independent weighted normal-equation, component
normalization, RSS and reconstruction audits. Nine default-fixest saved-output
disagreements remain explicit; all finite coefficient comparisons pass.
Frequency controls use the native parent and independent frequency semantics.

Fourteen focused Schur tests pass in a fresh release target. The new boundary
check compares original separate restoration with combined reconstruction for
unit/supplied weights, common scales 1e-100 through 1e100, RHS counts 3/5/8/52,
padded strides, 1/8/16 workers and poisoned private cache storage. Restored
means, group totals, reconstruction workspace and final values are bitwise
exact. Existing independent MAP, weighted-operator, weighted-cut, memory,
shared/PPML and charged-fallback checks pass. Three final-artifact Stata
fixtures pass with fixture and terminal sentinels, including independent
`areg` plus explicit second-FE dummies and atomic transaction checks.

The initial archive parser incorrectly expected two eligible initial Schur
traces per process. These logs contain one eligible initial trace; the small
warmup does not produce another. Its failed assertion log is retained. The
parser was corrected and downstream audits resumed without replacing any
measurement or rerunning an application. No scientific assertion failed.

The next substantive experiment should retain useful work from the narrow
Schur admission probe while preserving weighted admission, rejected-input
fallback, iteration charging and memory bounds. The 111 native controls,
sixteen fresh-seed holdouts and two final five-pair blocks remain outstanding.
