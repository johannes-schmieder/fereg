{smcl}
{* *! version 0.5.0 26aug2026}{...}
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

{p 8 16 2}{cmd:fereg,} {cmd:version}

{p 8 16 2}{cmd:fereg,} {cmd:doctor}
[{cmd:threads(}{it:#}{cmd:)} {cmd:verbose}]

{p 8 16 2}{cmd:fereg,} {cmd:selftest}
[{cmd:threads(}{it:#}{cmd:)}]

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
    {cmd:solver(auto|map|cg|it|squarem|lsmr|cmg)}{col 38}fixed-effect solver
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
{cmd:solver(auto)} is the default. It uses direct one-pass demeaning for one
fixed effect. Before the bounded multi-effect pilot, categorical intercept-only
designs select CMG when they have no nuisance Schur columns, at least eight
observations per largest-effect level, and at least a 10:1 core level-count
ratio. Other designs select a certified MAP, CG, or LSMR route after the pilot.
Auto-selected LSMR may make at most one recorded
fallback to certified CG. The result matrix records the selected method,
pilot, iterations, fallback, and final certificate.

{pstd}
Explicit methods are unaccelerated symmetric {cmd:map}, conjugate-gradient
accelerated MAP {cmd:cg}, Irons--Tuck {cmd:it}, {cmd:squarem}, and modified
{cmd:lsmr}. {cmd:cmg} constructs one worker--firm graph for the two
largest categorical intercept effects and uses CMG-preconditioned PCG for all
right-hand sides. Additional absorbed intercept effects are supported when
their combined non-base dimension is at most 64. Absorbed slopes are not
supported by {cmd:cmg}. A failed CMG construction or solve falls back visibly
to certified CG; a successful bulk solve is polished by CG when needed. All
explicit methods fail closed on a failed independent fixed-effect certificate.

{pstd}
The automatic CMG route is available in both memory modes, includes hierarchy
setup in timing and memory preflight, and records requested {cmd:auto} with
used solver code 7. Unsupported or failed setup returns to the existing auto
route; a failed CMG solve uses certified CG for the remaining fit work.

{pstd}
{cmd:preconditioner()} is valid only with {cmd:solver(auto)} or
{cmd:solver(lsmr)}. Its choices are {cmd:auto}, {cmd:off}, {cmd:diagonal}, and
{cmd:schwarz}; {cmd:none} aliases {cmd:off}, and {cmd:additive} aliases
{cmd:schwarz}. Use {cmd:preconditioner(off)} with {cmd:solver(cmg)}.

{pstd}
{cmd:memory(auto)} preflights fast and low-memory plans. {cmd:memory(fast)}
retains dense native workspaces. {cmd:memory(low)} processes regressors in
blocks and may use bounded temporary scratch storage, which is removed on
success, error, or Break. {cmd:poolsize()} sets the block width.

{pstd}
{cmd:threads(0)} uses the native default. A positive value requests a local
native thread team for that call. {cmd:deterministic} fixes the work partition
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
The beta does not implement PPML, IV/GMM, group outcomes with individual fixed
effects, HAC or Driscoll--Kraay covariance estimators, other GLMs, or GPU
execution. Unsupported requests return an explicit error.

{marker utilities}
{title:Installation diagnostics}

{pstd}
The following calls do not estimate a model:

{phang2}{cmd:. fereg, version}{p_end}
{phang2}{cmd:. fereg, doctor verbose}{p_end}
{phang2}{cmd:. fereg, selftest}{p_end}

{pstd}
{cmd:version} checks the ado/plugin handshake. {cmd:doctor} reports the loaded
artifact and, with {cmd:verbose}, the Stata host and processor information.
{cmd:selftest} runs the native diagnostic calculation. {cmd:threads(#)} may be
supplied to {cmd:doctor} or {cmd:selftest}; zero selects the native default.

{marker examples}
{title:Examples}

{pstd}
Each estimation example creates its own synthetic data and wraps the block in
{cmd:preserve}/{cmd:restore}, so it can be copied into a do-file without
replacing the caller's dataset. The blocks set Stata's random-number seed.
Example 12 additionally requires an installed {cmd:reghdfe} and Stata 19.

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
          quietly areg y x1 x2, absorb(worker firm) dfabsorb
          scalar __fereg_doc_b_areg = _b[x1]
          scalar __fereg_doc_rss_areg = e(rss)
          scalar __fereg_doc_N_areg = e(N)

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
          quietly areg y x1 x2, absorb(worker firm) dfabsorb
          timer off 99
          timer on 97
          quietly fereg y x1 x2, absorb(worker firm) dof(pairwise) ///
              threads(`comparison_threads')
          timer off 97
          timer on 98
          quietly reghdfe y x1 x2, absorb(worker firm) dof(pairwise)
          timer off 98
          timer on 99
          quietly areg y x1 x2, absorb(worker firm) dfabsorb
          timer off 99
          timer on 97
          quietly fereg y x1 x2, absorb(worker firm) dof(pairwise) ///
              threads(`comparison_threads')
          timer off 97
          timer on 98
          quietly reghdfe y x1 x2, absorb(worker firm) dof(pairwise)
          timer off 98
          timer on 99
          quietly areg y x1 x2, absorb(worker firm) dfabsorb
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

{pstd}
Example 12 warms all three commands before timing, verifies the coefficient on
{cmd:x1}, the estimation sample size, and RSS, and averages three end-to-end
runs. It uses the current Stata processor setting for Stata's commands and
passes the same allowance to {cmd:fereg}. Change {cmd:local N} and the number
of repetitions for a more demanding local comparison. Release claims require
the frozen exact-SHA benchmark suite, not this example.

{marker requirements}
{title:Requirements and development status}

{pstd}
{cmd:fereg} requires Stata 18 or newer and exactly one matching self-contained
plugin on the ado-path: {cmd:fereg_windows.plugin},
{cmd:fereg_linux.plugin}, or the universal {cmd:fereg_macos.plugin}. Users do
not need Rust, Python, Julia, a compiler, BLAS, or a separate native runtime.

{pstd}
Version 0.5.0 is unreleased beta source. Public OLS is active, while
{cmd:feppml} deliberately returns {cmd:r(498)} without posting an estimate.
See {cmd:STATUS.md} and {cmd:docs/compatibility_matrix.md} in the source
distribution for exact-SHA qualification evidence and platform caveats.

{marker author}
{title:Author}

{p 4 4 2}
Johannes F. Schmieder, Boston University, USA

{p 4 4 2}
Email: {browse "mailto:johannes@bu.edu":johannes@bu.edu}

{marker also}
{title:Also see}

{p 0 24}
Online: {help regress}, {help areg}, {help xtreg}, {help fvvarlist},
{help weights}, {help fereg_postestimation:fereg postestimation},
{help feppml}
{p_end}

{p 4 4 2}
Web: {browse "https://www.stata.com/new-in-stata/high-dimensional-fixed-effects/":Stata 19 high-dimensional fixed effects}
{.-}
