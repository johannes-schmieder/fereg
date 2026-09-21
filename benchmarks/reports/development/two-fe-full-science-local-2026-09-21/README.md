# Complete small two-FE science matrix, 2026-09-21

All 1,518 declared cells complete without a process failure. All 3,036 native
explicit-dummy checks pass. Baseline and candidate coefficients, covariance,
samples and requested outputs agree exactly in every cell. This is local
development science evidence; concurrent timings are diagnostic only.

The matrix covers eleven graphs, K=1/6, unit/AW/PW/FW weights, six inference
requests and no/saved/all outputs. Unsupported PW IID combinations are excluded
as declared. N=1,920, except weak graphs at their minimum registered N=7,680.
The [pilot report](../two-fe-science-pilots-local-2026-09-21/README.md) records
the size correction and independent dense-SVD/sparse-dummy cross-check.
FW inference includes literal observation expansion. No ordinary fixest
weight call is treated as an FW inference comparison.

The baseline remains `0efa78bd`, plugin
`bc9e5c6c522af7292757c6d20753a7671e9728baa75f8ee6ee67c6cb09c8453e`.
Candidate `slope-recovery2` is the production checkpoint committed at
`12de1adb`, plugin
`bef34c8c45052866f2bde2b52e0041183013f58e089ba86995f7b246cf3c9c41`.
Its categorical two-FE kernels are unchanged from the baseline. Executed
controller source is committed at `7f53dbe8`; captured worker/source hashes,
reference pins and individual run identities remain in the archive.

| Checks | Pass | Mismatch / failure |
|---|---:|---:|
| Baseline native oracle | 1,518 | 0 |
| Candidate native oracle | 1,518 | 0 |
| Default fixest oracle | 356 | 766 |
| Reference representatives at 1e-10 | 48 | 18 |
| Follow-up of those 18 at 4e-12 | 12 | 6 |

The default comparison failures remain explicit. Separate diagnostics use
66 graph/width/weight full-output, two-way cross-clustered representatives;
they do not rerun every inference request. Where the recorded correction
differs, full FE rank is requested only after the runner independently verifies
that neither effect is nested in either cluster. Tighter fitting and recovery
resolve sixty representatives. Six sparse-graph cases retain disagreements
under the unchanged coefficient/covariance/output tolerances. These results
establish neither universal reference parity nor a reference defect.

An intervening 1e-12 reference diagnostic failed all eighteen calls because
the pinned fixest requires a tolerance strictly above 2.2204e-12. Those failures,
their error text and successful native calls remain recorded; the supported
4e-12 attempts use new directories. Across all 102 repeated native calls,
six scientific exports and complete output vectors are byte-identical to the
original matrix. No diagnostic timing replaces a default timing.

The main archive has 4,158 application receipts. The reference archive adds
204 calls, including eighteen rejected reference calls. Plans, progress,
workers, complete numerical exports and output vectors are preserved. Raw
licensed logs and input data remain local. The collector verifies complete,
disjoint manifest coverage, explicit application status, terminal markers,
native oracle acceptance and exact samples before reporting native success.

Shared measurement fingerprints and duplicate output vectors are stored once
under `objects/`. `archive-hashes.json` maps each original path to its archive
member; result fingerprints use explicit `$archive_ref` entries. Replacing
those references and serializing sorted, indented JSON reconstructs the
portable original result bytes. All 104,370 main entries, including 4,158
fingerprints and 2,772 vector mappings, pass hash reconstruction and ZIP
integrity checks. Original and portable hashes remain distinct. The
[summary](summary.json) and [reference summary](reference-summary.json) retain
every cell and outcome.

Large performance screens, narrower kernel improvements, measured automatic
routing, final repeated blocks and Linux confirmation remain open. No release
qualification, installed-package replacement or push is represented here.
