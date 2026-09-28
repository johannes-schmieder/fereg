# Portable coupling: PPML controls

Job **7762256** completed all **45 applications**, **15 native pairs** and **15 strict-stock pairs** with every registered scientific check passing. This is bounded development evidence for computational source **c6c1df25**, snapshot `e07c7ec3…`, Linux plugin `e9d9f706…`; baseline is boxed-owner plugin `36f068f6…`.

Five predeclared cases use fresh seed **202609282**, with three alternating rotations. Public `feppml` and pinned, unmodified fixest 0.14.2 `fepois` perform robust inference without optional saved observation outputs. Stock uses `glm.tol=1e-10` and `fixef.tol=1e-11`. Input loading and extraction stay outside command clocks; complete process time and RSS distributions are in `summary.json`. The registered 7,680-row warmup precedes each measured input.

| Case | N / K / threads / mode | Baseline / candidate / stock ms | Paired C/B median [min, max] | Paired C/stock median [min, max] | Native slower pairs | B / C / stock peak MiB |
|---|---|---:|---:|---:|---:|---:|
| two-slopes | 30,720 / 6 / 4 / fast | 584 / 536 / 1489 | 0.876 [0.757, 0.959] | 0.348 [0.260, 0.376] | 0/3 | 108.8 / 103.4 / 282.4 |
| three-slopes | 30,720 / 6 / 4 / fast | 2654 / 2315 / 35089 | 0.707 [0.652, 0.974] | 0.065 [0.054, 0.079] | 0/3 | 105.8 / 103.0 / 298.9 |
| three-larger | 61,440 / 10 / 8 / fast | 6550 / 7156 / 115192 | 0.840 [0.665, 1.160] | 0.061 [0.048, 0.066] | 1/3 | 146.8 / 149.1 / 353.0 |
| three-low | 61,440 / 10 / 8 / low | 9271 / 6112 / 114158 | 0.659 [0.589, 0.893] | 0.054 [0.049, 0.068] | 0/3 | 148.7 / 143.8 / 357.5 |
| categorical-control | 30,720 / 6 / 4 / fast | 457 / 424 / 342 | 0.888 [0.869, 1.037] | 1.233 [1.199, 1.272] | 1/3 | 119.5 / 125.1 / 279.8 |

The four slope paired medians improve **12.4–34.1%**. All four beat strict stock by at least 10% on these designs. The larger fast case remains variable: its paired median improves 16.0%, yet its candidate marginal median (7,156 ms) exceeds baseline (6,550 ms), and one pair loses 16.0%. These are different statistics; three pairs do not establish a stable improvement of that size. Raw distributions are preserved.

The categorical control is **1.233 times stock**, missing the 1.20 ceiling. Its native paired median improves, but this unchanged numerical route does not establish a causal kernel gain. This small-fit control remains a separate optimization target.

The panels here have many more groups and different connectivity from the earlier small-group unbalanced slope benchmarks. Their three-FE stock fits take 35–115 seconds. Do not transfer these ratios to historical 119–232 ms reference fits or to saved-mean work. Likewise, the current block does not adjudicate default-tolerance reference accuracy failures.

Exact input/sample, convergence, coefficients, full finite covariance, standard errors and deviance pass; native rank/DoF checks pass. Independent explicit-design WLS and Poisson/Hessian/sandwich checks passed separately on Linux 7762056 and Mac. There is no new independent-oracle fit inside this timing block and no saved-mean comparison here.

No numerical storage was added. Full-process candidate peaks differ by roughly −5.4 to +2.3 MiB on slope cases and +5.6 MiB on the categorical control. These include input data and estimator/runtime overhead.

Scheduler accounting: failed=0, exit_status=0, 1,143 seconds, 28 slots on scc-wo4, maxvmem 1.588G. Both controller and wrapper terminal success sentinels were observed. A prospective wall allowance amendment from 20 to 30 minutes preserved every case, limit, repetition and resource except h_rt; the block finished in about 19 minutes. No build, profiler or other fereg fit ran alongside this block.

Together with fresh OLS confirmation, these results support adopting the small portable kernel change for bounded development. They do not close the low-memory OLS stock loss, core-scaling concern, categorical PPML control loss, or the broader repair plan.
