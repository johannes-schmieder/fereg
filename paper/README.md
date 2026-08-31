# fereg working paper

This directory contains a generic working-paper draft intended for eventual
submission to the *Stata Journal*.  It deliberately does not use the journal's
LaTeX class.

Build the benchmark tables and vector figures from the repository's captured
evidence and then compile the manuscript:

```bash
python3 reproducibility/build_tables.py --repo-root ../
python3 reproducibility/build_figures.py --repo-root ../
latexmk -cd -pdf figures/akm_size_memory.tex
latexmk -cd -pdf figures/akm_core_memory.tex
latexmk -pdf -interaction=nonstopmode -halt-on-error fereg.tex
```

The table and figure builders read existing benchmark receipts only.  They do
not launch Stata and cannot rerun any benchmark.  `make paper` performs all
steps when called from this directory in the fereg repository.

The red “(to be updated)” tags are deliberate release-stage placeholders.  A
submission pass should resolve them after the public release artifact, license
files, release date, exact release commit, and cross-platform qualification
receipts are frozen.

Files:

- `fereg.tex`: manuscript and technical appendix
- `references.bib`: bibliography
- `tables/`: generated benchmark tables
- `reproducibility/examples.do`: self-contained Stata examples
- `reproducibility/build_tables.py`: evidence-to-LaTeX table builder
- `reproducibility/build_figures.py`: frozen evidence-to-vector-figure builder
- `reproducibility/readme.txt`: replication notes for editors and readers
