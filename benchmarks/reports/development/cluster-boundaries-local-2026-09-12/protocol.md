# P3/P7 bounded cluster-boundary search

Baseline main fdf46244aa4d286c1d4ac30565236458ed1ef648 and the accepted 264ca6c9a3ec376a165221d368a720f191b8828950bfe318ce6e6f357b3d27f0 plugin. Source snapshots match all 150 accepted native inputs; installed dist and unrelated files remain untouched.

Both the OLS ordered reducer and PPML paired-independent reducer currently advance each provisional worker boundary linearly to the end of its sorted cluster. A giant group can cause many boundaries to rescan the same tail. Test a shared upper-bound search with an immediate-boundary fast check. Preserve every boundary, empty chunk, within-group row order and forward/reverse merge exactly. No new allocation, chunk count, scheduling regrouping or SPI/worker policy.

Require at least 10% median paired total-command improvement or a repeatable 20 ms saving, with inference/setup attribution, on a skewed cluster target. Protect balanced clusters, weak graphs, small/narrow, weights, multiway subsets and memory modes; investigate regressions beyond max(10 ms,5%). Retain modeled/native/scratch/RSS, solver decisions and independent certificates. Start with one skewed OLS and one skewed PPML case, then a few rotated seeds and focused controls if promising. Bound this local experiment; no SCC or qualification.
