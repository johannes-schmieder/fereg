# Practical whole-action memory reporting

**Adopt schema-2 practical reporting under ADR 0074.** The fixed development
checks meet the larger-of-10%-or-64-MiB target. Strict proof coverage remains
incomplete; this is neither a hard RSS bound nor release qualification.

## What changed

The preceding [CMG investigation](../cmg-memory-attribution-2026-09-18/README.md)
and [expected-heap repair](../cmg-expected-heap-2026-09-18/README.md) separated
conservative reservations from expected native use. The remaining Stata growth
comes from `ereturn post, buildfvinfo` with factor-variable coefficient stripes.
Numeric-only posting does not show the observation-sized growth. Stata retains
the extra data capacity after its temporary work finishes.

Native owners are released before ado posting. The incremental forecast takes
the maximum of those two phases, adds the resident Stata/native baseline once,
and also checks entry allocation. Resident bytes do not enter the additional
reserve. Posting counts matrix copies and an engineering estimate of factor
metadata workspace: round `max(20*N_original, Stata_allocated/10)` to 32 MiB.
The constants describe measured Stata-19 publication behavior, not a proof of
its allocator. No current observed peak enters the prediction.

Schema 2 publishes a combined estimate when the baseline and expected native
stages are known. Strict proof bits remain visible. Stata versions outside the
calibration, multiple-frame unknowns and unresolved attempt storage stay
partial. Admission, runtime/stack headroom, scientific calculations, tolerances,
automatic routing and component defaults are unchanged. This is reporting
after construction, not a new preallocation forecast or a memory reduction.

## Evidence

The source snapshot is `b58efa34…081d11`, based on `a11b2884`. The isolated Linux
plugin is `02bbde02…9b7714`. The baseline is the previously verified native
checkpoint (`a2ebe892…fd9d7`, artifact `bb1eb0cf…8018`), whose compiled estimator
source matches the starting main. Full hashes are in [identities.json](identities.json)
and [source.sha256](source.sha256). After testing, only help text and inventory
version references changed among the snapshot files; numerical and executable
source stayed identical. Inputs and reference pins are bound separately.

- All **16 unchanged inputs** (12 calibration cases and four prior holdouts)
  pass native and whole-process accuracy checks and publish practical status 2.
  Process errors range from −1.42 to +39.24 MiB; these small cases predominantly
  use the absolute allowance. They do not establish ten-percent relative
  accuracy. Cases include narrow/wide OLS, missing/generated controls, weak
  weighted bridges, tiny/mixed components, disconnected three-FE, slopes,
  supported weights/VCE, low-memory inference, PPML separation and saved outputs.
- Four publication holdouts pass on **both Mac and Linux Stata 19**. Allocation
  growth agrees across platforms; the largest model error is 32.35 MiB. The
  German-shaped holdout predicts 100,744,432 bytes versus 100,647,188 observed.
  Eight earlier scale cases and numeric/factor probes are preserved. Their
  portable do-files omit only an unused local adopath; original hashes remain.
- The unchanged German 5% receipt is **replayed, not re-estimated**. Prediction
  is 4,424,928,225 bytes versus 4,347,969,536 observed RSS (**+1.77%**). Native
  prediction remains +3.77%. Publication predicts 100,757,680 bytes versus
  100,565,748 measured from pre-plugin to return. The original successful fit
  retains 4,361,733 rows with exact preserved scientific results. Its schema-1
  receipt is untouched; this is not a claim that the new binary ran that fit.
- **150 native tests**, Linux export/actual-loader checks, licensed Mac/Linux
  memory-contract fixtures, **52 Python tests**, both structural inventories
  and repository checks pass. Failure/interruption/repeated-call behavior is
  covered by the native suite; publication schema corruption has focused tests.
  Historical schema-1 evidence still replays unchanged.

Three alternating fresh-process pairs per affected performance case give:

| Case | Baseline median | Candidate median | Change |
|---|---:|---:|---:|
| Narrow OLS | 0.215 s | 0.211 s | −4 ms |
| Wide generated OLS | 0.739 s | 0.732 s | −7 ms |
| One-FE PPML | 0.249 s | 0.250 s | +1 ms |

All nine comparisons have exact scientific tables and sample hashes. No
command/phase exceeds both +5% and +10 ms. Native allocation medians are
unchanged; scratch and fallback counts are zero in these paired cases.
[phase-summary.json](phase-summary.json) retains phase timing, allocation and RSS.
These are development non-regression checks, not new speedup claims.

## Execution and limits

Job **7641143** completed all substantive checks, then its final wrapper used
an undefined macro instead of `_rc` and returned exit 2 after the fixture's
pass sentinel. That failure is preserved. Job **7641149** corrected only the
wrapper, verified the completed receipts and reran that small fixture: terminal
pass, scheduler failed=0/exit=0, six seconds. No estimation or performance pair
was repeated. The first job used 283 seconds and 3.036G scheduler maxvmem; both
reserved 28 slots. The application used one/eight native workers; reservation
size is not a claim of 28-way execution.

Run `python3 benchmarks/reports/development/whole-action-practical-2026-09-18/audit.py`
to replay the retained decisions without estimation. Sanitized traces and
synthetic scientific tables are in `diagnostics.zip`; original hashes bind
the remote receipts. Raw licensed logs, datasets and installed artifacts are
excluded. Scheduler scripts remain locally archived with hashes; portable
measurement helpers and the fixed case list are included.

The practical memory milestone is closed for this calibrated scope. Strict
construction/runtime proofs, arbitrary allocator settings, long-lived process
fragmentation and untested platform extrapolations remain explicit limitations.
Unknown coverage fails to partial. Mac native linking remains blocked by the
unaccepted Xcode license; review packaging and formal platform qualification
remain separate. No installed plugin, dependency checkout, push or release changed.
