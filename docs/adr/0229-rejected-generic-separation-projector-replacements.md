# ADR 0229: Reject generic separation projector substitutions

Date: 2026-09-27. Status: rejected experiments; no production routing change.

The [source-bound repair report](../../benchmarks/reports/development/separation-graph-repair-local-2026-09-27/README.md)
proves exact FE-only separation beyond zero-only FE levels on two original
SCC failure inputs: 81 additional rows in two FE and 70 in three FE.

A memory-admitted retained-CG replacement gets past the initial weighted MAP
failure but misses those rows. The final PPML certificates then prevent
publication. An exact graph starting proposal correctly identifies recession
directions and passes exhaustive independent small-graph checks. With the old
MAP projector it still fails on the large originals. Combining that graph
proposal with the generic retained-CG projector fails a separate explicit
Poisson/Hessian fixture at the finite-score certificate.

Do not adopt any of these substitutions or relax their failed certificates.
All production edits were removed; implementations and failed evidence remain
in the report. The existing separation method order, scientific criteria,
publication policy, memory routes and samples remain unchanged.

A successor needs two distinct authorities: a verified representation of a
proposed direction in the model span, and a reliable current-weight projection
for the remaining finite-plus-FE problem. For an explicitly constructed FE
potential, independently reconstructing from its coefficients may establish
span membership without another ill-conditioned weighted fit; that design still
needs its own validated private certificate boundary. A two-FE submodel direction
can certify deletions in larger models but cannot certify their absence of
separation. Full-model terminal checks remain mandatory.

This decision closes the bounded generic-CG/graph substitution experiment. It
does not establish that graph-assisted separation or coefficient-space Krylov
solves cannot work, and it does not repair the separate constrained-CG problems.
