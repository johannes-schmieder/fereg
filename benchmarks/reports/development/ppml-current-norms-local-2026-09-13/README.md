# P4: repair current-weight norms after fused PPML projection

Repair the retained linear session against accepted `c6ad4c4c6ee7fa84c0d34583a769e366a3f198a8`.
This is a correctness repair with a measured cost, not an optimization gain.
It enforces ADR 0020's existing current-weight/rank contract. No rank threshold,
weight convention, solver region or publication policy is changed.

## Reproducer and repair

The fused changing-response/retained-design path refreshed the projected
matrix and raw reference norms, but carried old `within_norms` into the new
weight generation. Frozen finite solves do not consume that field; subsequent
strict rank and score checks do. Non-fused re-projection already refreshed it.
The earlier pass audit's statement that within norms refresh was therefore
too broad for this path; its historical report is preserved unchanged.

The [new regression test](../../../../crates/fereg-core/src/ols/prepared_fit.rs)
uses four two-row groups, `x=g±0.001`, initial unit-valued supplied weights,
and new weights `(1,1e-8)` within each group. Independently, each group's
weighted within sum of squares is

```text
w0*w1/(w0+w1) * (x1-x0)^2.
```

The correct current within/raw squared-norm ratio is about `5.34e-15`, below
the unchanged `1e-9` FE-collinearity cutoff. Combining the old within norm
with the new raw norm gives about `2.67e-7`, above it. A fresh session correctly
reports rank zero. The old retained strict session instead accepts rank one.
The finalized test fails against old production code and passes with the repair.

Under the existing strict transaction, the prior rank-one basis cannot be
recertified after becoming FE-collinear. The repaired session therefore rejects
strict staging, leaves the basis generation unchanged and exposes no current
output. An explicit basis refresh then permits the new canonical rank-zero
solve and matches the fresh reference. This is a private session counterexample
with an independent group-moment calculation, not a claimed public Poisson
dataset demonstrating incorrect published estimates.

The [production patch](implementation.patch) recomputes within norms after
fused matrix compaction with the existing stable current-frame norm routine.
It uses retained scratch and preserves arbitrary valid frame divisors. It
covers fused one/two/three-FE categorical MAP sessions, including eligible
checkpoint consumers. Generic/low-memory re-projection, ordinary specialized
joint LSMR and one-shot OLS keep their implementations. Original raw reference
norms, useful warm projection, finite rank/QR policy and independent checks
remain. No observation-sized allocation is added.

## Cost and protected results

The [prospective protocol](protocol.md) screens the repair against the old
accepted source, investigating increases above max(10 ms,5%). Three public
single-seed primaries, five controls, two extra predeclared FW seeds and one
separate private allocation-audit pair give **11 pairs / 22 measured commands**.
All runs use isolated artifacts, fresh local Stata/MP 19 processes on the M2
Ultra and a small untimed warm-up. Arm order rotates; native threads are
separate from the Stata license processor cap. No tests/builds run during
timing. These are local repair-cost measurements, not qualification.

| Public first screen | N / K / workers | Command seconds | Change |
|---|---|---|---:|
| Complete one FE, robust | 983040 / 50 / 16 | 9.731 → 10.197 | +4.8% |
| FW, exposure, sparse missing, robust | 245760 / 50 / 4 | 3.219 → 3.380 | +5.0% |
| PW, offset, sparse missing, two-way cluster | 245760 / 50 / 16 | 2.948 → 3.085 | +4.6% |

The FW increase exceeds the exact review threshold. Two further seeds give
3.210→3.381 and 3.220→3.398 seconds. Across all three FW pairs the median
paired cost is **+5.3%**, with increases of 161/171/178 ms. The cost is real
in this bounded comparison; it is not dismissed as noise. The other two
primaries have only the single screen pair and no repeatability claim.

