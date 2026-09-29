# ADR 0009: Ordinary factors follow built-in Stata sample precedence

**Status:** Accepted
**Date:** 2026-09-29

## Context

Legacy validates ordinary factors before outcome missingness; SPEC put outcome
processing first. Johannes requested standard Stata behavior where possible.
Controlled Stata 19 `regress` probes exclude an invalid-factor row when its
outcome/regressor is missing, `if/in` excludes it, or its aw is zero/missing.
Pinned reghdfe 6.14.1 instead fails these cases inside `partial_out` with r(3598).
Fresh-process repetitions confirm the difference. Eligible invalid factors
fail with r(452) under both commands.

## Decision

- Validate ordinary factors after SPEC §2 availability/weight exclusions,
  before singleton pruning, separation and rank decisions. Nonnegative integer
  values are required even for base/omitted terms; eligible violations use
  Stata's r(452). Preserve the original observation identity in diagnostics.
- Do not restrict FE or cluster identifiers to ordinary-factor domains.
- This supersedes the legacy candidate-wide ordinary-factor domain check only;
  the other sample, weight and domain-error rules remain authoritative.

## Consequences

- Shared source preparation applies this rule to both estimators. The Stata
  expansion seam must use the same available sample at M5/M6 integration.
- Engine regressions cover exclusions, masks, original row identity and checks
  before singleton deletion. `reference_factor_precedence.do` reproduces the
  built-in behavior and optionally the pinned comparator difference.
- Legacy/public defaults remain unchanged until their cutover gates pass.

## Alternatives rejected

- Preserving the legacy/reghdfe edge case conflicts with Johannes's preference
  for built-in Stata behavior; deleting invalid factor rows would change the model.
