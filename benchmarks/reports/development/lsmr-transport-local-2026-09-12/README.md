# Borrowed LSMR transport: adopted local lifetime improvement

Adopt [ADR 0048](../../../../docs/adr/0048-borrowed-lsmr-transport-lifetimes.md)
against `15bd21dd978d014bc477bac072e504b83c8f4dba`. Delay the wholly overwritten
candidate allocation until the upstream solve ends; release RHS transport
before materialization, then release demeaned output and unrequested coefficient
storage before independent certification. The input and requested coefficients
remain available for their original scientific consumers.

[Identities](identities.json) bind all 150 native inputs and the isolated
universal plugins. Only `fe/borrowed_map.rs` changes among those inputs. Baseline
plugin is `6274ed404586b728dbdffeb62d449788b23dfdeed8b0106068e27d4599db40ed`;
candidate is `6c364b062dc505f478ad9079a8106db4e89c0aed4079735ad89d939f7d1a0497`.
Adopted native source matches the measured snapshot. Installed dist is untouched.

## Complete-command and memory evidence

Seventeen successful pairs / 34 measured commands use fresh local Stata/MP 19
processes after untimed warm-ups on the M2 Ultra. Six pairs are repeated affected
primaries, six are OLS controls, four are separate pilots, and one is a traced
PPML control. Primary arm order rotates over three seeds. Native threads are
separate from Stata's eight-processor license cap; affinity is uncontrolled.
These are development findings, not external-comparator or release claims.

| Automatic weak-graph target | N / K / threads | Command medians | Median paired command change | Native peak medians | Peak reduction |
|---|---|---:|---:|---:|---:|
| AW, one-way cluster | 30,720 / 10 / 4 | 0.060→0.060 s | −1.6% | 30,372,354→27,312,122 bytes | 10.1% |
| AW, robust, beyond Schur width guard | 122,880 / 65 / 4 | 0.997→1.013 s | +0.9% | 469,170,277→403,306,597 bytes | 14.0% |

Adoption rests on repeatable lower allocation, not a command-speed improvement.
The wide primary saves exactly 65,863,680 native peak bytes in each pair,
matching one 122,880×67×8-byte matrix (65 regressors, the normalization constant
and response). The smaller primary savings range from 2.995 to 3.195 MB;
allocator scheduling changes the exact peak. Independently calculated median
ratios and medians need not agree.

On the wide case, input medians are 0.04827→0.04853 s, native work
0.92033→0.93570, and outer work 0.02819→0.02879. The three total increases are
5, 9 and 27 ms, within the prospectively declared max(10 ms, 5%) review envelope.
The small primary's native medians are 0.04895→0.04921 s. The allocation change
does not remove a solver iteration, fallback or independent certificate.

Native scratch remains zero for these fast cases. Primary sampled RSS medians
are 0.1465→0.1451 and 0.6769→0.6701 GiB. RSS is a separately sampled process
measure; no universal RSS saving is claimed. Public selected-model bytes remain
13,394,888 and 268,552,608 respectively: **both native peaks still exceed their
reported models**. This report does not close P0 memory admission/reporting.

## Coverage, pilots and protected requests

The [original protocol](protocol.md) and prospective
[first](followup-protocol.md) / [second](wide-auto-protocol.md) follow-ups preserve
the route-discovery sequence. The initial K50 automatic pilot selected CG and
retained exactly the same peak. Explicit fast simple OLS LSMR uses the unchanged
owned adapter; its pilot and three repeated controls also retain identical
371,860,382-byte peaks. These are controls, not evidence for a wider borrowed-path
benefit. The K65 automatic route is outside the existing Schur width envelope
and selects the changed borrowed LSMR path in both arms.

Small/narrow weak OLS improves 22→17 ms and saves 184,320 native bytes, but still
uses about 55 MB against a 1.045 MB public model. Strong weighted Schur improves
104→97 ms with unchanged peak; complete one-FE IID is 585→586 ms, unchanged peak.
These single pairs are protected controls, not repeatability claims.

The PPML weak PW/offset/cluster checkpoint control is 257→267 ms (+3.9%), with
native peaks 78,869,206→78,867,934 bytes, inside its 123,416,310-byte model. All
seven outer / 127 reported inner iterations, zero halvings, zero fallback and
zero strict profile rejections agree. Strict/final time increases about 8 ms;
no PPML speed or material peak saving is claimed.

Two further requests fail closed on both artifacts. Diagnostic workers expose
the same errors without relaxing any estimator checks:

| Protected request | Baseline / candidate | Error |
|---|---|---|
| Weak N30,720/K10, FW cluster, explicit Schwarz LSMR, low memory | r(459) / r(459) | borrowed MAP attempted to construct an uncertified result |
| Weak N7,680/K10, AW, saved FE/residuals, explicit Schwarz LSMR | r(459) / r(459) | borrowed absorption coefficient product is not certified and aligned |

[Failed requests](failed-requests.json) retain three original timed failed calls,
four separate diagnostic failed calls, and one untimed warm-up failure.
The warm-up failure appended a duplicate solver option; the driver was corrected
to use the existing worker solver argument and both pilot arms reran under a new
prefix. Neither certificate failure is fixed or counted as passing scientific
evidence. The two successful independent oracles below provide separate retained
batch and saved-output coverage; they do not erase these failures.

## Scientific validation and cost attribution

Every successful pair preserves full b/V, sample hashes, status/DoF, FE/cluster
tables, original certificates and non-time solver decisions exactly. No tolerances,
weight semantics, rank policy, convergence criterion or output rule changes.
[Validation](validation.json) records 62 focused Rust tests, covering retained
batches, original-reference continuation, forced CG continuation, coefficient
reconstruction and constrained admission. Both artifacts pass the existing
`ols_lsmr_reuse.do` and `within_weighted_ols.do` independent explicit-dummy
fixtures, with explicit r(0) sentinels. These cover AW conventional/robust
covariance, residuals/saved FE, pool sizes 1/5/10, and one/seven threads.
Universal binary audit and actual native arm64 ABI/self-tests pass.

The removed input-copy payload is one 8NR-byte matrix; its nominal read/write
traffic is 16NR bytes. During upstream Krylov work the candidate is no longer
live. During final certification the RHS and returned demeaned banks, plus any
unrequested coefficient bank, have already been released. These lifetime and
traffic counts are source-derived, not hardware counters. Output zeroing and
the required column-major-to-row-major copy remain. Timings do not establish a
separate kernel-speed gain. The allocation/model diagnosis is in the
[P0 audit](memory-audit.md).

## Replay and evidence

[OLS comparisons](ols/comparisons.json), [PPML comparisons](ppml/comparisons.json),
their summary CSVs and checksummed numeric archives retain complete successful
exports, imported-input hashes, phase/solver diagnostics, modeled/native/scratch
and sampled RSS. Raw licensed logs, binaries and datasets stay outside Git.

Copy the bound baseline and candidate snapshots into a scratch directory and
select it with `FEREG_LOCAL_BENCH_ROOT`. Set `STATA_MP_PATH` to licensed Stata
and put `benchmarks/local_fixest_gap` on `PYTHONPATH`. The archived `compare.py`,
`wide-auto.py` and `ppml-control.py` use the existing paired workers; the recorded
protocols distinguish pilots, repeated controls and affected primary cases.
The driver intentionally stops on failed estimation: replay the failing controls
separately and retain their errors. No SCC, full qualification, dependency
update, installed-plugin replacement or push ran.
