# CMG container admission checkpoint

Development snapshot `5240e58-cmg-containers1` is based on main
`5240e589400c888690666265263ad96f9b694aa6` plus the recorded working changes.
This is an incremental comparison with `5240e58-input-memory3`, not committed
main or formal release evidence. The production CMG pin remains `90e1fe0`.

The supplemental bound counts hierarchy/workspace descriptor banks, Arc headers,
component/permutation spare capacity, lazy workspace growth, old/new container
banks, and terminal LDL packed storage overlapping dense construction. Ordinary
and retained-session admission use the same checked helper. The retained report
is a bound, not an exact allocation claim. Numerical kernels are unchanged.

Job 7606295 passes 38 CMG, 137 plugin and 46 PPML-memory checks (221 total),
including exact/one-byte-short admission, small/disconnected/tiny-weight graphs,
lazy workspace growth and overflow. Linux build/export/actual-loader checks pass.
The job then exits 1 on a missing harness identity key before estimation. The
original failure is preserved. Job 7606394 changes only that wrapper metadata;
it reuses identical sources/artifacts and passes 9 alternating pairs, 18 fresh
Stata commands, all terminal sentinels and full-node isolation checks. Both
reserve 28 cores and execute on four physical cores. Scheduler receipts and
original log hashes are included; raw licensed logs remain on SCC.

| Case | Baseline median seconds | Candidate median seconds |
| --- | ---: | ---: |
| Small CMG OLS | 0.081 | 0.075 |
| Frequency-weighted clustered CMG OLS | 0.205 | 0.215 |
| CMG PPML | 0.278 | 0.264 |

Scientific matrices, sample/order, solver non-time fields and saved outputs
match exactly. No command or reported phase exceeds both 5% and 10 ms slowdown.
[Phase costs](phase-costs.json) include allocation, RSS, scratch and PPML fallback
counts. OLS native allocation medians match; PPML rises from 10,004,442 to
10,431,056 bytes. The cause is not established and no memory-reduction claim is
made. Candidate artifact SHA256 is
`0ed36e83cc3d0a6386e766f25508efd2396fd2c19c05d5b6bc0c2714bbcf0205`.

Job 7606305 separately passes 18 tests on an isolated CMG joined-worker-shutdown
draft. The [patch](cmg-joined-shutdown-draft.patch) is unintegrated and does not
change production dependency pins. Its later integration experiment is recorded
in the [worker-lifetime report](../candidate-worker-lifetime-2026-09-16/README.md).

Whole-action coverage remains incomplete. This supplement does not prove all
hierarchy-build temporaries or Rayon/crossbeam runtime storage. The whole-action
ratio is undefined and publication eligibility is false; see
[validation](validation.json), [jobs](jobs.json), and the
[source-bound dependency inventory](dependency-source.json).
