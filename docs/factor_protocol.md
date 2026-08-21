# Versioned factor and term protocol

## Version

Protocol `1.0` is a Phase 0–1 metadata contract. A protocol-major mismatch is
fatal, and a protocol minor newer than the plugin is fatal. During the pre-1.0
architecture phase, the ado contract string must match exactly; compatibility
is not inferred from the leading zero in semantic versions.

## Variable table

Every Stata-visible source or output has a one-based plugin position and a
numeric role code. Current codes are defined by `VariableRole` in
`stfixest-core/src/metadata.rs`, including candidate marker, dependent variable,
continuous/categorical sources, weights, offset/exposure, FE and cluster
sources, and future output buffers.

## Ordinary term table

One row represents one expanded coefficient column and records:

1. term order;
2. operator type;
3. number of components;
4. omitted flag;
5. any-base flag;
6. coefficient column;
7. time-series-pending flag (Phase 0 marks a Stata TS operator but does not materialize it);
8. reserved field.

A component table records term/component order, source position, continuous or
categorical kind, numeric level, base/omitted flags, and time-series-pending flag. Stata’s documented `_ms_parse_parts` scalars `r(base)`/`r(level)` and their component-indexed forms are used rather than reverse-engineering the operator text. Stata
retains the canonical coefficient name separately for exact posting.

## Absorbed terms

An absorbed-term row records categorical arity, intercept inclusion, and slope
count. Component rows distinguish categorical sources from slope sources. The
prototype represents:

- `firm`
- `firm#year`
- `firm##c.time`
- `firm#c.time`
- `firm##c.(time tenure)`

Categorical `##` expansion inside `absorb()` is deliberately rejected by the
prototype rather than guessed.

## Cluster terms

Cluster interactions are tuples of original source codes, never concatenated
strings. `firm#year` therefore remains collision-safe even when either source
is a string variable.

## No observation-scale expansion

The Stata parser calls `fvexpand` and `_ms_parse_parts` to obtain metadata but
never calls `fvrevar`, `egen group()`, or dummy-generation commands. Complex
time-series materialization is reserved for a narrowly documented later
exception; protocol 1.0 only marks such terms.
