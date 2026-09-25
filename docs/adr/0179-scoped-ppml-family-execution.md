# ADR 0179: scoped PPML execution and bounded family row work

Status: accepted for local development, 2026-09-24.

## Decision

After loading ends, the Stata PPML adapter runs native preparation, separation,
IRLS and inference in one scoped task on its resolved team. A stack-owned
mutex/condition-variable exchange relays each existing interrupt point to the
Stata calling thread and returns its result before computation resumes. The SPI
callback remains non-Send/non-Sync and is never captured by the worker. Both
peers close the exchange on unwind; the scope joins before returning a result or
propagating a panic to the existing ABI containment boundary. Publication stays
on the calling thread. One-thread calls retain direct execution.

The complete team is primed before entry. The embedding allocation snapshot
moves inside the native task, before core preflight, so its live scoped-job
allocation is charged to every core phase. This introduces no new pool, numeric
worker count, public option, solver route or supported developer command.

Within a finalized sample, `PreparedPoissonInputs` borrows the immutable outcome,
prior weights and offset and retains ADR 0140's two cached outcome terms. For
N>=4096 and more than one resolved worker, mean and working-state rows use the
installed team. Working Hessian weights, responses and score residuals share a
row pass. The deviance's non-near branch uses the already cached log outcome.
Independent final objective replay and the null objective remain uncached.

Objective row contributions use a reusable bank of two doubles per row, capped
at min(N,4096*T) rows. Each tile is filled in parallel; both compensated sums are
then updated serially in original observation order across tiles. There is no
parallel sum tree or reassociation. The first failing observation remains the
reported error, and failed trial buffers have no acceptance authority. Small
and one-thread evaluations retain scalar execution. The bank belongs to the
mutable IRLS workspace, not the immutable family frame or accepted Hessian.

## Why the scoped task is necessary

The first row-scheduled candidate submitted each phase from the calling thread.
It passed science but failed the real T4 warmed-iteration audit: one allocation
event, zero net growth. Rayon injects external submissions into Crossbeam's
block queue, which allocates successor blocks; source inspection explains this
failure mechanism. Keeping numerical work in one installed scope removes those
repeated external submissions. The replacement passes the unchanged real
16-iteration T1/T4 audits and a separate 256-cycle work/callback allocation test.
Do not bypass the allocation audit or add a special audit-only execution route.

## Scientific authority and memory

This changes computation placement and row ownership, not the mathematical
iteration. Current-weight FE/finite projection, model-space, rank, both inverse
sides, objective, score, reconstruction, consecutive outer convergence,
separation and final-Hessian checks remain. No check or tolerance is removed,
rescheduled or relaxed. Family tests compare scalar and scheduled bits across
thread counts, tile boundaries and changing/extreme states. Full public results
retain independent mathematical oracle requirements; reference agreement alone
is insufficient.

The tile adds **16*min(N,4096*T) bytes**, zero for scalar execution: 256 KiB at
T4 and 512 KiB at T8 for the large cases. Both memory routes charge it beside
all ten existing mutable/immutable observation-vector equivalents. Mutable
capacity telemetry includes its actual reserved capacity. Checked sizing and
fallible reservation preserve fail-closed admission. There is no N-by-K bank.
The scoped job adds 1,392 observed live heap bytes on this Mac artifact; its
actual footprint, not that platform-specific number, enters embedding admission.
Selected modeled peaks may remain dominated by another phase. This is not a
physical-cap qualification or a repair of the static allocation inventory.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-scoped-family-local-2026-09-24/README.md)
retains all rotations, losses, identities, errors and independent checks. Fresh
threaded medians improve 3.2–6.7%; serial is flat, with three paired losses.
All fresh inner and outer counts are unchanged. Small explicit-oracle calls
include 1–2 ms losses, and small-fit distributions include an individual loss.
Six OLS comparison pairs have exact scientific exports.

There are 215 distinct focused Rust passes and eleven successful final-candidate
licensed fixture runs, including actual Ctrl-C and zero-allocation audits.
The inherited checkpoint-count assertion remains failing. An existing licensed
transaction fixture expects wire minor 2 although the current contract is 3;
both artifacts fail there. A preserved diagnostic copy changes only that version
assertion and sentinel and passes all remaining checks for both artifacts.

All 321 measured PPML states pass independent score/span/objective replay;
107 native pairs and 107 direct stock pairs pass their numerical comparisons.
Nine small states pass explicit Poisson/Hessian/sandwich/mean oracles. Three of
those came from a generator case with a modulo-zero warning: its actual matrix
science is retained, its timing is unranked, and a separately predeclared valid
graph supplies the replacement slope oracle. Pre-fit setup errors remain visible.

This is a shared execution improvement, not completion of the <=0.90-times-stock
multi-FE goal. Do not tune these scheduling constants on the consumed fresh seeds
202609404–202609408. Remaining phase budgets favor shared current-frame original-
input statistics and reconstruction, with low-slope canonical work the largest
measured cost. Tiny serial family differences do not justify another tuning loop.
