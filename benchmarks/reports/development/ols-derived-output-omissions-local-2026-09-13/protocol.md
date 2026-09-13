# OLS derived-output omission repair

Accepted baseline is 834703b, isolated plugin fdc3e141. The candidate changes
only the plugin's derived-output coefficient selector and its closed-form test.
Both sources exclude the pending saved-Schur/recovery prototype. Accept an
absent value with a generated-column identity only for the existing FE-collinear
or numerical-rank-deficient statuses. It contributes no finite fitted term.
Retain rejection of missing kept values and malformed identities; all fitted
coefficient/status, rank, inference and public omission semantics remain.

Require a pre-fix closed-form failure and post-fix success in fast/low modes,
the plugin tests, and the four-case independent Stata dummy/HC1/cluster oracle
with unit/AW/FW/PW and both kinds of omitted generated column. The unchanged
baseline must preserve prior estimates and publish no saved outputs on r(498).
The candidate must preserve its ordinary no-output fit and pass independent
residual/identified covariance checks. The initial test's wrong wire-status
assertion (4 instead of the existing 3 for FE collinearity) is a fixture mistake;
preserve it separately from the actual pre-fix publication failure.

Replay the exact stopped absorbed-slope warm/full requests and the original
rank-deficient saved warm/full requests on the corrected candidate. Check the
48-fit ordinary saved-output oracle with expect_schur=0; do not use this repair
to adopt the separate saved optimization or claim its pending performance gains.

After builds and scientific validation finish, compare six fixed controls from
the existing direct-CG cost evidence: saved unit, saved PW, weak degree-three,
low FW clustered, small AW and complete IID. Preserve exact b/V/sample/status/
DoF/solver/saved exports, existing data and rotated arm order. No data generation,
build/test/diagnostic overlaps timing. Record full command and phase time,
native/public base/scratch/RSS and all fallback/retry costs. Investigate an
increase above both 5% and 10 ms with two reversed same-request pairs. This
correctness repair has no speedup hypothesis. Keep failing original requests,
diagnostics and functional checks outside timing aggregates.

Build only isolated trees; preserve main dist and unrelated Veneto work. No
agents, SCC, dependency update, qualification or push. Return to the original
saved-Schur evaluation after committing the independently validated repair.
