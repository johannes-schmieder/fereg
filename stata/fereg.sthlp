{smcl}
{* *! version 0.5.3 07nov2026}{...}
{.-}
help for {cmd:fereg} {right:(Johannes F. Schmieder)}
{.-}
{vieweralsosee "fereg postestimation" "help fereg_postestimation"}{...}
{vieweralsosee "feppml" "help feppml"}{...}

{title:Title}

{p 4 4 2}
{cmd:fereg} {hline 2} high-dimensional fixed-effect OLS and weighted least
squares

{marker quickstart}
{title:Quick start}

{pstd}
{cmd:fereg} estimates linear models with optional categorical fixed effects and
group-specific continuous slopes. A typical
two-way fixed-effect model with firm-clustered standard errors is

{phang2}{cmd:. fereg log_wage education experience, ///}{p_end}
{phang3}{cmd:absorb(worker firm) vce(cluster firm)}{p_end}

{pstd}
The default solver inspects the fixed-effect design, selects a certified route,
and returns an error rather than posting an estimate that fails its numerical
or covariance checks. Unless {cmd:keepsingletons} is specified, singleton
observations are removed to a fixed point.

{marker syntax}
{title:Syntax}

{p 8 16 2}{cmd:fereg,} {cmd:selftest}
[{cmd:threads(}{it:#}{cmd:)} {cmd:verbose}]

{p 8 16 2}{cmd:fereg} [{cmd:,} {cmd:level(}{it:#}{cmd:)} {it:display_options}]

{p 8 16 2}
{cmd:fereg} {it:depvar} [{it:indepvars}] {ifin} {weight},
[{it:options}]

  {it:option}{col 38}description
  {hline 76}
  {ul:Model and saved output}
    {cmd:absorb(}{it:terms}{cmd:)}{col 38}fixed effects and absorbed slopes
    {cmd:noconstant}{col 38}suppress the reported normalized intercept
    {cmd:keepsingletons}{col 38}retain observations in singleton FE levels
    {cmd:residuals(}{it:newvar}{cmd:)}{col 38}save estimation residuals

  {ul:Inference}
    {cmd:vce(unadjusted|robust)}{col 38}IID or HC1 covariance
    {cmd:vce(cluster }{it:clustervars}{cmd:)}{col 38}one- or multiway clustering
    {cmd:dof(pairwise|firstpair)}{col 38}absorbed-DoF graph adjustment
    {cmd:dof(continuous|none)}{col 38}slope-only or no redundancy adjustment

  {ul:Solver and execution}
    {cmd:solver(}{it:method}[{cmd:,} {it:suboptions}]{cmd:)}{col 38}{help fereg##solver:fixed-effect solver}; default {cmd:auto}
    {cmd:threads(}{it:#}{cmd:)}{col 38}native threads; default 0 (automatic)
    {cmd:memory(auto|fast|low)}{col 38}memory plan; default auto
    {cmd:poolsize(}{it:#}{cmd:)}{col 38}low-memory block width; default 10
    {cmd:tolerance(}{it:#}{cmd:)}{col 38}numerical tolerance; default 1e-8
    {cmd:iterate(}{it:#}{cmd:)}{col 38}solver iteration limit; default 16,000
    {cmd:deterministic}{col 38}fix native partition and merge order
    {cmd:timeit}{col 38}print timing, memory, RSS, and thread diagnostics
    {cmd:verbose(}{it:#}{cmd:)}{col 38}record verbosity request; default 0
  {hline 76}

{pstd}
The public weight forms are {cmd:[aw=}{it:exp}{cmd:]},
{cmd:[fw=}{it:exp}{cmd:]}, and {cmd:[pw=}{it:exp}{cmd:]}. The command
rejects importance weights ({cmd:iweight}) with {cmd:r(101)}.

{marker runtime}
{title:Native runtime}

{pstd}
Both estimators use the installed fereg native plugin. Use {cmd:fereg, selftest}
to inspect and check it. A missing binary
returns {cmd:r(601)}.

{marker description}
{title:Description}

{pstd}
{cmd:fereg} fits OLS and weighted least-squares models with no fixed effects,
one fixed effect, or multiple high-dimensional fixed effects. The regressor
list before the comma accepts continuous variables, numeric factor main
effects, all-categorical factor interactions, and time-series operators applied
to continuous variables. Interaction notation involving continuous regressors,
such as {cmd:i.region#c.tenure} or {cmd:c.tenure#c.age}, is unsupported in this
list. Group-specific continuous slopes are supported inside {cmd:absorb()},
for example {cmd:absorb(region#c.tenure)} or {cmd:absorb(region##c.tenure)};
see {help fereg##absorb:Absorbed terms}. The regressor list may be omitted
for a model containing only absorbed effects and the constant.

{pstd}
Stata evaluates {cmd:if}, {cmd:in}, factor-variable syntax, and time-series
semantics. One native call then validates the candidate rows, constructs the
final sample, encodes fixed effects and clusters, removes singletons, absorbs
the fixed effects, estimates the finite regression and covariance matrix,
checks the result, and writes requested outputs. No partial estimate is posted
after an error or Break.

{pstd}
The output reports sample size, the model F-test and p-value, R-squared,
adjusted and within R-squared, and Root MSE above the coefficient table.
Clustered models also show counts for each cluster dimension. The absorbed
degrees-of-freedom table below the coefficients reports categories, redundant
parameters and coefficient counts. Its {cmd:*} marker denotes effects nested
within clusters; {cmd:?} means redundancy may be understated. Undefined
statistics remain missing. Replay after {cmd:bootstrap:} or {cmd:jackknife:}
uses Stata's prefix inference display.

{marker absorb}
{title:Absorbed terms}

{pstd}
Terms inside {cmd:absorb()} are separated by spaces. Categorical sources may be
numeric or fixed-width strings.

  {it:syntax}{col 34}interpretation
  {hline 76}
    {cmd:worker firm}{col 34}separate worker and firm intercept effects
    {cmd:firm#year}{col 34}one intercept effect for each firm-year cell
    {cmd:firm##c.time}{col 34}firm intercepts and firm-specific time slopes
    {cmd:firm#c.time}{col 34}firm-specific time slopes without firm intercepts
    {cmd:worker#c.(time tenure)}{col 34}two slope-only channels by worker
    {cmd:worker##c.(time tenure)}{col 34}worker intercept and two slope channels
    {cmd:fe_worker=worker}{col 34}save a normalized worker effect
  {hline 76}

{pstd}
Categorical interactions use {cmd:#}. Categorical {cmd:##} expansion inside
{cmd:absorb()} is unsupported; write the desired main and interaction effects
explicitly. The special {cmd:##c.} form means an absorbed categorical intercept
plus a continuous slope. The {cmd:#c.} form is slope-only.

{pstd}
A name to the left of {cmd:=} requests a normalized saved fixed-effect
channel. For a term with slopes, the intercept uses the base name and slope
channels use {cmd:_Slope1}, {cmd:_Slope2}, and so on. A slope-only term creates
only suffixed slope variables. Individual FE normalizations are not unique;
the fitted absorbed contribution is the stable identified object.

{pstd}
Saved names and {cmd:residuals()} must be new, distinct variables. Do not use
{cmd:savefes()}; name each requested effect inside {cmd:absorb()}.

{marker sample}
{title:Sample and singleton handling}

{pstd}
After Stata applies {cmd:if} and {cmd:in}, the native estimator applies outcome,
regressor, fixed-effect, cluster, and weight rules. Missing or invalid values
outside the candidate sample are not inspected. By default, singleton fixed-
effect observations are deleted iteratively until the sample stabilizes.
{cmd:keepsingletons} retains them. {cmd:e(sample)} marks the final sample.

{marker weights}
{title:Weights}

{pstd}
Analytic weights may be rescaled without changing the estimates. Zero or
missing analytic weights remove candidate rows; negative values are errors.
Frequency weights must be strictly positive integers and behave as literal row
multiplicities. Probability weights must be strictly positive and require
robust or clustered inference. If {cmd:vce()} is omitted with probability
weights, {cmd:vce(robust)} is selected automatically.

{marker inference}
{title:Covariance and degrees of freedom}

{pstd}
If neither probability weights nor {cmd:vce()} is specified, {cmd:fereg} uses
IID inference. {cmd:vce(unadjusted)}, {cmd:vce(ols)}, and {cmd:vce(iid)} are
equivalent. {cmd:vce(robust)} requests HC1 inference.

{pstd}
{cmd:vce(cluster }{it:clustervars}{cmd:)} accepts one or more cluster
dimensions. Separate dimensions with spaces. Use {cmd:#}, as in
{cmd:state#year}, to form one interaction dimension; {cmd:##} is invalid in a
cluster term. Multiway covariance uses inclusion-exclusion and the common
finite-sample correction after absorbed-effect nesting is accounted for.

{pstd}
{cmd:dof(pairwise)} is the default reference-compatible absorbed-DoF policy.
{cmd:firstpair}, {cmd:continuous}, and {cmd:none} expose the corresponding
bounded reference controls. Successful inference always requires positive
residual degrees of freedom.

{marker solver}
{title:Solver, memory, and threads}

{pstd}
{cmd:solver()} holds every fixed-effect solver choice. Its first word is the
iterative method; sub-options after a comma name the reduction of the problem,
the preconditioner, and the acceleration of alternating projections. The
results report what ran in the same words (see {help fereg##stored:Stored results}).

  {it:request}{col 44}route
  {hline 76}
    {cmd:solver(auto)}{col 44}automatic route; the default
    {cmd:solver(map)}{col 44}alternating projections (MAP)
    {cmd:solver(map, accelerate(irons-tuck))}{col 44}Irons--Tuck accelerated MAP
    {cmd:solver(map, accelerate(squarem))}{col 44}SQUAREM accelerated MAP
    {cmd:solver(lsmr} [{cmd:, precondition(}{it:p}{cmd:)}]{cmd:)}{col 44}modified LSMR on the full system
    {cmd:solver(cg} [{cmd:, reduce(}{it:r}{cmd:) precondition(}{it:p}{cmd:)}]{cmd:)}{col 44}preconditioned CG, full or eliminated
    {cmd:solver(cg, precondition(cmg))}{col 44}CMG-preconditioned CG, full graph
    {cmd:solver(cg, reduce(eliminate) precondition(cmg))}
    {col 44}experimental CMG Schur CG
  {hline 76}

  {it:suboption}{col 44}meaning
  {hline 76}
    {cmd:reduce(auto|full|eliminate)}{col 44}keep every effect, or eliminate the
    {col 44}effect with the most levels exactly
    {cmd:precondition(auto|none|jacobi|block|}{col 44}preconditioner of the Krylov method
    {space 4}{cmd:schwarz|cmg)}
    {cmd:accelerate(none|irons-tuck|squarem)}{col 44}MAP acceleration; {cmd:map} only
    {cmd:cmgdegree(4|5|6)}{col 44}CMG-Schur cutoff; default 5
    {cmd:cmgsamples(}{it:#}{cmd:)}, {cmd:cmgseed(}{it:#}{cmd:)}{col 44}sampled trees, seed; default 0
  {hline 76}

  {it:solver(cg, ...)}{col 30}{cmd:reduce(auto)}{col 46}{cmd:reduce(full)}{col 62}{cmd:reduce(eliminate)}
  {hline 76}
    {cmd:precondition(auto)}{col 30}elimination{col 46}Schwarz, else{col 62}Jacobi, escalating
    {col 30}(2-5 effects){col 46}Jacobi{col 62}to block
    {cmd:precondition(none)}{col 30}full{col 46}no preconditioner{col 62}r(198)
    {cmd:precondition(jacobi)}{col 30}elimination{col 46}Jacobi{col 62}Jacobi
    {cmd:precondition(block)}{col 30}elimination{col 46}r(198){col 62}block-Jacobi
    {cmd:precondition(schwarz)}{col 30}full{col 46}additive Schwarz{col 62}r(198)
    {cmd:precondition(cmg)}{col 30}CMG graph{col 46}CMG graph{col 62}CMG Schur
  {hline 76}

{pstd}
The method is the first word. Sub-choices left at {cmd:auto} resolve to a
supported formulation of that method, and an explicit sub-choice is never
replaced: an unsupported combination returns {cmd:r(198)} and names an
alternative. {cmd:solver(auto)} chooses the reduction, method, and
preconditioners itself; to fix one, name the method, for example
{cmd:solver(cg, precondition(jacobi))} or {cmd:solver(lsmr, precondition(none))}.
{cmd:accelerate(cg)} is rejected because its residual certificate does not
ensure projection accuracy. {cmd:cmgdegree()}, {cmd:cmgsamples()}, and
{cmd:cmgseed()} require {cmd:solver(cg, reduce(eliminate) precondition(cmg))}.
Option names are lowercase; values are case-insensitive.

{pstd}
{cmd:solver(lsmr)} and {cmd:solver(cg)} with a preconditioner other than
{cmd:cmg} run on categorical intercept effects: {cmd:reduce(full)} solves for
every effect's coefficients at once, {cmd:reduce(eliminate)} eliminates the
effect with the most levels exactly (two to five effects) and iterates on the
others. Additive Schwarz uses one subdomain per connected component of each
pair of the largest effect with another effect; each subdomain's local solve
is exact (dense or envelope Cholesky) where that is cheap and approximate
Cholesky otherwise. {cmd:precondition(auto)} on the full system uses Schwarz
when it fits the memory limit and Jacobi otherwise; on the elimination, a
Jacobi recurrence that stalls escalates to block-Jacobi (exact dense solves of
small effects, CMG on large ones). An explicit {cmd:jacobi}, {cmd:block},
{cmd:none}, or {cmd:schwarz} never escalates. Explicit requests fail closed:
non-convergence within {cmd:iterate()} or a solver workspace beyond the memory
limit is an error, not a substituted route. With absorbed slopes,
{cmd:solver(lsmr)} uses the general modified-LSMR solver and these
{cmd:solver(cg)} choices are unavailable.

{pstd}
{cmd:solver(auto)} is the default. One categorical effect uses direct
demeaning. Two to five categorical-intercept effects first eliminate the
effect with the most levels exactly and solve the remaining effects by
Jacobi-preconditioned CG; a recurrence that stalls escalates to block-Jacobi
preconditioning, with exact dense solves of small effects and CMG on large
ones, and later solves of the same design start on the blocks. Six or more
categorical-intercept effects, and designs the elimination declines, use CG on
the full system with additive Schwarz preconditioning (Jacobi when Schwarz does
not fit the memory limit) within half the iteration budget. If that declines
too, the established route finishes the fit and the result reports a
certified fallback: two effects continue with diagonal-preconditioned Schur
PCG (a structural rule may build a CMG preconditioner when eligible work
stalls), three or more effects with LSMR or a short MAP pilot. Every route must
pass the requested certificate. Ownership and repeated-cell optimizations are
automatic for {cmd:solver(auto)} with a global intercept and exactly two
categorical-intercept terms; they need no additional option and do not change
the fitted model.

{pstd}
{cmd:solver(map)} is unaccelerated symmetric MAP; {cmd:accelerate()} selects
Irons--Tuck or SQUAREM extrapolation. {cmd:solver(cg, precondition(cmg))}
constructs one worker--firm graph for the two largest categorical intercept
effects and uses CMG-preconditioned PCG for all right-hand sides. Additional
absorbed intercept effects are supported when their combined non-base
dimension is at most 64. Absorbed slopes are not supported. A failed CMG
construction or solve falls back visibly to certified CG over the MAP operator
(reported as {cmd:map, accelerate(cg)}); a successful bulk solve is polished
by it when needed. All explicit methods fail closed on a failed independent
fixed-effect certificate.

{pstd}
{cmd:solver(cg, reduce(eliminate) precondition(cmg))} is an experimental
CMG-preconditioned Schur solver for exactly two categorical intercept effects:
the larger effect is eliminated implicitly. It supports weights, finite
regressors, saved effects, and both memory modes; absorbed slopes are
unsupported. Its advanced controls are {cmd:cmgdegree(4|5|6)} (default 5),
{cmd:cmgsamples(}{it:k}{cmd:)} (default 0), and {cmd:cmgseed(}{it:s}{cmd:)}
(default 0), given inside {cmd:solver()}, for example
{cmd:solver(cg, reduce(eliminate) precondition(cmg) cmgsamples(2) cmgseed(7))}.
An explicit seed requires a positive sample count. See the
{browse "https://github.com/johannes-schmieder/fereg/blob/main/docs/SPEC.md":numerical specification}
for construction details and parameter ranges.

{pstd}
Automatic CMG upgrades are available in both memory modes and include hierarchy
setup in timing and memory admission. They report
{cmd:cg, reduce(eliminate) precondition(cmg)}; the explicit full-graph route
reports {cmd:cg, reduce(full) precondition(cmg)}. These are distinct methods.

{pstd}
{cmd:memory(auto)} preflights fast and low-memory plans. {cmd:memory(fast)}
retains dense native workspaces. {cmd:memory(low)} processes regressors in
blocks and may use bounded temporary scratch storage, which is removed on
success, error, or Break. {cmd:poolsize()} sets the block width.

{pstd}
{cmd:threads(0)} resolves the native default worker budget; a positive integer
sets the available native team for the call. {cmd:e(fereg_requested_threads)} records the request
and {cmd:e(fereg_threads)} records the resolved command budget. Stages may use fewer
workers according to work, independent tasks and admitted storage. Existing
stage traces distinguish lane limits and independent work from this budget.
{cmd:deterministic} fixes the work partition
and merge order for the same architecture and thread count; it may be slower.
{cmd:timeit} prints the selected memory plan, native allocation, scratch, RSS,
timing, and requested/effective threads. {cmd:verbose(#)} records the
requested verbosity; use {cmd:timeit} to print execution diagnostics.

{marker postestimation}
{title:Prediction and postestimation}

{pstd}
{cmd:predict} without an option, or with {cmd:xb}, returns the finite-regressor
linear prediction. The following statistics require
{cmd:residuals(}{it:newvar}{cmd:)} at estimation time:

  {it:statistic}{col 26}result
  {hline 64}
    {cmd:xbd}{col 26}finite prediction plus absorbed contribution
    {cmd:d}{col 26}absorbed contribution only
    {cmd:residuals}{col 26}estimation residual
  {hline 64}

{pstd}
Standard {cmd:test} uses the posted coefficient and covariance matrices.
{cmd:estat summarize} reports estimation-sample summaries. See
{help fereg_postestimation:fereg postestimation}.

{pstd}
Typing {cmd:fereg} without a variable list redisplays the last {cmd:fereg}
results; {cmd:level()} and Stata's standard display options apply. This replay
also lets {cmd:estimates replay}, {cmd:bootstrap:} and {cmd:jackknife:} display
{cmd:fereg} results. The {cmd:by:} prefix is not supported.

{marker stored}
{title:Stored results}

{pstd}
{cmd:fereg} stores the usual {cmd:e(b)}, {cmd:e(V)}, {cmd:e(sample)}, sample,
rank, residual and absorbed DoF, fit statistics, weight metadata, cluster
counts, singleton counts, and absorbed-effect metadata. {cmd:ereturn list}
shows the complete result set.

{pstd}
The lines above the coefficient table name the route used for absorption in
{cmd:solver()} syntax ({cmd:Fixed-effect solver:}), add {cmd:(escalated)} when a
solve switched to a stronger preconditioner, list the local solves of a block
or Schwarz preconditioner, and
state that the accuracy check passed at the effective tolerance; estimates that
fail the check are never posted. The use of a certified fallback is shown below
them.
{cmd:e(fereg_version)} records the release that produced the results. No
developer trace is required.

{pstd}
Fixed-effect solver results use the vocabulary of {cmd:solver()}:

  {it:part}{col 26}values
  {hline 76}
    reduction{col 26}{cmd:full}, {cmd:eliminate}, {cmd:none}
    method{col 26}{cmd:map}, {cmd:cg}, {cmd:lsmr}, {cmd:direct}, {cmd:derived}, {cmd:none}
    preconditioner{col 26}{cmd:none}, {cmd:jacobi}, {cmd:block}, {cmd:schwarz}, {cmd:cmg}
    acceleration{col 26}{cmd:none}, {cmd:cg}, {cmd:irons-tuck}, {cmd:squarem}
  {hline 76}

{pstd}
{cmd:direct} is the exact projection onto a single intercept effect (or the
constant alone). {cmd:derived} is a final residual formed from certified
projections without a new solve. {cmd:none} means there was nothing to absorb.
{cmd:block} is block-Jacobi over the retained effects of the eliminated system.
{cmd:schwarz} is additive Schwarz over factor-pair subdomains of the full
system. Acceleration {cmd:cg} is conjugate gradients over the symmetric MAP
operator, which verifies or completes a {cmd:solver(cg, precondition(cmg))}
projection.

  {it:result}{col 30}contents
  {hline 76}
    {cmd:e(fe_solver)}{col 30}the normalized request, e.g. {cmd:cg, reduce(full)}
    {cmd:e(fe_route)}{col 30}absorption route in {cmd:solver()} syntax, e.g.
    {col 30}{cmd:cg, reduce(eliminate) precondition(block)}
    {cmd:e(fe_reduction)}{col 30}reduction used for absorption
    {cmd:e(fe_method)}{col 30}method used
    {cmd:e(fe_preconditioner)}{col 30}preconditioner used
    {cmd:e(fe_local_solvers)}{col 30}with {cmd:block}: local solve of each retained
    {col 30}effect, e.g. {cmd:firm=cmg year=dense}; with {cmd:schwarz}:
    {col 30}subdomains by local solve, e.g. {cmd:dense=4 approximate=2}
    {col 30}({cmd:trivial}, {cmd:dense}, {cmd:skyline} (envelope Cholesky),
    {col 30}{cmd:approximate} (approximate Cholesky))
    {cmd:e(fe_subdomains)}{col 30}Schwarz subdomains (scalar), with {cmd:schwarz}
    {cmd:e(fe_acceleration)}{col 30}MAP acceleration used
    {cmd:e(fe_iterations)}{col 30}iterations (scalar; also {cmd:e(ic)})
    {cmd:e(fe_escalations)}{col 30}solves that escalated to a stronger preconditioner
    {cmd:e(fe_fallback)}{col 30}1 if a certified fallback finished the projection
    {cmd:e(fe_residual_}{it:*}{cmd:)}{col 30}the same items for the final residual projection
    {cmd:e(fe_cmgdegree)}{col 30}cutoff, with {cmd:cg, reduce(eliminate) precondition(cmg)}
    {cmd:e(fe_cmgsamples)}{col 30}sampled trees, with the same request
    {cmd:e(fe_cmgseed)}{col 30}sampling seed, with the same request
    {cmd:e(fe_solve)}{col 30}numeric diagnostics; see below
  {hline 76}

{pstd}
An escalation is a Jacobi recurrence of the eliminated system that continues
from its iterate with block-Jacobi preconditioning, or a Jacobi recurrence of
the full system that continues with additive Schwarz. Automatic fits
with two to five categorical intercept effects first try the exact elimination
of the largest effect: their route reads
{cmd:cg, reduce(eliminate) precondition(jacobi)}, or
{cmd:precondition(block)} with {cmd:e(fe_local_solvers)} after an
escalation. Six or more effects, and the elimination's declines, read
{cmd:cg, reduce(full) precondition(schwarz)} (or {cmd:precondition(jacobi)}
when Schwarz did not fit the memory limit). When that declines, the established
automatic routes run and {cmd:e(fe_fallback)} is 1: for two
effects, {cmd:cg, reduce(eliminate) precondition(jacobi)} is the guarded
Schur-PCG recurrence without multigrid and {cmd:precondition(cmg)} means a CMG
preconditioner was actually applied. An allocated but unused hierarchy is not
reported as CMG use. A MAP continuation reports {cmd:map} and sets
{cmd:e(fe_fallback)}.

{pstd}
{cmd:e(fe_solve)} is a 2-by-12 matrix with rows {cmd:absorption} and
{cmd:residual} and columns {cmd:iterations}, {cmd:escalations},
{cmd:fallback}, {cmd:warnings}, {cmd:pilot_iterations},
{cmd:pilot_contraction}, {cmd:setup_seconds}, {cmd:solve_seconds},
{cmd:certificate_absinf}, {cmd:certificate_relinf}, {cmd:certificate_rel2},
and {cmd:normal_equation}. It contains no codes.
Without {cmd:residuals()}, {cmd:xbd()}, {cmd:d()} or saved fixed effects, the
default memory mode certifies the within residual of the absorbed outcome and
regressors directly (method {cmd:derived}, zero iterations) instead of
absorbing it again. This applies to automatic absorption of one, two or more
categorical effects. Explicit solvers, low memory and a failed check absorb the
residual again.
Low-memory absorption combines all blocks: a fallback takes precedence;
otherwise CMG use remains visible even when another block did not need it, and
block-Jacobi use and escalations of any block are kept.
A zero-iteration {cmd:map} route may be an already-orthogonal input check.
The route identifies the certified result, not every attempted construction.

{phang2}{cmd:. display "Absorption: `e(fe_route)'"}{p_end}
{phang2}{cmd:. display "Final residual: `e(fe_residual_route)'"}{p_end}
{phang2}{cmd:. matrix list e(fe_solve)}{p_end}

{pstd}
Package-specific matrices include {cmd:e(fe_solve)} for solver work and
convergence, {cmd:e(fereg_runtime)} for memory/timing/thread diagnostics,
{cmd:e(fereg_certificate)} for independent checks, and
{cmd:e(fereg_status)}, {cmd:e(fereg_dof)}, {cmd:e(fereg_fe)},
{cmd:e(fereg_cluster)}, and {cmd:e(fereg_subset)} for detailed model and
inference metadata. Macros and scalars beginning with {cmd:e(fereg_)} record
requested and effective numerical policies. Utility calls return loader and
protocol information in {cmd:r()}.

{dlgtab:Memory report}

{pstd}
{cmd:e(fereg_memory)} is an 11-by-2 presence/value report for action
memory. Its schema version and estimate status distinguish unavailable (0),
partial (1), and calibrated practical estimates (2). Schema 2 supplies a
combined peak and additional reserve when the Stata-19 baseline and all native
stages are known. It accounts for native execution and later result posting as
separate phases. The reserve excludes already resident memory. Calibration
targets the larger of 10% and 64 MiB on development cases; it is an estimate,
not a guaranteed RSS bound. Strict proof gaps and safety headroom remain visible.
Unknown scopes stay partial, without a combined peak or reserve. Absent values
have presence zero and value zero. This report does not change memory admission
or the existing runtime report. With multiple Stata frames, used memory is
unavailable because Stata's used-data counter is frame-local; the allocated
baseline includes the resident segments and shared pools.

{marker limitations}
{title:Limitations}

{pstd}
Fixed-width string categorical sources are supported; {cmd:strL} sources are
not. When {cmd:absorb()} contains an interaction or absorbed slope, explicit
RHS base operators such as {cmd:ib2.region} and {cmd:fvset} base overrides
are unsupported.

{marker utilities}
{title:Installation diagnostics}

{pstd}
The following calls check the installation without estimating a model or changing
the current data and estimation results:

{phang2}{cmd:. fereg, selftest}{p_end}
{phang2}{cmd:. fereg, selftest verbose}{p_end}
{phang2}{cmd:. fereg, selftest threads(4)}{p_end}

{pstd}
{cmd:selftest} reports the ado and plugin versions, checks their compatibility,
and runs bounded checks of protocol handling, factor encoding, metadata and
worker execution. It does not provide comprehensive OLS or PPML qualification.
{cmd:verbose} adds build identity, protocol versions, plugin path, Stata host and
processor information, and individual check results. Failed checks always show
their details and return a nonzero code. Available version information is shown
before compatibility or check failures. {cmd:threads(#)} selects the worker count;
zero selects the native default.

{pstd}
After installing or updating fereg in a Stata session that has already used
it, restart Stata: the loaded plugin can stay in memory until Stata
exits (on Windows it always does). On Windows, restart before updating, because
a loaded plugin file cannot be overwritten. A plugin that does not match the
installed ado files fails with {cmd:r(499)} and asks for a restart.

{marker examples}
{title:Examples}

{pstd}
Each estimation example creates its own synthetic data and wraps the block in
{cmd:preserve}/{cmd:restore}, so it can be copied into a do-file without
replacing the caller's dataset. The blocks set Stata's random-number seed.
The clickable link runs the entire marked block through {cmd:fereg_run}, which
also restores the caller's data. Example 2 compares whichever regression
commands are installed and uses a table command supported by your Stata release.

{space 2}{hline 8} {it:Example 1 - Basic two-way fixed effects} {hline 8}
{cmd}{...}
          preserve
{* example_start - basic_two_way}{...}
          clear
          set seed 2026082601
          set obs 600
          generate long worker = ceil(_n/6)
          bysort worker: generate byte period = _n
          generate long firm = mod(worker + 7*period, 30) + 1
          generate double x = rnormal()
          bysort worker: generate double worker_fe = rnormal() if _n == 1
          bysort worker: replace worker_fe = worker_fe[1]
          bysort firm: generate double firm_fe = .7*rnormal() if _n == 1
          bysort firm: replace firm_fe = firm_fe[1]
          generate double y = 1 + .8*x + worker_fe + firm_fe + rnormal()
          fereg y x, absorb(worker firm)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 76}
{space 2}{it:({stata fereg_run basic_two_way using fereg.sthlp:click to run})}

{space 2}{hline 8} {it:Example 2 - Compare coefficients and estimation times} {hline 8}

{pstd}
Simulate 250,000 observations on 25,000 workers and 10,000 firms, then compare
{cmd:fereg}, {cmd:reghdfe}, {cmd:greg}, and {cmd:areg} when available.
Every worker and firm appears more than once, so there are no singletons.
The table uses {helpb etable} in updated Stata 17 or newer; otherwise it uses
{helpb estimates table}, including on Stata 15 and 16. No table add-on is needed.
Multiple absorbed effects in {cmd:areg} require Stata 19 or newer.

{cmd}{...}
          preserve
{* example_start - benchmark}{...}
          version 15.0
          clear
          set seed 2026082612
          local N 250000
          set obs `N'
          generate long worker = ceil(_n/10)
          bysort worker: generate byte period = _n
          generate long firm = mod(worker + 7919*period, 10000) + 1
          generate double x1 = rnormal()
          generate double x2 = rnormal()
          bysort worker: generate double worker_fe = rnormal() if _n == 1
          bysort worker: replace worker_fe = worker_fe[1]
          bysort firm: generate double firm_fe = .5*rnormal() if _n == 1
          bysort firm: replace firm_fe = firm_fe[1]
          generate double y = 1 + .6*x1 - .3*x2 + worker_fe + firm_fe + rnormal()

          * Attach elapsed time to e() without a table add-on.
          tempname add_runtime greg_result
          program define `add_runtime', eclass
              args seconds cmd
              ereturn scalar runtime = `seconds'
              * Complete greg's table metadata without changing b or V.
              if "`cmd'" == "greg" {c -(}
                  ereturn local cmd "greg"
                  ereturn local depvar "y"
              {c )-}
          end

          * Time estimation only; data generation is excluded.
          local models
          foreach cmd in fereg reghdfe greg areg {c -(}
              if "`cmd'" == "areg" & c(stata_version) < 19 {c -(}
                  display as text "areg skipped: multiple FEs require Stata 19."
                  continue
              {c )-}
              capture which `cmd'
              if _rc {c -(}
                  display as text "`cmd' skipped: command is not installed."
                  continue
              {c )-}
              local fit "`cmd'"
              local options
              if "`cmd'" == "fereg" local options "threads(`c(processors)')"
              if "`cmd'" == "areg" {c -(}
                  local fit "version 19: areg"
                  local options "dfabsorb tolerance(1e-12) iterate(10000)"
              {c )-}
              if "`cmd'" == "greg" {c -(}
                  local old_beta : copy global GTOOLS_BETA
                  local old_table : copy global GTOOLS_GREGTABLE
                  global GTOOLS_BETA "I KNOW WHAT I AM DOING"
                  global GTOOLS_GREGTABLE 1
                  local options "mata(`greg_result')"
                  mata: st_local("mata_before", invtokens(direxternal("__*")'))
              {c )-}
              timer clear 99
              timer on 99
              capture quietly `fit' y x1 x2, absorb(worker firm) `options'
              local rc = _rc
              timer off 99
              quietly timer list 99
              local seconds = r(t99)
              if "`cmd'" == "greg" {c -(}
                  global GTOOLS_BETA `"`old_beta'"'
                  global GTOOLS_GREGTABLE `"`old_table'"'
                  * Remove only temporary Mata objects newly created by greg.
                  mata: st_local("mata_after", invtokens(direxternal("__*")'))
                  local mata_new : list mata_after - mata_before
                  if "`mata_new'" != "" mata: mata drop `mata_new'
              {c )-}
              if `rc' {c -(}
                  display as text "`cmd' skipped: estimation returned r(`rc')."
                  continue
              {c )-}
              assert e(N) == `N'
              `add_runtime' `seconds' `cmd'
              estimates store `cmd'
              local models `models' `cmd'
          {c )-}
          timer clear 99
          program drop `add_runtime'

          if "`models'" != "" {c -(}
              capture which etable
              if _rc == 0 & c(stata_version) >= 17 {c -(}
                  etable, estimates(`models') column(estimates) keep(x1 x2) ///
                      cstat(_r_b, nformat(%9.6f)) ///
                      cstat(_r_se, nformat(%9.6f)) ///
                      mstat(N, label("Observations") nformat(%12.0fc)) ///
                      mstat(runtime, label("Time (s)") nformat(%9.3f))
              {c )-}
              else {c -(}
                  estimates table `models', keep(x1 x2) ///
                      b(%9.6f) se(%9.6f) stats(N runtime) stfmt(%12.3f)
                  display as text "runtime is elapsed estimation time in seconds."
              {c )-}
          {c )-}
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 76}
{space 2}{it:({stata fereg_run benchmark using fereg.sthlp:click to run})}

{pstd}
The true coefficients are 0.6 on {cmd:x1} and -0.3 on {cmd:x2}.
The table reports each command's own standard errors; absorbed degrees-of-freedom
rules can differ. Times are one elapsed run per command, including any initial
loading, and depend on the machine, processor setting, and command versions.
{cmd:areg} uses {cmd:dfabsorb} and a tighter convergence tolerance;
{cmd:fereg} receives the current Stata processor allowance. This is a local
illustration, not a performance qualification. Change {cmd:local N} to adjust
sample size. The example uses timer 99 and stores estimates under the command
names, replacing stored results with those names. It temporarily enables
{cmd:greg}'s beta regression and coefficient-table bridge, then restores the
two {cmd:GTOOLS} globals.

{space 2}{hline 8} {it:Example 3 - Factor variables, interactions, and if} {hline 8}
{cmd}{...}
          preserve
{* example_start - factor_if}{...}
          clear
          set seed 2026082602
          set obs 960
          generate long worker = ceil(_n/8)
          bysort worker: generate byte period = _n
          generate long firm = mod(worker + 5*period, 40) + 1
          generate byte region = 1 + floor(4*runiform())
          generate byte season = 1 + floor(3*runiform())
          generate double x = rnormal()
          bysort worker: generate double worker_fe = rnormal() if _n == 1
          bysort worker: replace worker_fe = worker_fe[1]
          bysort firm: generate double firm_fe = .6*rnormal() if _n == 1
          bysort firm: replace firm_fe = firm_fe[1]
          generate double y = 2 + .5*x + .2*(region == 2 & season == 3) + ///
              .08*period + worker_fe + firm_fe + rnormal()
          fereg y c.x i.region##i.season i.period if inrange(period,2,7), ///
              absorb(worker firm) vce(robust)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 76}
{space 2}{it:({stata fereg_run factor_if using fereg.sthlp:click to run})}

{space 2}{hline 8} {it:Example 4 - Analytic, frequency, and probability weights} {hline 8}
{cmd}{...}
          preserve
{* example_start - weights}{...}
          clear
          set seed 2026082603
          set obs 900
          generate long worker = ceil(_n/9)
          bysort worker: generate byte period = _n
          generate long firm = mod(worker + 3*period, 30) + 1
          generate double x = rnormal()
          generate double z = rnormal()
          generate double aw = .5 + runiform()
          generate int fw = 1 + mod(worker + period, 3)
          generate double pw = .5 + runiform()
          bysort worker: generate double worker_fe = rnormal() if _n == 1
          bysort worker: replace worker_fe = worker_fe[1]
          bysort firm: generate double firm_fe = .5*rnormal() if _n == 1
          bysort firm: replace firm_fe = firm_fe[1]
          generate double y = 1 + .7*x - .2*z + worker_fe + firm_fe + rnormal()
          fereg y x z [aw=aw], absorb(worker firm) vce(robust)
          fereg y x z [fw=fw], absorb(worker firm)
          fereg y x z [pw=pw], absorb(worker firm)
          display as text "The pweight call selected vce(" as result e(vce) ///
              as text ")."
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 76}
{space 2}{it:({stata fereg_run weights using fereg.sthlp:click to run})}

{space 2}{hline 8} {it:Example 5 - Multiway and interacted clusters} {hline 8}
{cmd}{...}
          preserve
{* example_start - clusters}{...}
          clear
          set seed 2026082604
          set obs 1200
          generate long worker = ceil(_n/10)
          bysort worker: generate byte period = _n
          generate int year = 2010 + mod(period-1,5)
          generate long firm = mod(worker + 7*period, 40) + 1
          generate byte state = mod(firm-1,10) + 1
          generate double x = rnormal()
          bysort worker: generate double worker_fe = rnormal() if _n == 1
          bysort worker: replace worker_fe = worker_fe[1]
          bysort firm: generate double firm_fe = .6*rnormal() if _n == 1
          bysort firm: replace firm_fe = firm_fe[1]
          generate double y = 1 + .6*x + worker_fe + firm_fe + ///
              .05*(year-2010) + rnormal()
          fereg y x, absorb(worker firm year) vce(cluster state#year worker)
          display as text "Declared cluster dimensions: " ///
              as result e(N_clustervars)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 76}
{space 2}{it:({stata fereg_run clusters using fereg.sthlp:click to run})}

{space 2}{hline 8} {it:Example 6 - A fixed-width string fixed effect} {hline 8}
{cmd}{...}
          preserve
{* example_start - string_fe}{...}
          clear
          set seed 2026082605
          set obs 720
          generate long worker = ceil(_n/8)
          bysort worker: generate byte period = _n
          generate long firm = mod(worker + 5*period, 24) + 1
          generate str4 firm_code = string(firm,"%02.0f")
          generate double x = rnormal()
          bysort worker: generate double worker_fe = rnormal() if _n == 1
          bysort worker: replace worker_fe = worker_fe[1]
          bysort firm: generate double firm_fe = .8*rnormal() if _n == 1
          bysort firm: replace firm_fe = firm_fe[1]
          generate double y = 2 + .4*x + worker_fe + firm_fe + rnormal()
          fereg y x, absorb(worker firm_code) vce(robust)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 76}
{space 2}{it:({stata fereg_run string_fe using fereg.sthlp:click to run})}

{space 2}{hline 8} {it:Example 7 - An interacted firm-year fixed effect} {hline 8}
{cmd}{...}
          preserve
{* example_start - interacted_fe}{...}
          clear
          set seed 2026082606
          set obs 1200
          generate long worker = ceil(_n/12)
          bysort worker: generate byte period = _n
          generate int year = 2015 + mod(period-1,6)
          generate long firm = mod(worker + 5*ceil(period/2), 30) + 1
          generate double x = rnormal()
          bysort worker: generate double worker_fe = rnormal() if _n == 1
          bysort worker: replace worker_fe = worker_fe[1]
          bysort firm year: generate double firm_year_fe = ///
              .5*rnormal() if _n == 1
          bysort firm year: replace firm_year_fe = firm_year_fe[1]
          generate double y = 1 + .9*x + worker_fe + firm_year_fe + rnormal()
          fereg y x, absorb(worker firm#year) vce(cluster firm)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 76}
{space 2}{it:({stata fereg_run interacted_fe using fereg.sthlp:click to run})}

{space 2}{hline 8} {it:Example 8 - Firm intercepts and firm-specific slopes} {hline 8}
{cmd}{...}
          preserve
{* example_start - absorbed_slope}{...}
          clear
          set seed 2026082607
          set obs 1200
          generate long worker = ceil(_n/12)
          bysort worker: generate byte period = _n
          generate double trend = period - 6.5
          generate long firm = mod(worker + 5*ceil(period/3), 25) + 1
          generate double x = rnormal()
          bysort worker: generate double worker_fe = rnormal() if _n == 1
          bysort worker: replace worker_fe = worker_fe[1]
          bysort firm: generate double firm_fe = .6*rnormal() if _n == 1
          bysort firm: replace firm_fe = firm_fe[1]
          bysort firm: generate double firm_slope = .08*rnormal() if _n == 1
          bysort firm: replace firm_slope = firm_slope[1]
          generate double y = 2 + .7*x + worker_fe + firm_fe + ///
              firm_slope*trend + rnormal()
          fereg y x, absorb(worker firm##c.trend) vce(robust)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 76}
{space 2}{it:({stata fereg_run absorbed_slope using fereg.sthlp:click to run})}

{space 2}{hline 8} {it:Example 9 - Multiple slope-only absorbed channels} {hline 8}
{cmd}{...}
          preserve
{* example_start - slope_only}{...}
          clear
          set seed 2026082608
          set obs 1500
          generate long worker = ceil(_n/15)
          bysort worker: generate byte period = _n
          generate double trend = period - 8
          generate long firm = mod(worker + 7*period, 30) + 1
          generate double x = rnormal()
          generate double z = rnormal()
          bysort firm: generate double firm_slope = .07*rnormal() if _n == 1
          bysort firm: replace firm_slope = firm_slope[1]
          bysort worker: generate double worker_slope1 = ///
              .05*rnormal() if _n == 1
          bysort worker: replace worker_slope1 = worker_slope1[1]
          bysort worker: generate double worker_slope2 = ///
              .10*rnormal() if _n == 1
          bysort worker: replace worker_slope2 = worker_slope2[1]
          generate double y = .5*x + firm_slope*trend + ///
              worker_slope1*trend + worker_slope2*z + rnormal()
          fereg y x, absorb(firm#c.trend worker#c.(trend z))
          display as text "Reported constant: " as result e(report_constant)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 76}
{space 2}{it:({stata fereg_run slope_only using fereg.sthlp:click to run})}

{space 2}{hline 8} {it:Example 10 - Saved effects, residuals, and prediction} {hline 8}
{cmd}{...}
          preserve
{* example_start - saved_outputs}{...}
          clear
          set seed 2026082609
          set obs 800
          generate long worker = ceil(_n/8)
          bysort worker: generate byte period = _n
          generate long firm = mod(worker + 5*period, 32) + 1
          generate double x = rnormal()
          bysort worker: generate double worker_fe = rnormal() if _n == 1
          bysort worker: replace worker_fe = worker_fe[1]
          bysort firm: generate double firm_fe = .7*rnormal() if _n == 1
          bysort firm: replace firm_fe = firm_fe[1]
          generate double y = 1 + .75*x + worker_fe + firm_fe + rnormal()
          fereg y x, absorb(fe_worker=worker fe_firm=firm) residuals(resid)
          predict double xb, xb
          predict double xbd, xbd
          predict double absorbed, d
          predict double resid_check, residuals
          assert abs(y - xb - absorbed - resid) < 1e-7 if e(sample)
          assert abs(xbd - xb - absorbed) < 1e-7 if e(sample)
          assert abs(resid_check - resid) < 1e-12 if e(sample)
          test x
          estat summarize y x
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 76}
{space 2}{it:({stata fereg_run saved_outputs using fereg.sthlp:click to run})}

{space 2}{hline 8} {it:Example 11 - No finite regressors and disconnected components} {hline 8}
{cmd}{...}
          preserve
{* example_start - disconnected}{...}
          clear
          set seed 2026082610
          set obs 960
          generate long worker = ceil(_n/8)
          bysort worker: generate byte period = _n
          generate byte component = 1 + (worker > 60)
          generate long firm = mod(worker + period, 20) + 1
          replace firm = firm + 20 if component == 2
          bysort worker: generate double worker_fe = rnormal() if _n == 1
          bysort worker: replace worker_fe = worker_fe[1]
          bysort firm: generate double firm_fe = .8*rnormal() if _n == 1
          bysort firm: replace firm_fe = firm_fe[1]
          generate double y = 3 + worker_fe + firm_fe + rnormal()
          fereg y, absorb(worker firm) vce(robust)
          display as text "Final observations: " as result e(N)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 76}
{space 2}{it:({stata fereg_run disconnected using fereg.sthlp:click to run})}

{space 2}{hline 8} {it:Example 12 - Low-memory execution and solver diagnostics} {hline 8}
{cmd}{...}
          preserve
{* example_start - low_memory}{...}
          clear
          set seed 2026082611
          set obs 50000
          generate long worker = ceil(_n/10)
          bysort worker: generate byte period = _n
          generate double x1 = rnormal()
          generate double x2 = rnormal()
          generate double x3 = rnormal()
          generate double x4 = rnormal()
          bysort worker: generate double worker_fe = rnormal() if _n == 1
          bysort worker: replace worker_fe = worker_fe[1]
          generate double y = 1 + .4*x1 - .2*x2 + .1*x3 + .3*x4 + ///
              worker_fe + rnormal()
          fereg y x1-x4, absorb(worker) memory(low) poolsize(2) ///
              solver(auto) threads(2) deterministic timeit
          matrix list e(fe_solve)
          matrix list e(fereg_runtime)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 76}
{space 2}{it:({stata fereg_run low_memory using fereg.sthlp:click to run})}

{marker requirements}
{title:Requirements}

{pstd}
{cmd:fereg} requires Stata 15 or newer and one self-contained plugin for its
platform: {cmd:fereg_macos.plugin}, {cmd:fereg_linux.plugin}, or
{cmd:fereg_windows.plugin}. Users do not need a compiler or separate runtime.

{pstd}
The package provides Linux x86-64, Windows x86-64 and universal macOS
binaries. Linux requires glibc 2.28 or newer. Mac testing uses Apple Silicon;
the Intel Mac code has not been tested in a licensed Intel Mac Stata session.
Windows x86-64 Stata/MP 19 GUI can load eligible numeric inputs in parallel;
other Windows hosts use serial input. Break during Windows loading may be
delayed or unavailable. See the
{browse "https://github.com/johannes-schmieder/fereg/blob/main/docs/installation.md":installation guide}
and {browse "https://github.com/johannes-schmieder/fereg/blob/main/docs/VALIDATION.md":platform validation record}.

{marker sources}
{title:Sources and acknowledgements}

{pstd}
{cmd:fereg} builds on open-source numerical software. Its LSMR solver and
pairwise Schwarz preconditioner use an adapted
{browse "https://github.com/johannes-schmieder/within":fork} of
{browse "https://github.com/py-econometrics/within":within}, developed by
Alexander Fischer, Kristof Schröder, and contributors at the appliedAI
Institute for Europe and in the PyFixest community. Fischer and Schröder's
{browse "https://github.com/py-econometrics/within-paper":graph-preconditioning paper}
describes the approach. The fork adapts data access, iteration control,
memory allocation, reproducible reductions, and cancellation for Stata.

{pstd}
The CMG backend uses a
{browse "https://github.com/johannes-schmieder/CMG":Rust port} of
Ioannis Koutis and Gary L. Miller's
{browse "https://github.com/ikoutis/cmg-solver":MATLAB/C implementation},
based on the combinatorial multigrid method of Koutis, Miller, and Tolliver
(2011). These numerical components are integrated with Stata estimation,
automatic solver selection, and independent convergence checks.

{pstd}
David Roodman's {browse "https://github.com/droodman/julia.ado":julia package}
and accompanying Stata Journal article (2025) provide a precedent for parallel
copying of data from Stata. {cmd:fereg} confines this to a qualified read-only
numeric loading phase; the precedent does not establish thread safety for
arbitrary Stata plugin operations.

{pstd}
We also acknowledge {browse "https://github.com/sergiocorreia/reghdfe":reghdfe},
{browse "https://github.com/sergiocorreia/ppmlhdfe":ppmlhdfe}, and
{browse "https://github.com/py-econometrics/pyfixest":PyFixest} as development
and compatibility references. The native bridge uses the official
{browse "https://www.stata.com/plugins/":Stata Plugin Interface} sources
published by StataCorp LP. Licenses and code provenance are documented in
{browse "https://github.com/johannes-schmieder/fereg/blob/main/THIRD_PARTY_NOTICES.md":Third-party notices};
{cmd:references.lock.toml} records the pinned comparison packages.

{marker references}
{title:Selected references}

{phang}
Fischer, Alexander, and Kristof Schröder (2026).
{browse "https://github.com/py-econometrics/within-paper":Graph Preconditioning for High-Dimensional Fixed Effects Regression}.
Working paper.

{phang}
Koutis, Ioannis, Gary L. Miller, and David Tolliver (2011).
{browse "https://doi.org/10.1016/j.cviu.2011.05.013":Combinatorial Preconditioners and Multilevel Solvers for Problems in Computer Vision and Image Processing}.
{it:Computer Vision and Image Understanding} 115(12): 1638–1646.

{phang}
Roodman, David (2025).
{browse "https://doi.org/10.1177/1536867X251341105":Julia as a Universal Platform for Statistical Software Development}.
{it:Stata Journal} 25(2): 255–284.

{phang}
Fong, David Chin-Lung, and Michael A. Saunders (2011).
{browse "https://doi.org/10.1137/10079687X":LSMR: An Iterative Algorithm for Sparse Least-Squares Problems}.
{it:SIAM Journal on Scientific Computing} 33(5): 2950–2971.

{phang}
Gao, Yuan, Rasmus Kyng, and Daniel A. Spielman (2026).
{browse "https://doi.org/10.1137/24M1673577":AC(k): Robust Solution of Laplacian Equations by Randomized Approximate Cholesky Factorization}.
{it:SIAM Journal on Scientific Computing} 48(3): A1643–A1684.

{phang}
Somaini, Paulo, and Frank A. Wolak (2016).
{browse "https://doi.org/10.1515/jem-2014-0008":An Algorithm to Estimate the Two-Way Fixed Effects Model}.
{it:Journal of Econometric Methods} 5(1): 143–152.

{phang}
Guimarães, Paulo, and Pedro Portugal (2010).
{browse "https://doi.org/10.1177/1536867X1101000406":A Simple Feasible Procedure to Fit Models with High-Dimensional Fixed Effects}.
{it:Stata Journal} 10(4): 628–649.

{phang}
Gaure, Simen (2013).
{browse "https://doi.org/10.1016/j.csda.2013.03.024":OLS with Multiple High Dimensional Category Variables}.
{it:Computational Statistics & Data Analysis} 66: 8–18.

{phang}
Correia, Sergio (2016).
{browse "https://www.scorreia.com/research/hdfe.pdf":A Feasible Estimator for Linear Models with Multi-Way Fixed Effects}.
Working paper.

{phang}
Correia, Sergio, Paulo Guimarães, and Thomas Zylkin (2020).
{browse "https://doi.org/10.1177/1536867X20909691":Fast Poisson Estimation with High-Dimensional Fixed Effects}.
{it:Stata Journal} 20(1): 95–115.

{phang}
Irons, Bruce M., and Robert C. Tuck (1969).
{browse "https://doi.org/10.1002/nme.1620010306":A Version of the Aitken Accelerator for Computer Iteration}.
{it:International Journal for Numerical Methods in Engineering} 1(3): 275–277.

{phang}
Varadhan, Ravi, and Christophe Roland (2008).
{browse "https://doi.org/10.1111/j.1467-9469.2007.00585.x":Simple and Globally Convergent Methods for Accelerating the Convergence of Any EM Algorithm}.
{it:Scandinavian Journal of Statistics} 35(2): 335–353.

{marker also}
{title:Also see}

{p 0 24}
Online: {help regress}, {help areg}, {help xtreg}, {help fvvarlist},
{help weights}, {help fereg_postestimation:fereg postestimation},
{help feppml}
{p_end}

{p 4 4 2}
Web: {browse "https://www.stata.com/new-in-stata/high-dimensional-fixed-effects/":Stata 19 high-dimensional fixed effects}


{marker author}
{title:Author}

{pstd}
Johannes F. Schmieder, Boston University, USA

{pstd}
Website: {browse "https://johannes-schmieder.com/":johannes-schmieder.com}
{break}GitHub: {browse "https://github.com/johannes-schmieder":johannes-schmieder}
{break}Email: {browse "mailto:johannes@bu.edu":johannes@bu.edu}

{pstd}Comments and suggestions are welcome.
{.-}
