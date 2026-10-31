{smcl}
{* *! version 0.5.0 31oct2026}{...}
{vieweralsosee "fereg" "help fereg"}{...}
{title:Title}

{phang}{bf:fereg postestimation} {hline 2} prediction, tests, and summaries

{title:Syntax}

{p 8 15 2}{cmd:predict} {it:newvar} [{ifin}] [{cmd:, xb}]

{p 8 15 2}{cmd:predict} {it:newvar} [{ifin}],
{cmd:xbd}|{cmd:d}|{cmd:residuals}

{p 8 15 2}{cmd:test} {it:hypothesis}

{p 8 15 2}{cmd:estat summarize} [{it:varlist}]

{p 8 15 2}{cmd:fereg} [{cmd:,} {cmd:level(}{it:#}{cmd:)} {it:display_options}]

{title:Description}

{pstd}
{cmd:predict, xb} returns the reported finite-regressor linear prediction and
is the default. It is defined on requested observations that also belong to
{cmd:e(sample)}. {cmd:xbd} returns the finite prediction plus the absorbed
contribution, {cmd:d} returns the absorbed contribution alone, and
{cmd:residuals} returns the estimation residual.

{pstd}
{cmd:xbd}, {cmd:d}, and {cmd:residuals} require
{cmd:residuals(}{it:newvar}{cmd:)} on the estimation command. They return
{cmd:r(498)} if those residuals were not saved, and {cmd:r(111)} if the saved
residual variable has since been removed. Only one prediction statistic may be
specified.

{pstd}
{cmd:test} uses the posted {cmd:e(b)} and {cmd:e(V)}. {cmd:estat summarize}
summarizes estimation-sample variables through Stata's standard estimator
summary machinery. {cmd:fereg} without a variable list redisplays the last
results, which also serves {cmd:estimates replay}, {cmd:bootstrap:} and
{cmd:jackknife:}.

{title:Solver diagnostics}

{pstd}
{cmd:e(fereg_solver_used)} names the absorption method and
{cmd:e(fereg_solver_residual)} names the method of the final residual
projection. The output line above the coefficient table states that the
accuracy check passed and at which tolerance.
{cmd:e(fereg_solver_fallback)} records whether either phase used a fallback.
Use {cmd:matrix list e(fereg_solver)} for the two named rows, iteration counts,
fallback indicators and certificates. On the automatic two-effect path,
{cmd:cg} is Schur PCG and {cmd:cmgschur} is CMG-preconditioned Schur PCG.
See {help fereg##stored:Stored results} for the numeric codes and Low-block
aggregation. These diagnostics do not require saving residuals.

{title:Example}

{phang2}{cmd:. fereg y x, absorb(worker firm) residuals(resid)}{p_end}
{phang2}{cmd:. predict double xb, xb}{p_end}
{phang2}{cmd:. predict double fitted, xbd}{p_end}
{phang2}{cmd:. predict double absorbed, d}{p_end}
{phang2}{cmd:. predict double resid_check, residuals}{p_end}
{phang2}{cmd:. test x}{p_end}
{phang2}{cmd:. estat summarize y x}{p_end}

{title:Also see}

{p 0 24}
Online: {help fereg}, {help predict}, {help test}, {help estat summarize}
{p_end}
