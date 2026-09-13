# P5 near-capacity QR scratch repair: unadopted core evidence

A new native-entry test against the [parallel-FE candidate](../parallel-fe/README.md)
exposes a real failure: N16384, K16, 7736 two/three-row FE groups, with one exact
duplicate column. The existing matrix MAP/QR route succeeds with rank 15;
the eligible centered route fails `Allocation: centered scratch staging exceeds
its budget`. Its 2,228,224-byte displaced-matrix cap includes 1,113,984 bytes
of means, just 128 bytes below the half-bank cutoff. The scratch owner requested
a 1 MiB I/O buffer even though this QR caller transfers only a 128 KiB column.
The [reproducer patch](reproducer.patch), failing source manifest and log preserve
that failure; it was not dismissed as an unsupported model.

The isolated repair reuses the same scratch owner with a column-sized buffer
capped at its existing 1 MiB maximum. File layout, read/write logic, finite-value
checks, private-file cleanup and QR/rank arithmetic are unchanged. Ordinary
column-major and row-major constructors retain their previous buffer policy.
Centered staging preflight uses the same column bound and charges actual buffer
capacity before further work. The near-limit test now passes for unit/AW/FW/PW
under fit-only, strided and packed score-cache policies: twelve rank/residual
comparisons against the matrix route. No numerical tolerance is relaxed.

The [182-input final snapshot](receipt.json) changes only centered finite code
and the scratch module since the measured parallel-FE stage. All **64 distinct
focused tests** and non-test core checking pass. The new scratch test checks
empty, small, 128 KiB-column and above-1 MiB-column shapes, full linear transfers,
column reads, ranges crossing column boundaries, overflow and cleanup. Existing
row-major, shared positional read, poisoning, layout and failure tests pass.
Independent weighted/frequency QR, covariance, scoped/saved-output and allocation
checks remain included in the final centered suite.

The initial cross-column test range went one value beyond its two-column file
and correctly failed. The fixture length was reduced to the actual file extent;
production range validation remains unchanged. That failed log and the original
candidate5 source snapshot are preserved separately from final candidate5-final.

This is a core correctness/ownership repair, **not an adoption or a measured
command gain**. No plugin was built from this final snapshot and no public
command timing/native-allocation observation is attributed to it. The preceding
40–43% native-peak reduction and IID/AW/PW first-screen command changes belong
to immutable plugin `41555c29`, before this QR-specific change. Scratch I/O and
complete-command fallback cost need measurement. Full overlapping admission and
other boundary shapes still require review; this twelve-case family does not
prove the whole memory envelope. Remaining IID overhead, protected controls,
repeated primary seeds and separate PPML evaluation stay open. Installed dist,
unrelated work and all historical receipts are preserved.
