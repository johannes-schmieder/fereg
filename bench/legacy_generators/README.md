# Preserved generator sources

These byte-identical copies come from the legacy snapshot for migration into
the M1.4 suite. The accompanying helper modules/manifests preserve generator
context; the old benchmark entry points remain in benchmarks/ until M7.
Do not execute the copied controllers as new campaigns: paths, staging and
contracts still describe the original layout. Port the generation functions
and declared designs into bench/generate.py with fixed-seed identity checks.

Original sources: benchmarks/systematic/ and benchmarks/two_fe_dense/ at
legacy-v0.5.0-beta (e61cb9ee). Real-data scripts copied under bench/real/veneto/
have the same migration-only role; no real dataset is copied.
