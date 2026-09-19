# Guarded Mac numeric accessor adoption

The Mac numeric column batch now validates observation bounds, numeric variable
types and destination pointers once, then uses the pinned SDK's direct `vdata`
callback. Scalar reads, stores and other platforms keep their checked accessors.
No global fast-mode switch is enabled. Callback failures still stop the private
input load before estimation or publication.

These fresh isolated builds start at `d51558fcf65989589fd3feeebc66f1aff4444cfa`.
Each uses its own Cargo target directory, release optimization, fat LTO, one
codegen unit, unwind containment and an arm64 Mac plugin. The source manifests,
overlays, static-library and plugin hashes are in [identities.json](identities.json).
The accessor plugin is
`1c2397ca2be82cd0ddde40b3b94ab4c9142f0fe83d22763181feaaac0d74e588`.

Three rotated, fresh-process full-command pairs per case give:

| N | K | Native threads | Baseline median | Accessor median | Pinned fixest median |
|---:|---:|---:|---:|---:|---:|
| 983,040 | 10 | 16 | 221 ms | 221 ms | 144 ms |
| 983,040 | 50 | 16 | 561 ms | 528 ms | 471 ms |

The wide improvement is 5.9%; the narrow command is unchanged. This is still
short of the owner's 5% win over fixest on both cases. Eleven protected control
cells have a geometric mean candidate/baseline median ratio of 0.9766 and a
maximum ratio of 1.0145. All 84 accepted calls pass input/sample identity and
scientific checks; native coefficient, covariance, status, DoF, fit and
certificate matrices are bit-identical. Fixest comparisons retain the existing
tolerances. [Measurements](measurements.json), [summaries](summary.json) and
[scientific checks](science.json) preserve the numeric receipts.

Both builds pass the real-plugin ABI/panic harness. The adopted bridge passes
the portable mock and Mac UBSan guard test, including reordered variables,
invalid addresses/types/pointers, integer overflow, missing callback, main-thread
confinement and a fifth-read failure. Three licensed fixtures pass explicit
terminal and fixture sentinels: `ols_native_input`, `candidate_input_oracle`,
and `estimate_ols_eclass_transaction`. These include an independent explicit
dummy oracle, storage/missing handling, restrictions and transaction rollback.
The earlier [accessor experiment](../stata-input-accessor-local-2026-09-19/README.md)
separately compared all five numeric storage types and all 27 missing codes.

This is Mac development evidence, not cross-platform or release qualification.
Raw licensed logs, datasets, source snapshots and binaries remain ignored under
`target/one-fe-optimization-20260919`. No installed package was replaced.
