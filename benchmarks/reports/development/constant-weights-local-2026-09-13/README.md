# P2 constant-weight specialization: evaluated and rejected

Reject the [prototype](implementation-reviewed.patch) against accepted native
`4e18c223a6b741b04bbcee35d10790729f5c8171`. None of four complete-command targets
meets the prospectively specified gain of both 5% and 10 ms. The unchanged
accepted source is restored; the source-bound patch, isolated plugin and all
measurements are preserved. This closes the requested evaluation, not every
possible future constant-weight optimization.

## Mechanism and scientific boundary

The existing normalized-underflow validation pass also detects whether every
raw supplied weight exactly equals the maximum. A private scalar flag makes
normalized arithmetic use existing unit-weight loops for norms, group means
and CG dots. No additional observation-sized allocation or validation pass is
introduced. Nonconstant input retains the weighted arithmetic, including a
one-ULP difference. The extra flag/comparison and kernel branching are part of
the measured cost.

The original weight slice and raw scale remain separate from the kernel view.
CG/LSMR handoffs still receive the original supplied values; independent raw
certificate moments still multiply by the original scale. Relabeling a
constant AW/FW/PW as public unit weights would violate sample-size, singleton,
DoF and covariance rules and is not this prototype. Public preparation,
inference, sample and transaction code is unchanged. Existing admission of
constant weights to weighted Schur also remains unchanged.

## Screen and evidence

[Identities](identities.json) bind all 152 native inputs, the one-file patch,
both isolated universal plugins and the unchanged installed artifact. Builds
use Rust 1.85.1 with locked dependencies. The prospective [protocol](protocol.md)
specifies four targets plus two controls at seed 104729, rotated order, fresh
Stata/MP 19 processes on the local M2 Ultra and small untimed warm-ups. The
narrow serial target was included before any timings because weight arithmetic
could occupy a larger fraction there. No build or test runs alongside timing.

There are **six pairs / twelve successful measured commands**. All target raw
weights equal 128. Native thread counts are independent of Stata's eight-processor
license cap; macOS affinity is uncontrolled. These single-seed screening results
are not repeatability, external-comparator, SCC or qualification claims.

| Target | N / K / threads | Command seconds | Change | Native phase seconds |
|---|---|---:|---:|---:|
| AW one FE, narrow IID | 983040 / 1 / 1 | 0.239 → 0.237 | −2 ms / −0.8% | 0.17615 → 0.17536 |
| AW one FE, wide IID | 983040 / 50 / 16 | 0.627 → 0.614 | −13 ms / −2.1% | 0.24329 → 0.24158 |
| FW one FE, clustered | 245760 / 50 / 4 | 0.402 → 0.407 | +5 ms / +1.2% | 0.29432 → 0.29648 |
| AW strong two FE, Schur | 245760 / 50 / 4 | 0.312 → 0.315 | +3 ms / +1.0% | 0.20556 → 0.20837 |

The apparent 13-ms wide gain is mostly outside the modified kernel: native
time decreases only 1.7 ms, input 6.6 ms and outer Stata work 4.6 ms. Initial
solver setup/solve changes 0.00941/0.02453→0.00910/0.02438. Narrow setup/solve
is slightly slower, 0.00475/0.03063→0.00492/0.03085. These phase measurements
do not establish a material kernel gain. No full matrix copy, projection,
certificate, setup build or fallback is removed. Original raw-weight storage
and three validation traversals remain; division/multiplication and group-mass
accumulation are omitted only inside constant normalized kernels.

Nonconstant AW robust and weak-AW controls remain 0.245→0.245 and 0.058→0.058
seconds. No command crosses the protected max(10 ms, 5%) slowdown envelope.
All initial/final solver, iteration, pilot and retry fields are exact; MAP,
Schur CG and weak LSMR choices are retained. No fallback occurs in the captures.

The four target native peaks are exact at 94.57, 933.73, 236.41 and 325.06 MB,
inside unchanged models. Sampled process RSS moves in both directions: narrow
AW 0.204→0.190 GiB, wide AW 1.530→1.398, FW cluster 0.383→0.446 and Schur
0.457→0.462. No causal RSS improvement is claimed from this screen. The weak
control is 27.05→27.21 MB against the unchanged 13.03 MB model; its existing
model discrepancy remains open. All scratch peaks remain zero. Full numeric
results and phases are preserved in [comparisons](comparisons.json),
[summary](summary.csv) and the [checksummed archive](archive.json).

## Validation and disposition

Every pair has exact full b/V, sample, status, DoF, fit/certificate, FE/cluster/
subset exports and non-time solver diagnostics. Existing independent runtime
rank, FE, score, bread, covariance and determinant certificates pass.

[Validation](validation.json) records two focused Rust tests and passing
universal static/native arm64 ABI checks. The new test verifies constant
projection equality to the established unit path, pointer-preserving raw
handoffs and correctly scaled raw certificates at 1/7 workers. It includes
the minimum subnormal, minimum normal and maximum finite weights, a one-ULP
nonconstant pair and normalized-underflow rejection. The existing weighted-cut
and common-scale Schur test also passes. A new independent full-estimation
oracle and additional timing seeds were intentionally not run: the prospective
screen did not justify further adoption work. Numerical equality alone is not
an adoption decision.

The prototype's exact source/patch was verified before reversing it from main.
No production change or new ADR is adopted. P2 constant specialization is
evaluated-and-rejected for insufficient demonstrated benefit; fresh low-memory
one-FE blocks are next. P0's reporting gaps and P3–P7/R1–R5 remain open. No
agents, dependency update, cluster work, installed-dist rebuild, qualification
or push occurred; unrelated Veneto changes and historical evidence remain.

Replay `compare.py` with existing `benchmarks/local_fixest_gap` modules on
`PYTHONPATH`, `FEREG_LOCAL_BENCH_ROOT` selecting isolated baseline/candidate
trees and `STATA_MP_PATH` selecting local Stata. The committed runner is
byte-identical to the measured runner. Licensed logs, datasets and binaries
remain outside Git.
