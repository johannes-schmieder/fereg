# Retained dense moment standardization: prospective local evaluation

Accepted native baseline 6ad8b201b73dcc1e550827a6b713011288292910; main
25034ab adds rejected raw-norm evidence only. Isolated baseline plugin is
ef4f1a61a8f5c240e9a7a6de8b2aef835344ab51206e784d12b68c8480d09a75.
Raw-reference and fresh-input certificate prototypes remain rejected/absent.

A diagnostic statistical sample of the accepted large one-FE command identifies
retained finite moments as a dominant worker cost. Source inspection finds
repeated division/checking of right-column values within each row's cross-products.
The prototype caches the selected standardized row suffix once per worker
partition for scaled dense designs of at most 64 active columns, only where the
layout assigns at least two left columns per partition (the final tail can
contain one). Use a fixed 64-double local array,
preserve every Gram/RHS observation-order FMA and retain wider, scratch, unscaled
and single-column-partition paths. No heap allocation, Gram partial bank or
observation-sized state. Conservatively bound extra local stack payload by
512 bytes per partition, at most 32 KiB per moment call. No rank/scale/routing
or certificate policy changes.

Test exact scalar moments and 1/4/16-thread identity at small and 63/64/65-column
boundaries, selected/gapped columns, unit/provided weights, scales/extreme finite
values, bad values and scratch controls. Run existing finite-kernel, retained
fresh/frozen/strict, stale-rank/generation/rollback tests and both independent
full-dummy Poisson/Hessian/sandwich Stata oracle arms on isolated artifacts.

Screen complete/FW/PW one-FE targets at seed104729, rotating arms. If any saves
both 5% and 20 ms and science passes, repeat all three at seeds8675309 and20260819.
Adopt only with a median paired gain of both 5% and20ms on a primary and savings
in changed finite solving. Retain all observations; investigate primary/protected
increases above max(10ms,5%). Controls seed31415927: weak two FE, low FW,
small one FE, saved/separated, strong explicit two-FE MAP, plus a 65-control
one-FE case guarding the cache boundary. If adoption remains plausible, run a
separate PW private allocation audit requiring zero events/live/peak growth,
stable capacities and fresh/warm equivalence. Exact full b/V and discrete science/
solver fields, unchanged independent certificates, command/phase time, modeled/
native/RSS/scratch, iterations/retries/fallbacks are required.

The initial two-command identical-binary sampling probe captured only one valid
callgraph before the watcher failed on an exiting child AccessDenied. Both fits
and their scientific comparison passed; those diagnostic timings are excluded
from adoption and no second callgraph is claimed. No builds/tests or sampling
during subsequent performance measurement. No installed-dist replacement,
agents, SCC, push, comparator claim or qualification.
