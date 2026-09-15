# German 5% worker comparison on SCC

The user requested a bounded cluster comparison of feppml with ppmlhdfe and
fereg with reghdfe, and authorized storage in the project. This is a separate
new development run; it does not restart the stopped optimization campaign.

Source is `06609452bab13516a84a48363014c6685482f7d4`, containing the committed
memory and checkpoint repairs. The isolated Linux plugin is `83d27773`;
source archive, full plugin digest, job IDs and script hashes are recorded in
`submission-evidence.json`. Main and installed Mac dist are unchanged.

Sampling selects workers deterministically at probability 0.05 using
splitmix64 of the original worker identifier XOR seed 20260915. All selected
worker observations in 2001–2009 are retained, in original row order. This is
approximately 5% of workers and rows, not an exact fixed-size 5% row draw.
The common Stata input is built on SCC after verifying the original Parquet
checksum; its roundtrip and selection receipts are required before fits.
No wage trimming, keepsingletons or external singleton removal is applied.

OLS uses logrwage2 and conventional covariance. PPML uses exp(logrwage2) and
robust covariance. Both use the same 18 age-profile/year coordinates and
worker/establishment absorption. Native commands request auto, fast memory,
four threads (the SCC Stata license limit); PPML uses pool four. Each family reserves 16 slots/48 GiB and
runs two alternating fresh-process pairs on one node. The native memory cap
is 40 GiB, leaving 8 GiB for Stata and driver overhead. References are pinned
by the repository lock and reverified before each family starts.

## Accepted smoke evidence

The isolated shared build/smoke job 7580010 passes both standard OLS and PPML
fixtures. Initial launcher job 7580017 and capped launcher job 7580023 pass
all four commands on a 9,000-row synthetic worker panel. Independent audit
job 7580039 passes sample/order, common b/V and normalized DoF comparisons,
OLS normal-equation/model-space checks, native PPML score/model-space/replay,
and ADR 0026 objective adjudication. These synthetic runs validate the launcher
and audit, not empirical performance. Scheduler accounting is retained separately.

Synthetic OLS maximum coefficient difference is 5.00e-15, covariance 1.22e-18;
PPML differences are 8.88e-16 and 6.75e-14. The reference PPML reconstructed
mean passes stationarity but its coefficient-binding model-space check remains
indeterminate. It is retained as a diagnostic under ADR 0026, not promoted to
an objective witness. The native public mean passes all independent checks.

The initially planned eight-thread request was corrected before any empirical
fit: the smoke logs identify a four-core Stata license. Both native and reference
commands therefore use four threads, matching the successful smoke.

The final capture adds OLS solver/FE/runtime/receipt matrices to the already
exercised post-timing export loop. Its pre-timer and estimation bytes match
the successful launcher. The exact smoke and final capture files are preserved.

## Empirical run pending

- 7580024: verified input and common worker-sample preparation; initially held
  until the data transfer completes.
- 7580025: fereg versus reghdfe, dependent on preparation and launcher smoke.
- 7580026: feppml versus ppmlhdfe, with the same dependencies.
- 7580041: independent comparisons after both families and audit smoke.

No empirical sample count, estimator success, speed ratio or full-data PPML
convergence is established here. Collect complete scheduler accounting, explicit
application receipts and scientific outputs before accepting any comparison.
Report command/phase times, process and native memory, fallback/retry costs,
common sample/rank/DoF, b/V and independent certificates. GNU time logs and
SGE accounting supply process resource costs; native public metadata supply
phase/allocation/solver costs. The copied PPML pair helper's old local timing
label and macOS RSS parser do not define the SCC timing or resource evidence.

The existing full local PPML fit and its monitor remain separate. Source and
data transfer overlap it; no local build or numerical audit was started for
this SCC comparison. That overlap must be disclosed if reporting local full
run timing. No push or release qualification is implied.
