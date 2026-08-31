PYTHON ?= python3
LATEXMK ?= latexmk

.PHONY: paper tables figures clean

paper: tables figures
	$(LATEXMK) -pdf -interaction=nonstopmode -halt-on-error fereg.tex

tables:
	$(PYTHON) reproducibility/build_tables.py --repo-root ..

figures:
	$(PYTHON) reproducibility/build_figures.py --repo-root ..
	$(LATEXMK) -cd -pdf -interaction=nonstopmode -halt-on-error figures/akm_size_memory.tex
	$(LATEXMK) -cd -pdf -interaction=nonstopmode -halt-on-error figures/akm_core_memory.tex

clean:
	$(LATEXMK) -C fereg.tex
	$(LATEXMK) -cd -C figures/akm_size_memory.tex
	$(LATEXMK) -cd -C figures/akm_core_memory.tex
