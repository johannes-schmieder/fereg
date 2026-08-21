{smcl}
{* *! version 0.1.0 20aug2026}{...}
{title:Title}

{phang}{bf:stfixest_parse} {hline 2} developer metadata parser prototype

{title:Syntax}

{p 8 15 2}
{cmd:stfixest_parse} [{it:factor-variable varlist}] {ifin},
[{cmd:absorb(}{it:terms}{cmd:)} {cmd:cluster(}{it:terms}{cmd:)}]

{title:Description}

{pstd}
This developer command returns versioned numeric metadata matrices and source
variable names without materializing ordinary factor columns, dummy variables,
or grouped interaction identifiers. It is supplied for licensed-Stata parser
validation and is not an estimation command.

{pstd}
Results include {cmd:r(model_terms)}, {cmd:r(model_components)},
{cmd:r(absorb_terms)}, {cmd:r(absorb_components)}, {cmd:r(cluster_terms)}, and
{cmd:r(cluster_components)} plus counts and canonical-name macros.
