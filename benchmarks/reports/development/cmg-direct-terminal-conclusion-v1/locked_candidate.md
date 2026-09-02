# Locked direct-terminal routing candidates

Locked before running the fresh holdout. This is development evidence, not
qualification.

The current ADR-0021/0022 eligibility and iterative-terminal guard remain the
first stages. These candidates apply only to auto-eligible CMG hierarchies
whose terminal reason is `Direct`.

## Primary candidate

Route to direct CG when the earlier direct-terminal cost score rejects CMG,
and both conditions below hold:

1. hierarchy depth-work `levels * recursive_work / N` exceeds 200, or the
   nuisance block has at least nine non-base columns; and
2. execution is serial, or all right-hand sides fit in one thread batch
   (`threads == 1 || rhs <= threads`).

Otherwise retain CMG. Calibration selected CG in 20 of 61 cost-rejected
cells. It displaced no CMG win above five percent. Aggregate command time fell
from 30.994 to 23.192 seconds (1.336x), with no solver fallback.

The depth threshold lies in the measured calibration gap from 101.9 to 477.9.
The nuisance split distinguishes four from nine non-base columns. The batch
condition preserves the repeated-RHS CMG wins observed when 17 right-hand
sides run on eight threads.

## Secondary extended candidate

In addition to the primary candidate, route to CG for a serial 17-RHS cell
when recursive hierarchy work exceeds 500,000 and the direct terminal factor
is absent or has at least 100 columns. This selected CG in 25 of 61 cells,
displaced no CMG win above five percent, and improved aggregate time by 1.405x.

This clause is less attractive because it combines three thresholds and may
be overfit. It will be measured on the same holdout but is not the preferred
rule unless it generalizes materially better than the primary candidate.

Schwarz-LSMR is not a candidate destination. At `1e-10`, two columns in a
representative 17-column solve missed fereg's certificate by about 9--11
percent. `1e-11` fixed that case with two extra iterations, but a different
ordinary five-RHS, one-thread calibration cell then exceeded the bounded run
time. Tightening the tolerance therefore moves rather than removes the cliff.
