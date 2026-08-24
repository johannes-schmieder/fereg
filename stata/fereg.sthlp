{smcl}
{* *! version 0.1.0 20aug2026}{...}
{vieweralsosee "fereg" "help fereg"}{...}
{vieweralsosee "feppml" "help feppml"}{...}
{title:Title}

{phang}{bf:fereg} {hline 2} fixed-effect OLS/WLS and native-plugin diagnostics

{title:Syntax}

{p 8 15 2}{cmd:fereg,} {cmd:version}

{p 8 15 2}{cmd:fereg,} {cmd:doctor} [{cmd:threads(}{it:#}{cmd:)} {cmd:verbose}]

{p 8 15 2}{cmd:fereg,} {cmd:selftest} [{cmd:threads(}{it:#}{cmd:)}]

{p 8 15 2}
{cmd:fereg} {it:depvar indepvars} {ifin} {weight},
[{cmd:absorb(}{it:terms}{cmd:)} {cmd:vce(}{it:vcetype}{cmd:)}
{cmd:threads(}{it:#}{cmd:)} {cmd:memory(}{it:mode}{cmd:)}
{cmd:poolsize(}{it:#}{cmd:)} {cmd:tolerance(}{it:#}{cmd:)}
{cmd:iterate(}{it:#}{cmd:)} {cmd:deterministic}]

{title:Description}

{pstd}
This Phase 0-1 package validates the Stata 18+ loader, Stata Plugin Interface
3.0 bridge, action handshake 1.0, model wire 2.1, panic boundary, preprocessing primitives,
and native thread machinery. The public OLS/WLS interface is installed but
intentionally returns {cmd:r(498)} without posting {cmd:e()} results until its
scientific activation gates are complete.

{pstd}
{cmd:threads(0)} selects the native default. A positive value requests a local
native thread team; Stata SPI calls remain on Stata's calling thread.

{title:Stored results}

{pstd}{cmd:fereg} stores in {cmd:r()} the ado version,
{cmd:r(action_protocol)}, {cmd:r(model_wire)}, resolved plugin name/path, and
requested thread count. Native diagnostics are printed as unambiguously
labeled machine-readable {cmd:fereg.*=...} lines.

{title:Requirements}

{pstd}Stata 18 or newer and exactly one platform plugin on the ado-path:
{cmd:fereg_windows.plugin}, {cmd:fereg_linux.plugin}, or the universal
{cmd:fereg_macos.plugin}.
