# Boxed-owner confirmation on prior frozen inputs

90 fits, 36 paired repetitions.

Command milliseconds are medians. Ratios show the median of paired ratios [minimum, maximum].
NR means at least one registered science comparison is ineligible; raw clocks do not override that exclusion.

| Case | Baseline ms | Candidate ms | C/B | C/default | C/strict | B/C peak MiB |
|---|---:|---:|---:|---:|---:|---:|
| two-dense-fast | 1297.0 | 538.0 | 0.425 [0.347, 0.469] | not rerun | 0.847 [0.587, 0.887] | 144.7 / 104.1 |
| three-sparse-fast | 2677.0 | 2674.0 | 1.008 [0.890, 1.116] | not rerun | not rerun | 155.1 / 160.1 |
| three-sparse-low | 7270.0 | 7367.0 | 1.021 [0.686, 1.390] | not rerun | not rerun | 155.6 / 154.1 |
| two-dense-larger | 4187.0 | 2219.0 | 0.526 [0.448, 0.715] | not rerun | 1.173 [0.930, 1.354] | 545.7 / 311.4 |

Process RSS includes the full warm-plus-timed estimator process, including loaded data and requested outputs.
Setup/loading and extraction are outside the command clock; their process-time distributions remain in the JSON receipt.

## Individual-input baseline controls

A combined median must not hide a repeatable loss on one input. Loss counts use paired ratios greater than one.

| Case | Seed | C/B median [min, max] | Slower pairs / eligible |
|---|---:|---:|---:|
| two-dense-fast | 202609280 | 0.425 [0.347, 0.469] | 0 / 9 |
| three-sparse-fast | 202609280 | 1.008 [0.890, 1.116] | 5 / 9 |
| three-sparse-low | 202609280 | 1.021 [0.686, 1.390] | 6 / 9 |
| two-dense-larger | 202609280 | 0.526 [0.448, 0.715] | 0 / 9 |


Source **03292138**, Linux plugin `36f068f6…`, completed as job **7761995**. All 36 native science pairs and 18 strict-stock pairs pass. This is the unchanged nine-rotation registration after a missing-input-link repair; the failed pre-fit attempt 7761945 remains preserved separately. Seed 202609280 was already consumed and is not fresh validation.

The prior 852b03a9 low-control median of 1.200 becomes 1.021 in this separately paired boxed-owner block. Its range remains broad, and six of nine pairs still lose. The fast control is 1.008 with five losses. This supports retaining the bounded two-partition owner, with small control losses visible; it does not prove that stack layout caused the earlier slowdown. There is no native arithmetic or routing change in this candidate.

Two-partition medians are 0.425/0.526 of baseline. The smaller case clears the 0.90 strict-stock goal; the larger case is 1.173 and still misses that ambition. Per-pair larger strict-stock ratios range 0.930–1.354. These are medians on this exact source and node allocation, not universal 20% ceilings. No saved-FE, robust, clustered, weighted or PPML performance claim follows from this IID/residual-output screen; independent licensed fixtures cover science on those relevant boundaries.
