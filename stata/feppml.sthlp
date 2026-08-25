{smcl}
{* *! version 0.9.0 24aug2026}{...}
{vieweralsosee "fereg" "help fereg"}{...}
{title:Title}

{phang}{bf:feppml} {hline 2} planned Rust-first fixed-effect PPML command

{title:Release-candidate status}

{pstd}
The command name and preliminary syntax are installed, but PPML is outside the
0.9.0 OLS release-candidate scope. Every invocation returns {cmd:r(498)}
without posting {cmd:e()} results. No partial or uncertified PPML estimate is
published.

{title:Planned syntax}

{p 8 15 2}
{cmd:feppml} {it:depvar indepvars} {ifin} {weight},
[{cmd:absorb(}{it:terms}{cmd:)} {cmd:vce(}{it:vcetype}{cmd:)}
{cmd:offset(}{it:varname}{cmd:)} | {cmd:exposure(}{it:varname}{cmd:)}
{cmd:separation(}{it:methods}{cmd:)} {cmd:threads(}{it:#}{cmd:)}
{cmd:memory(}{it:mode}{cmd:)} {cmd:poolsize(}{it:#}{cmd:)}]