The added observation-by-control norm pass sits in projection/strict work.
First-screen projection times are 1.1426→1.3853, 0.4666→0.5541 and
0.3632→0.4548 seconds; strict/final scopes are 3.3184→3.5429,
0.9220→0.9725 and 0.8642→0.8775. Preparation/inference remain close.
These scopes overlap and must not be added. No new independent norm timer
or hardware-traffic measurement is claimed. Current-frame certified norm
reuse is the next experiment to remove this cost without retaining stale state.

Primary native peaks are exactly unchanged at 2522.61, 633.49 and 645.34 MB,
inside unchanged models of 2962.95, 745.43 and 935.51 MB. All eleven measured
native peaks are exact between arms and fit their models. Full scratch/model,
phase, solver and process-RSS receipts are in the numeric archive. RSS moves
in both directions; weak is 0.1247→0.1565 GiB and explicit MAP is
0.0644→0.0919 GiB. No process-memory benefit is claimed.

Protected command differences: weak two FE +4 ms, unchanged low FW +12 ms
(1.5%), small one FE −6 ms, saved/separated −1 ms and explicit strong two-FE
MAP +2 ms. None crosses its review threshold. These single pairs do not prove
causal control speed changes. The private audited PW pair is 7.723→8.683 s
(+12.4%), reported separately from ordinary commands. Both arms retain zero
audit allocation events and zero live/peak growth, passing the same capacity,
allocation-free and fresh/warm-equivalence checks.

All paired full b/V exports are byte-identical. Samples, rank/status, DoF,
weight/cluster/separation structure and discrete solver decisions match.
Independent runtime certificates pass; saved outputs match under unchanged
tolerances. Outer/inner work, line searches, retries and fallback diagnostics
remain in [pair records](comparisons.json) and the
[checksummed numeric archive](archive.json). These ordinary-case comparisons
do not override the adversarial failure difference required by the reproducer.

## Source identity and validation

[Identities](identities.json) bind 152 native inputs, production/test patch,
both isolated artifacts and the unchanged installed plugin. Candidate SHA-256:
`313e85aa7ccd298f8333e9dc3c3845ab963b38bbfcf37964a88a2944c06687e6`;
baseline: `b8333c4ac9e28137c1ee457f95d7cf637bed824ab8d80cad67b67d6f84527b35`.
Universal builds use Rust 1.85.1 and locked dependencies; static audits and
the actual native arm64 ABI/self-test pass.

[Validation](validation.json) records **21 distinct candidate Rust tests**,
including the finalized negative/positive reproducer, strict basis/rollback,
two-FE fusion, three-FE joint-state rebuilding, identical-frame reuse and
generation invalidation. Both artifacts pass the same independent
[full-dummy Poisson/Hessian/sandwich oracle](../../../../tests/stata/parity_ppmlhdfe/feppml_one_fe_dummy_oracle.do)
for unit/FW/PW with robust/clustered inference and offsets.

Development failures are retained: an unnecessary clone warning, a test-only
Debug requirement, missing baseline fixture scaffolding, and initial test
expectations of automatic strict/warm rank replacement. Those expectations
were corrected to enforce the already accepted explicit-basis/strict-rejection
contract; the independent collinearity cutoff was not changed.

An additional check exposed a shared Cargo-target freshness hazard: switching
between copied source trees reused the baseline test executable even though
candidate source hashes matched. Accepted candidate test results were rerun in
a fresh, separate target directory. A forced recompilation of the changed
release crate produced **exactly the already measured plugin bytes**, and its
static/native checks passed again. Thus the timing evidence remains bound to
the repaired source; shared-target test results are not counted as validation.
The development workflow now records this precaution.

Adopt the correctness repair despite its documented cost; the defective cache
is not an acceptable performance baseline for further norm-reuse claims. P4
fresh-input checks and certified current-frame norm reuse remain open. No new
ADR is needed because the scientific policy is unchanged. No installed binary,
unrelated Veneto work or historical receipt was replaced; no push, agents,
upstream writes, cluster work or release qualification occurred.
