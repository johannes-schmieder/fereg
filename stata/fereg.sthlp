{smcl}
{* *! version 0.5.0 26oct2026}{...}
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
{cmd:fereg} estimates linear models while absorbing one or more categorical
fixed effects and, when requested, group-specific continuous slopes. A typical
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
    {cmd:solver(auto|map|it|squarem|lsmr|cmg|cmgschur)}{col 53}fixed-effect solver
    {cmd:cmgdegree(4|5|6)}{col 38}experimental CMG-Schur cutoff; default 5
    {cmd:cmgsamples(}{it:#}{cmd:)}{col 38}sampled CMG-Schur trees; default 0
    {cmd:cmgseed(}{it:#}{cmd:)}{col 38}sampled CMG-Schur seed; default 0
    {cmd:preconditioner(auto|off|diagonal|schwarz)}{col 38}LSMR preconditioner
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
recognizes {cmd:iweight} only to return the pinned {cmd:r(101)} rejection.

{marker runtime}
{title:Native runtime}

{pstd}
Both estimators use the installed fereg native plugin. Use {cmd:fereg, selftest}
to inspect and check it. A missing binary
returns {cmd:r(601)}. The former {cmd:engine()} option has been removed.

{marker description}
{title:Description}

{pstd}
{cmd:fereg} fits OLS and weighted least-squares models with no fixed effects,
one fixed effect, or multiple high-dimensional fixed effects. The finite
regressor list accepts Stata continuous variables, numeric factor main effects,
all-categorical factor interactions, and time-series operators applied to
continuous variables. Categorical-by-continuous and continuous-by-continuous
RHS interactions are not yet supported. The regressor list may be omitted for
a no-finite-regressor model.

{pstd}
Stata evaluates {cmd:if}, {cmd:in}, factor-variable syntax, and time-series
semantics. One native call then validates the candidate rows, constructs the
final sample, encodes fixed effects and clusters, removes singletons, absorbs
the fixed effects, estimates the finite regression and covariance matrix,
checks the result, and writes requested outputs. No partial estimate is posted
after an error or Break.

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
{cmd:solver(auto)} is the default. One categorical effect uses direct
demeaning. With {cmd:preconditioner(auto)}, two categorical-intercept effects
start with diagonal-preconditioned Schur PCG. If eligible outcome or packed
regressor/outcome work has stalled at iteration 4, 8 or 12, or remains at
iteration 16, a structural rule builds a combinatorial multigrid (CMG)
preconditioner, applies it on the worker team and reuses it across later
blocks. Easy problems may finish without multigrid. Optional setup refusal keeps the
existing recurrence; numerical failure can continue with certified MAP inside
the original iteration budget.

{pstd}
Eligible designs with three or more categorical-intercept effects use LSMR
with reusable preparation. It starts with a diagonal preconditioner and builds
the additive Schwarz preconditioner only when the projected work is large.
With three unit-weight effects, a short MAP pilot or certified structural
shortcut may finish first. Every route must pass the requested certificate.
Slopes and explicit solver/preconditioner requests retain their supported
routes. Ownership and repeated-cell optimizations are automatic for Auto/Auto
with a global intercept and exactly two categorical-intercept terms; they need
no additional option and do not change the fitted model.

{pstd}
Explicit methods are unaccelerated symmetric {cmd:map},
Irons--Tuck {cmd:it}, {cmd:squarem}, and modified
{cmd:lsmr}. {cmd:cmg} constructs one worker--firm graph for the two
largest categorical intercept effects and uses CMG-preconditioned PCG for all
right-hand sides. Additional absorbed intercept effects are supported when
their combined non-base dimension is at most 64. Absorbed slopes are not
supported by {cmd:cmg}. A failed CMG construction or solve falls back visibly
to certified CG; a successful bulk solve is polished by CG when needed.
Explicit {cmd:lsmr} iterates to one hundredth of {cmd:tolerance()} (at least
1e-15, and never less strictly than {cmd:tolerance()}) and certifies at
{cmd:tolerance()}. All
explicit methods fail closed on a failed independent fixed-effect certificate.

{pstd}
{cmd:solver(cmgschur)} is an experimental OLS solver for exactly two
categorical intercept fixed effects. It uses an implicit worker-eliminated
Schur operator and a CMG preconditioner built from a hybrid graph.
{cmd:cmgdegree(4|5|6)} controls the largest distinct-employer degree whose
workers are eliminated from the preconditioner graph; higher-degree workers
remain as auxiliary vertices by default. The default cutoff is 5.
{cmd:cmgsamples(}{it:k}{cmd:)} optionally replaces each higher-degree worker
with the average of {it:k} sampled weighted trees on its distinct firms.
The count must be an integer in 0..4294967295. Its default, zero, retains
the original hybrid graph. With a positive count, {cmd:cmgseed(}{it:s}{cmd:)}
selects a seed in 0..9007199254740991; its default is zero. Write these
integers as decimal digits. An explicit {cmd:cmgseed()} requires positive
{cmd:cmgsamples()}, including when the requested seed is zero.
The graph is sampled once and remains fixed throughout the solve.
These options change only preconditioner construction; the Schur operator,
fitted model, and sample are unchanged. The solve retains its independent
final numerical certificate. Weights,
regressors, saved fixed effects, and both memory modes are supported.
It is unavailable for PPML, absorbed slopes, or other numbers of fixed effects.
{cmd:cmgdegree()}, {cmd:cmgsamples()}, and {cmd:cmgseed()} are accepted only
with {cmd:solver(cmgschur)}.
The solver matrix uses code 8 for this route. {cmd:e(fereg_cmg_degree)},
{cmd:e(fereg_cmg_samples)}, and {cmd:e(fereg_cmg_seed)} record the requested
cutoff, tree count, and seed. This experimental explicit option is included in
the current beta on all three platforms. Its default unsampled graph differs
from the automatic upgrade, which uses cutoff 5, one sampled tree and seed zero.

{pstd}
Explicit {cmd:solver(cg)} returns error 198 because its residual certificate
does not establish projection accuracy. Use {cmd:solver(lsmr)} or
{cmd:solver(cmg)}. Low-memory CMG projects raw residuals through the full graph
solve before final certification.

{pstd}
Automatic upgrades are available in both memory modes and include hierarchy
setup in timing and memory admission. Their used-solver name is {cmd:cmgschur}
(code 8). The explicit full-graph {cmd:solver(cmg)} route reports {cmd:cmg}
(code 7). These are distinct methods.

{pstd}
{cmd:preconditioner()} is valid only with {cmd:solver(auto)} or
{cmd:solver(lsmr)}. Its choices are {cmd:auto}, {cmd:off}, {cmd:diagonal}, and
{cmd:schwarz}; {cmd:none} aliases {cmd:off}, and {cmd:additive} aliases
{cmd:schwarz}. Use {cmd:preconditioner(off)} with {cmd:solver(cmg)} or
{cmd:solver(cmgschur)}.

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
timing, and requested/effective threads. In this beta, {cmd:verbose(#)} is
stored for interface compatibility; {cmd:timeit} is the printed diagnostic.

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

{marker stored}
{title:Stored results}

{pstd}
{cmd:fereg} stores the usual {cmd:e(b)}, {cmd:e(V)}, {cmd:e(sample)}, sample,
rank, residual and absorbed DoF, fit statistics, weight metadata, cluster
counts, singleton counts, and absorbed-effect metadata. {cmd:ereturn list}
shows the complete result set.

{pstd}
Normal output shows the absorption and final residual-check methods.
{cmd:e(fereg_solver_requested)} preserves the request, including {cmd:auto};
{cmd:e(fereg_solver_used)} names the method used for outcome/regressor
absorption, and {cmd:e(fereg_solver_residual)} names the final residual method.
{cmd:e(fereg_solver_fallback)} equals one if either phase used a certified
fallback. No developer trace is required.

{pstd}
For automatic two-effect fits, {cmd:cg} means the guarded Schur-PCG recurrence
without multigrid and {cmd:cmgschur} means a CMG preconditioner was actually
applied. An allocated but unused hierarchy is not reported as CMG use.
Explicit {cmd:solver(cg)} remains unsupported. A MAP continuation reports
{cmd:map} and sets fallback.

{pstd}
{cmd:e(fereg_solver)} is a 2-by-16 matrix with rows {cmd:absorption} and
{cmd:residual}. Columns {cmd:requested} and {cmd:used} use these numeric codes:
1 auto, 2 map, 3 lsmr, 4 cg, 5 it, 6 squarem, 7 cmg, 8 cmgschur.
Column 3 is {cmd:iterations}; column 4 is the stop code: 1 no fixed effects,
2 certified, 5 derived certificate; column 11 is {cmd:fallback}.
Without {cmd:residuals()}, {cmd:xbd()}, {cmd:d()} or saved fixed effects, the
default memory mode certifies the within residual of the absorbed outcome and
regressors directly (stop code 5, zero iterations) instead of absorbing it again.
This applies to automatic absorption of one, two or more categorical effects;
the residual row then repeats the absorption method. Explicit solvers, low
memory and a failed check absorb the residual again.
Low-memory absorption combines all blocks: a fallback takes precedence;
otherwise CMG use remains visible even when another block did not need it.
A zero-iteration {cmd:map} row may be an already-orthogonal input check.
The method identifies the certified result, not every attempted construction.

{phang2}{cmd:. display "Absorption: `e(fereg_solver_used)'"}{p_end}
{phang2}{cmd:. display "Residual check: `e(fereg_solver_residual)'"}{p_end}
{phang2}{cmd:. matrix list e(fereg_solver)}{p_end}

{pstd}
Package-specific matrices include {cmd:e(fereg_solver)} for routing and
convergence, {cmd:e(fereg_runtime)} for memory/timing/thread diagnostics,
{cmd:e(fereg_certificate)} for independent checks, and
{cmd:e(fereg_status)}, {cmd:e(fereg_dof)}, {cmd:e(fereg_fe)},
{cmd:e(fereg_cluster)}, and {cmd:e(fereg_subset)} for detailed model and
inference metadata. Macros and scalars beginning with {cmd:e(fereg_)} record
requested and effective numerical policies. Utility calls return loader and
protocol information in {cmd:r()}.

{marker limitations}
{title:Current limitations}

{pstd}
Fixed-width string categorical sources are supported; {cmd:strL} sources are
not. Categorical-by-continuous and continuous-by-continuous RHS interactions
are not yet supported. When {cmd:absorb()} contains an interaction or absorbed
slope, explicit RHS base operators such as {cmd:ib2.region} and {cmd:fvset}
base overrides are also unavailable in the current final-sample rebasing path.

{pstd}
{cmd:fereg} does not implement PPML; use {help feppml}. The beta does not
implement IV/GMM, group outcomes with individual fixed effects, HAC or
Driscoll--Kraay covariance estimators, other GLMs, or GPU execution.
Unsupported requests return an explicit error.

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
zero selects the native default. The former {cmd:version} and {cmd:doctor}
options are no longer supported.

{pstd}
After installing or updating fereg in a Stata session that has already used
it, restart Stata: the previously loaded plugin can stay in memory until Stata
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
also restores the caller's data. Example 12 additionally requires an installed
{cmd:reghdfe} and Stata 19.

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

{space 2}{hline 8} {it:Example 2 - Factor variables, interactions, and if} {hline 8}
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

{space 2}{hline 8} {it:Example 3 - Analytic, frequency, and probability weights} {hline 8}
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

{space 2}{hline 8} {it:Example 4 - Multiway and interacted clusters} {hline 8}
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

{space 2}{hline 8} {it:Example 5 - A fixed-width string fixed effect} {hline 8}
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

{space 2}{hline 8} {it:Example 6 - An interacted firm-year fixed effect} {hline 8}
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

{space 2}{hline 8} {it:Example 7 - Firm intercepts and firm-specific slopes} {hline 8}
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

{space 2}{hline 8} {it:Example 8 - Multiple slope-only absorbed channels} {hline 8}
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

{space 2}{hline 8} {it:Example 9 - Saved effects, residuals, and prediction} {hline 8}
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

{space 2}{hline 8} {it:Example 10 - No finite regressors and disconnected components} {hline 8}
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

{space 2}{hline 8} {it:Example 11 - Low-memory execution and solver diagnostics} {hline 8}
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
          matrix list e(fereg_solver)
          matrix list e(fereg_runtime)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 76}
{space 2}{it:({stata fereg_run low_memory using fereg.sthlp:click to run})}

{space 2}{hline 8} {it:Example 12 - Benchmark against reghdfe and Stata 19 areg} {hline 8}
{cmd}{...}
          preserve
{* example_start - benchmark}{...}
          version 19.0
          which reghdfe
          clear
          set seed 2026082612
          local N 250000
          local repetitions 3
          local comparison_threads = c(processors)
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

          quietly fereg y x1 x2, absorb(worker firm) dof(pairwise) ///
              threads(`comparison_threads')
          scalar __fereg_doc_b_fereg = _b[x1]
          scalar __fereg_doc_rss_fereg = e(rss)
          scalar __fereg_doc_N_fereg = e(N)
          quietly reghdfe y x1 x2, absorb(worker firm) dof(pairwise)
          scalar __fereg_doc_b_reghdfe = _b[x1]
          scalar __fereg_doc_rss_reghdfe = e(rss)
          scalar __fereg_doc_N_reghdfe = e(N)
          quietly areg y x1 x2, absorb(worker firm) dfabsorb ///
              tolerance(1e-12) iterate(10000)
          scalar __fereg_doc_b_areg = _b[x1]
          scalar __fereg_doc_rss_areg = e(rss)
          scalar __fereg_doc_N_areg = e(N)

          display as text "Coefficient on x1: fereg=" ///
              as result %12.8f __fereg_doc_b_fereg ///
              as text " reghdfe=" as result %12.8f __fereg_doc_b_reghdfe ///
              as text " areg=" as result %12.8f __fereg_doc_b_areg
          display as text "RSS: fereg=" as result %12.4f __fereg_doc_rss_fereg ///
              as text " reghdfe=" as result %12.4f __fereg_doc_rss_reghdfe ///
              as text " areg=" as result %12.4f __fereg_doc_rss_areg

          assert __fereg_doc_N_fereg == __fereg_doc_N_reghdfe
          assert __fereg_doc_N_fereg == __fereg_doc_N_areg
          assert reldif(__fereg_doc_b_fereg,__fereg_doc_b_reghdfe) < 1e-6
          assert reldif(__fereg_doc_b_fereg,__fereg_doc_b_areg) < 1e-6
          assert reldif(__fereg_doc_rss_fereg,__fereg_doc_rss_reghdfe) < 1e-6
          assert reldif(__fereg_doc_rss_fereg,__fereg_doc_rss_areg) < 1e-6

          timer clear 97
          timer clear 98
          timer clear 99
          timer on 97
          quietly fereg y x1 x2, absorb(worker firm) dof(pairwise) ///
              threads(`comparison_threads')
          timer off 97
          timer on 98
          quietly reghdfe y x1 x2, absorb(worker firm) dof(pairwise)
          timer off 98
          timer on 99
          quietly areg y x1 x2, absorb(worker firm) dfabsorb ///
              tolerance(1e-12) iterate(10000)
          timer off 99
          timer on 97
          quietly fereg y x1 x2, absorb(worker firm) dof(pairwise) ///
              threads(`comparison_threads')
          timer off 97
          timer on 98
          quietly reghdfe y x1 x2, absorb(worker firm) dof(pairwise)
          timer off 98
          timer on 99
          quietly areg y x1 x2, absorb(worker firm) dfabsorb ///
              tolerance(1e-12) iterate(10000)
          timer off 99
          timer on 97
          quietly fereg y x1 x2, absorb(worker firm) dof(pairwise) ///
              threads(`comparison_threads')
          timer off 97
          timer on 98
          quietly reghdfe y x1 x2, absorb(worker firm) dof(pairwise)
          timer off 98
          timer on 99
          quietly areg y x1 x2, absorb(worker firm) dfabsorb ///
              tolerance(1e-12) iterate(10000)
          timer off 99

          quietly timer list
          scalar __fereg_doc_t_fereg = r(t97)/`repetitions'
          scalar __fereg_doc_t_reghdfe = r(t98)/`repetitions'
          scalar __fereg_doc_t_areg = r(t99)/`repetitions'
          display as text "Mean warm seconds over `repetitions' repetitions:"
          display as text "  fereg             " as result %9.4f __fereg_doc_t_fereg
          display as text "  reghdfe           " as result %9.4f __fereg_doc_t_reghdfe
          display as text "  Stata 19 areg HDFE" as result %9.4f __fereg_doc_t_areg
          display as text "  fereg/reghdfe     " ///
              as result %9.3f (__fereg_doc_t_fereg/__fereg_doc_t_reghdfe)
          display as text "  fereg/areg        " ///
              as result %9.3f (__fereg_doc_t_fereg/__fereg_doc_t_areg)
          timer clear 97
          timer clear 98
          timer clear 99
          scalar drop __fereg_doc_b_fereg __fereg_doc_rss_fereg ///
              __fereg_doc_N_fereg __fereg_doc_b_reghdfe ///
              __fereg_doc_rss_reghdfe __fereg_doc_N_reghdfe ///
              __fereg_doc_b_areg __fereg_doc_rss_areg __fereg_doc_N_areg ///
              __fereg_doc_t_fereg __fereg_doc_t_reghdfe __fereg_doc_t_areg
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 76}
{space 2}{it:({stata fereg_run benchmark using fereg.sthlp:click to run benchmark})}

{pstd}
Example 12 warms all three commands before timing, verifies the coefficient on
{cmd:x1}, the estimation sample size, and RSS, and averages three end-to-end
runs. It uses a tighter {cmd:areg} convergence tolerance for numerical parity,
the current Stata processor setting for Stata's commands, and passes the same
allowance to {cmd:fereg}. Change {cmd:local N} and the number
of repetitions for a more demanding local comparison. Release claims require
the frozen exact-SHA benchmark suite, not this example.

{marker requirements}
{title:Requirements and development status}

{pstd}
{cmd:fereg} requires Stata 18 or newer and one self-contained plugin for its
platform: {cmd:fereg_macos.plugin}, {cmd:fereg_linux.plugin}, or
{cmd:fereg_windows.plugin}. Users do not need a compiler or separate runtime.
There is one maintained engine and no engine selector.

{pstd}
Version 0.5.0 is available as a binary beta for Linux x86-64, Windows x86-64
and macOS (Apple Silicon/Intel universal). See {cmd:docs/VALIDATION.md} for
platform-specific test coverage and limitations, and
{cmd:docs/fixest_benchmarks.md} and {cmd:docs/veneto_benchmarks.md} for
descriptive comparisons with fixest, PyFixest, reghdfe and {cmd:areg}.

{marker sources}
{title:Sources and acknowledgements}

{pstd}
Development and compatibility validation use the independently maintained
{browse "https://github.com/sergiocorreia/reghdfe":reghdfe},
{browse "https://github.com/sergiocorreia/ppmlhdfe":ppmlhdfe}, and
{browse "https://github.com/py-econometrics/pyfixest":PyFixest} projects as
scientific or behavioral references. They are not called by {cmd:fereg} at
runtime, and their source is not copied into the package.

{pstd}
The native bridge uses the official
{browse "https://www.stata.com/plugins/":Stata Plugin Interface} sources
published by StataCorp LP. Exact reference revisions, licenses, and dependency
boundaries are recorded in the source distribution's
{cmd:references.lock.toml} and {cmd:THIRD_PARTY_NOTICES.md}.

{marker also}
{title:Also see}

{p 0 24}
Online: {help regress}, {help areg}, {help xtreg}, {help fvvarlist},
{help weights}, {help fereg_postestimation:fereg postestimation},
{help feppml}
{p_end}

{p 4 4 2}
Web: {browse "https://www.stata.com/new-in-stata/high-dimensional-fixed-effects/":Stata 19 high-dimensional fixed effects}


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
