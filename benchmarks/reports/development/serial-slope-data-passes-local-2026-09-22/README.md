# Serial slope data-pass refinements

The frozen combined candidate `rn1` improves the weighted K50 robust fit with
saved effects and predictions by 13–15% in two independent five-pair local
blocks. Its medians are 894/858 ms versus the preceding `owf1` baseline's
1,012/1,005 ms and stock fixest's 1,990/1,963 ms. Several narrow fits improve
modestly; the serial three-slope-only PW cases still take about 1.6 times
fixest's time. This checkpoint does not close those losses or the milestone.

The first change traverses contiguous triangle rows in the shared dense
sandwich reducer and OLS score-Gram certificate. Arithmetic and reduction
order, separate production/certificate results and allocation are preserved.
The second combines each serial cached-slope regressor's reference norm and
maximum into one row pass, with a fixed 512-byte stack bank. It leaves other
routes, threaded fits and strided score caches on their existing path.

| Serial saved-output case | Block 1 paired candidate/prior | Block 2 paired candidate/prior |
|---|---:|---:|
| AW K10 robust | 0.9768 | 0.9847 |
| AW K10 two-way cluster | 0.9467 | 0.9764 |
| PW K6, three slopes only, robust | 1.0043 | 1.0090 |
| PW K6, three slopes only, two-way cluster | 0.9962 | 0.9624 |
| AW K50 robust | 0.8694 | 0.8520 |
| Unit K10 robust | 0.9583 | 0.9870 |
| FW K10 robust | 0.9624 | 0.9421 |

[All timings](timings.md) retain the two three-pair screens, including mixed
narrow results. The first compares covariance-only `st1` with `owf1`; the second
isolates the serial reference pass against `st1`. Both confirmations compare
the same frozen `rn1` with `owf1` and stock fixest. FW uses native controls;
ordinary fixest weights do not supply Stata FW inference.

All 338 screen/confirmation applications pass with clean isolation. Their 118
native pairs have exact six-bundle numerical exports, samples and saved vectors.
Independent output reconstruction and normal-equation checks pass. Tracked
native peak allocation is equal in every pair. These checks complement the
existing explicit-dummy/literal-FW contracts; they are not a new qualification
matrix. The candidate's ABI check and four focused release fixtures pass; the
covariance-only artifact also passes thirteen sandwich fixtures and four focused
OLS/PPML fixtures, including mutation/allocation failures and literal FW parity.

Eighteen diagnostic applications preserve seven exact native pairs. The full
current-command profiler also captures the pinned instrumented fixest phases.
It identifies native computation and saved-effect recovery as material remaining
costs. Final native diagnostics put AW K10 reference passes at 14.2 to 6.4 ms
and PW K6 at 6.9 to 3.7 ms. For K50, covariance construction falls from 307.6 to
174.7 ms. These single traces explain the change; they are not headline timing
evidence or a claim that all phases improved.

The archive preserves all 356 calls, source identities, controllers, reference
library hashes, filtered timing traces, test logs and numerical audit receipts.
`verify.py` reconstructs all four complete captured source trees and checks
portable hashes and audit bindings. Large output arrays remain local with
explicit hashes; binaries and raw licensed logs are excluded. The `st1` build
used a nested isolated build directory, exposed to the unchanged runner through
local artifact aliases. This does not change its recorded source or plugin hash.

The measured sources include the preceding six uncommitted OLS graph/window
drafts. Receipts attach to those complete captured sources, not to an isolated
commit or to the old final-block source. This refinement is retained for local
development under ADR 0096. Linux confirmation and remaining serial losses
remain open; no push, installation or release is performed.
