{smcl}
{* *! version 0.1.0 20aug2026}{...}
{vieweralsosee "stfixest" "help stfixest"}{...}
{title:Title}

{phang}{bf:stfeols} {hline 2} planned Rust-first fixed-effect OLS/WLS command

{title:Phase status}

{pstd}
The command name and preliminary syntax are installed, but estimation is
intentionally disabled in version 0.1.0 Phase 0-1. Every invocation returns
{cmd:r(498)} without posting {cmd:e()} results. This prevents an architectural
prototype from being mistaken for a validated estimator.

{title:Planned syntax}

{p 8 15 2}
{cmd:stfeols} {it:depvar indepvars} {ifin} {weight},
[{cmd:absorb(}{it:terms}{cmd:)} {cmd:vce(}{it:vcetype}{cmd:)}
{cmd:threads(}{it:#}{cmd:)} {cmd:memory(}{it:mode}{cmd:)}
{cmd:poolsize(}{it:#}{cmd:)} {cmd:tolerance(}{it:#}{cmd:)}
{cmd:iterate(}{it:#}{cmd:)} {cmd:deterministic}]

{pstd}
Factor-variable and time-series syntax is accepted by the preliminary parser.
No coefficient, variance, degrees-of-freedom, singleton, fixed-effect, or
postestimation behavior is claimed yet.
