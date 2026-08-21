# Model wire v2: expanded term graph

## Version and scope

The model wire is `2.0`. It is independent of the plugin action handshake,
which remains `1.0`. Model wire v1 is rejected; there is no lossy adapter.
Unknown major versions and newer minor versions are also rejected.

Version 2 carries a data-independent graph of coefficient columns already
expanded by Stata. It does not read observations, bind SPI varlist positions,
choose a sample, select factor bases, insert an intercept, construct
`ColumnRecipe`s, or enable estimator dispatch. Those remain later gates.

## Candidate marker ownership

`stfixest_parse` requires `touse(varname)`. The caller creates the marker and
owns its lifetime; the parser never returns a `tempvar` that disappears when
the parser returns. The parser does not also accept `if`/`in`; the caller must
evaluate those restrictions into the marker exactly once. Estimation commands
already create their own candidate markers and remain fail-closed with
`r(498)`.

The marker determines the level universe passed to `fvexpand`. Rust will be
authoritative for all later missingness, weight, singleton, and separation
restrictions once the data path exists.

## Numeric tables and registries

All identifiers and orders are explicitly one-based. A `SourceId` indexes the
source registry, not the SPI varlist. Conversion to a zero-based Rust index is
permitted only after complete cardinality and range validation.

The source table has width 2:

1. source id;
2. reserved zero.

The term table has width 8:

1. term id;
2. term order;
3. coefficient slot;
4. raw Stata term kind (`1` variable, `2` factor, `3` generic interaction,
   `4` product, `5` explicitly unavailable);
5. component count;
6. whole-column omission flag;
7. reserved zero;
8. reserved zero.

Raw kind `3` is deliberately not reclassified as categorical-by-categorical.
The ordered components carry the actual shape.

The component table has width 13:

1. parent term id;
2. component order within the term;
3. source id;
4. kind (`1` continuous, `2` categorical, `3` unavailable);
5. level state (`1` absent, `2` present, `3` unavailable);
6. finite level value, or zero when absent/unavailable;
7. base status (`0` false, `1` true, `2` unavailable);
8. component omission status using the same status codes;
9. time-series state (`1` none, `2` pending Stata materialization,
   `3` materialized, `4` unavailable);
10. raw component-operator registry slot;
11. raw time-series-operator registry slot;
12. reserved zero;
13. reserved zero.

The ordered string registries contain source names, canonical expanded tokens,
raw component operators, and raw time-series operators. Their cardinalities
must exactly match the corresponding table. The ado return locals encode raw
operator entries as `<byte-length>:<operator>` tokens, so an empty operator is
represented unambiguously as `0:`. The eventual bridge decoder must remove
this transport framing without changing the operator text.

Canonical token and raw component operator are separate. For example, local
Stata 18 expands `ibn.g` to a token such as `1bn.g`, while `_ms_parse_parts`
reports raw operator `1`; both facts survive the wire. Component base and
omission flags come only from Stata metadata. They are never inferred from
token spelling.

## Empty-table normalization

Stata matrices cannot have zero rows. A declared count of zero therefore
requires exactly one all-zero sentinel row of the documented width. For a
positive declared count, the row count must match exactly and sentinel rows
are forbidden. Missing values, nonfinite values, negative/fractional counts,
wrong widths, and any nonzero reserved field are rejected.

Because Stata's optional `varlist` syntax expands an omitted varlist to the
variables in memory, a no-regressor graph is requested explicitly with the
developer-only `emptymodel` parser option. It produces zero declared term,
component, and source counts plus the required all-zero sentinel rows; it does
not guess that an omitted varlist means an empty model.

## Fail-closed checks

Decoding rejects duplicate or gapped ids/orders/slots, orphaned or misordered
components, component-count disagreement, out-of-range sources, absent or
mismatched registries, illegal level states, unknown enum/status codes, and
unknown raw operators. Explicit `Unavailable` values survive decoding so a
capture can remain lossless, but the bindable/executable validation stages
reject them. Pending time-series work is also rejected by executable metadata
validation.

## Companion metadata

The current ado parser still returns absorbed-term and cluster tuple tables for
capture and review. They are not executable model-wire compilers and do not
authorize FE construction, factorization, sample mutation, or estimator use.

Model wire v2 is a Phase 2 hardening artifact only. The Phase 2 exit gate has
not passed.
