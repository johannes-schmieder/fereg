# Implementation status

This branch is developed in small, auditable checkpoints.

## Completed source checkpoints

- Phase 0-1 architecture, metadata parser, protocol, mock SPI, diagnostics, and cross-platform build machinery.
- Native authoritative sample preparation, weight validation, compact factor sources, interactions, and blockwise model-matrix generation.
- Queue-based singleton pruning, connected components, weighted intercept/slope projection, symmetric MAP absorption, and scale-stable FE orthogonality certificates.
- Pinned `within` modified-LSMR/Schwarz adapter, per-estimate Rayon pools, deterministic automatic MAP/LSMR routing, and certified fallback.

## Current validation boundary

The numerical Rust source is compiled and tested in GitHub Actions on the pull-request branch. Licensed-Stata parser, plugin-loading, reference-parity, and performance certification remain separate gates. Unsupported estimator entry points remain fail-closed until those gates are implemented and exercised.
