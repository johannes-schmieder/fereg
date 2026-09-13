# P0: attempted-solver memory reporting, initial implementation

This is an **unadopted, incomplete reporting prototype** against accepted
`7663a87614e37bea73e5dbc037c3ad0c7afccdda`. It is separate from the preceding
[adopted batching/memory improvement](../lsmr-batch-adoption-local-2026-09-13/README.md).
No plugin was built or timed for this prototype, and the public OLS runtime
matrix is unchanged. There is no new performance or complete-memory claim.

## Implemented and tested boundary

`SolveReport` carries the maximum incremental workspace estimate recorded by
instrumented attempts. Zero explicitly means no increment was recorded, not
proof that a route needs no workspace or is fully covered. The implementation
reuses estimates already computed for joint admission; it does not add an
observation-sized allocation, change a budget decision or alter arithmetic.

Borrowed automatic LSMR records its estimate when attempted and preserves it
through errors or uncertified continuation to CG. An LSMR route rejected before
construction records only the CG estimate. Retained explicit LSMR records the
estimate for each actual RHS block. Schur records the attempted full workspace,
keeps it after CG continuation, and transfers a rejected probe's estimate into
the next absorption report. Low-memory batch merging takes the maximum across
successive batches, independent of merge order. Disjoint attempts must not be
summed merely because they occurred in one command.

Focused tests distinguish a larger failed LSMR attempt from the smaller final
CG estimate, reject charging an unbuilt LSMR route, preserve exact retained-batch
residuals/coefficients at 1/7 workers, retain Schur attempt estimates, and test all
six orderings of mixed LSMR/CG report merging. The merged maximum can belong to
an earlier LSMR batch even when the aggregate reports CG. Full independent MAP
comparisons remain in the weighted Schur fixture. [Validation](validation.json)
binds the exact prototype inputs and test names; [patch](implementation.patch)
preserves the implementation against the accepted commit.

## Remaining propagation audit

| Boundary | Current source behavior | Required next work |
|---|---|---|
| `fe/borrowed_map.rs` automatic/retained LSMR | Instrumented estimates are available only where existing budget planning runs | Cover explicit transient/checkpoint use and callers without a memory budget; reconcile installed versus configured worker identity without silently changing admission |
| `ols/prepared_fit.rs` Schur probe/full solve and low batches | Prototype preserves disjoint attempted maxima | Carry the command total through outcome, regressor blocks and residual correction |
| `fe/cmg.rs::build_profiled_internal` | Preflight returns only success/failure; rejected hierarchy/error returns lose its estimate | Transfer attempted construction cost on success, planned rejection and recoverable failure; distinguish rejection before allocation |
| `fe/cmg.rs::absorb` | A successful graph route retains hierarchy/workspace while CG polishes it | Account for simultaneous retained CMG plus CG workspace; a maximum of separate route estimates is insufficient for this overlap |
| `fe/lsmr.rs`, MAP/accelerated and slope constructors | New field is initially zero | Audit generic/slope representation and remaining route estimates; zero is not coverage evidence |
| `ols/prepared_scores.rs::absorb_score_block` | Certifies absorption, then returns `into_parts().0`, discarding the report | Retain score-reconstruction attempts, including its separate CMG session/setup and low-memory blocks |
| `estimate.rs` saved-FE recovery | Recovery occurs after fitting/inference and may use a separate reconstruction route | Carry its estimate and overlap with retained output state |
| `result.rs` / `estimate.rs::build_runtime_shadow` | Shadow carries execution metadata; selected bytes come only from `MemoryPlan::selected_bytes()` | Publish the checked execution-plus-complete-attempt total and ratio only after all relevant producers are included |

The retained hierarchy overlap and discarded score report are concrete reasons
not to fix the public scalar by looking only at the final solver label, or even
only at the fit's initial and residual reports. They are source findings, not
new allocator measurements. Existing opaque-factor model limits and preserved
certificate failures remain unresolved.

The prototype remains uncommitted production work pending complete propagation,
focused memory/admission/scientific tests and a bounded source-identified command
comparison. Installed dist, dependency pins, unrelated Veneto work and historical
receipts remain intact. No SCC, broad profile, qualification or push ran.
