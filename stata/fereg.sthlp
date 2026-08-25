{smcl}
{* *! version 0.9.0 24aug2026}{...}
{vieweralsosee "fereg" "help fereg"}{...}
{vieweralsosee "feppml" "help feppml"}{...}
{title:Title}

{phang}{bf:fereg} {hline 2} high-dimensional fixed-effect OLS/WLS

{title:Syntax}

{p 8 15 2}{cmd:fereg,} {cmd:version}

{p 8 15 2}{cmd:fereg,} {cmd:doctor} [{cmd:threads(}{it:#}{cmd:)} {cmd:verbose}]

{p 8 15 2}{cmd:fereg,} {cmd:selftest} [{cmd:threads(}{it:#}{cmd:)}]

{p 8 15 2}
{cmd:fereg} {it:depvar} [{it:indepvars}] {ifin} {weight},
[{cmd:absorb(}{it:terms}{cmd:)} {cmd:vce(}{it:vcetype}{cmd:)}
{cmd:dof(}{it:method}{cmd:)} {cmd:keepsingletons}
{cmd:residuals(}{it:newvar}{cmd:)} {cmd:noconstant}
{cmd:solver(}{it:method}{cmd:)} {cmd:preconditioner(}{it:method}{cmd:)}
{cmd:threads(}{it:#}{cmd:)} {cmd:memory(}{it:mode}{cmd:)}
{cmd:poolsize(}{it:#}{cmd:)} {cmd:tolerance(}{it:#}{cmd:)}
{cmd:iterate(}{it:#}{cmd:)} {cmd:deterministic} {cmd:timeit}
{cmd:verbose(}{it:#}{cmd:)}]

{title:Description}

{pstd}
{cmd:fereg} fits linear models after absorbing numeric or fixed-width string
fixed effects, categorical FE interactions, and group-specific slopes. It
supports no-regressor and disconnected models, singleton deletion or
{cmd:keepsingletons}, {cmd:if}/{cmd:in}, Stata time-series operators, analytic,
frequency, and probability weights, and iid, robust, or multiway-clustered
inference. Importance weights are rejected with {cmd:r(101)} to match the
pinned reference contract.

{pstd}
Absorbed terms use reghdfe-style notation. Examples include
{cmd:absorb(firm worker)}, {cmd:absorb(firm#year)},
{cmd:absorb(firm##c.time)}, and {cmd:absorb(fe=firm)}. A name on the left of
{cmd:=} requests a normalized saved FE; slope channels use suffixed names.

{pstd}
{cmd:vce()} accepts {cmd:unadjusted} (also {cmd:ols} or {cmd:iid}),
{cmd:robust}, or {cmd:cluster} followed by one or more cluster dimensions.
{cmd:dof()} accepts {cmd:pairwise} (default), {cmd:firstpair}, {cmd:none}, or
{cmd:continuous}.

{pstd}
{cmd:memory(auto)} preflights both execution plans and chooses a safe mode.
{cmd:memory(fast)} retains dense native workspaces; {cmd:memory(low)} uses
blockwise processing and bounded temporary scratch files that are removed on
success, error, or Break. {cmd:poolsize()} controls the low-memory block width.

{pstd}
{cmd:solver(auto)} is the default certified fast path. Explicit methods are
{cmd:map}, modified {cmd:lsmr}, conjugate-gradient accelerated MAP {cmd:cg},
Irons--Tuck accelerated MAP {cmd:it}, and {cmd:squarem}. LSMR accepts
{cmd:preconditioner(auto)}, {cmd:off}, {cmd:diagonal}, or {cmd:schwarz};
{cmd:none} and {cmd:additive} remain aliases for {cmd:off} and {cmd:schwarz}.
Accelerated candidates are retained only when the independent FE-moment
certificate improves on ordinary symmetric MAP sweeps.

{pstd}
{cmd:threads(0)} selects the native default. A positive value requests a local
native thread team for that estimation call. {cmd:deterministic} fixes native
work partition and merge order. Stata SPI calls remain on Stata's calling
thread. {cmd:timeit} prints native timing, memory, scratch, allocation, RSS,
and effective-thread diagnostics.

{title:Prediction and postestimation}

{pstd}
After estimation, {cmd:predict} supports {cmd:xb} (the default), {cmd:xbd}
(linear prediction plus absorbed contribution), {cmd:d} (absorbed
contribution), and {cmd:residuals}. The last three require
{cmd:residuals(newvar)} at estimation time. Standard {cmd:test} works from the
posted coefficient and covariance matrices. {cmd:estat summarize} is
supported.

{title:Stored results}

{pstd}
Estimation stores the usual {cmd:e(b)}, {cmd:e(V)}, {cmd:e(sample)}, sample,
rank, DoF, fit, VCE, cluster, singleton, and absorbed-FE metadata. Additional
{cmd:e(fereg_solver)} stores the numeric solver-certificate matrix, while
{cmd:e(fereg_solver_requested)} records the requested solver mode. Other
{cmd:e(fereg_*)} scalars and matrices record requested and effective numerical
policies, memory-plan estimates, observed native
allocation, scratch use, RSS, and timing. Diagnostic invocations store loader
and protocol information in {cmd:r()}.

{title:Requirements}

{pstd}Stata 18 or newer and exactly one platform plugin on the ado-path:
{cmd:fereg_windows.plugin}, {cmd:fereg_linux.plugin}, or the universal
{cmd:fereg_macos.plugin}. This is the untagged 0.9.0 release-candidate source;
see {cmd:STATUS.md} in the source distribution for qualified-platform receipts
and deferred native-Intel/Windows licensed-Stata caveats.
