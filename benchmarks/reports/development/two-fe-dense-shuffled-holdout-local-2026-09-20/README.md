# Dense two-FE shuffled holdout: target not met

The frozen `densecore` candidate fails the shuffled narrow holdout. At
983,040 observations and sixteen workers, its five-pair median runtime ratio
against stock fixest is 1.038690, with zero native wins. The original 10%
margin remains the target. The campaign was stopped during its subsequent
primary block; regression checks and the second final block did not run.

| Holdout | Regressors | Native median, ms | Median paired ratio | Wins |
|---|---:|---:|---:|---:|
| Fresh ordered seed 104729 | 10 | 318 | 0.738824 | 5/5 |
| Fresh ordered seed 104729 | 50 | 779 | 0.515894 | 5/5 |
| Shuffled seed 31415927 | 10 | 339 | 1.038690 | 0/5 |
| Shuffled seed 31415927 | 50 | 823 | 0.734991 | 5/5 |

All four holdouts pass the numerical comparisons. Before these timings, all
eight saved-output cases pass unchanged tolerances against separately tightened
fixest estimation/recovery (1e-11). Maximum saved-FE disagreement is below
2.04e-10. These tighter-reference times do not replace stock default timings.
The three artifact-specific Stata fixtures and actual-plugin ABI check pass.
All 513 frozen source entries matched the worktree before the campaign.

The follow-up three-pair instrumented diagnostic localizes the shuffled narrow
native initial absorption at 185 ms: the Schur operator takes 98 ms over 35
iterations, recurrence 23 ms, initialization 10 ms, reconstruction 4 ms and
certification 15 ms; the separate probe/setup accounts for the balance.
Input remains approximately 52 ms. Instrumented fixest demeaning is about
227 ms in its first trace, including 166 ms forming group means. Its shuffled
input is materially faster than the earlier ordered diagnostic.

The next prototype reuses idle certificate scratch for neighbor codes in
membership order, removing repeated observation-code lookups without changing
arithmetic or allocating storage. It is outside this frozen artifact and
has not been accepted by this report.

This archive contains 105 distinct completed application calls and three
uncompleted attempts. One attempt was interrupted when the campaign was
stopped. Two subsequent diagnostic attempts failed before starting R because
the interrupted campaign left its private scratch directory. After verifying
that the worker was gone, that directory was preserved separately and the
diagnostic completed. No completed timing was removed. An initial archive
matched overlapping series prefixes and double-counted eleven calls; the
corrected selector matches the case boundary and asserts unique run names.
The erroneous archive remains under local target storage.

The native plugin is
`6e65eacaae121686f2a42057edcff55b484540cd99ff8c3ce6c89bb782d37e3e`.
Identities bind the source and artifacts, and reference fingerprints distinguish
stock from instrumented fixest. All work is local Mac development; no installed
package, remote state or release qualification changed.
