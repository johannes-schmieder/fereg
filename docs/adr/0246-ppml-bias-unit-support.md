# ADR 0246: independent-unit support for PPML mean correction

**Status:** Accepted
**Date:** 2026-09-28
**Decision owner:** Johannes Schmieder

Johannes approved the proposed documentation, native diagnostics, and default
refusal of a requested correction when any retained FE level belongs to only
one bias unit. This narrows the supported correction surface of ADR 0227;
ordinary PPML estimation is unchanged.

`biasunit()` defines independent score units. It is not a deletion unit and
need not nest within either FE set. An individual FE's score sums to zero at
the fitted solution. If its entire support lies in one unit, the corresponding
empirical score variation cannot generally capture its estimation noise.

Both correction methods therefore require at least two distinct units within
every retained level of both FE sets, including when `biasfe()` is omitted.
Rust checks actual membership on the final retained sample before correction
solves. It reports counts of affected levels in the error and fails the entire
requested transaction. Relabelling IDs, frequency copies, more probes, and
more rows within a unit cannot bypass the restriction. Units must still follow
the sampling assumptions; the command never splits them automatically.

A separate `e(bias_support)` matrix reports per-effect support distributions
and fitted-information concentration. The 21 existing diagnostic rows retain
their order. PPML request 1.8/result 1.5 bind the new mandatory correction output;
ordinary action and OLS wires remain unchanged. Validators and Stata 18 compiled
helpers move together, and stale combinations fail closed.

This is a conservative supported-scope restriction, not a theorem that every
slope correction fails whenever a level has only one unit. Passing it is not a
statistical certificate. No arbitrary threshold for small counts or information
dominance is imposed. Graph bottlenecks and unit-dependent contrasts can remain
after this screen; stronger graph/rank/leverage tests are outside this change.
