# Large-panel optimum enclosures

This development-only witness addresses the two retained beta.10/reference
fitted-value disagreements. It does not change an estimator, reference, rank
rule or tolerance. The [record](../bench/results/within-companion-optimum-enclosure-20261030.json)
binds the original captures, failed diagnostic, refinement and terminal SCC
accounting. The [implementation](../crates/fereg-oracle/python/panel_optimum_enclosure.py)
and [dense analytic tests](../tests/python/test_panel_optimum_enclosure.py) are
independent of fereg, within and fixest.

The scope is the frozen unweighted three-FE panels: five observations per worker,
sixteen finite regressors, worker and firm indicators, and a 32-level third FE.
All PPML outcomes and fitted means are positive. Neither missing observations
nor separation is permitted in these witnesses. Nonunique FE coordinates are
not accuracy targets; the identified finite coefficients and fitted response are.

## Residual and model-space bounds

Write the explicit design as `A = [D, Z]`, where `D` contains worker/firm
indicators and `Z` the sixteen finite regressors and 31 third-FE indicators.
Omitting one third-FE indicator removes a constant already in `D`. For a stored
prediction `p`, let `r = y-p`. Set `W=I` for OLS and `W=diag(p)` for PPML.

The witness constructs an observation vector `e` satisfying `A'e=A'r`.
First balance worker scores, then route the remaining firm/worker scores along
a spanning forest of the mover graph. Distribute each cell flow in proportion
to its observation weights. This is an explicit feasible flow, not an iterative
estimate of the FE solution. A breadth-first forest shortens its paths; the
first depth-first diagnostic and its less useful bound remain retained.

Stayer workers have the same firm on all five observations. Their weighted
within-worker contrasts annihilate every column of `D`. The Gram matrix of
those contrasts of `Z`, denoted `G`, is positive definite in both frozen panels.
A 47-dimensional correction using `G` balances the remaining `Z` scores without
changing the `D` scores. Its weighted norm and that of the graph correction give
an upper bound `E` on the dual norm of the full model score. The bound uses the
triangle inequality and also accounts for the effect of the graph's rounding
correction on the `Z` scores; it does not require exact orthogonality between
the computed corrections.

For every vector `v=D alpha+Z gamma`, stayer contrasts show
`||v||_W^2 >= gamma'G gamma >= lambda ||gamma||^2`. The matrix bound `lambda>0`
is checked using an approximate inverse: recompute `I-BG`, bound its norm,
and use `(1-||I-BG||)/||B||` minus the Gram formation allowance. The result does
not assume a particular eigensolver's accuracy. This controls all 47 identified
`Z` coordinates, including the sixteen reported finite coefficients.

Separately reconstruct one explicit feasible predictor `f0` using the stored
finite coefficients, recovered third-FE coefficients and worker/firm potentials.
Its measured distance from the stored predictor includes an arithmetic allowance.
The recovered FE coordinates need not themselves be accurate: their only role
is to exhibit a feasible point near the stored predictor.

For OLS, orthogonal projection onto `col(A)` gives
`||fitted_optimum-f0|| <= E+||p-f0||`. Each fitted error is therefore bounded by
its measured row distance plus this radius, and each finite coefficient error
by the radius divided by `sqrt(lambda)`.

## Poisson curvature

For PPML let `d_i=|f0_i-log(p_i)|`, including mean/predictor reconstruction and
rounding allowances, and `d=max(d_i)`. Let
`D=||sqrt(p) * expm1(d_i)||`. The score at the feasible point has dual norm at
most `E+D`. On a ball of radius `R` in the `W` norm, its Hessian is bounded below
by `exp(-d-R/sqrt(min(p))) W`.

The first radius `R0=2 exp(d)(E+D)` is admissible if
`r0=R0/sqrt(min(p)) < log(2)`: on the boundary the objective's outward radial
derivative is positive. Strict convexity in the predictor then places the
unique optimum inside that ball. All outcomes are positive, so no separation
or infinite-coordinate optimum is involved.

Using the curvature bound on this already admitted ball sharpens the radius to
`R1=exp(d+r0)(E+D)=R0 exp(r0)/2`. This algebraic improvement requires no new fit,
data scan, tolerance or stopping rule. The resulting row mean error is at most
`p_i * expm1(d_i+R1/sqrt(p_i))`; the finite coefficient bound is
`R1/sqrt(lambda)`.

The stored first-pass maximum tolerance ratio can also be sharpened without
rescanning. With `q=R1/R0<1`, convexity of `expm1` bounds the new envelope by
`q` times the old envelope plus `(1-q) p_i expm1(d_i)`. The latter divided by
`atol+rtol*p_i` is at most `D/(sqrt(min(p))*rtol)`. This gives the recorded
all-row bound and a separate bound for the originally disputed rows.

## Numerical accounting and checks

The calculation treats the saved binary64 inputs as the data. Worker and firm
scores sum the signed original outcomes and predictions with `math.fsum` before
cancellation. Flow arithmetic and explicit model-space checks use extended
precision. Global scores use 128-row extended-precision leaves and an explicit
balanced reduction; bounds cover subtraction, multiplication and reduction.
The small Gram matrix has a conservative formation allowance proportional to
`gamma_(100*n+10000)` times the uncentered weighted design energy. Flow moment
residuals and root compatibility are checked explicitly. Standard floating
arithmetic assumptions apply; this is a scientific enclosure with roundoff
allowances, not a formally verified interval-arithmetic implementation.

The six retained computational functions have identical Python syntax trees to
those run on SCC. Twelve small dense OLS/constructed Poisson checks passed there.
Eighteen expanded checks on Mac include off-model predictor perturbations and
compare against explicit dense solutions or constructed Poisson optima. The
bridge witness separately checks exact graph-implied predictions, including
weighted designs and extra model columns.

For the retained OLS fit, every fitted error is bounded by `1.93753e-10`, and
each finite coefficient error by `3.60405e-12`. The largest fitted error bound
is less than 20% of its unchanged tolerance. For PPML, the sharpened all-row
tolerance ratio is at most `0.704551`, and each finite coefficient error is at
most `6.74899e-10`, below the smallest coefficient tolerance `1.27198e-9`.
All 1,317 OLS and nineteen PPML disputed rows are covered. These conclusions
apply directly to those two retained beta.10 captures. The subsequent
[Mac checks](../bench/results/within-enclosed-companions-mac-20261030.json) and
[Linux completion](../bench/results/within-enclosed-continuation-linux-20261030.json)
also pass every beta.10/beta.11 call at one and four workers on both designs.
Those checks add the independently bounded center uncertainty to each new
result's distance from that center before applying the original tolerance.
They preserve the original failed reference comparisons and establish no new
performance or fixed-worker repeatability claim.
