fereg paper reproducibility notes
=================================

The manuscript is a generic working paper prepared for eventual Stata Journal
submission.  It reports only captured benchmark evidence.

Benchmark tables
----------------

From paper/, run

    python3 reproducibility/build_tables.py --repo-root ..

The script performs read-only checks on two frozen evidence bundles:

1. benchmarks/reports/captured/linux-x86_64/stata19/adr13-qualified/
   fma-repair-5/
2. benchmarks/reports/captured/linux-x86_64/stata19/adr14-vce-qualified/
   contiguous-repair-8/

It checks the expected source and plugin identities, success/preflight counts,
and table structure, then writes tables/benchmark_ols.tex and
tables/benchmark_vce.tex.  It never invokes Stata, qsub, or a benchmark driver.
The performance comparison printed in the paper is limited to fereg and the
pinned reghdfe 6.14.1 comparator.

Benchmark figures
-----------------

From paper/, run

    python3 reproducibility/build_figures.py --repo-root ..
    latexmk -cd -pdf figures/akm_size_memory.tex
    latexmk -cd -pdf figures/akm_core_memory.tex

The builder verifies the registered paper-akm-scaling-memory-v1 bundle,
including all file hashes, 115 fresh-process measurements, 23 complete cells,
five repetitions per cell, and 11 scientific preflights.  It then writes two
PGFPlots sources.  The LaTeX commands create vector PDFs; neither step invokes
Stata, qsub, or a benchmark driver.

Examples
--------

reproducibility/examples.do contains small synthetic examples corresponding to
the manuscript.  They are demonstrations rather than benchmark workloads.
They require a working fereg installation for the host operating system and
Stata 18 or newer.

Release-stage fields
--------------------

Red “(to be updated)” tags in the manuscript identify fields that should be
frozen for submission: public release date and URL, exact release commit and
artifact checksums, GPL-3.0-only file synchronization, and completed platform
qualification.  The source commits and plugin checksums attached to the two
reported benchmark exercises are already fixed and must not be replaced by a
later source identity.
