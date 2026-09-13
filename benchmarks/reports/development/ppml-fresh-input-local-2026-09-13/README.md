# P4: evaluate and reject standalone fresh-input certificate removal

**Disposition: evaluated and rejected.** Retain accepted native `ccc1f33` and
its same-call current-frame norm reuse. The isolated prototype and every
measurement remain available here; no result applies to a different artifact.

## What the prototype evaluates

The [patch](implementation.patch) explicitly marks freshly copied responses
and freshly constructed design blocks at five retained-session call sites.
A new categorical initial entry skips the raw certificate only for one FE:
project once and then independently certify against the original raw reference
norms. That first sweep counts against the unchanged iteration budget.

The ordinary entry still checks a warm candidate at iteration zero. Fused
response/design refresh, finite residual checks and dense/scratch retained
reprojection keep that entry. Multi-FE categorical calls use the original
initial-check path; generic/slope/low-memory projectors delegate unchanged.
The prototype changes no weights, raw reference scales, rank/QR policy, final
certificates, memory admission, topology, worker partition or public transaction.

The omitted work is the raw FE certificate's group-mean/scaling traversal.
Raw reference-norm construction, projection, final certification and fresh-design
compaction/norm arithmetic remain. No matrix copy, allocation or retained norm
buffer is removed. This isolates the fresh-certificate question from the larger
remaining matrix/pass opportunities.

## Prospective decision and complete-command results

The [protocol](protocol.md) requires a repeated saving of both 2% and 10 ms
on a primary, a corresponding changed-phase saving, and no unexplained protected
increase beyond max(10 ms,5%). One initial target exceeded the command screen,
authorizing two further seeds for all three targets. Nine primary pairs, five
controls and two low-memory control rechecks give **16 pairs / 32 commands**.
No private allocation audit ran after the adoption decision became negative.

All arms use fresh local Stata/MP 19 processes on the M2 Ultra, a small untimed
warm-up, rotated order and isolated source-bound plugins. Native workers are
separate from the license processor cap. Affinity is uncontrolled, and no builds
or tests run during timing. These are local development findings.

| Public target | N / K / workers | Command medians, seconds | Median paired change |
|---|---|---:|---:|
| Complete one FE, robust | 983040 / 50 / 16 | 9.683 → 9.751 | +0.7% |
| FW, exposure, sparse missing, robust | 245760 / 50 / 4 | 3.223 → 3.209 | −0.8% |
| PW, offset, sparse missing, two-way cluster | 245760 / 50 / 16 | 2.979 → 2.821 | −4.7% |

Median paired changes need not equal the ratios of the displayed medians.

The PW command result passes the numerical gain threshold, but its gain is
predominantly in unchanged finite solving. Median finite-solve time falls
0.8643→0.7388 s while the changed projection scope moves only
0.3662→0.3621 s; strict/final moves 0.8493→0.8388 s. These independently computed
scopes overlap and must not be added. The PW pair differences are −138, −24
and −245 ms. The existing numerical/discrete checks remain exact throughout.
No commensurate reduction in the changed work establishes the apparent large
command gain; its cause remains unresolved and is not claimed as an optimization.

The complete and FW projection medians fall 1.1411→1.1177 and
0.4715→0.4575 s, but their command medians do not meet the adoption threshold.
The initial complete screen saves 28 ms; the repeats cost 68 and 213 ms.
FW pairs save 5, 47 and 26 ms. The source removes real certificate work, but
this standalone interface/control-flow change does not establish a material,
explained complete-command benefit on the selected workload set.

This rejection is not a proof that every fresh-input optimization is futile.
It does not reject fused fresh-design norm construction, centered tiles, or a
future implementation with a different layout and a demonstrated command gain.

## Protected cases and memory

The initial low-FW control slows 0.756→0.828 s (+72 ms,9.5%), exceeding the
review envelope. Its strict/final scope rises 0.2320→0.3045 s while projection
falls 0.1737→0.1698 s. Reversed/rotated independent-seed checks give
0.715→0.619 and 0.683→0.712 s. All three observations remain; their changing
direction does not establish a causal regression or explain away the original.
The median paired change is +4.2%, distinct from the ratio of command medians.

Other protected single pairs are weak two FE 0.136→0.134 s, small one FE
0.030→0.027, saved/separated 0.028→0.026 and strong explicit two-FE MAP
0.086→0.085. These controls do not support independent speed claims.

All sixteen native peaks are exactly unchanged and within unchanged models.
Primary peaks remain 2522.61, 633.49 and 645.34 MB against models of
2962.95, 745.43 and 935.51 MB. Primary sampled RSS medians are
1.9587→1.9590, 0.5259→0.5329 and 0.5422→0.5598 GiB; low-FW medians are
0.0741→0.0914 GiB. No native or resident-memory saving is claimed.
Full phase, model, scratch, RSS, iteration, retry and fallback observations are
in [comparisons](comparisons.json) and the [checksummed archive](archive.json).

## Scientific validation, identity and restoration

Full b/V exports are byte-identical in all sixteen pairs. Samples, rank/status,
DoF, weight/cluster/separation structure and discrete solver decisions are exact.
Independent runtime certificates pass; saved outputs meet unchanged tolerances.
No measured first-sweep count exception or comparator relaxation was needed.

[Validation](validation.json) records 28 distinct candidate tests in a fresh
candidate-only Cargo target. The weighted group-mean oracle compares fresh and
prior projection values exactly at 1/4/16 workers and changing weights/widths.
Warm residuals still return at iteration zero. Tests cover fresh zero matrices,
one-sweep accounting, zero-budget rejection, one-sweep noncertification,
unchanged multi-FE warm checks, current-frame norm reuse, strict rollback,
stale-rank rejection and generation invalidation. Both independent full-dummy
Poisson/Hessian/sandwich Stata oracle arms pass for unit/FW/PW and robust/clustered
inference with offsets. No scientific expectation or tolerance was relaxed.

[Identities](identities.json) bind all 152 native inputs and both artifacts.
Candidate SHA-256 is `ffead1551a48bdf5706a0a7b929f704de5e847f4caf16a259ad70b6262caabc5`;
baseline is `a4d4a33aeb8ac6a0c870f38bb86b57d1b10cd81386c004dccb0970b1a992cfee`.
Changed source mtimes were refreshed before using the release cache, and both
architectures visibly compiled the changed core. Universal static audits and
the actual native arm64 ABI/self-test pass.

After archiving and verifying the exact source/patch, only this prototype was
reversed. All 152 restored native hashes match accepted `ccc1f33`.
The restoration check is recorded in validation. The installed plugin, all
unrelated Veneto work and historical receipts remain unchanged. No new ADR is
needed for a rejected experiment. No push, agents, cluster work or qualification.

For replay, place the included scripts in an isolated experiment root containing
baseline/candidate source and artifacts. Set `FEREG_LOCAL_BENCH_ROOT`, `PYTHONPATH`
to `benchmarks/local_fixest_gap`, and `STATA_MP_PATH`; follow the retained protocol.
Licensed logs, binaries, machine paths and datasets remain outside Git. The
P4 fresh-input/current-frame norm evaluation is complete: fused norm reuse is
adopted, standalone fresh-certificate removal is rejected. Broader fresh-design
norm construction and repeated-pass work remain open.
