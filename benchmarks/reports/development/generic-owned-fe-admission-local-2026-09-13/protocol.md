# Generic owned-FE memory admission diagnosis

Accepted source f576bc5, excluding the unadopted saved-output precision repair.
That repair passes nineteen local science/cost pairs but sends the original slope
control's final projection through generic LSMR, increasing native peak from
28.87 to 38.54 MB against a public 18.93 MB execution base. Source inspection
finds that the generic owned-FE adapter passes AbsorbConfig into absorb_fixed_effects,
but neither explicit nor automatic generic branches consult solver_memory_budget.

First reproduce the bypass independently in four closed-form intercept/slope
projections: unrestricted calls must agree with the exact known residual, while
a one-byte joint budget must fail with Allocation. Test accepted source in an
isolated directory with unique Cargo output; retain failures and source identity.
Do not narrow output coverage, relax precision, or adopt the saved-output repair
before the added generic workspace has checked admission. No cluster/qualification.
