{smcl}
{* *! version 0.5.0 24aug2026}{...}
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
is the default. {cmd:xbd} returns the finite prediction plus the absorbed
contribution; {cmd:d} returns only the absorbed contribution; and
{cmd:residuals} returns estimation residuals. {cmd:xbd}, {cmd:d}, and
{cmd:residuals} require a retained {cmd:residuals(newvar)} from estimation.

{pstd}
{cmd:test} uses the posted {cmd:e(b)} and {cmd:e(V)}. {cmd:estat summarize}
summarizes estimation-sample variables through Stata's standard estimator
summary machinery.
