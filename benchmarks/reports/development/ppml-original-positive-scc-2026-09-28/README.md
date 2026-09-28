# Original positive PPML discrepancy: native stopping is inaccurate

Job **7763328** completed the bounded accuracy diagnostic on 2026-09-28.
Original and current fereg fail the unchanged coefficient envelope for **x8**;
pinned strict fixest passes. These are shared-node accuracy checks, not new
performance rankings. Historical scientific NR remains NR.

## Input and independent authority

Both original strong_d2 N61440/K10 positive-outcome scopes (T4 and T28,
seed104729) use the same input. The original SCC Python runtime reproduces its
DTA SHA `bbc70aa2a611acfd57db1b15e55061cb0b802213b12feb584f03f7f89f83a5de`
and every column hash. There are 30,720 first groups and 768 second groups;
all outcomes are positive and every group has at least two rows. No separation
or sample ambiguity is involved.

The new bounded [full sparse-design oracle](../../../systematic/two_fe_ppml_oracle.py)
uses direct grounded full-Hessian Newton solves, independently of both native
and fixest absorption. Its robust finite covariance uses full-design influence
columns. Separate scalar full-dummy Poisson/sandwich checks pass on connected
and disconnected small designs, together with a known nonzero-residual optimum.
The existing ADR0194/0029 60-digit outward Decimal certificate independently
bounds optimum coefficient error: radius **8.656554e-14**. Numerical point
convergence alone is not the authority for the coefficient diagnosis.

Total model rank is **31497**, deviance **105949.66951089134**. The exact
certificate and coordinate intervals are in `results/`; covariance comparisons
are point-oracle checks, not certified optimum covariance intervals.

| Fit | IRLS iterations | Maximum coefficient error | Certified coefficient check |
|---|---:|---:|---|
| Original fereg T4 and T28 | 4 | 1.3539155e-10 | x8 outside |
| Current fereg T4 | 4 | 1.3539155e-10 | x8 outside |
| Original strict fixest T4 and T28 | 5 | about 1.05e-15 | all inside |
| New strict fixest, outer 1e-10 / FE 1e-11 | 5 | 1.051243e-15 | all inside |
| Outer 1e-12 / FE 1e-11 | 6 | 5.477931e-17 | all inside |
| Outer 1e-10 / FE 3e-12 | 5 | 1.041268e-15 | all inside |
| Outer 1e-12 / FE 3e-12 | 6 | 5.179776e-17 | all inside |

Native x8 is `0.00031290150329649999`; the enclosed optimum is centered at
`0.0003129016386880486`. The coefficient envelope is still
`1e-10 + 1e-7*abs(beta)`. Sample, rank, objective, SE and robust covariance
checks pass for native; its coefficient accuracy does not. See
[comparison.json](results/comparison.json) for all checks and coordinate bounds.
The four new reference probes use one worker; the native diagnostic uses four.
No matched-thread timing conclusion follows from this table.

## Binding and execution

Current computational snapshot `4c3d7c44dec6535da0506e1f771516ec1615ce92c43beb6e53520fd0c949771e`
contains 211 files; Linux plugin
`a7e95e6712c6b05b0217a9ddffa5b8f14c0f8d95443c2b983ce459f16a0e4596`.
The stock library is unmodified pinned fixest 0.14.2. The registration binds
original receipts, original runtime fingerprint, exact input and all probe
settings. Deployment checks every staged file, source input and stock library
file before computation.

Accounting: failed=0, exit_status=0, four slots, 92 seconds, maxvmem 1.607G.
All five expected application/parent sentinels are present. The process succeeds
because the diagnostic completed; its scientific conclusion is a native failure.
[receipt.json](receipt.json) binds the sanitized outputs and raw-log hashes;
raw licensed logs and binary means are retained remotely, not committed.

The positive-outcome diagnostic does not adjudicate the separate original
zero-outcome discrepancy. A production stopping repair and its fresh validation
must have their own source-bound report.
