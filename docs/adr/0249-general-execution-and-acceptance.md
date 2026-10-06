# ADR 0249: General execution and deliberate algorithm changes

**Status:** Accepted direction by Johannes, 2026-10-04; scoped by the
[second simplification program](../../milestones2.md).

## Decision

Replace competing runtime pipelines through qualified changes within the working
engine. Preserve public features, supported explicit solvers, memory controls,
scientific tolerances, fixed-configuration repeatability and atomic transactions.
The frozen comparator and native-distribution gate remain unchanged.

Scoped tasks may deliberately change automatic algorithms, arithmetic order,
storage and internal admission policy. Declare affected paths, fields, routes,
errors and independent witnesses before implementation. Require SPEC science
and stable discrete model decisions; changes to sample/rank/omission/separation
semantics still require Johannes's decision. Unaffected outputs remain exact.
Pure refactors retain ADR 0248's exact scientific and route checks. A failed
exact check cannot be relabeled as an intentional change after the fact.

For the new execution policy, `threads(n)` is the available worker budget.
Stages may use less concurrency according to general work/resource policies,
with transparent requested, resolved and effective concurrency. Replace
feature-specific ceilings rather than moving them into a new planner. Preserve
main-thread SPI and repeatable reductions; more workers need not improve a
saturated stage, but unexplained material scaling cliffs fail qualification.

Organize algorithms around mathematical structure: direct one-FE work, a
justified two-FE graph family and general supported FE work, with finite no-FE
operations. Genuine slope/rank/connectivity/support requirements remain.
Clustering, outputs and memory choices compose with those operations instead
of generating separate fitting pipelines or benchmark-cell routing fences.

Bounded declared performance tradeoffs may fund substantial simplification and
broad gains. Freeze workloads, holdouts and budgets before changes; compare
both the qualified parent and a fixed program baseline. Report slow families,
memory, failed cells and scaling alongside aggregate time. The roadmap's
default allowance is at most 5% or 1 ms per workload, whichever is larger,
with a named benefit and no automatic entitlement to regress. Changing that
allowance requires Johannes's decision before the affected implementation.

## Consequence

Complete architectural retirements are implementation/qualification batches;
focused development precedes frozen integrated acceptance. Predetermined,
calibrated screening/confirmation may target unresolved workloads without
discarding observations or widening the allowance; historical rules remain.
Every structural milestone retires named code and counts all replacements.
Scientific authority stays with independent witnesses. The new plan governs
future migrations; this ADR does not assert that worker semantics or algorithms
have already changed in the current runtime. ADRs 0247/0248 otherwise remain
active, including the ratchet, evidence gates, timebox and one status ledger.

Johannes's 2026-10-05 execution override pauses GitHub testing and restricts
qualification to the lead's Mac until resumed. Local acceptance precedes
dependent work; no CI or remote-platform gate applies during this period.
Preserve scientific gates, measured performance and historical failures. His
subsequent M28–M30 override permits architectural acceptance of inconclusive
performance with an independently reviewed substantial code-quality benefit.
Keep the allowance and raw verdict unchanged; uncertainty remains recorded,
not a timing pass. Established regressions still require correction or his
separate decision. Report Mac scope and deferred remote evidence without claims.

Johannes explicitly accepts the reviewed M30 candidate on 2026-10-06 despite
three established M26 regressions (OLS residual-robust, slope OLS and PPML
residual-robust), and defers automatic 8/16-worker timing to a quiet Mac window.
The [specific acceptance](../../bench/history/2026-10-06-m30-architectural-acceptance.json)
retains failed measurements and missing evidence; other allowances remain unchanged.

Johannes separately accepts the reviewed M31 candidate on 2026-10-06 despite
large Fast OLS being 8.2–8.6% slower than fixed M26, defers its remaining broad
and scaling timing, and authorizes dependent M32–M35 work. The
[specific acceptance](../../bench/history/2026-10-06-m31-architectural-acceptance.json)
retains the failed verdict and open evidence; future allowances remain unchanged.

Johannes accepts M32 on 2026-10-06 and authorizes comparable architectural
acceptance prospectively: science/transactions and Astra review pass, a
substantial retirement is delivered, every parent comparison is within, and
only previously accepted fixed-M26 cost families remain unresolved without
observed worsening beyond their accepted costs. Preserve all observations,
the allowance and unresolved verdict. New established regressions require a
separate decision; final integrated performance remains open. The
[M32 acceptance](../../bench/history/2026-10-06-m32-architectural-acceptance.json)
records the identified artifact and scope.
