# Additional two-FE graph, weight and output coverage

All twenty-four remaining cells in the expanded three-pair screen complete:
198 successful calls, with no detected competing process in any cell. The
candidate beats default fixest in all eighteen comparable cells by more than
10%. Six frequency-weight cells use the pinned native baseline; ordinary
fixest weights do not reproduce Stata frequency-weight inference.

The independent grounded sparse-direct audits pass all 48 native fits, and
144 repeated native result checks match exactly. All 36 candidate saved-output
audits pass. Nine reference inference audits pass and nine disagree; the
original reference disagreements remain failures and are preserved in full.
These are exploratory timings, not final independent-block acceptance.

The unchanged inputs have N=245,760, K=6 and sixteen native/R workers. Each
cell requests a crossing cluster with saved effects or two nested clusters
without outputs. The baseline is source `0efa78bd`; sf1 includes the current
captured graph/fit drafts plus the separately accepted slope optimization.
No inference, sample or convergence tolerance changes for these comparisons.

| Graph | Weight | Request | Baseline ms | sf1 ms | fixest ms | Paired comparator ratio |
|---|---|---|---:|---:|---:|---:|
| two_fe_sparse | none | cluster_cross_saved | 511 | 188 | 853 | 0.2171 |
| two_fe_sparse | none | cluster2_nested_none | 137 | 124 | 798 | 0.1544 |
| two_fe_sparse | aw | cluster_cross_saved | 598 | 199 | 793 | 0.2509 |
| two_fe_sparse | aw | cluster2_nested_none | 151 | 136 | 752 | 0.1835 |
| two_fe_sparse | pw | cluster_cross_saved | 650 | 201 | 811 | 0.2500 |
| two_fe_sparse | pw | cluster2_nested_none | 155 | 138 | 756 | 0.1838 |
| two_fe_sparse | fw | cluster_cross_saved | 637 | 199 | — | 0.3139 |
| two_fe_sparse | fw | cluster2_nested_none | 158 | 141 | — | 0.8924 |
| unbalanced | none | cluster_cross_saved | 112 | 93 | 137 | 0.6788 |
| unbalanced | none | cluster2_nested_none | 91 | 74 | 93 | 0.8043 |
| unbalanced | aw | cluster_cross_saved | 121 | 105 | 166 | 0.6325 |
| unbalanced | aw | cluster2_nested_none | 99 | 82 | 122 | 0.6833 |
| unbalanced | pw | cluster_cross_saved | 118 | 107 | 164 | 0.6524 |
| unbalanced | pw | cluster2_nested_none | 102 | 82 | 120 | 0.6667 |
| unbalanced | fw | cluster_cross_saved | 122 | 108 | — | 0.8917 |
| unbalanced | fw | cluster2_nested_none | 100 | 84 | — | 0.8400 |
| disconnected_blocks | none | cluster_cross_saved | 105 | 93 | 278 | 0.3357 |
| disconnected_blocks | none | cluster2_nested_none | 86 | 68 | 195 | 0.3487 |
| disconnected_blocks | aw | cluster_cross_saved | 116 | 107 | 290 | 0.3728 |
| disconnected_blocks | aw | cluster2_nested_none | 93 | 74 | 224 | 0.3304 |
| disconnected_blocks | pw | cluster_cross_saved | 118 | 101 | 289 | 0.3517 |
| disconnected_blocks | pw | cluster2_nested_none | 93 | 75 | 225 | 0.3319 |
| disconnected_blocks | fw | cluster_cross_saved | 118 | 103 | — | 0.8729 |
| disconnected_blocks | fw | cluster2_nested_none | 97 | 78 | — | 0.8125 |

`verify.py` checks portable evidence, full baseline/sf1 source reconstruction
and the independent-auditor binding. Large output arrays remain local with
explicit hashes; the committed independent audits record their checks. Raw
licensed logs are excluded. The earlier partial and ring/cycle screens used
different artifacts, so their union is not one final-candidate campaign.
Broader protection, final blocks, routing decisions and Linux confirmation
remain open. No installation, push or qualification is performed.
