# All eight original sparse-zero native failures confirmed repaired

SCC job **7763027** passes all eight original cases on accepted source
`a7b46c2f`, computational snapshot `1f290d1c…`, Linux plugin `8e2d5af2…`.
Both two- and three-FE sparse designs use N61,440/K10/T4, public automatic
PPML, default memory selection, robust inference and the original warm-up.
No new solver, tolerance or sample policy change was made for this check.

Generation ran under the pinned SCC Python environment. The full input hash
and every column's IEEE-754 hash match the original receipt before fitting;
Stata import hashes are checked again. An independently implemented graph
algorithm constructs exact integer FE potentials, proves zero on positive
outcomes and nonpositive on zeros, then applies independent singleton/zero-level
closure. Its retained original-row hash matches the native sample in every case.
The third FE and finite coefficients of that witness are exactly zero.

| FE partitions | Original seed | Proven separated rows | Further closure rows | Retained rows | Diagnostic command seconds |
|---|---:|---:|---:|---:|---:|
| 2 | 104729 | 631 | 3 | 60806 | 4.842 |
| 2 | 8675309 | 547 | 1 | 60892 | 5.105 |
| 2 | 20260819 | 641 | 0 | 60799 | 5.543 |
| 2 | 31415927 | 554 | 0 | 60886 | 4.986 |
| 3 | 104729 | 580 | 1 | 60859 | 12.184 |
| 3 | 8675309 | 554 | 0 | 60886 | 11.597 |
| 3 | 20260819 | 586 | 0 | 60854 | 12.170 |
| 3 | 31415927 | 543 | 1 | 60896 | 11.925 |

Every application converges and publishes the registered four-worker team.
The original four two-FE worker failures and four three-FE command timeouts
therefore have current successful replays, supplementing ADR 0230's earlier
single-seed and independent small Poisson/Hessian/sandwich evidence. The
integer witness independently proves this deletion/sample boundary; it is
not a new independent large-design coefficient or three-FE rank oracle.
All unchanged native current-weight, score, objective and final-Hessian
certificates remain mandatory.

These clocks are diagnostic on a shared four-slot node, not headline timing
distributions. No reference fits were rerun and no stock timing rank is created.
Original reference sample/separation differences and unavailable comparison
records remain distinct adjudication work. Historical failed receipts stay
unchanged; this result does not repair unrelated large timeouts or constrained
fit failures.

Accounting is clean: **98 seconds**, four slots on scc-tb4, failed=0 and
exit_status=0. Both application and wrapper terminal sentinels pass. The job
started after the host-wait timing job ended. Input/proof/native receipts are
retained with hashes; raw licensed logs remain on SCC.
