# Serial slope setup and packed-cache profile

Six diagnostic calls compare the current `rrc1` plugin with instrumented `sp2d`
in AW K10, PW K6 with three slopes only, and AW K50. All use N=245,760, one
worker, robust inference and full outputs. Every call passes; the three native
comparisons have exact numerical exports, samples and output arrays. The
instrumented plugin passes the actual ABI check. These single traces locate
costs and do not establish a performance improvement.

| Case | Current slope setup | Instrumented slope setup | Full-sample packed-cache row pass |
|---|---:|---:|---:|
| AW K10 | 12.63 ms | 12.88 ms | 10.13 ms |
| PW K6, three slopes only | 25.80 ms | 24.56 ms | 5.64 ms |
| AW K50 | 12.23 ms | 12.83 ms | Not used |

Setup is read from the initial solver report, row 1/column 9, whose source
contract is `write_solver_row` in the plugin. It is included in the coarse fit
`rhs` phase and must not be added to that phase a second time. The new cache
timer separates allocation, row copying/raw-mean accumulation and final merging.
For AW K10 and PW K6, allocation takes 0.31/0.19 ms and merging is below 0.001 ms.

The measured K50 fit uses the strided score cache. A packed-cache trace with
N=7,680 belongs to warm-up, not the measured fit. The original audit retained
the last event by label; the second audit preserves all cache events and selects
measured events by observation count. Both auditor sources and receipts remain
archived. No warm-up timing is used as a full-sample phase measurement.

These findings prioritize the serial slope-geometry construction, particularly
PW's roughly 26 ms setup. Its raw Gram and independent rank-aware QR loops merit
separate measurement before changing them. Packed-cache raw-column lookup is
a smaller opportunity. An existing bounded numeric-column binding could avoid
repeated recipe dispatch while preserving raw-mean scaling and fixed-block
reductions; this remains unimplemented and unmeasured.

The archive contains the six applications, complete `rrc1`/`sp2d` source
identities, instrumenter/build/controller sources, filtered traces and both
audits. Integrity verifies 270 stored entries, six omitted local arrays and two
source reconstructions. Raw licensed logs and binaries are excluded. The six
earlier graph/window drafts remain part of the captured sources, with their
Linux acceptance still pending. There is no new production change, solver rule,
performance acceptance, installation, push or release in this diagnostic.
