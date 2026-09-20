# One-FE inference and output breadth

This is a bounded local Mac development comparison, not a release matrix. It
reuses the isolated `normonly` artifact from the completed one-FE optimization
and the pinned stock fixest 0.14.2 installation. It changes no estimator code.

`run.py` stages the existing fresh-process measurement workers, adds explicit
VCE and observation-output requests, and audits imported numeric values before
timing. The data preserve the original one-FE observations and add two
deterministic crossing cluster variables. Small explicit-dummy oracles cover
the full coefficient covariance and normalized requested outputs.

Run `--part smoke --label smoke --repeats 1` before
`--part primary --label main --repeats 3`. The root directory is positional;
`--cases` and `--widths` select bounded subsets. `--trace` records native phases
in a separate fereg-only run. Set `STATA_MP_PATH` to the licensed executable,
put `Rscript` on `PATH`, and set `R_LIBS_USER` to the pinned stock library plus
its dependencies. Existing run directories are never overwritten. Process
sampling requires access to the local process list.

The complete command includes Stata-to-native loading, inference, and any
requested saved FE/residual publication. The output bundle also times public
`predict` calls for `xb` and `xbd`. R times `feols`, then `fixef(sorted=FALSE)`,
expansion through the fitted object's group codes, normalization, and requested
residual/fitted-vector recovery. File import and audit export are excluded on
both sides. Default small-sample corrections are made explicit in R and
independently checked against Stata's correction denominator and cluster DoF.

`collect.py ROOT OUTPUT` creates a new compact archive with timing ranges,
paired ratios, sampled RSS, input/source/artifact hashes, science receipts,
worker sources and phase comparisons. It verifies all 509 native source entries
against the committed source. No raw licensed Stata logs are copied.

The September 19 experiment also retains an initial smoke worker that expanded
FE values by name. The timed primary and the second saved-output smoke use the
more efficient fitted group codes. See the
[completed report](../reports/development/one-fe-inference-output-breadth-2026-09-19/README.md).
