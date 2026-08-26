{smcl}
{* *! version 0.5.0 26aug2026}{...}
{vieweralsosee "fereg" "help fereg"}{...}
{title:Title}

{phang}{bf:fereg postestimation} {hline 2} prediction, tests, and summaries

{title:Syntax}

{p 8 15 2}{cmd:predict} {it:newvar} [{ifin}] [{cmd:, xb}]

{p 8 15 2}{cmd:predict} {it:newvar} [{ifin}],
{cmd:xbd}|{cmd:d}|{cmd:residuals}

{p 8 15 2}{cmd:test} {it:hypothesis}

{p 8 15 2}{cmd:estat summarize} [{it:varlist}]

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
summary machinery.

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
