# ADR-0001: one self-contained plugin per platform

**Status:** accepted.

All Rust and C native code is linked into one `.plugin` file per platform. The
macOS artifact is universal. This avoids sidecar discovery, ABI drift, runtime
installation requirements, and clean-machine failures. The cost is larger
binaries and target-native release builds, which are acceptable.
