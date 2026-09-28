# Same-artifact OLS slope thread confirmation

**Baseline = T4; candidate = the identical binary at T8; strict stock = T8.**

81 headline fits, 27 paired repetitions. Three separate maximum-thread probe fits also pass.

Command milliseconds are medians. Ratios show the median of paired ratios [minimum, maximum].
NR means at least one registered science comparison is ineligible; raw clocks do not override that exclusion.

| Case | Baseline ms | Candidate ms | C/B | C/default | C/strict | B/C peak MiB |
|---|---:|---:|---:|---:|---:|---:|
| fresh-fast-t8 | 855.0 | 730.0 | 0.896 [0.733, 1.068] | not rerun | 0.958 [0.740, 1.145] | 122.6 / 123.1 |
| fresh-low-t8 | 1061.0 | 959.0 | 0.965 [0.845, 1.074] | not rerun | 1.253 [1.054, 1.660] | 117.3 / 120.5 |
| fresh-larger | 1726.0 | 1810.0 | 1.049 [0.912, 1.218] | not rerun | 0.919 [0.862, 1.053] | 279.2 / 289.3 |

Process RSS includes the full warm-plus-timed estimator process, including loaded data and requested outputs.
Setup/loading and extraction are outside the command clock; their process-time distributions remain in the JSON receipt.

## Individual-input baseline controls

A combined median must not hide a repeatable loss on one input. Loss counts use paired ratios greater than one.

| Case | Seed | C/B median [min, max] | Slower pairs / eligible |
|---|---:|---:|---:|
| fresh-fast-t8 | 202609281 | 0.896 [0.733, 1.068] | 2 / 9 |
| fresh-low-t8 | 202609281 | 0.965 [0.845, 1.074] | 3 / 9 |
| fresh-larger | 202609281 | 1.049 [0.912, 1.218] | 6 / 9 |


This block reuses the byte-identical seed 202609281 validation inputs to confirm repeatability. It is not new source validation or a candidate-versus-old-binary gain. Computational source c6c1df25, Linux plugin e9d9f706…, is identical in both native arms. Both set Stata processors to four; the independently requested and asserted native team is four/eight. Every process pins to its requested cores. All 27 native and 27 strict-stock scientific pairs pass, including exact sample/DoF and requested residual/fitted outputs. IID inference and residual/fitted work match the prior OLS slope harness.

The earlier five-repetition fast T8/T4 median loss of 15.4% does **not** persist: nine directly alternating pairs give **0.896**, with two slower pairs. Low also favors T8 at median **0.965**, three slower pairs. The larger input still has a small median loss: **1.049**, six slower pairs, spanning 0.912–1.218. These distributions do not justify a blanket T4 cap. They also do not establish universally monotone scaling; the larger loss remains visible.

Low-memory performance versus strict stock remains **1.253**, above the 1.20 ceiling and close to the prior fresh 1.277. Fast and larger T8 ratios are 0.958/0.919, competitive but above the 0.90 ambition. The larger T8 process peak is about 10.1 MiB above T4; fast/low differences are roughly 0.5/3.2 MiB.

Job **7762473**: failed=0, exit_status=0, 448 seconds, 28 isolated slots on scc-wo4, maxvmem 1.525G. Probe, rotation and wrapper terminal sentinels passed. No compilation, profiling or competing fereg fits ran during timing.

Decision: keep the current native thread policy for this OLS path while investigating the repeatable low-memory stock gap. Do not tune a guard to these reused inputs or claim the overall core-scaling goal has passed. The next bounded thread check addresses prior narrow PPML losses above eight cores, with a small-fit comparison and fresh wide control.
