# ADR 0105: fresh reported-column means from bound numeric sources

**Status:** Accepted for local development
**Date:** 2026-09-22

Reuse the existing numeric source binding when reported-intercept assembly has
neither raw moments nor a score cache. Each selected column still runs the
original maximum-scaled, compensated weighted-mean routine, including its fresh
denominator and original row order. The binding borrows immutable, sample-aligned
numeric slices; an explicit intercept is represented by the existing constant
source. It admits at most 64 columns and at least 4,096 active rows.

Schedule independent column destinations on the installed worker pool when
N>=16,384, selected width>=8 and N*width>=524,288. Otherwise use the serial
bound path. Generated recipes and wider models retain the generic accessor;
existing OLS raw-moment and score-cache paths remain first choices. Validate
column axes before filling the already allocated K-element destination.

This applies to fresh and strict PPML assembly and eligible uncached OLS
assembly. There is no cache across calls or weight generations, no additional
heap buffer, and no new matrix owner. Normalization, projection, rank, residual,
score, final-Hessian and transaction certificates remain unchanged. Parallel
tasks receive Rust-owned slices and never call the Stata SPI.

The [source-bound report](../../benchmarks/reports/development/ppml-fresh-means-local-2026-09-22/README.md)
records 111 successful headline attempts and 45 byte-identical native scientific
pairs. One-FE screen/fresh medians improve 163→144 and 441→388 ms; wider two-FE
improves 491→440 ms. Three-FE is essentially flat. K65 and absorbed-slope
controls have small losses. Retain this inexpensive change without tuning the
cutoffs to those individual cases.

The artifact passes 382 distinct focused Rust tests, the actual-plugin ABI
harness and four licensed fixtures with independent dummy Poisson/OLS and
current-Hessian inference. Tracked peak differences span −0.71 to +3.79 MB;
no exact peak-neutrality or constrained-memory qualification is claimed.
Disconnected PPML stock coefficients fail the frozen large-cell tolerance and
remain unranked. The bounded oracle now independently prunes recursive
singletons and verifies sample identity; its earlier mismatched-sample failure
remains archived. Frozen scientific tolerances and benchmark definitions are
unchanged.
