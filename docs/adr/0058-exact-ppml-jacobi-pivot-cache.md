# ADR 0058: Reuse exact pivots in wide PPML covariance eigensystems

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-13

PPML covariance production and independent publication inspection retain the
existing maximum-pivot Jacobi algorithm. Above 64 identified dimensions, each
rotation reuses the upper-triangle row maxima instead of rescanning every
upper entry. The selected pivot is exactly the first row and first column
that the exhaustive search would select, including ties and all-zero stopping.

Before a rotation, its two pivot rows and rows whose maximum may be changed
are marked invalid. Invalid rows are rescanned afterward; other rows update
only their two affected upper entries. The cache stores nonnegative absolute
magnitudes and uses their sign bit, including negative zero, as the temporary
invalidation marker. It is local to one matrix eigensystem; no numerical state
is shared between covariance matrices, current-weight frames or estimations.

The already required eigenvalue vector holds these maxima until the final
matrix diagonal replaces them. Its lifetime remains inside the existing
modeled work/eigenvector/eigenvalue overlap. There is no additional heap
allocation, retained workspace, worker pool or SPI call. Up to 64 dimensions
uses the original exhaustive pivot selection. Allocation failures still fail
before any result publication.

Rotation arithmetic, threshold, iteration cap, raw/final eigensystem checks,
CGM clipping and independent reconstruction/orthogonality certificates remain
unchanged. ADR 0016 governs covariance policy; ADR 0056 governs independent
inspection of the actual published matrix. This is neither a new eigensolver
nor permission to drop a certificate or change rank decisions.

The [source-bound local report](../../benchmarks/reports/development/ppml-pivot-cache-local-2026-09-13/README.md)
records exhaustive-pivot comparison tests, known-spectrum/rejection checks,
independent Poisson/Hessian/sandwich oracles, full-command pairs and memory
controls. It establishes bounded local development adoption, not cross-platform
qualification or a general claim for every covariance dimension.
