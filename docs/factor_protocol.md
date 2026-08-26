# Model wire 2.1: expanded term graph

## Version and ownership

Model wire 2.1 is independent of action protocol 1.3 and OLS result wire 1.3.
Model wire v1, unknown majors, and newer minors are rejected. Version 2.0 is
accepted only for the compatible diagnostic shape with empty absorbed tables
and registries; it cannot claim absorbed metadata.

Stata owns syntax expansion and supplies canonical factor/omission metadata.
Rust owns observation reads, authoritative sample construction, final
factorization, singleton pruning, compact recipes, estimation, and outputs.
`fereg_parse` remains a diagnostic view of the metadata graph, not a second
estimator.

## Candidate marker and operators

The caller owns the `touse` marker and evaluates `if`/`in` once.
`fvexpand` and `_ms_parse_parts` provide term/component metadata without
materializing dummy columns. Stata time-series operators are preserved
separately from canonical tokens and are materialized against the active
time/panel settings before native binding.

Stata's base/omission flags are authoritative; Rust does not infer them from
token spelling. After final sample construction, vanished factor levels become
canonical all-zero/omitted recipes without changing coefficient-slot identity.

## Tables

All ids and orders are one based. Empty Stata matrices use one all-zero
sentinel row of the required width.

The source table has width 2: source id and reserved zero.

The term table has width 8: term id, term order, coefficient slot, Stata term
kind, component count, whole-column omission flag, and two reserved zeros.

The component table has width 13: parent term, component order, source id,
component kind, level state/value, base status, omission status, time-series
state, raw component/time-series registry slots, and two reserved zeros.

The absorbed-term table has width 4: absorbed-term id, categorical-source
count, intercept flag, and slope-source count. The absorbed-component table has
width 4: parent term, categorical/slope role, order within role, and source id.
Each term has a descriptive label and optional saved-name registry entry.

String registries use byte-length framing, so empty entries remain explicit as
`0:`. Duplicate source references are valid and bind to one shared source.

## Executable 0.5.0 surface

Public estimation consumes continuous terms, factors, ordinary interactions,
numeric/string/interacted absorbed effects, intercept and slope channels,
saved-FE names, and cluster source tuples. Time-series components must be
materialized, not pending. No-regressor models use an explicit empty graph
rather than treating an omitted Stata varlist as every variable in memory.

Decoding rejects malformed framing, nonfinite table values, nonzero reserved
fields, duplicate/gapped ids or orders, orphaned components, count
disagreement, illegal enum/status values, unknown sources/operators, and
unsupported pending work before observation access.
