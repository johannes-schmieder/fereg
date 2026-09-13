# ADR 0055: Bounded caller-local estimation request transport

**Status:** Implemented for user-authorized local development
**Date:** 2026-09-13

Keep the existing canonical framed OLS/PPML request inline through 4,000 payload
bytes. For longer metadata, the ado transaction retains the payload in its local
`fereg_request_bundle` and passes only `bundle_local=<byte-count>` to the same
substantive plugin call. The native adapter reads that fixed local through the
existing main-thread SPI bridge. No arbitrary macro name, global state, second
plugin call, scientific wire version or public option is introduced.

The descriptor is the only action argument and uses a canonical positive decimal
byte count. Before reading the macro, Rust checks a 16 MiB metadata capacity and
fallibly reserves one buffer for the payload, existing decoder prefix, one extra
content byte and the terminator. The SPI character limit excludes the terminator;
the extra content byte detects truncation of an overlong macro. The first NUL
must occur at the declared byte count. Missing, short, long, non-UTF-8 or malformed
frames fail before observation reads or publication. Existing closed request
decoders, exact handshakes and result validation remain authoritative.

The transport buffer is released after parsing and before source/output metadata
validation and observation loading. Its bound is independent of N and finite
rank; PPML still creates no observation-sized preparation state before admission.
The 4,000-byte crossover is a conservative local transport choice, not an asserted
universal Stata limit or an estimator width limit. Stata's own macro capacity may
be lower than the native metadata cap.

The [source-bound report](../../benchmarks/reports/development/request-transport-local-2026-09-13/README.md)
records the long-name OLS/PPML independent oracles, rollback, malformed framing,
SPI terminator review and paired cost controls. It also preserves the separate
wide PPML warm-up covariance validation failure exposed after transport succeeds.
This repair does not accept the wider moment prototype or change PSD policy.

The [Stata SPI documentation](https://www.stata.com/plugins/index.html) specifies
macro truncation and the additional terminator capacity. Tests must model those
semantics rather than assume that an undersized destination is rejected.
