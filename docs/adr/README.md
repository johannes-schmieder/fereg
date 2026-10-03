# Active architecture decisions

Only the decisions listed as active here bind future work. All historical
`docs/adr/0001-*` through `0246-*` documents are non-binding history, including
the former one-plugin requirement. Requirements already incorporated into
SPEC remain binding through SPEC, not through historical implementation rules.
Do not resurrect "retain prior policy" instructions from historical documents.

## Active successors

- [0247](0247-incremental-refactoring.md): working copy and frozen comparator.
- [0248](0248-refactoring-acceptance.md): scientific/change checks, gates and ratchet.
- Rebuild 0003, 0008 and 0009 remain active only as amended/indexed below.

## Staged rebuild dispositions

| ADR | Status | Binding disposition |
|---|---|---|
| 0001 | Superseded by 0247 | Abandon rewrite and scheduled M7 retirement. |
| 0002 | Deferred | Interface consolidation deferred; proposed Schur default withdrawn; supported LSMR/CMG retained. |
| 0003 | Accepted, amended by 0248 | SPEC and independent accuracy bind; comparator differences may be scientifically justified. |
| 0004 | Deferred | Keep current memory modes/options in M0–M1. |
| 0005 | Superseded by 0248 | Fixed-configuration determinism; scientific agreement across thread counts. |
| 0006 | Deferred | No option-removal campaign; unsafe CG remains rejected. |
| 0007 | Superseded by 0248 | Replay, bounded evidence and prerequisite gates. |
| 0008 | Accepted, amended by 0247 | Licensing direction/gate retained; abandoned-engine pins and M7 dependency schedule withdrawn. |
| 0009 | Accepted, amended by 0247 | Factor sample precedence implemented in next; frozen plugin unchanged. |
| 0010 | Superseded by 0248 | Complexity ratchet; no aggregate line ceilings. |
| 0011 | Superseded by 0247 | Rewrite-specific callback APIs abandoned; existing interruption guarantees retained. |

Staged texts live in [rebuild-adr](../cleanup/rebuild-adr/README.md). Deferred
items require a separately scoped decision; they do not automatically resume.
ADRs are one page maximum, for durable decisions rather than experiments.
