# Independent saved final-residual precision repair

Accepted baseline: f576bc5, original omission artifact fe066c02, verified against
all 176 accepted inputs. The pending five-file Schur/recovery optimization is
preserved independently and excluded. Change only the existing final OLS residual
projection when saved FE coefficients are requested. Use at most 1e-12 tolerance,
preserve tighter requests, retain the current solver family and existing joint
workspace admission, and leave initial projection/reference/rank policy intact.
A retained CMG final call must consume the final precision without mutating its
stored session configuration. Low-memory retained LSMR is dropped before final
projection and therefore does not bypass this dispatch.

First validate a closed-form crossed-cell residual with a tiny known FE component
across automatic, CG, MAP, LSMR and CMG with unit and nonconstant positive weights.
Record a failing accepted-source test, then passing candidate tests. Run focused
prepared-fit and CMG tests. Replay the exact original rank-deficient and PW
requests, comparing to the already-computed independent direct/SVD and full
LSQR/LSMR oracles, full b/V, row masks, rank/DoF and native certificates. Stop on
unresolved scientific failures; do not replace inputs or loosen tolerances.
Run the 48-fit saved dummy oracle, four omission/weight oracle cases and existing
initial exact/one-byte-short rollback checks. Build only in the isolated source
root; bind both architecture builds and actual native ABI checks to the input
manifest and candidate hash.

After science passes, compare bounded original saved unit/PW/rank/slope/three-FE/
explicit-CG/weak/low-FW cases plus a no-save OLS and PPML control, rotating order.
Retain time, phases, native/base/scratch/RSS and solver/fallback/retry differences.
Scientific baseline errors require independent adjudication and are excluded from
passing accuracy-matched speed claims. This is a correctness repair; disclose
costs crossing both 5% and 10ms and use two reversed/seed follow-ups if unexplained.
Do not overlap builds/tests/oracles/data generation or archive compression with
timed comparisons. No cluster work or qualification in this step.

Before timing: include the existing weighted-cut and small-AW saved controls too, for twelve planned pairs. Known weak/PW baseline errors require the original independent pointwise oracles on both new captures; keep these outside accuracy-matched performance aggregates.

Revised candidate: final CMG absorption consumes the session. If the stricter
certificate fails, release its graph hierarchy before admitting the existing
CG correction; preserve the original residual reference norms and report the CG
fallback. No ordinary graph retry on this near-zero projected right-hand side.
Retain all prior precision-probe failures and costs. Run the original two
functional oracle replays plus the automatic-CMG control that exposed the costly
retry and its independent explicit-dummy residual/FE oracle. Resume a fresh full
cost comparison only after science and terminal-CMG admission/lifetime tests.
Ensure the requested explicit-CG selector is actually passed into the harness;
the earlier mislabeled capture used automatic CMG and is additional evidence.
