# Final fallback reporting correction

Preserve all cfeed644 measurements and archives. The final candidate moves Schur
cost/warning merging to after the LSMR-to-MAP fallback message is formed. This
preserves the Schur warning and keeps that LSMR message's iteration count specific
to the LSMR attempts. It changes no arithmetic, guard, memory or iteration policy.
A forced Schur-cap plus double LSMR-certificate rejection test requires both
warnings, two checkpoint retries, certified MAP and unchanged independent projection.

Repeat focused tests/build, the same nine-fit independent oracle, all original
three-seed targets and eleven controls against the original accepted baseline,
explicit MAP references, public memory/saved boundaries and candidate attempt
traces. Reuse original frozen data; no generation or concurrent timing work.
All cfeed644 and revised-source results remain separate, with no pooling.

The new full-fallback test initially fails its unchanged2e-10 independent MAP
comparison. Source inspection identifies stale residual energy: after a capped
attempt, the shared recurrence resized existing scalar banks without clearing
same-length contents. The prototype cleared before its initial attempt but not
the later MAP continuation. Move the scalar reset into the shared recurrence,
which owns its mathematical r-prime-M-inverse-r initialization. Preserve the
failing source, manifest and log; do not loosen the test or any runtime tolerance.
