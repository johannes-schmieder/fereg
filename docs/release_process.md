# Release process

1. Freeze `AGENTS.md`, the compatibility matrix, and exact upstream commits.
2. Run repository checks, Python oracles, strict C/mock-SPI tests, formatting,
   clippy with warnings denied, all Rust tests, and appropriate sanitizer/Miri
   or fuzz jobs.
3. Build on target-native CI: manylinux 2.28 x86-64, Windows MSVC x86-64, and
   macOS x86-64 plus arm64 slices.
4. Audit architecture, required exports, symbol visibility, OS/runtime baseline,
   and dynamic dependencies. Reject an external numerical/runtime side library.
5. Dynamically load the final Rust-linked artifact on the build host. Require
   `pginit`, version, doctor with 32 threads, protocol rejection, and panic
   containment to pass before writing its SHA-256 file.
6. Run clean-machine Stata 18+ installation, repeated reload, doctor, selftest,
   parser, and fail-closed command tests on each target.
7. For estimator releases, run the full parity, identity, property, and
   large-scale suites and enforce the corresponding scientific phase gate.
8. Assemble the cross-platform release only from the three checksum-verified
   native artifacts. Produce deterministic release/source ZIPs and preserve all
   logs, environment metadata, and benchmark reports.
9. Update help and changelog only with features that passed their gate.

A binary built with `target-cpu=native`, an unpinned reference, a missing binary
or loader audit, or a skipped licensed-Stata smoke test is not a release
artifact.
