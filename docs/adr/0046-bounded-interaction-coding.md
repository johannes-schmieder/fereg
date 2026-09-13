# ADR 0046: bounded categorical interaction coding

Status: accepted for local development, 2026-09-12.

Categorical FE/cluster interactions may use an exact mixed-radix mapping when
the checked product of component level counts fits u32 and the entire u32 table
fits within the displaced `physical_rows * sizeof(usize)` ordering allocation.
The output vector temporarily holds tuple slots. Scanning the table in lexical
order and remapping those slots preserves contiguous canonical codes, including
unused component levels and row permutations. Larger/unrepresentable products
retain comparison sorting without speculative table allocation.

The shared helper serves ordinary OLS interacted clusters and the existing
categorical factorizer used by OLS/PPML topology, rebasing and exact validation.
Callers retain authoritative component validation; generic validation precedes
allocation. Reservations are fallible/capacity checked. There is no persistent
order cache, N-by-arity key matrix, new worker/SPI access, or numerical policy.
Simple cluster borrowing and existing repeated-topology sharing remain intact.
Sample, weight, singleton, rank, DoF, covariance, certification and publication
contracts are unchanged.

The [source-bound report](../../benchmarks/reports/development/interaction-codes-local-2026-09-12/README.md)
records 27 paired comparisons against `9c2bcc5`. Repeated interacted-cluster OLS
gains 10.5%, and interacted-FE/cluster OLS gains 6.8% with a repeatable 24–26 ms
command saving. Separate traces locate those savings in preparation. PPML's
3.4% total gain has weaker phase attribution and does not independently meet the
prospective target. Primary native peaks are unchanged; all scientific and
non-time solver comparisons are exact. Forty-one distinct focused tests and both
independent Stata/Mata oracle arms pass. The report binds the reviewed artifact
to the measured source and preserves existing graph memory-accounting gaps.

Skew scheduling, score reconstruction, broader finite generated-column work and
intercept-solver eligibility remain open. This is local development evidence,
not a release or cross-platform qualification decision.
