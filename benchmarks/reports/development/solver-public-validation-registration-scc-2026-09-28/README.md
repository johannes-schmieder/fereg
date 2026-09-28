# Conditional public solver validation — SCC registration, 2026-09-28

Job **7767737** registers the public scientific follow-up to the separate
[nonretaining-CG](../nonretaining-cg-registration-scc-2026-09-28/README.md) and
[Schwarz admission](../schwarz-imbalance-candidate-2026-09-28/README.md) candidates.
At registration neither candidate has a core or public runtime result. This is
development validation, with no production adoption or performance claim.

The job uses 16 slots, 3G/core and a 45-minute wall limit, with a 35-minute
controller deadline. Scheduler dependencies are **7766045, 7766146, 7767016,
7767269 and 7767319**, so it cannot build or run fits alongside the current
headline sweep. Any further headline jobs must also wait for this job. Submission
and exact arguments are in [submission.json](submission.json); remote root:
`/projectnb/welfgr/fereg/dev/20260928-solver-public-01`.
The [17:20 UTC scheduler check](scheduler-check.json) confirms all four sweep
tasks running, all five follow-ups held, the exact public-job predecessors and
all 16 deployed file hashes.

## Independent gates and source identity

Each arm requires clean accounting from its eight-slot core job, the exact
unique completion/pass markers, and complete registered test evidence. CG also
requires its baseline failure, all 72 candidate oracle cases, both complete
12-test sets and green boundary controls. Schwarz requires all seven nonempty,
disjoint test groups, the required named tests and the exact distinct count.
Missing, pending or failed evidence refuses that arm before building. Accounting
lag gets at most four observations with 20-second waits; tests are never retried.
The two patches are never combined. A passing arm can proceed independently of
the other arm's numerical refusal.

Accepted package **161b9491 / c09eeab353a8445aae85f75891f112964246e876c4865ec9e2f757d83922e236**
and Linux plugin **59d0341ae2c760b4701381e1a2153f1238ab94d752457429bb8642380f503920**
remain unchanged. Each candidate receives a separate source tree and Cargo target.
The controller verifies the 259 accepted inputs, baseline binary, 264 package/build
inputs and 12 fixture/helper inputs, then binds the complete patched source map
before building. Complete-inventory checks run before and after each successful
build/fixture sequence; plugin identity is checked again after the fixtures.
The existing Linux build includes binary audit and actual-plugin ABI/load checks.
These bindings cover local package/build files, not the entire external compiler
and dependency environment.

The first read-only preflight found that the accepted remote tree lacks
`rust-toolchain.toml`. The exact repository file is therefore an explicit staged
input, as in the core diagnostics; the accepted tree and old inventory were not
changed. All other 263 package files and 12 helper/fixture inputs matched the
remote source. The first submission wrapper failed before invoking `qsub` because
login Python 3.6 rejects `capture_output`. Python-3.6-compatible submission
arguments fixed it; exactly one job was submitted. The batch script loads Python
3.12.4. Both preflight corrections are preserved in the adjacent JSON receipts.

Deployment SHA-256:
`6c7038b81212e8582687ca55a68b63d5d7d7c05a4d07c198c1f3e095d5243e09`.
[deployment.json](deployment.json) binds the 16 staged files. Seven executable
helpers are preserved under [harness/](harness/); registration, local tests and
proof receipts are adjacent. The remaining identical bytes are the
[accepted source map](../current-source-sweep-scc-2026-09-28/source.json),
[CG patch](../nonretaining-cg-registration-scc-2026-09-28/candidate.patch),
[Schwarz patch](../schwarz-imbalance-candidate-2026-09-28/candidate.patch),
[new fixture](../../../../tests/stata/development/ols_near_within_cmg_oracle.do)
and repository `rust-toolchain.toml`. Their hashes were checked against deployment
before the handover commit. All 259 production inputs also still match.

## Public checks

Each passing candidate runs eight existing licensed fixtures: OLS/PPML smoke,
weighted OLS, LSMR reuse, three-FE batched OLS, multipartition slope OLS,
categorical final PPML and three-partition slope PPML. The exact paths are in
[registration.json](registration.json). Together they protect explicit-dummy
weighted solves, fast/low modes and RHS pools, wide controls at T1/4/16,
saved-effect/residual reconstruction, slope/excluded paths, independent
Poisson/Hessian/sandwich inference, FW/PW semantics, clustered inference and
transaction rollback. This is focused coverage, not qualification.

CG additionally runs a new public near-within oracle. Its 48 observations have
four Hadamard replicates within each of 12 FE tuples. Tuple-constant weights and
orthogonal noise give known coefficients (.7, −.4) and known residuals, separately
from either solver recurrence. An exact rational proof checks FE/control normal
equations and rank for unit/AW/FW/PW frames. Explicit Stata dummy regressions also
check finite coefficients, HC1 covariance, sample size and residual DoF. The
fixture has 32 fast/low, T1/4, auto/CMG calls and four low-mode saved-FE calls;
public tolerance is 1e−12, coefficient/covariance envelope 1e−10 and residual/
reconstruction envelope 1e−9. It first runs against the unchanged baseline plugin
as a diagnostic. Baseline failure does not accept the candidate. The new fixture
has **not yet run in licensed Stata**.

Builds are capped at 900 seconds and individual fixture processes at 240 seconds,
subject to the overall deadline. Process groups are killed and reaped on timeout;
temporary Stata logs remain under the remote work directory. The job restricts
CPU affinity to its 16 reserved cores, uses Cargo eight threads and BLAS one.

## Evidence and acceptance still pending

[Six focused harness tests](local-tests.txt) pass: core accounting/marker gates,
missing/duplicate/failed case refusal, exact fixture metadata, and bounded process
success/failure/timeout/deadline behavior. Python AST and shell syntax checks pass.
[oracle-proof.json](oracle-proof.json) records exact normal-equation/rank checks
for all four weight frames. No estimator build or numerical fit accompanied these
local checks, and none is claimed for this stage yet.

Collect `output/summary.json`, intermediate progress, per-arm source identities,
core accounting, build logs, plugin hashes and fixture results. Acceptance also
requires this job's clean 16-slot accounting and exact terminal markers:
`FEREG_SOLVER_PUBLIC_COMPLETE` and `FEREG_SOLVER_PUBLIC_AT_LEAST_ONE_PASS`.
The second marker and exit zero mean **at least one arm passed**, never both.
Inspect every arm; refused, failed and timed-out arms stay visible. Exit two means
neither arm passed. A scheduler/transport observation failure is not authority
to resubmit or restart the job.

Passing public science still leaves the exact constrained-memory reproducer,
uncontended baseline/candidate/stock rotations, fresh cases and relevant controls
before performance adoption. Integrate candidates separately, with appropriate
Mac controls. No runtime reduction, memory reduction or fix to constrained fits
is established by this registration.
