# Performance contract

Performance is an acceptance gate for the 0.9.0 OLS source candidate, not a
marketing claim. Timing or memory comparisons count only after exact sample,
omission, coefficient, VCE, DoF, fit, output, and certificate parity passes
against the pinned `reghdfe` closure. PPML is outside this contract.

Required benchmark reporting:

- wall-clock median and dispersion across warm runs;
- peak resident memory;
- effective sample and singleton/separation deletion counts;
- FE/IRLS iteration counts and final certificates;
- thread count, CPU model, OS, Stata edition/version, and plugin hash;
- comparison against the pinned reference versions in `references.lock.toml`.

Approved performance cases contain at least 10 million observations and cover
dense and sparse two-way FEs, low mobility, absorbed slopes, clustered VCE, and
moderate regressor counts. Comparator and candidate arms run on the same node
and in the same job with registered one- and eight-thread comparisons; 16/32
thread scaling is supplemental where useful work justifies it.

The governing runtime gates are:

- geometric-mean candidate/reference runtime ratio no greater than 0.40
  (at least 2.5x faster);
- at least 80 percent of approved cases at least 2x faster;
- registered single-thread comparisons at least 1.5x faster; and
- no unexplained case more than 25 percent slower.

The governing memory gates are:

- candidate peak RSS no worse than reference in at least 80 percent of cases;
- no unexplained case above 1.25 times reference RSS;
- observed native allocation within 25 percent of the registered model;
- no repeated-call leak; and
- a constrained-memory case where `memory(low)` succeeds while
  `memory(fast)` fails preflight.

Medians use at least five warm runs after one cold run. Acceptance requires
`qacct`, Stata sentinels, logs, scientific hashes, and output schemas to agree.
Exploratory Phase 0–1 thresholds are superseded by ADR 0007.
