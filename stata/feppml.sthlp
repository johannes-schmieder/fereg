{smcl}
{* *! version 0.5.0 26sep2026}{...}
{.-}
help for {cmd:feppml} {right:(Johannes F. Schmieder)}
{.-}
{vieweralsosee "fereg" "help fereg"}{...}
{vieweralsosee "factor variables" "help fvvarlist"}{...}
{vieweralsosee "time-series variables" "help tsvarlist"}{...}

{title:Title}

{p 4 4 2}
{cmd:feppml} {hline 2} Poisson pseudo-maximum likelihood with
high-dimensional fixed effects

{marker quickstart}
{title:Quick start}

{pstd}
{cmd:feppml} estimates a Poisson mean model with a log link while absorbing
zero, one, or several high-dimensional fixed effects. The outcome may be a
nonnegative count or a nonnegative fractional value. A two-way fixed-effect
model with firm-clustered inference is

{phang2}{cmd:. feppml patents R_and_D capital, ///}{p_end}
{phang3}{cmd:absorb(firm year) vce(cluster firm)}{p_end}

{pstd}
Robust covariance is the default. The command removes singleton fixed-effect
observations unless {cmd:keepsingletons} is specified and, by default, applies
the certified separation sequence {cmd:fe simplex ir}. Estimation, inference,
requested observation-level output, and the completion receipt are one guarded
transaction. An error or Break posts no partial result.

{marker engines}
{title:Engine selection}

{pstd}
{cmd:engine(next)} selects the maintained engine and is the default for OLS
and PPML. {cmd:engine(legacy)} selects a separate frozen plugin and prints a
short note on every call. An explicit option takes precedence over the
{cmd:FEREG_ENGINE} global; an unset global defaults to {cmd:next}. Invalid
selections return {cmd:r(198)}. A missing selected binary returns {cmd:r(601)};
the command never switches engines automatically.

{pstd}
The legacy comparator is the frozen native artifact from
{cmd:legacy-comparator-v1} with the current shared ado files. It is useful for
comparison but can contain bugs; it does not define correct results. Next
corrects factor availability, full-matrix OLS multiway covariance, continuous
slope DoF validation and an exactly absorbed frequency-singleton factor.
Numerical agreement does not require identical bits across engines or thread
counts. Engine identity is stored in {cmd:e(feppml_engine)}; stored estimates retain it.
Use {cmd:fereg, version engine(legacy)}, {cmd:doctor}, or {cmd:selftest} to inspect
the selected plugin.

{marker syntax}
{title:Syntax}

{p 8 16 2}
{cmd:feppml} {it:depvar} [{it:indepvars}] {ifin} {weight},
[{it:options}]

  {it:option}{col 42}description
  {hline 78}
  {ul:Model, sample, and adjustment}
    {cmd:absorb(}{it:terms}{cmd:)}{col 42}fixed effects and absorbed slopes
    {cmd:keepsingletons}{col 42}retain singleton fixed-effect observations
    {cmd:offset(}{it:varname}{cmd:)}{col 42}add a known log-mean adjustment
    {cmd:exposure(}{it:varname}{cmd:)}{col 42}add the log of a positive exposure
    {cmd:separation(}{it:methods}{cmd:)}{col 42}certified separation methods

  {ul:Inference}
    {cmd:vce(robust)}{col 42}robust covariance; the default
    {cmd:vce(unadjusted)}{col 42}compatibility request mapped to robust
    {cmd:vce(cluster }{it:clustervars}{cmd:)}{col 42}one- through five-way clustering

  {ul:Saved observation-level output}
    {cmd:residuals(}{it:newvar}{cmd:)}{col 42}save a residual; default response
    {cmd:residualtype(}{it:type}{cmd:)}{col 42}select the saved residual formula
    {cmd:xb(}{it:newvar}{cmd:)}{col 42}save offset plus finite predictor
    {cmd:d(}{it:newvar}{cmd:)}{col 42}save total absorbed contribution
    {cmd:eta(}{it:newvar}{cmd:)}{col 42}save the complete linear predictor
    {cmd:xbd(}{it:newvar}{cmd:)}{col 42}alias for {cmd:eta()}
    {cmd:mu(}{it:newvar}{cmd:)}{col 42}save the fitted conditional mean
    {cmd:dropreason(}{it:newvar}{cmd:)}{col 42}save the original-row drop code

  {ul:Coefficient mean-bias correction}
    {cmd:biascorrection(exact|randomized)}{col 42}optional second-order correction
    {cmd:biasunit(}{it:varname}{cmd:)}{col 42}explicit independent score units
    {cmd:biasfe(}{it:stub}{cmd:)}{col 42}save raw/corrected FE contributions
    {cmd:biasprobes(}{it:#}{cmd:)}{col 42}fixed random probes; default 256
    {cmd:biasseed(}{it:#}{cmd:)}{col 42}random seed; default 739391
    {cmd:biasstream(}{it:#}{cmd:)}{col 42}independent stream; default 0

  {ul:Execution}
    {cmd:engine(next|legacy)}{col 42}native engine; default next
    {cmd:solver(auto|map|lsmr|cmg)}{col 42}fixed-effect solver; default {cmd:auto}
    {cmd:memory(auto|fast|low)}{col 42}memory plan; default {cmd:auto}
    {cmd:poolsize(}{it:#}{cmd:)}{col 42}low-memory block width; default 10
    {cmd:threads(}{it:#}{cmd:)}{col 42}native threads; default 0 (automatic)
    {cmd:deterministic}{col 42}fix native partition and merge order
    {cmd:tolerance(}{it:#}{cmd:)}{col 42}outer tolerance; default 1e-8
    {cmd:iterate(}{it:#}{cmd:)}{col 42}outer iteration limit; default 10,000
    {cmd:timeit}{col 42}print native total-to-commit time
  {hline 78}

{pstd}
The public weight forms are {cmd:[fw=}{it:exp}{cmd:]} and
{cmd:[pw=}{it:exp}{cmd:]}. {cmd:aweight} and {cmd:iweight} are recognized only
to return {cmd:r(101)}. The structural constant is always included; there is
no public {cmd:noconstant}, preconditioner, DoF-policy, or developer-injection
option.

{marker description}
{title:Description}

{pstd}
On final-sample observation {it:i}, {cmd:feppml} fits

{p 8 12 2}
{it:eta_i} = {it:offset_i} + {it:X_i beta} + {it:d_i},
{break}{it:mu_i} = exp({it:eta_i}).

{pstd}
{it:d_i} is the sum of the absorbed fixed-effect channels. The command always
uses the structural {cmd:_cons} normalization. With an absorbed intercept, the
constant may lie in the absorbed span and is then marked rather than counted
as a second identified direction.

{pstd}
The finite regressor list may be empty. It accepts continuous variables,
numeric factor main effects, all-categorical factor interactions, and Stata
lag, lead, and difference operators on continuous variables. Stata materializes
time-series terms before the native call, preserving panel gaps and permitting
a retained row to use a lag or lead from a source row outside {cmd:if} or
{cmd:in}. Categorical-by-continuous and continuous-by-continuous interactions
on the finite right-hand side are not supported.

{marker absorb}
{title:Absorbed terms and saved effects}

{pstd}
Terms inside {cmd:absorb()} are separated by spaces. Categorical sources may be
numeric or fixed-width strings.

  {it:syntax}{col 36}interpretation
  {hline 76}
    {cmd:worker firm}{col 36}separate worker and firm intercept effects
    {cmd:firm#year}{col 36}one intercept effect for each firm-year cell
    {cmd:firm##c.time}{col 36}firm intercepts and firm-specific time slopes
    {cmd:firm#c.time}{col 36}firm-specific slopes without firm intercepts
    {cmd:worker#c.(time tenure)}{col 36}two slope-only channels by worker
    {cmd:worker##c.(time tenure)}{col 36}worker intercept and two slope channels
    {cmd:fe_worker=worker}{col 36}save a normalized worker contribution
  {hline 76}

{pstd}
Categorical interactions use {cmd:#}. General categorical {cmd:##} expansion
inside {cmd:absorb()} is unsupported; write the desired main and interacted
effects explicitly. The special {cmd:##c.} form means a categorical intercept
plus a continuous slope, while {cmd:#c.} is slope-only.

{pstd}
A name to the left of {cmd:=} requests final-state normalized contribution
channels. For an intercept-plus-slope term, the intercept contribution uses the
base name and slope contributions use {cmd:_Slope1}, {cmd:_Slope2}, and so on.
A slope-only term creates only suffixed variables. These variables contain
observation-level contributions, including the slope source; requested
channels therefore add directly to {cmd:d()}. Individual coordinates need not
be uniquely identified, but their total fitted contribution is.

{pstd}
All requested names, including mechanically derived {cmd:_Slope#} names, must
be new and mutually distinct. Saved effects are missing outside {cmd:e(sample)}.
Requesting them cannot change the sample, estimates, covariance, objective, or
iteration path.
Saved contributions must also pass reconstruction checks against the total
fitted contribution. A failed check returns an error without publishing partial
results.

{marker sample}
{title:Outcome, sample, and adjustment}

{pstd}
Stata first evaluates {cmd:if} and {cmd:in}. The native estimator then applies,
in order, outcome, regressor, weight, fixed-effect, cluster, and
offset/exposure availability and domain rules. Missing values outside the
candidate sample are not inspected. Missing values on candidate rows are
sample exclusions in that registered order.

{pstd}
The outcome must be finite and nonnegative; fractional values and zeros are
valid. A finite negative candidate outcome returns {cmd:r(459)}. A final sample
with no rows or no positive weighted outcome returns {cmd:r(2001)} because no
finite PPML intercept exists.

{pstd}
{cmd:offset()} and {cmd:exposure()} are mutually exclusive. A missing
adjustment excludes the row. A retained offset must be finite. A retained
exposure must be finite and strictly positive and is transformed once to its
natural logarithm. Invalid retained adjustment values return {cmd:r(459)}.

{pstd}
Unless {cmd:keepsingletons} is specified, singleton observations are removed
to a fixed point. A separation deletion can induce another singleton cascade.
Singleton and separation deletions are counted separately. {cmd:e(sample)} and
every requested fitted output use the final post-separation ledger.

{marker weights}
{title:Weights}

{pstd}
Frequency weights must be finite, strictly positive integers representable in
the supported exact multiplicity range. They are literal row replications for
the objective, effective sample size, robust meat, cluster scores, and DoF.
An invalid retained fweight returns {cmd:r(401)}. Consequently {cmd:e(N)} and
{cmd:e(N_effective)} equal the sum of retained fweights, while
{cmd:e(N_physical)} counts retained physical rows.

{pstd}
Probability weights must be finite and strictly positive on retained rows;
invalid values return {cmd:r(459)}. Internally they are normalized to sum to
the retained physical count. A common rescaling therefore leaves coefficients,
covariance, objectives, residuals, and fitted values unchanged.
{cmd:e(N_effective)} equals {cmd:e(N_physical)} for pweights. Missing weights
exclude rows, and the original public weight expression is stored in
{cmd:e(wexp)}.

{marker separation}
{title:Separation and drop reasons}

{pstd}
The default {cmd:separation(fe simplex ir)} sequence applies exact zero-only
intercept-FE deletion, bounded full-model simplex detection, and comprehensive
iterated rectifier detection in registered order. {cmd:separation(default)}
requests that same sequence. {cmd:mu} is optional and is not in the default.
An explicit request may name any nonrepeated subset of {cmd:fe},
{cmd:simplex}, {cmd:ir}, and {cmd:mu}. {cmd:separation(none)} disables proactive
separation detection but does not disable singleton pruning. {cmd:none} and
{cmd:default} must appear alone.

{pstd}
Every accepted deletion has an independent certificate. A deletion restarts
the registered method sequence on a newly rebased sample until a fixed point
is reached. If the bounded simplex workspace does not fit, the default chain
may continue only when comprehensive IR certifies the same ledger generation;
an unresolved or simplex-only over-cap request returns {cmd:r(909)}.

{pstd}
{cmd:dropreason(}{it:newvar}{cmd:)} creates a long integer variable on the
complete original observation axis. Retained rows have code 0; unlike fitted
outputs, excluded rows are not missing. The codes are

  {it:code}{col 12}{it:meaning}
  {hline 66}
    0{col 12}retained in {cmd:e(sample)}
    1{col 12}excluded by {cmd:if} or {cmd:in}
    2{col 12}missing outcome
    3{col 12}missing finite regressor
    4{col 12}invalid or missing weight availability class
    5{col 12}missing fixed-effect source
    6{col 12}missing cluster source
    7{col 12}missing offset or exposure
    8{col 12}singleton fixed-effect observation
    9{col 12}exact fixed-effect separation
   10{col 12}simplex-certified separation
   11{col 12}iterated-rectifier-certified separation
   12{col 12}{cmd:mu}-certified separation
   13{col 12}other model exclusion
  {hline 66}

{pstd}
Invalid retained outcomes, weights, offsets, or exposures are fatal domain
errors rather than successful drop codes. {cmd:e(feppml_drop_counts)} reports
the count for every code, and {cmd:e(separated)} sums codes 9 through 12.

{marker inference}
{title:Covariance and degrees of freedom}

{pstd}
Robust covariance is always the effective nonclustered PPML covariance.
{cmd:vce(robust)} is the default. For pinned compatibility,
{cmd:vce(unadjusted)} records an unadjusted request but maps to the same robust
covariance; model-based IID covariance is not in the public surface, and
{cmd:vce(iid)} and {cmd:vce(ols)} return {cmd:r(198)}.

{pstd}
{cmd:vce(cluster }{it:clustervars}{cmd:)} accepts one through five cluster
dimensions. Separate dimensions with spaces. Use {cmd:#}, as in
{cmd:state#year}, to form one interaction dimension; {cmd:##} is invalid in a
cluster term. Multiway covariance uses collision-safe grouping and CGM
inclusion-exclusion, followed by one common {it:G_min}/({it:G_min}-1)
correction and the registered positive-semidefinite repair.

{pstd}
Structural residual DoF are

{p 8 12 2}
{it:N_effective} - {it:finite within rank} - {it:absorbed rank}.

{pstd}
Clustered reported DoF are additionally capped at {it:G_min}-1. Publication
requires positive reported DoF and at least two groups in every cluster
dimension. Inference is z and Wald chi-squared, not OLS t and F. PPML does not
store OLS RSS, TSS, MSS, R-squared, adjusted R-squared, RMSE, F, or
{cmd:e(df_r)} fields.

{marker bias}
{title:Optional coefficient mean-bias correction}

{pstd}
{cmd:biascorrection(exact)} or {cmd:biascorrection(randomized)} requests an
unprojected second-order correction at the accepted ordinary fit. The supported
model has exactly two categorical intercept FE sets and ordinary numeric
controls, with the existing offset/exposure and unweighted, fweight, or pweight
semantics. Absorbed slopes and finite factor expansions are outside this scope.
{cmd:biasunit(}{it:varname}{cmd:)} is required and names the numeric independent
score unit; it is never inferred from {cmd:vce(cluster)}. At least two retained
units are required. Rows within a unit may be dependent.

{pstd}
Let x include both FE sets and every identified control, a denote the effective
fitting weight, and mu the ordinary fitted mean. Define
S_g = sum_g a*x*(y-mu), J_g = sum_g a*mu*x*x', J = sum_g J_g,
K = sum_g S_g*S_g', and V = J^(-1)*K*J^(-1). The two increments are

{p 8 12 2}
c_D = J^(-1)*sum_g J_g*J^(-1)*S_g,
{break}c_C = (1/2)*J^(-1)*sum_rows a*mu*x*(x'*V*x),
{break}beta_bc = beta_hat + c_D + c_C.

{pstd}
This V is a full-system intermediate, with no HC1, DoF, multiway-cluster, or
PSD adjustment. It is not the public {cmd:e(V)}. No coefficient caps are used.
The approximation requires an interior fit and appropriate independent-unit
sampling and observation-process assumptions; exposure syntax alone does not
establish those assumptions. High leverage, weak support, few units, and sparse
outcomes can limit its accuracy. It does not supply corrected confidence
intervals, unbiased rates, or unbiased variances of estimated effects.

{pstd}
{bf:Choosing bias units and FE support.} {cmd:biasunit()} defines independent
score units, not deletion units; no leave-one-unit-out PPML refits are used.
Units need not nest within either FE set. Every retained level of {bf:both}
FE sets must contain at least two distinct bias units. Otherwise the whole
requested correction fails, including when {cmd:biasfe()} is omitted.

{pstd}
Absorbing individual {cmd:workerid} and using {cmd:biasunit(workerid)} is
rejected: each worker FE has one supporting unit and its fitted unit score
is forced to zero. The empirical score covariance can miss its estimation
noise. Worker-group FEs with multiple independent workers per group can pass,
provided the other FE set also has multiple units per level. Finer spell units
are defensible only under appropriate independence assumptions. Crossing units
are allowed; nesting alone does not establish validity.

{pstd}
The screen uses actual membership on the final retained sample. Copying or
relabelling IDs, frequency copies, or more rows within the same unit do not
create independent support. Sample restrictions can remove support. More probes
or exact computation cannot repair it; exact evaluates an approximate statistical
correction. Do not split dependent units merely to pass. This conservative
supported-scope restriction does not claim that every slope increment must be
wrong, and passing does not guarantee adequate bias reduction.

{pstd}
The ordinary sample is preserved. A missing bias-unit ID on any retained row
rejects the transaction, rather than dropping that row. Any outcome-dependent
separation deletion also rejects a requested correction. Other ordinary
exclusions retain their usual interpretation; the correction refers to that
retained sample. Frequency copies stay within the supplied independent unit.
Common weight scaling leaves the correction invariant.

{pstd}
Exact mode streams units and is limited to 10,000 units and one billion
row-unit contributions. Randomized mode uses a fixed {cmd:biasprobes()} count
(default 256, at least 2), {cmd:biasseed()} (default 739391), and
{cmd:biasstream()} (default 0). Seed and stream are unsigned 32-bit integers.
Exact mode rejects these randomization options. Philox4x32-10 assigns signs by
numeric-unit and probe identity without changing Stata's RNG. Use different
streams for separate branches. Signs are stable across row order, native
threads, and memory routes; floating-point results obey the ordinary numerical
reproducibility limits. The production implementation streams one RHS at a time.
Small supports may reuse a cached information factor; larger supports and
{cmd:memory(low)} reuse the sparse weighted-linear solver.

{pstd}
Ordinary {cmd:e(b)}, {cmd:e(V)}, likelihood, means, and {cmd:predict} remain
attached to the ordinary estimate. The additional coefficient row matrices,
in ordinary coefficient order, are {cmd:e(b_bias_raw)}, {cmd:e(b_bias_d)},
{cmd:e(b_bias_c)}, {cmd:e(b_bias_total)}, and {cmd:e(b_bc)}. Omitted positions
and the absorbed normalized constant are zero. Numerical standard deviations
are {cmd:e(bias_mcse_d)}, {cmd:e(bias_mcse_c)}, and
{cmd:e(bias_mcse_total)}; {cmd:e(bias_cov_dc)} reports the numerical covariance
between increments. These quantify random-probe variation conditional on the
fit, not sampling uncertainty. Up to eight predetermined probe groups supply
the estimates, including covariance in the total. Exact-mode values are zero.

{pstd}
{cmd:biasfe(}{it:stub}{cmd:)} creates 18 double variables, named
{it:stub}{cmd:_1_}{it:channel} and {it:stub}{cmd:_2_}{it:channel} in absorb order.
Channels are {cmd:raw}, {cmd:d}, {cmd:c}, {cmd:total}, {cmd:bc}, {cmd:sed},
{cmd:sec}, {cmd:covdc}, and {cmd:setotal}. Names must be new and fit Stata's
name limit. Values are missing outside {cmd:e(sample)}. Within each connected
component the smallest canonical second-FE level is anchored at zero, with
the constant assigned to the first FE. Raw effects and both increments share
this support-based linear normalization. Ordinary saved FEs keep their usual
normalization. Re-anchoring a branch requires subtracting its chosen second-FE
anchor and adding it to its first FE in every raw and increment channel.
Branch supports must admit a compatible common anchor.
The numerical SE channels apply to the reported anchor. Cross-level covariances
are not returned, so shifting these SE channels does not give valid numerical
SEs for a different anchor.

{pstd}
{cmd:e(bias_diagnostics)} is a named 21-by-1 matrix: method (1 exact, 2 randomized),
unit count, probes, seed, stream, numerical groups, solve count, maximum
normalized score error, maximum reconstruction error, correction seconds,
workspace bound in bytes, maximum absolute total increment, minimum/maximum
mean, minimum/maximum rows per unit, route (1 sparse, 2 cached factor), validity
(1 numerical checks and necessary support screen passed), certificate tolerance, minimum FE-level
information, and maximum FE-indicator leverage lower bound. The leverage field
is a lower bound on maximum full-system leverage, not a full hat-diagonal
estimate. Exact mode uses G+2 solves; numerical diagnostics make randomized
mode use B+2*min(8,B) solves. Allocation, solve, interruption, or validation
failure restores prior results and creates no requested output variables.

{pstd}
{cmd:e(bias_support)} has rows {cmd:fe1} and {cmd:fe2} in absorb order and
columns {cmd:levels}, {cmd:min_units}, {cmd:p25_units}, {cmd:median_units},
{cmd:p75_units}, {cmd:max_units}, {cmd:single_unit_levels},
{cmd:single_unit_fraction}, {cmd:min_info_units}, {cmd:median_info_units}, and
{cmd:max_unit_info_share}. Quantiles are unweighted across retained levels and
use nearest rank. Single-unit counts and fractions are zero on success;
refusals report affected level counts in the error and publish no new results.

{pstd}
For each level, aggregate information a*mu within each bias unit, divide by
that level's total to obtain unit shares p_g, and compute 1/sum_g(p_g^2).
The minimum and median of these concentration equivalents are returned along
with the largest unit share across levels. This is not a certified effective
sample size. Inspect small counts and dominant shares; no universal cutoff is
imposed. Graph bottlenecks or single-unit-dependent contrasts may remain even
when every level has two units. Neither {cmd:e(bias_validity)} nor the
{cmd:valid} diagnostic certifies independence or statistical accuracy.

{marker execution}
{title:Memory, iterations, and threads}

{pstd}
{cmd:memory(auto)} preflights complete fast and low-memory fit-lifetime plans
and selects a qualified plan that fits. {cmd:memory(fast)} retains reusable
native workspaces. {cmd:memory(low)} uses bounded column pools and may use
validated private scratch storage. Scratch is removed on success, error,
panic, or Break. If no certified plan fits, the command returns {cmd:r(909)}
before publishing output. {cmd:poolsize()} sets the low-memory block width.

{pstd}
{cmd:threads(0)} uses the native default ceiling; a positive integer sets the
maximum native team for the call. Rust may select a smaller team according to
the model and workload before memory planning. {cmd:e(feppml_requested_threads)}
records the request and {cmd:e(feppml_threads)} records the selected team.
{cmd:deterministic} fixes reduction
partitions and merge order for the same platform, route, and thread count.
{cmd:solver(auto)} is the default. It selects LSMR for exactly two or three
categorical intercept effects when fast-memory execution and the complete
specialized workspace fit the native memory plan; otherwise it selects MAP.
Absorbed slopes and low-memory execution select MAP. Auto does not select CMG.
{cmd:solver(map)} requests the canonical MAP-owned projection family explicitly.
For multiple effects with absorbed slopes, this family uses a current-weight
coefficient-space Schur/CG kernel when its workspace fits. It prepares weighted
group bases once per absorption call, stops columns independently, and accepts
only freshly reconstructed and certified residuals. Its reported iterations
count Krylov steps; they are not directly comparable to symmetric MAP sweeps.
Insufficient optional workspace retains generic MAP.
{cmd:solver(lsmr)} selects one native joint-incremental LSMR engine. Its
concrete identity is {cmd:lsmr_fused} with no active finite controls and
{cmd:lsmr_joint} otherwise. {cmd:solver(cmg)} selects only prepared fused-FWL
CMG. The requested family, selected engine, final effective route, checkpoint,
retry, and fallback work are reported separately. Explicit LSMR and CMG fits
with two or three categorical effects use independently re-certified
current-weight LSMR checkpoint projection at every retained sample size.
The canonical session owns the first full-width rank decision, strict
convergence checkpoint, and exact final-Hessian synchronization.

{pstd}
The initial LSMR rollout supports at least two categorical intercept fixed
effects; CMG supports exactly two or three. For three effects, its two largest
retained dimensions form the graph core; the remaining dimension may have at
most 64 non-base indicators. This limit does not count finite regressors.
Both retain the full PPML output
surface, including saved fixed effects recovered from the accepted canonical
final-Hessian state, and require the fast memory route.
{cmd:memory(auto)} may select that route; explicit {cmd:memory(low)} is not yet
supported for these two solvers. Recognized requests outside this envelope
return {cmd:r(498)}. A viable selected route that cannot fit its modeled
workspace returns {cmd:r(909)}. Numerical CMG nonconvergence triggers a visible,
permanent ordinary-frame MAP fallback. A failed independent checkpoint
certificate permits one tighter LSMR batch on untouched authoritative inputs;
a second numerical rejection falls back visibly to MAP. Certification
tolerances do not change. Unsupported topology, memory failure, invalid input, interruption, and
internal errors remain fatal.

{pstd}
The outer IRLS tolerance defaults to 1e-8 and must lie strictly between zero
and one. {cmd:iterate()} defaults to 10,000 and must be positive. Publication
requires two consecutive accepted iterations satisfying the deviance and
independent score criteria, followed by a strict final solve and rank,
predictor, Hessian, covariance, and fixed-effect certificates. {cmd:timeit}
prints native total-to-commit seconds; detailed phase, allocation, memory,
scratch, and thread diagnostics are stored in {cmd:e(feppml_runtime)} and
{cmd:e(feppml_*)} scalars.

{marker saved}
{title:Saved observation-level output}

{pstd}
Observation-sized values are retained only when explicitly requested. All
fitted and residual outputs below are double variables, are finite on
{cmd:e(sample)}, and are missing elsewhere. {cmd:dropreason()} is the sole
complete-original-axis exception.

  {it:option}{col 28}{it:value on e(sample)}
  {hline 76}
    {cmd:xb(}{it:newvar}{cmd:)}{col 28}offset/exposure plus finite {it:X beta}
    {cmd:d(}{it:newvar}{cmd:)}{col 28}total absorbed contribution {it:d}
    {cmd:eta(}{it:newvar}{cmd:)}{col 28}{it:offset + X beta + d}
    {cmd:xbd(}{it:newvar}{cmd:)}{col 28}alias for {cmd:eta()}
    {cmd:mu(}{it:newvar}{cmd:)}{col 28}{it:exp(eta)}
    {cmd:residuals(}{it:newvar}{cmd:)}{col 28}selected residual formula
  {hline 76}

{pstd}
{cmd:eta()} and {cmd:xbd()} are aliases and may not be combined. Requested
output names and saved-effect names must be new and distinct.
{cmd:residuals()} defaults to {cmd:residualtype(response)}. The supported
residual types and formulas are

  {it:type}{col 22}{it:value}
  {hline 68}
    {cmd:response}{col 22}{it:y - mu}
    {cmd:score}{col 22}{it:y - mu} (the unweighted public score residual)
    {cmd:pearson}{col 22}{it:(y - mu) / sqrt(mu)}
    {cmd:working}{col 22}{it:(y - mu) / mu}
    {cmd:deviance}{col 22}{it:2[mu-y+y log(y/mu)]}; zero case {it:2mu}
    {cmd:anscombe}{col 22}{it:1.5(y^(2/3)-mu^(2/3))/mu^(1/6)}
  {hline 68}

{pstd}
A non-{cmd:none} {cmd:residualtype()} requires {cmd:residuals()}, and
{cmd:residuals()} may not be combined with {cmd:residualtype(none)}.
{cmd:residualtype(none)} without {cmd:residuals()} is an accepted no-output
request. The deviance value is the nonnegative per-row contribution, not a
signed square-root residual.

{marker predict}
{title:Prediction}

{pstd}
{cmd:predict} accepts an optional {cmd:if} or {cmd:in} and writes values only
where that restriction and {cmd:e(sample)} are both true. The default is
{cmd:mu}.

  {it:statistic}{col 24}{it:result}
  {hline 72}
    default or {cmd:mu}{col 24}{it:exp(eta)}
    {cmd:xb}{col 24}offset/exposure plus current finite predictor
    {cmd:d} or {cmd:absorbed}{col 24}total absorbed contribution
    {cmd:eta} or {cmd:xbd}{col 24}complete linear predictor
    {cmd:response}{col 24}{it:y - mu}
    {cmd:score}{col 24}{it:y - mu}
    {cmd:pearson}{col 24}{it:(y - mu) / sqrt(mu)}
    {cmd:working}{col 24}{it:(y - mu) / mu}
    {cmd:deviance}{col 24}nonnegative per-row deviance contribution
    {cmd:anscombe}{col 24}Anscombe residual formula above
  {hline 72}

{pstd}
{cmd:predict, xb} is always available and reconstructs from the posted
coefficients, current finite covariates, and current offset or exposure
variable. With no absorbed effects, {cmd:d} is zero and the complete prediction
surface is reconstructible without saved observation-level output.

{pstd}
With absorbed effects, {cmd:predict, d} requires a retained {cmd:d()} variable.
{cmd:eta}, {cmd:mu}, and all residual statistics require a retained compatible
{cmd:d()}, {cmd:eta()}/{cmd:xbd()}, or {cmd:mu()} variable from estimation. If
none was requested, the command returns {cmd:r(498)} before creating the
prediction variable; {cmd:xb} remains available.

{pstd}
When several compatible outputs exist, retained {cmd:d()} is authoritative:
the prediction combines it with the current finite covariates and adjustment.
Without {cmd:d()}, retained {cmd:eta()} is the first snapshot fallback and
retained {cmd:mu()} is the second. Snapshot fallbacks deliberately do not
track later edits to finite covariates or adjustment variables. Deleting the
authoritative retained variable produces {cmd:r(111)}. Combining members of
the {cmd:d}/{cmd:absorbed} or {cmd:eta}/{cmd:xbd} alias pair, or requesting
more than one statistic, returns {cmd:r(198)} and creates no target.

{marker stored}
{title:Stored results}

{pstd}
{cmd:feppml} stores {cmd:e(b)}, {cmd:e(V)}, and the final {cmd:e(sample)}.
{cmd:e(N)} is effective N: it equals retained physical rows unless fweights
are used. Core sample and model scalars are

{p 8 12 2}
{cmd:e(N_physical)}, {cmd:e(N_effective)}, {cmd:e(N_candidate)},
{cmd:e(N_full)}, {cmd:e(rank)}, {cmd:e(feppml_model_rank)}, {cmd:e(df_m)},
{cmd:e(df_resid)}, {cmd:e(df_structural)}, {cmd:e(df_a)},
{cmd:e(N_hdfe)}, and {cmd:e(N_hdfe_extended)}.

{pstd}
Fit and convergence scalars include {cmd:e(deviance)}, {cmd:e(ll)},
{cmd:e(ll_0)}, {cmd:e(r2_p)}, {cmd:e(mu_0)}, conditionally {cmd:e(chi2)} and
{cmd:e(df_chi2)}, {cmd:e(sum_y)}, {cmd:e(ic)} (outer iterations),
{cmd:e(inner_iterations)}, {cmd:e(step_halvings)}, and {cmd:e(converged)}.
Sample-mutation scalars include {cmd:e(num_singletons)},
{cmd:e(drop_singletons)}, and {cmd:e(separated)}. Weight results include
{cmd:e(sumweights)} and, when representable and applicable,
{cmd:e(raw_sumweights)}, {cmd:e(weight_multiplier)}, and their logarithms.
Clustered fits additionally store {cmd:e(N_clustervars)},
{cmd:e(N_clust1)} through {cmd:e(N_clust5)} as applicable,
{cmd:e(N_clust)}, and {cmd:e(G_min)}.

{pstd}
The closed diagnostic matrix set is

{p 8 12 2}
{cmd:e(feppml_status)}, {cmd:e(feppml_fit)}, {cmd:e(feppml_dof)},
{cmd:e(feppml_fe)}, {cmd:e(feppml_cluster)}, {cmd:e(feppml_subset)},
{cmd:e(feppml_separation)}, {cmd:e(feppml_inference)},
{cmd:e(feppml_drop_counts)}, {cmd:e(feppml_separation_sequence)},
{cmd:e(feppml_certificate)}, {cmd:e(feppml_solver)},
{cmd:e(feppml_runtime)}, and
{cmd:e(feppml_receipt)}.

{pstd}
With absorbed channels, {cmd:e(dof_table)} contains categories, redundancy,
rank, approximation, and nesting by channel. {cmd:e(feppml_status)} preserves
canonical coefficient slots and distinct base, omitted, FE-collinear, and
numerically rank-deficient statuses. {cmd:e(feppml_receipt)} contains the
versioned identity and final COMPLETE marker. Scalars beginning
{cmd:e(feppml_)} expose bounded VCE/PSD, separation-ledger, memory, route,
allocation, timing, thread, and certificate evidence.

{pstd}
{cmd:e(feppml_solver)} has rows {cmd:map}, {cmd:lsmr_fused},
{cmd:lsmr_joint}, and {cmd:cmg_prepared}. Its fixed 22-column result-wire 1.2
layout records repeated request/selection/effective/checkpoint identities and
per-engine frame, retry, fallback, iteration, timing, and certificate maxima.
The macros {cmd:e(solver)}, {cmd:e(feppml_solver_requested)},
{cmd:e(feppml_solver_selected)}, {cmd:e(feppml_solver_engine)},
{cmd:e(feppml_solver_effective)}, {cmd:e(feppml_solver_engine_effective)},
{cmd:e(feppml_solver_checkpoint)}, and
{cmd:e(feppml_solver_fallback_reason)} provide named identities;
{cmd:e(feppml_solver_fallback_count)} is the total visible solver fallback
count. {cmd:e(feppml_linear_route)} remains a compatibility alias for the
engine that produced the accepted final canonical linear frame, not the
ordinary-frame effective route.

{pstd}
Standard macros include {cmd:e(cmd)} = {cmd:feppml}, {cmd:e(cmdline)},
{cmd:e(model)} = {cmd:poisson}, {cmd:e(depvar)}, {cmd:e(indepvars)},
{cmd:e(vce)}, {cmd:e(absvars)}, {cmd:e(separation)}, {cmd:e(memory)},
{cmd:e(adjustment)}, weight and cluster metadata when applicable, and
{cmd:e(predict)} = {cmd:feppml_p}. Requested output names are stored as
{cmd:e(resid)}, {cmd:e(xb_var)}, {cmd:e(d_var)}, {cmd:e(eta_var)},
{cmd:e(mu_var)}, and {cmd:e(drop_reason_var)} when present. Use
{cmd:ereturn list} and {cmd:matrix list} for the complete versioned inventory.

{marker failure}
{title:Failure behavior and atomicity}

{pstd}
Every requested output is privately named until the complete result and all
identities validate. Syntax or data error, nonconvergence, failed certificate,
allocation failure, protocol mismatch, panic, or user Break restores the prior
named or unnamed estimates, {cmd:e(sample)}, data, sort order, and output
namespace. No partial requested variable or public {cmd:e()} result remains.

  {it:return code}{col 18}{it:failure class}
  {hline 76}
    {cmd:r(1)}{col 18}user interruption
    {cmd:r(101)}{col 18}unsupported aweight or iweight
    {cmd:r(110)}{col 18}duplicate output or saved-effect name
    {cmd:r(198)}{col 18}syntax, existing output name, or incompatible options
    {cmd:r(401)}{col 18}invalid frequency-weight domain
    {cmd:r(430)}{col 18}nonconvergence, failed numerical certificate, or unrepresentable prediction type
    {cmd:r(459)}{col 18}invalid outcome, pweight, offset, exposure, or data domain
    {cmd:r(2001)}{col 18}empty/zero-outcome sample, nonpositive DoF, or too few clusters
    {cmd:r(498)}{col 18}recognized but unsupported solver, feature, or FE-dependent prediction
    {cmd:r(499)}{col 18}native protocol mismatch
    {cmd:r(909)}{col 18}allocation or memory preflight failure
    {cmd:r(3499)}{col 18}caught panic or internal invariant failure
  {hline 76}

{pstd}
Prediction without a current {cmd:feppml} result returns {cmd:r(301)}; a
deleted retained prediction source returns {cmd:r(111)}. These failures also
leave the requested target absent.

{marker limitations}
{title:Current limitations}

{pstd}
Fixed-width string categorical sources are supported; {cmd:strL} sources are
not. Categorical-by-continuous and continuous-by-continuous finite RHS
interactions are unsupported. When {cmd:absorb()} contains an interaction or
absorbed slope, explicit RHS base operators such as {cmd:ib2.region} and
{cmd:fvset} base overrides are unavailable in the current final-sample
rebasing path.

{pstd}
The public surface does not include a model-based IID VCE, user-selectable
preconditioner or DoF policy, IV/GMM, group outcomes with individual fixed effects,
HAC or Driscoll--Kraay covariance, other GLMs, GPU execution, or undocumented
developer controls. Unsupported requests fail explicitly.

{marker examples}
{title:Examples}

{pstd}
Each example creates its own synthetic data and wraps the block in
{cmd:preserve}/{cmd:restore}. The clickable link runs the marked executable
block through {cmd:fereg_run}, which independently restores the caller's data.
Every block checks the command identity, convergence, an additional stored
contract, and the fitted-mean prediction on the final sample.

{space 2}{hline 8} {it:Example 1 - Basic PPML and robust inference} {hline 8}
{cmd}{...}
          preserve
{* example_start - ppml_basic}{...}
          clear
          input long obs_id double(y x)
          1 0.5 0
          2 1.0 0
          3 1.5 0
          4 1.0 1
          5 3.0 1
          6 5.0 1
          end
          feppml y c.x, mu(saved_mu)
          assert `"`e(cmd)'"' == "feppml"
          assert e(converged) == 1
          assert `"`e(vce)'"' == "robust"
          assert e(N_physical) == 6
          assert abs(_b[_cons]) <= 1e-7
          assert abs(_b[x] - ln(3)) <= 1e-7
          predict double predicted_mu, mu
          assert reldif(predicted_mu, saved_mu) <= 1e-12 if e(sample)
          assert predicted_mu > 0 if e(sample)
          assert predicted_mu < . if e(sample)
          assert missing(predicted_mu) if !e(sample)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 78}
{space 2}{it:({stata fereg_run ppml_basic using feppml.sthlp:click to run})}

{space 2}{hline 8} {it:Example 2 - Absorbed slopes and requested output} {hline 8}
{cmd}{...}
          preserve
{* example_start - ppml_absorbed_outputs}{...}
          clear
          set obs 144
          generate long obs_id = _n
          generate byte g = 1 + mod(_n - 1, 6)
          generate byte h = 1 + mod(floor((_n - 1) / 3), 8)
          generate double z = 0.35 + mod(5 * _n, 23) / 17
          generate double x = mod(11 * _n, 31) / 15 - 1
          generate double eta_true = 0.18*x + 0.04*(g-3) + ///
              0.025*(g-3)*z - 0.018*(h-4)
          generate double y = exp(eta_true) * (0.88 + 0.03*mod(_n,7))
          feppml y c.x, absorb(fe_g=g##c.z fe_h=h) separation(none) ///
              keepsingletons residuals(saved_response) ///
              residualtype(response) xb(saved_xb) d(saved_d) ///
              eta(saved_eta) mu(saved_mu) dropreason(drop_code)
          assert `"`e(cmd)'"' == "feppml"
          assert e(converged) == 1
          assert e(N_hdfe) == 2 & e(N_hdfe_extended) == 3
          assert e(sample) == 1
          assert abs(saved_d-fe_g-fe_g_Slope1-fe_h) <= 2e-8 if e(sample)
          assert abs(saved_eta-saved_xb-saved_d) <= 2e-8 if e(sample)
          assert reldif(saved_mu,exp(saved_eta)) <= 2e-8 if e(sample)
          assert abs(saved_response-(y-saved_mu)) <= 2e-8 if e(sample)
          assert drop_code == 0 if e(sample)
          predict double predicted_mu, mu
          assert reldif(predicted_mu,saved_mu) <= 2e-8 if e(sample)
          assert predicted_mu > 0 if e(sample)
          assert predicted_mu < . if e(sample)
          assert missing(predicted_mu) if !e(sample)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 78}
{space 2}{it:({stata fereg_run ppml_absorbed_outputs using feppml.sthlp:click to run})}

{space 2}{hline 8} {it:Example 3 - Frequency and probability weights} {hline 8}
{cmd}{...}
          preserve
{* example_start - ppml_weights}{...}
          clear
          input long obs_id double(y x fw pw)
          1  1 0 1 1
          2  4 1 3 2
          3  2 2 2 3
          4  8 3 4 4
          5  7 4 2 2
          6 14 5 3 3
          end
          feppml y c.x [fw=fw], separation(none)
          assert `"`e(cmd)'"' == "feppml"
          assert e(converged) == 1
          assert `"`e(wtype)'"' == "fweight"
          assert e(N_physical) == 6 & e(N_effective) == 15 & e(N) == 15
          feppml y c.x [pw=pw], separation(none) mu(pw_mu)
          assert `"`e(cmd)'"' == "feppml"
          assert e(converged) == 1
          assert `"`e(wtype)'"' == "pweight"
          assert e(N_physical) == 6 & e(N_effective) == 6 & e(N) == 6
          assert reldif(e(sumweights),e(N_physical)) <= 1e-12
          predict double predicted_mu, mu
          assert reldif(predicted_mu,pw_mu) <= 1e-12 if e(sample)
          assert predicted_mu > 0 if e(sample)
          assert predicted_mu < . if e(sample)
          assert missing(predicted_mu) if !e(sample)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 78}
{space 2}{it:({stata fereg_run ppml_weights using feppml.sthlp:click to run})}

{space 2}{hline 8} {it:Example 4 - Multiway and interacted clusters} {hline 8}
{cmd}{...}
          preserve
{* example_start - ppml_clusters}{...}
          clear
          set obs 600
          generate long obs_id = _n
          generate long worker = ceil(_n/6)
          bysort worker: generate byte period = _n
          generate long firm = mod(worker + 7*period,30) + 1
          generate byte state = 1 + mod(firm-1,10)
          generate int year = 2018 + mod(period-1,3)
          generate double x = mod(13*_n,37)/18 - 1
          generate double eta_true = 0.25*x + 0.002*(worker-50) + ///
              0.015*(firm-15)
          generate double y = exp(eta_true) * (0.9 + 0.04*mod(_n,5))
          feppml y c.x, absorb(worker firm) separation(none) ///
              vce(cluster state#year worker) mu(saved_mu)
          assert `"`e(cmd)'"' == "feppml"
          assert e(converged) == 1
          assert `"`e(vce)'"' == "cluster"
          assert e(N_clustervars) == 2
          assert e(N_clust1) > 1 & e(N_clust2) > 1
          assert e(N_clust) == min(e(N_clust1),e(N_clust2))
          predict double predicted_mu, mu
          assert reldif(predicted_mu,saved_mu) <= 1e-12 if e(sample)
          assert predicted_mu > 0 if e(sample)
          assert predicted_mu < . if e(sample)
          assert missing(predicted_mu) if !e(sample)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 78}
{space 2}{it:({stata fereg_run ppml_clusters using feppml.sthlp:click to run})}

{space 2}{hline 8} {it:Example 5 - Certified separation and drop codes} {hline 8}
{cmd}{...}
          preserve
{* example_start - ppml_separation}{...}
          clear
          set obs 24
          generate long obs_id = _n
          generate byte fe = ceil(_n/6)
          generate double x = mod(_n-1,3) - 1
          generate double y = 0.5 + mod(5*_n,7)
          replace y = 0 if fe == 4
          feppml y c.x, absorb(fe) mu(saved_mu) dropreason(drop_code)
          assert `"`e(cmd)'"' == "feppml"
          assert e(converged) == 1
          assert `"`e(separation)'"' == "fe simplex ir"
          assert e(separated) == 6 & e(N_physical) == 18
          assert e(sample) == (fe < 4)
          assert drop_code == 0 if fe < 4
          assert drop_code == 9 if fe == 4
          assert missing(saved_mu) if fe == 4
          predict double predicted_mu, mu
          assert reldif(predicted_mu,saved_mu) <= 1e-12 if e(sample)
          assert predicted_mu > 0 if e(sample)
          assert predicted_mu < . if e(sample)
          assert missing(predicted_mu) if !e(sample)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 78}
{space 2}{it:({stata fereg_run ppml_separation using feppml.sthlp:click to run})}

{space 2}{hline 8} {it:Example 6 - Time-series operators, panel gaps, and if/in} {hline 8}
{cmd}{...}
          preserve
{* example_start - ppml_time_series}{...}
          clear
          set obs 36
          generate long obs_id = _n
          generate byte panel = ceil(_n/12)
          generate byte within_panel = 1 + mod(_n-1,12)
          generate int period = within_panel + (within_panel>=5) + ///
              (within_panel>=9)
          generate double x = 0.3*panel + mod(7*_n,19)/9
          generate double y = 0.6 + mod(11*_n,17)/4
          generate byte candidate = within_panel != 6
          xtset panel period
          generate double manual_lag = L.x
          generate double manual_lead = F.x
          generate double manual_difference = D.x
          generate byte expected_sample = candidate & inrange(obs_id,2,35) & ///
              !missing(y,manual_lag,manual_lead,manual_difference)
          feppml y L.x F.x D.x if candidate in 2/35, separation(none) ///
              mu(saved_mu)
          assert `"`e(cmd)'"' == "feppml"
          assert e(converged) == 1
          assert e(N_physical) > 0
          assert e(sample) == expected_sample
          assert e(sample) if obs_id == 2
          assert e(sample) if within_panel == 7 & inrange(obs_id,2,35)
          predict double predicted_mu, mu
          assert reldif(predicted_mu,saved_mu) <= 1e-12 if e(sample)
          assert predicted_mu > 0 if e(sample)
          assert predicted_mu < . if e(sample)
          assert missing(predicted_mu) if !e(sample)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 78}
{space 2}{it:({stata fereg_run ppml_time_series using feppml.sthlp:click to run})}

{space 2}{hline 8} {it:Example 7 - Separate mean-corrected coefficients and effects} {hline 8}
{cmd}{...}
          preserve
{* example_start - ppml_bias_correction}{...}
          clear
          set obs 480
          generate long worker = ceil(_n/12)
          generate byte g = ceil(worker/10)
          generate byte h = 1 + mod(_n + worker,6)
          generate double x = sin(_n/7)
          generate double y = exp(.2*x+.1*g-.08*h)*(.75+.1*mod(_n,7))
          feppml y x, absorb(g h) biascorrection(randomized) ///
              biasunit(worker) biasprobes(256) biasseed(739391) ///
              biasstream(0) biasfe(bf) mu(saved_mu)
          assert `"`e(cmd)'"' == "feppml"
          assert e(converged) == 1
          assert e(bias_support)[1,2] == 10
          assert e(bias_support)[2,2] == 40
          matrix list e(bias_support)
          assert e(bias_diagnostics)[2,1] == 40
          assert e(bias_diagnostics)[5,1] == 0
          assert reldif(bf_1_bc,bf_1_raw+bf_1_total) < 1e-12
          matrix list e(b_bc)
          predict double predicted_mu, mu
          assert reldif(predicted_mu,saved_mu) <= 1e-12 if e(sample)
          assert predicted_mu > 0 if e(sample)
          assert predicted_mu < . if e(sample)
          assert missing(predicted_mu) if !e(sample)
{* example_end}{...}
          restore
{txt}{...}
{space 2}{hline 78}
{space 2}{it:({stata fereg_run ppml_bias_correction using feppml.sthlp:click to run})}

{marker requirements}
{title:Requirements}

{pstd}
{cmd:feppml} requires Stata 18 or newer and exactly one matching self-contained
package plugin on the ado-path: {cmd:fereg_windows.plugin},
{cmd:fereg_linux.plugin}, or the universal {cmd:fereg_macos.plugin}. Users do
not need Rust, Python, Julia, a compiler, BLAS, or a separate native runtime.

{marker sources}
{title:Sources and acknowledgements}

{pstd}
Development and compatibility validation use the independently maintained
{browse "https://github.com/sergiocorreia/ppmlhdfe":ppmlhdfe},
{browse "https://github.com/sergiocorreia/reghdfe":reghdfe}, and
{browse "https://github.com/py-econometrics/pyfixest":PyFixest} projects as
scientific or behavioral references. They are not called by {cmd:feppml} at
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
Online: {help poisson}, {help glm}, {help fvvarlist}, {help tsvarlist},
{help weights}, {help predict}, {help fereg}
{p_end}


{pstd}
{cmd:e(feppml_memory)} is an 11-by-2 presence/value report for action
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
