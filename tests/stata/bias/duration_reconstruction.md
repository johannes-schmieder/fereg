# Synthetic duration reconstruction regression

The compressed CSV contains only test data: the retained 11,520-row
`duration_960` confirmation replication 10131 from the frozen native PPML
campaign. It is not administrative or licensed microdata. Original failed
attempt and its binary remain in the campaign denominator.

Original retained Stata input SHA-256:
`32481498d0ee32cd4d31d67ba9e698337607cb638089aeaec31ab3ee5f0c5544`.
The CSV retains the independent units, row identities, two categorical effects,
controls, fitting weights, exposure, outcome and sample/branch indicators;
17 significant digits preserve floating-point values. Gzip timestamp is zero.

Run `scripts/ppml_bias_reconstruction.py` with a bound plugin, source manifest,
licensed Stata executable and fresh work directory. It checks ordinary-result
identity and independent full-design raw/exact/randomized corrections for
full/A/B fits, including four row orders. Ordinary fitted means near 1e-7
previously exposed cancellation when recovering raw effects from the IRLS
working response. Reconstruction retains the unchanged certificate tolerance.
