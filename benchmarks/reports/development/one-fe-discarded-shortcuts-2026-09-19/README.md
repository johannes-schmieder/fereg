# Shortcuts not adopted

Two additional prototypes preserved numerical results but did not show enough
repeatable full-command benefit to retain their extra code.

* **Plain numeric parser.** A guarded branch bypassed FV expansion and per-token
  `_ms_parse_parts`, constructing identical model-wire metadata. Sixteen metadata
  cases and three licensed fixtures passed. An initial three-block screen gave
  wide medians of 499/495 ms baseline/candidate. Five fresh pairs instead gave
  495/499 ms, with four of five candidate calls slower. Narrow confirmation
  medians were both 214 ms. The branch was removed.
* **Contiguous FE projection.** A native kernel used the existing scaled,
  compensated group-mean routine, then immediately centered the same group's
  rows. The ordinary separate certificate remained unchanged. Focused tests
  matched the general projection bit for bit across 1/12/52/64 RHS columns,
  large/small magnitudes, constants and tail groups. Three paired full-command
  blocks remained within run noise: narrow medians 206/203 ms, wide 495/489 ms.
  The directly measured initial-absorption gain was only about 0.6–0.7 ms, which
  does not justify this additional specialization. The kernel was removed.

All 56 measured calls passed the existing input, sample and scientific checks.
Numeric timings, source identities, comparison receipts and the final prototype
patch are preserved here. Two test-only compiler corrections followed the
contiguous release build; production kernel source is identical. The first
parser-audit script failed due to Stata's local-name length limit and was repaired
before the successful audit. Failed attempts remain in the ignored work root.
No scientific tolerance changed and no installed plugin was replaced.
