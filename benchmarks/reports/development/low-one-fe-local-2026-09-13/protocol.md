# Low-memory fresh one-FE blocks: prospective local experiment

Accepted native baseline 4e18c223a6b741b04bbcee35d10790729f5c8171;
repository checkpoint 01f95bdf9918bbebacbad353ef37a7be6bf9badd. Both artifacts
are isolated; 152 native inputs bind the candidate. Installed dist is preserved.

The candidate marks raw outcome and regressor blocks as fresh, consumes the
one-column projected outcome allocation, and reuses certified residual norms
only when the linear frame uses the projector's max-weight normalization.
Raw reference norms, final warm residuals, scratch layout, QR and memory
admission remain. Supplied weights incur a scalar max pass for this guard.

Before timing: focused low-memory, weighted-divisor, rank and memory tests;
both artifacts run the same explicit interaction-dummy Stata oracle with
unit/AW/FW/PW and legal IID/robust/clustered inference, frequency replication,
rescaling, rank deficiency and original-row residuals. Failure is retained.

Primary screen at seed 104729, rotated order:
- AW simple one FE, N983040/K1/thread1, pool1, IID;
- unit simple one FE, N245760/K10/thread4, pool10, IID;
- PW simple one FE, N245760/K16/thread16, pool5, two-way clustering, missing.

Require at least 5% AND 10 ms complete-command improvement, OR at least 8%
native action-peak reduction with command cost within max(10 ms,5%). If a
primary qualifies, repeat all primaries with seeds 8675309 and 20260819 and
rotated arm order; decide from repeated paired results and protected costs.
Otherwise archive and reject this bounded prototype without a larger campaign.

Protected single pairs: small AW/robust pool1, FW/interacted/cluster pool1,
AW/interacted saved FE and residuals, AW absorbed slope, weak AW two FE,
no-FE PW/robust and unchanged fast IID. Use seed31415927, rotate arm order.
Investigate increases above max(10 ms,5%); retain every observation and any
recheck, with no favorable replacement. Include existing low memory admission
and failure tests; observed/model gaps remain explicit unresolved gaps.

One separate phase-traced qualifying primary after ordinary measurements.
No build/test or other benchmark during timing. Fresh local Stata/MP processes,
small untimed warmup, existing input hashes and runtime receipts. Record full
b/V, sample/status/DoF, solver family/iterations/retries/fallback, independent
certificates, setup/solve timing, total/input/native/outer, native/model/scratch
peaks and sampled process RSS. Original registered numerical tolerances apply;
require exact discrete decisions unless the audited fresh-input contract
predicts a change in actual projection iteration counts. No hardware-traffic,
complete discarded-attempt cost, cross-platform, comparator or release claim.
