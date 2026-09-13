# Reporting prototype: bounded local cost review

Baseline: adopted native 7663a87614e37bea73e5dbc037c3ad0c7afccdda and its reviewed
isolated c9db0dceea82dc7b7b87a51979d0fd7ca1f83f1e71f04f034be32312527990f6 plugin.
Candidate: frozen 152-input snapshot and implementation-reviewed.patch in this
directory. Public OLS model reporting remains unchanged: incomplete slope,
saved-recovery and other solver coverage prohibits treating this as a complete
reporting fix or as an adopted optimization.

Run the fifteen predeclared pairs in compare.py: three seeds each for wide weak
OLS, low-memory CMG robust scores, wide CMG and weak PPML checkpoints; single
small weak, complete IID and saved one-FE controls. Rotate arms and use fresh
Stata processes with the existing small warm-up and no phase/native tracing.
Do not run tests or builds during timing. All runs use the existing local
worker, import/sample fingerprints and complete scientific exports.

This change targets faithful attempted-workspace accounting, not faster solves.
Investigate a command increase exceeding max(10 ms, 5%); preserve every failed
or slow pair. Full b/V, sample, status, DoF, certificate and non-time solver
decisions must remain exact in these arithmetic-preserving comparisons. Report
command, input/native/outer time, native peak, modeled bytes and sampled RSS.
Retain all outcomes; these are development diagnostics, not qualification or
cross-platform/comparator performance claims. A passing cost check does not
close the outstanding reporting coverage or opaque-memory-model questions.
