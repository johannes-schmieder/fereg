# Fresh OLS slope confirmation

100 fits, 35 paired repetitions.

Command milliseconds are medians. Ratios show the median of paired ratios [minimum, maximum].
NR means at least one registered science comparison is ineligible; raw clocks do not override that exclusion.

| Case | Baseline ms | Candidate ms | C/B | C/default | C/strict | B/C peak MiB |
|---|---:|---:|---:|---:|---:|---:|
| fresh-fast-t1 | 967.0 | 818.0 | 0.845 [0.835, 1.121] | not rerun | 0.385 [0.383, 0.388] | 108.8 / 109.5 |
| fresh-fast-t4 | 832.0 | 680.0 | 0.877 [0.594, 1.378] | not rerun | 0.713 [0.568, 0.932] | 123.5 / 124.5 |
| fresh-fast-t8 | 873.0 | 754.0 | 0.821 [0.745, 1.101] | not rerun | 1.055 [0.904, 1.057] | 125.1 / 118.6 |
| fresh-low-t8 | 1140.0 | 939.0 | 0.816 [0.749, 1.134] | not rerun | 1.277 [1.141, 1.459] | 118.1 / 120.1 |
| fresh-wide | 1888.0 | 1455.0 | 0.771 [0.486, 0.933] | not rerun | 0.856 [0.583, 0.948] | 165.9 / 168.2 |
| fresh-larger | 2332.0 | 2045.0 | 0.907 [0.817, 0.947] | not rerun | 1.063 [0.890, 1.134] | 289.8 / 287.3 |
| fresh-three-low | 7292.0 | 5992.0 | 0.822 [0.578, 1.420] | not rerun | not rerun | 154.5 / 159.6 |

Process RSS includes the full warm-plus-timed estimator process, including loaded data and requested outputs.
Setup/loading and extraction are outside the command clock; their process-time distributions remain in the JSON receipt.

## Individual-input baseline controls

A combined median must not hide a repeatable loss on one input. Loss counts use paired ratios greater than one.

| Case | Seed | C/B median [min, max] | Slower pairs / eligible |
|---|---:|---:|---:|
| fresh-fast-t1 | 202609281 | 0.845 [0.835, 1.121] | 1 / 5 |
| fresh-fast-t4 | 202609281 | 0.877 [0.594, 1.378] | 2 / 5 |
| fresh-fast-t8 | 202609281 | 0.821 [0.745, 1.101] | 2 / 5 |
| fresh-low-t8 | 202609281 | 0.816 [0.749, 1.134] | 1 / 5 |
| fresh-wide | 202609281 | 0.771 [0.486, 0.933] | 0 / 5 |
| fresh-larger | 202609281 | 0.907 [0.817, 0.947] | 0 / 5 |
| fresh-three-low | 202609281 | 0.822 [0.578, 1.420] | 1 / 5 |


Source snapshot `e07c7ec3…`, committed computational source **c6c1df25**, Linux plugin `e9d9f706…`. Baseline is the boxed-owner plugin `36f068f6…`. Job **7762198** completes all 100 applications, 35 native science pairs and 30 strict-stock pairs. All science checks pass. Seed 202609281 and all seven cases were declared before generation; no implementation or threshold changed in response.

The six new-path paired medians improve by **9.3–22.9%**. Their strict-stock ratios span **0.385–1.277**. Serial, T4 fast and K32 wide clear the 0.90 goal. Fast T8 and larger T8 are 5.5%/6.3% behind stock; low T8 is **27.7% behind** and misses the 1.20 ceiling. These remain open performance issues despite a useful kernel improvement.

The excluded three-FE low control improves at the median in this block (0.822; one slower pair of five), but its wide 0.578–1.420 range and the initial block's 11.1% median loss prevent a claim of a stable control gain. No changed mathematical path explains a speed gain there.

The fast T1/T4/T8 cells use byte-identical inputs. Candidate command medians are **818/680/754 ms**. Pairing the T8/T4 clocks by repetition gives median **1.154**, range 0.740–1.216, with three slower repetitions of five; stock improves in all five, median ratio 0.741. This is a scaling concern requiring nine-pair confirmation before a repeatable-loss claim. Thread counts are still exact requests in this source; no ceiling has been implemented.

No numerical storage or admission term was added. Fresh process peak changes are small: about +0.7/+1.0/-6.5/+2.0/+2.3/-2.5 MiB on the six new-path cases and +5.1 MiB on the excluded control. These full-process measurements include loading and warm/timed fits; they are distinct from the unchanged numerical payload. The large initial block's higher candidate peak does not persist on the fresh larger input.

PPML scientific validation passed separately. Shared PPML performance controls are pending as job 7762256, using robust estimation without optional saved observation outputs. No PPML timing claim follows from this OLS table.
