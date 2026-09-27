# Expanded SCC benchmark review — 2026-09-27

The [14-page expanded PDF](../../../../output/pdf/fereg-scc-benchmark-report-2026-09-27-expanded.pdf)
adds the requested design glossary, every comparator, reasons for unranked OLS
cells, failure attribution, all eight slope configurations, saved-output/inference
coverage and reconciliation with earlier OLS timing wins. Seven figures include
an all-comparator distribution. The OLS grid now distinguishes scientific
disagreements (`A`) from native timeouts (`T`); unregistered cells remain `--`.

This succeeds the [original illustrated report](../systematic-light-scc-pdf-2026-09-27/README.md)
without altering that PDF or its hashes. The [frozen campaign](../systematic-light-scc-final-2026-09-27/README.md)
and all original eligibility rules and receipts remain unchanged. Claims still
attach to `58284f76a65b226e6df09a48cafa5caa68461af2`.

## New evidence and limits

- All 24 unranked OLS/fixest configurations are accounted for: 21 scientific
  disagreement configurations and three native-timeout configurations. In the
  main categorical grid, these counts are 15 and three. Four width and two slope
  configurations account for the remaining disagreements.
- `diagnostic-extract.json` contains selected metadata, scientific checks and
  sanitized error lines from existing SCC outputs. `analyze.py` verifies 498
  record bindings against the original captured SHA inventory. The count includes
  repeated references to the same result; it is not 498 distinct fits.
- An independent union-find topology calculation exactly reproduces both group
  hashes of the captured sparse two-FE N61,440/seed104729 input. There are 38
  components, FE rank 12,865 and residual DoF 48,565, agreeing with fereg/reghdfe.
  Default fixest uses 48,528 residual DoF. This settles that graph-rank issue;
  it does not adjudicate coefficients or all production cells.
- In a separate captured three-FE example, diagnostic DoF rescaling reduces the
  relative covariance-diagonal discrepancy to 2.344e-9. No covariance, tolerance,
  frozen comparison, sample or timing rank was modified. Most sparse unranked
  cells also involve other failed checks.
- Of 84 warm OLS/fixest discrepancies, 83 pass comparison to reghdfe; the other
  is the independently adjudicated slope pilot with a retained reference timeout.
  Reference agreement is corroboration, not independent mathematical acceptance.
- All 150 pyfixest worker failures show the 10,000-iteration demeaning limit:
  141 in metadata and nine in warm-up exceptions. `failure-stages.json` records
  target-sentinel presence; all 65 gglm and eight ppmlhdfe worker failures are
  before target READY. Their exact numerical causes remain unlocalized.
- Four feppml sparse-zero rc459 failures are genuine native failure cases. The
  timed Stata command uses `capture quietly`; existing output does not identify
  the failed internal condition. No claim of a particular solver, separation,
  certificate or coding defect follows from the return code alone.
- `strong_d2/d3/d6` are all two-FE graph constructions. Degree is not the number
  of FE partitions. The earlier chat's six-dimensional description was wrong.
- Current timing excludes saved FE, clustered VCE, weights and published fitted
  outputs. All 32 native slope calls passed; incomplete comparator evidence is
  shown as coverage, never filled using diagnostic clocks.

No fits, reference modifications or benchmark jobs were run. Remote inspection
was read-only. Raw licensed logs remain remote; only error excerpts and hashes
are retained here. The current unrelated bias-correction work is outside scope.

## Reproduction and sources

```sh
python benchmarks/reports/development/systematic-light-scc-pdf-expanded-2026-09-27/analyze.py
python benchmarks/reports/development/systematic-light-scc-pdf-expanded-2026-09-27/figures.py
python benchmarks/reports/development/systematic-light-scc-pdf-expanded-2026-09-27/build_pdf.py
pdftoppm -png output/pdf/fereg-scc-benchmark-report-2026-09-27-expanded.pdf /tmp/fereg-expanded
```

Analysis needs numpy; figures need matplotlib/numpy; PDF authoring needs
reportlab. `qa.json` records page/content/bounds/render verification and the PDF
identity. `SHA256.json` binds this edition and its source evidence. All 14 rendered
pages were visually inspected before delivery. Byte identity can change if PDF
metadata is regenerated; preserve this edition's receipt.

Historical context comes from
[the two OLS Mac blocks](../current-ols-final-blocks-local-2026-09-21/README.md),
[their round closeout](../ols-round-closeout-local-2026-09-22/README.md), and
[exact-input slope adjudication](../systematic-slope-adjudication-2026-09-26/result.md).
The old 93/93 timing statement includes separately recorded reference
discrepancies and pertains to another artifact, platform and case mix. It is not
a like-for-like regression baseline. This report makes no release claim.
