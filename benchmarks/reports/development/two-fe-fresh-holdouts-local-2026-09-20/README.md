# Two independent fresh-seed holdout blocks

Local development on September 20, 2026, M2 Ultra, Stata/MP 19.0 with eight
licensed processors and sixteen requested native/R workers. These measurements
use the predeclared sixteen seed-104729 holdouts. They are separate from the
77 primary cells and do not complete that matrix or release qualification.

The unchanged `usefulpilot` artifact is bound to production `cdfbcf58`, plugin
`79141637b5252f27d6f86a721fb04cf5648df6b8d0d82ee26ea7e39b9b94de14`.
All 516 source files and the measured binary match. The embedded build date
is September 19. Both blocks use the pinned stock fixest library and default
reference tolerances, audited shared inputs, fresh processes, small warmups
and alternating arm order. Each block has five pairs per cell. The fresh
controller is source-manifest checked and contains no precision-clock patch.

## Timing

All sixteen cell medians are faster than fixest in both blocks. Worst median
paired ratios are 0.952191 and 0.947368. There are 158 strict per-pair wins
out of 160; both individual losses remain in the score receipts. Four cells
do not meet the aspirational 0.90 margin in either block: shuffled unit K=1
and K=5, shuffled AW K=6 IID, and degree-six AW K=6. They meet the declared
1.0 acceptance threshold.

| Holdout | K | Weight | Block A ratio | Block B ratio |
|---|---:|---|---:|---:|
| Shuffled dense IID | 1 | Unit | 0.9119 | 0.9091 |
| Shuffled dense IID | 5 | Unit | 0.9522 | 0.9474 |
| Shuffled dense IID | 50 | Unit | 0.6229 | 0.6314 |
| Shuffled dense IID | 6 | AW | 0.9097 | 0.9189 |
| Shuffled dense full outputs | 6 | AW | 0.7691 | 0.7712 |
| Shuffled dense IID | 50 | AW | 0.6002 | 0.5879 |
| Shuffled dense full outputs | 50 | AW | 0.6310 | 0.6344 |
| Degree three IID | 6 | Unit | 0.8219 | 0.8333 |
| Degree three IID | 6 | AW | 0.8875 | 0.8452 |
| Degree six IID | 6 | Unit | 0.8182 | 0.7971 |
| Degree six IID | 6 | AW | 0.9155 | 0.9155 |
| Unbalanced IID | 6 | Unit | 0.8052 | 0.8026 |
| Unbalanced IID | 6 | AW | 0.8625 | 0.8889 |
| Weak degree three IID | 6 | AW | 0.8309 | 0.8267 |
| Bottleneck ring IID | 6 | AW | 0.0572 | 0.0572 |
| Cycle IID | 6 | AW | 0.0586 | 0.0582 |

These are medians of paired ratios, not ratios of separate medians. Default
fixest does not meet the registered accuracy envelope for the final two rows;
their timings are default-reference speed comparisons, not matched-accuracy
parity claims. Default saved-output discrepancies also remain explicit.

## Scientific evidence

All 320 application calls succeed with verified return codes and terminal
sentinels. The sixteen native cells each preserve all six scientific bundles,
sample identities and any requested output vectors bitwise across their ten
repetitions. Independent output audits pass on all twenty native saved-output
runs. Every requested FE channel is checked for constancy, component
normalization, weighted normal equations, RSS and reconstruction.

The independently implemented grounded sparse-direct solver checks all nine
graph/weight combinations in each block: eighteen native checks pass, with
maximum coefficient error below 2.9e-15. The two default-reference graph
mismatches in each block are independently reproduced: maximum coefficient
errors are approximately 2.83e-6 for the bottleneck ring and 1.58e-5 for the
cycle. The weak degree-three reference passes. No tolerance was weakened and
no disagreement was relabeled a pairwise scientific pass.

`holdout-audit.json` joins the two immutable plans, per-cell ratios, exact
repeat checks and independent checks. Each block's score preserves every
pair, sample/input identity, source/reference identity and native allocation
peak. The generated workers and their hashes are retained. Allocation peaks
and process RSS remain descriptive here; the full native-control report
retains the baseline comparisons and unresolved early-estimate coverage gaps.

The [same-command precision report](../two-fe-single-command-clock-local-2026-09-20/README.md)
separately resolves native control 56 while preserving its earlier rounded
failures. The next step is both five-pair blocks across the unchanged 77-cell
primary inventory. No installed package, remote state or qualification changed.
