{smcl}
{* *! version 0.9.0 24aug2026}{...}
{title:Title}

{phang}{bf:fereg_parse} {hline 2} developer model-wire v2 parser

{title:Syntax}

{p 8 15 2}
{cmd:fereg_parse} [{it:factor-variable varlist}],
{cmd:touse(}{it:varname}{cmd:)}
[{cmd:absorb(}{it:terms}{cmd:)} {cmd:cluster(}{it:terms}{cmd:)}
{cmd:emptymodel}]

{title:Description}

{pstd}
This developer command returns versioned numeric metadata matrices and source
variable names without materializing ordinary factor columns, dummy variables,
or grouped interaction identifiers. It is supplied for licensed-Stata parser
validation and is not an estimation command.

{pstd}
The caller owns {cmd:touse()} and must evaluate any {cmd:if} or {cmd:in}
restriction before calling this parser. {cmd:emptymodel} explicitly requests
zero ordinary terms and the documented all-zero sentinel matrices.

{pstd}
Results include {cmd:r(model_sources)}, {cmd:r(model_terms)}, {cmd:r(model_components)},
{cmd:r(absorb_terms)}, {cmd:r(absorb_components)}, {cmd:r(cluster_terms)}, and
{cmd:r(cluster_components)} plus exact counts, canonical-token macros, and raw
component/time-series operator registries. The returned model wire version is
{cmd:r(model_wire_major)}.{cmd:r(model_wire_minor)}.

{pstd}
Model wire 2.1 adds length-framed absorbed-term labels and saved-name entries;
an absent saved name is represented unambiguously as {cmd:0:}. Public
estimation consumes this metadata through {cmd:fereg}; direct parser use is
intended only for diagnostics and qualification.
