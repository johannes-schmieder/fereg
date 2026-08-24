{smcl}
{* *! version 0.1.0 20aug2026}{...}
{vieweralsosee "fereg" "help fereg"}{...}
{title:Title}

{phang}{bf:feppml} {hline 2} planned Rust-first fixed-effect PPML command

{title:Phase status}

{pstd}
The command name and preliminary syntax are installed, but PPML estimation is
intentionally disabled in version 0.1.0 Phase 0-1. Every invocation returns
{cmd:r(498)} without posting {cmd:e()} results. IRLS, separation, VCE, fixed
effects, prediction, and postestimation remain behind later scientific gates.

{title:Planned syntax}

{p 8 15 2}
{cmd:feppml} {it:depvar indepvars} {ifin} {weight},
[{cmd:absorb(}{it:terms}{cmd:)} {cmd:vce(}{it:vcetype}{cmd:)}
{cmd:offset(}{it:varname}{cmd:)} | {cmd:exposure(}{it:varname}{cmd:)}
{cmd:separation(}{it:methods}{cmd:)} {cmd:threads(}{it:#}{cmd:)}
{cmd:memory(}{it:mode}{cmd:)} {cmd:poolsize(}{it:#}{cmd:)}]
