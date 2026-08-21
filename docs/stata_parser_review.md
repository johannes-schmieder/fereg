# Licensed-Stata parser review checklist

The parser is intentionally a prototype because `_ms_parse_parts` and
factor/time-series behavior must be observed in the actual Stata 18+ runtime.

For every fixture, record `r(source_vars)`, `r(coefficient_names)`, all six
numeric matrices, and the observation count before and after parsing. Confirm:

- base and omitted flags match Stata's expanded tokens;
- source positions are stable and one-based;
- component level values are correct for explicit and default bases;
- `#` versus `##` semantics are not conflated;
- string variables work in absorb and cluster metadata;
- grouped slopes remain one absorb term;
- `if/in` changes factor levels through the candidate marker as intended;
- no observation-scale helper columns are created;
- time-series terms are marked as pending materialization, not silently transformed; the pending flag must agree with `_ms_parse_parts` `r(ts_op)`/`r(ts_op#)`.

Any ambiguity must cause a syntax error or protocol revision, never a guessed
mapping.
