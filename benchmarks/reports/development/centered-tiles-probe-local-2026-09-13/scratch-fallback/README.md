# P5 centered scratch fallback: unadopted local prototype

This extends the [finite adapter](../finite-adapter/README.md) against accepted
`436fe2a295dfe96ba25f00f8eddb43a9352aa22d`. Only its test-only module changes
from that stage; all other 180 bound inputs are exact. The [receipt](receipt.json)
binds the current 181 inputs and [complete prototype patch](prototype.patch),
SHA-256 `388fde52b7eb0494a6c7e72404b62125d436f78382c55fd112d65ea33390111e`.
Earlier reports and patches are unchanged.

## Implemented fallback

After a guarded finite solve declines, the prototype can stage centered columns
into the existing private column-major OLS scratch owner using one N-element
transport buffer. It calls the established low-memory Householder QR with the
original reference norms, current within norms and unchanged weight/reference
scales. QR owns numerical omissions. It introduces no streaming-QR/PPML rank
policy and no resident N-by-K within matrix. The transformed scratch and outcome
are dropped before residuals are reconstructed from immutable centered columns.

Disk bytes are checked before creation. A conservative cumulative cap charges
all fit-reservation capacities and the scratch I/O buffer, including allocations
whose owners have already been dropped. Each allocation is checked before
reservation and its actual capacity is checked afterward. This cap is not the
complete estimator model: original inputs, group means, caller moments, metadata,
worker/runtime allocations and subsequent inference/output lifetimes remain
outside this particular counter. Full overlapping admission is still required.

Scratch files use the existing private create-new/RAII owner. A test observer
captures only their path identity to verify deletion; it is not a public API.
Workers read native immutable columns and disjoint output cells; no SPI occurs.

## Evidence

The final source passes **24 distinct focused tests**: 14 selected by `centered_`
(13 prototype tests and one existing slope-recovery test), plus ten existing
scratch-storage tests. All commands and terminal counts are retained in the
receipt. The previous 97-test/core-check receipt belongs to the preceding
source and is not relabeled as a new run.

Twelve forced-fallback comparisons cover exact deficiency and clearly weak full
rank: N257/K10 under unit/AW/FW/PW, N1024/K65/PW/four workers and
N65537/K3/FW/four workers. The last case crosses fast QR's parallel dot-reduction
boundary. Every rank/status, kept/pivot ordering, dropped coordinate and relative
rank threshold agrees with the established matrix result. The original exact
centered-tile/matrix oracle is retained alongside the new scratch comparison.

Scratch QR has different standardization, moment and parallel-reduction
arithmetic from fast QR. Maximum absolute coefficient/bread/residual differences
across these deliberately weak and deficient controls are
**8.79e-11 / 8.75e-7 / 5.90e-13**. Every coefficient, bread and residual entry
passes the existing mixed absolute/relative envelopes. The raw bread difference
must not be compared with an absolute-only cutoff: its asserted envelope is
`1e-10 + 1e-6 * abs(reference)`. Both implementations retain their own rank and
inverse certificates. No bitwise equality or published-covariance claim is made
for the scratch extension.

An independent eight-row rational contrast gives within `XX=111`, `XY=217/2`,
coefficient `217/222`, FW bread `1/111`, and AW/PW bread `5/222`. The new path
matches these values and each analytic centered residual at 1e-13/1e-14 bounds.
Literal integer row expansion under structural unit weights matches the FW
answer independently. An exact duplicate has the expected canonical omission.
An all-absorbed model returns rank zero with empty bread and the expected
within outcome. A synthetic overflowing centered column is rejected; finite
placeholder norms deliberately reach the scratch write guard, and that invalid
frame is not represented as a scientifically valid fit.

For the 64-row allocation fixture, all **23** injected fit-reservation failures
return an error. Every file created before failure is removed. The same exact
path checks pass after success, nonfinite centering, and a cap one byte below
the sufficient payload. Disk/staging admission failures occur before the first
fit reserve and before scratch creation. The cumulative payload boundary is
4,395 bytes and the scratch file is 1,536 bytes for that fixture.

## Costs and limitations

These are source-derived file lengths and observed cumulative reservation
charges, **not native peak memory or command timings**:

| Forced fallback | Charged fit/I/O payload, bytes | Scratch file, bytes |
|---|---:|---:|
| N257, 10 active columns, four weight kinds | 40,292–41,028 | 24,672 |
| N1024, 65 active columns, PW | 795,859–800,555 | 548,864 |
| N65537, 3 active columns, FW | 3,670,685–3,670,917 | 2,621,480 |

Stored generated width includes the interior absorbed column and reported
intercept. Staging writes those columns too; Householder QR then makes its
existing repeated column reads/writes. The scratch I/O payload can be as large
as 1 MiB. These costs prevent inferring an end-to-end or peak-memory win from
the absence of a resident within matrix, especially for small/narrow fits.
No new plugin, licensed oracle, public command comparison or timing campaign
has run. Existing production source and installed dist are unchanged from the
previous prototype stage, which itself remains unadopted.

The rank-zero/error test initially used a nonexistent error accessor method;
the preserved compile failure was corrected to the existing public field. All
numerical tests passed when compiled. Intermediate green logs remain separate
from final source-bound results.

Next bind the actual raw-reference cache and immutable frame, independently
certify initial projection and final residual/score reconstruction, integrate
original-row robust/clustered inference and saved outputs, and complete memory
admission. Then compare complete command/phase time, observed native peaks and
fallback I/O against isolated accepted artifacts, including small/narrow and
protected graphs. PPML still needs its own changing-weight/checkpoint evaluation.
Every P5 checkbox remains open. No agents, cluster work, qualification or push.
