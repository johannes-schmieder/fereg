{smcl}
{* *! version 0.1.0 20aug2026}{...}
{vieweralsosee "stfeols" "help stfeols"}{...}
{vieweralsosee "stfepois" "help stfepois"}{...}
{title:Title}

{phang}{bf:stfixest} {hline 2} StFixest native-plugin diagnostics

{title:Syntax}

{p 8 15 2}{cmd:stfixest,} {cmd:version}

{p 8 15 2}{cmd:stfixest,} {cmd:doctor} [{cmd:threads(}{it:#}{cmd:)} {cmd:verbose}]

{p 8 15 2}{cmd:stfixest,} {cmd:selftest} [{cmd:threads(}{it:#}{cmd:)}]

{title:Description}

{pstd}
This Phase 0-1 package validates the Stata 18+ loader, Stata Plugin Interface
3.0 bridge, action handshake 1.0, model wire 2.0, panic boundary, preprocessing primitives,
and native thread machinery. It does not contain an estimator.

{pstd}
{cmd:threads(0)} selects the native default. A positive value requests a local
native thread team; Stata SPI calls remain on Stata's calling thread.

{title:Stored results}

{pstd}{cmd:stfixest} stores in {cmd:r()} the ado version,
{cmd:r(action_protocol)}, {cmd:r(model_wire)}, resolved plugin name/path, and
requested thread count. Native diagnostics are printed as unambiguously
labeled machine-readable {cmd:stfixest.*=...} lines.

{title:Requirements}

{pstd}Stata 18 or newer and exactly one platform plugin on the ado-path:
{cmd:stfixest_windows.plugin}, {cmd:stfixest_linux.plugin}, or the universal
{cmd:stfixest_macos.plugin}.
