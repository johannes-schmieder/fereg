# Threading contract

## 0.5.0 implementation

`ThreadTeam` installs each parallel phase into a bounded process-local Rayon
pool at the resolved count. `threads(0)` uses available processors as its
ceiling; an explicit positive value sets the maximum team size. The public
command planner preserves that request and selects an effective count before
pool creation and solver admission. Under the candidate policy in ADR 0233,
automatic three-intercept-FE PPML with fast/auto memory and at most 15 declared
finite columns uses at most eight workers. Other workloads retain their prior
choice. Runtime matrices report requested and effective counts separately.
Ranges are contiguous, nonempty, and
returned in worker order. Native and plugin tests exercise 1, 2, 8, and 32
requested threads.

The cache owns at most one pool. A later install at the same count reuses its
registry and workers; a different count replaces it. Computation does not
depend on Rayon's ambient global pool. The cache retains no model, sample,
workspace, coefficient, or certificate state, so estimator ownership remains
per action and different thread counts remain safe across repeated Stata
calls. The current estimators reuse one resolved policy across parallel
preparation, matrix, score, reconstruction, and inference kernels.
Deterministic solver paths retain their registered reduction order. The
default fast path may choose a different certified reduction or solver route.
Fresh one-effect OLS inputs with at least 16,384 rows and eight RHS columns
also check finite values on the already installed team. This Boolean check
retains the same diagnostic before weight validation and workspace admission;
it changes no numerical reduction. Calls outside a team and one-thread calls
keep the serial check.
OLS preparation installs this team from eight numeric sources and 16,384 rows;
classification still selects errors in observation order. IID composition also
uses the resolved team: for large samples its independent residual scan can
overlap outcome mean/TSS scans without changing their arithmetic or error order.
Fast one-intercept-FE, unit-weight OLS fits with at least 16,384 rows, at most
16 model columns and at least eight workers can run the independent outcome
mean/TSS scan alongside fitting instead. This includes IID, robust and clustered
inference through either ordinary or categorical preparation. The source remains
immutable and no data buffer is added. Any scan error is deferred until the
original fit, DoF and weight checks have succeeded. Inference can consume the
same call's independently computed unit residual sum; it never substitutes a
fitted moment or skips the independent residual calculation.
Categorical IID and robust DoF validation runs inside the same resolved team
as inference composition. Its existing finite-source checks can then share
that team; check order and numerical algorithms are unchanged.
Fresh ordinary one-FE unit fits with 8–16 generated columns, at least 16,384
rows and eight installed workers can also overlap final residual projection
with reported bread/intercept completion. This uses the same team and existing
owners, without saved coefficients. An immutable packed or strided inference
score cache may remain live across both phases, as it does sequentially.
Projection errors remain deferred until bread and initial within-TSS checks
pass. Phase-allocation probes retain sequential execution.
After SPI loading ends, eligible unit one-FE fits may compute raw moments
from their already loaded matrix beside sample/FE preparation on the same
team. This requires 16,384 rows, 8–64 RHS columns, eight workers and admitted
workspace. Their private row owner and cache stay paired; exact prepared-value
matching gates reuse. Preparation and fit-input errors keep their original
precedence over a deferred moment error.
Dense integer FE encoding also uses an already installed team from 16,384
rows: the range scan preserves the first ineligible/nonfinite row, and indexed
collection fills the fallibly reserved code vector without another allocation.
Canonical levels, signed-zero normalization and hash fallback remain unchanged.
Generic automatic slope certificates with an admitted current-weight geometry
cache and at least 16,384 observations can use the installed team when it has
more than one worker. Each effect-level group writes a disjoint result slot;
the caller merges every RHS in the original order, retaining the same RMS FMA
recurrence and raw maximum. Residuals, weights and geometry are immutable during
this work. Fallible scratch admission preserves the serial path. It does not
start a pool or change the changing-weight reusable MAP implementation; see
ADR 0117.
The PPML multipartition coefficient solver can separately admit an ordered
group result bank for its fresh certificates (ADR 0124). It uses the existing
team, bounded RHS tiles and disjoint group slots, followed by the original
ordered scalar merge. No pool or parallel floating-point reduction is added.
Budget/allocation refusal retains serial certification and the same solver.
Retired scoped teams signal shutdown and join their worker handles after the
cache mutex is released. New and old teams can coexist during replacement.
The general Rust API avoids self-join when external asynchronous work drops the
last owner on one of its own workers; Stata actions use scoped work and release
their team owners on the calling thread.

The Stata PPML adapter keeps native work in one scoped task after input loading
(ADR 0179). A bounded stack exchange relays the existing safe-point callbacks to
the Stata calling thread. Workers never receive the SPI callback. Both peers
close on unwind, and the scope joins before publication or ABI error handling.
This avoids repeated external Rayon queue submissions during ordinary IRLS.
Prepared family rows use that installed team. IRLS objectives use fixed
4096-row compensated blocks and an ordered merge of sums and corrections
(ADR 0219), independent of scheduling and bank capacity. Independent final
objective replay retains its original row-order reduction (ADR 0212). Small
and T1 calls keep scalar family evaluation; T1 adapter execution stays on the
calling thread.

Independent PPML outer FE scores use the same installed team and existing two
scalar banks (ADR 0220). At N>=4096 with multiple workers, each direction owns
its outputs and traverses immutable group members in original row order. The
final reduction retains canonical direction order; scale and finite-column
checks stay separate and fresh. Small/T1 calls retain row scans. Read the resolved
team size directly and invoke serial accumulation without a team submission;
external submissions can allocate queue blocks inside warmed iteration windows.
No numeric bank, pool, cached score or cross-generation authority is added.

## SPI prohibition

The Mac numeric input pipeline uses `ThreadTeam::in_place_scope`: the scope
body stays on the Stata calling thread, while one job on the resolved team
transposes Rust-owned tiles and classifies missing values. Two 4,096-row buffers
bound the handoff. Both peers signal closure on error or unwind, and the scope
joins the worker before the loader returns. One-thread calls use the existing
column reader. This introduces no independent thread pool or retained data cache.

No worker may call `SF_*`, access Stata memory, read a Stata macro/matrix, write
an output variable, or poll Stata directly. The main thread copies data into
Rust-owned arrays, workers compute, and the main thread copies certified output
back.

## Determinism

Default mode is numerically reproducible under the registered scientific
tolerances but does not promise bitwise identity across thread counts.
`deterministic` fixes chunk ordering and merge order. Tests require
same-architecture, same-thread-count bitwise reproducibility for deterministic
routes. Cross-thread and cross-architecture bitwise identity are not claimed;
scientific tolerances and exact sample/status/DoF gates still apply.

Under ADR 0034, the pinned LSMR norm and dot kernels use one fixed vector
chunk per indexed reduction leaf. Worker stealing therefore cannot change
their addition tree. This is a kernel-level guarantee; other upstream
atomic and preconditioner reductions do not gain a general bitwise guarantee.

## Rayon decision

ADR 0002 recorded the earlier dependency-free construction. The 0.5.0 runtime
uses an exact-version Rayon dependency and a local pool while preserving the
same `ThreadTeam` abstraction. The crate and transitive dependency inventory is
captured in Cargo.lock and the release SBOM/license notices.

ADR 0020 fixes the PPML rule that retained projection, finite moments,
solve/reconstruction, and sandwich phases all run under the preflight-frozen
team count, including when an ambient pool has a different size. Qualification
checks the exact installed Rayon pool width with `rayon::current_num_threads()`;
the number of workers Rayon happened to schedule is diagnostic telemetry, not
a correctness gate.
