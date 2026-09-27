# Illustrated SCC benchmark review — 2026-09-27

The eight-page [PDF](../../../../output/pdf/fereg-scc-benchmark-report-2026-09-27.pdf)
is an illustrated interpretation of the immutable
[completed campaign](../systematic-light-scc-final-2026-09-27/README.md), not a new
benchmark. Its six figures cover eligibility, timing distributions, categorical
designs, thread scaling, slopes, reliability and memory. The original report,
receipts, definitions and hashes are unchanged. Claims attach to estimator and
controller `58284f76a65b226e6df09a48cafa5caa68461af2`, not the later PPML bias work.

`figures.py` consumes the original summary, paired-ratio and run CSVs. The heatmap
uses complete four-repeat configurations only. Missing, timed-out or scientifically
discrepant results remain unranked. The scaling panels show medians of individual
command times; other ratio plots use medians of paired ratios. Four seeded
repetitions describe observed variation, not confidence intervals.

`captured-phases.json` extracts 392 existing native result metadata records with
their original result paths, hashes, input hashes and hosts. All source hashes
match the original captured-output inventory. No new fits, instrumentation or
profiling were used. `analyze.py` reproduces the additional matched-reference and
phase summaries in `analysis.json`. The reported final route is 2=MAP, 3=LSMR in
the frozen plugin; it is not a per-iteration route history. Phase timers include
their associated checks. Different solver iteration counters are not equivalent
operation counts.

The new diagnostic observation is the sparse three-FE PPML T28 deterioration:
194.64s versus 24.33s at T8, with median projection time 152.68s versus 16.35s.
Reported final routes and iteration counts also differ. Those are measured
associations; causal attribution to routing, convergence or scheduling needs a
controlled experiment. On the 46 PPML configurations eligible against both fixest
settings, median native/default and native/strict ratios are 1.165 and 0.819;
the median ratio of strict/default command medians is 1.405.

Large strong-graph PPML losses concentrate in projection and finite solves, with
very little time in covariance formation. Dense PPML slopes win while OLS slopes
lose. ADRs 0123 and 0134 explicitly exclude public OLS from the corresponding
PPML coefficient/joint routes. Generalizing that organization is a candidate,
not a demonstrated OLS improvement. Sparse-zero failures and sparse OLS reference
disagreements need independent adjudication before broader performance claims.

Reproduction (Python environments need matplotlib/numpy for figures and
reportlab for the PDF; the analysis uses only the standard library):

```sh
python benchmarks/reports/development/systematic-light-scc-pdf-2026-09-27/analyze.py
python benchmarks/reports/development/systematic-light-scc-pdf-2026-09-27/figures.py
python benchmarks/reports/development/systematic-light-scc-pdf-2026-09-27/build_pdf.py
pdftoppm -png output/pdf/fereg-scc-benchmark-report-2026-09-27.pdf /tmp/fereg-report
```

`qa.json` records the final page/content/render checks and PDF identity.
All eight rendered pages were visually reviewed, including axes, tables, page
numbers and margins. `SHA256.json` binds this derivative's artifacts and source
CSVs. Re-rendering can change PDF metadata and byte identity; preserve this receipt.
This report is development evidence, not release qualification.
