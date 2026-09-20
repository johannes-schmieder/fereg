# Weighted two-FE final projection and recovery

This is the first implementation checkpoint of the broad two-way OLS plan.
It extends accepted-probe reuse and certified graph recovery to supplied
weights, preserving all tolerances, weighted normalization, pointwise output
checks, memory admission and fallback behavior. See [ADR 0077](../../../../docs/adr/0077-weighted-final-projection-and-graph-recovery.md).

Measurements are local Mac development evidence on the M2 Ultra, at sixteen
native workers and N=983040 unless noted. Stock fixest 0.14.2 and the baseline
plugin are pinned by hashes. Commands run sequentially in fresh processes with
a small warm-up and rotating arm order. Import/export is outside the command
timer; native input loading, inference and requested outputs are inside.
No build, test, audit or data generation ran concurrently with a timed command.

## Main comparisons

All rows below contain three repeats. Values are median command milliseconds;
ratios are medians of paired ratios, not ratios of medians.

| AW K=6 request | Baseline | Candidate | Default fixest | Candidate/fixest |
|---|---:|---:|---:|---:|
| Ordered IID | 329 | 327 | 481 | 0.6743 |
| Ordered saved FE | 980 | 447 | 717 | 0.6220 |
| Ordered FE, residuals, xb, xbd | 1013 | 493 | 706 | 0.7003 |
| Shuffled IID, fresh seed | 372 | 371 | 307 | 1.1987 |
| Shuffled full outputs, fresh seed | 1167 | 546 | 546 | 0.9907 |

All nine ordered candidate/reference pairs win. Shuffled IID loses all three;
shuffled full outputs win two of three. The shuffled loss remains an open
operator/probe target rather than being excluded from the broader milestone.

Default fixest saved outputs miss the unchanged tolerance in nine comparisons.
Six separate matched-accuracy calls set both fit and FE recovery tolerances to
1e-11; all AW saved/full outputs and PW two-cluster saved-output comparisons at
K=6/50 pass. Their timings remain separate from the default-reference claims.

## Attribution and controls

Separate phase calls localize strict weighted final projection to 237.4 ->
45.4 ms and FE recovery to 369.2 -> 37.4 ms. Initial absorption remains about
138-141 ms. The final projection reuses the accepted initial probe decision,
not its certificate; graph recovery certifies the complete final target.

Single native control pairs show FW full outputs at K=6/50 improving
999/1700 -> 494/1096 ms, and FW two-cluster saved outputs 1170/1780 -> 627/1266
ms. FW robust fits without outputs are 353/1053 -> 349/1069 ms. Ordinary
fixest weights are not a frequency-semantics comparator.

At N=245760, dispersed AW IID is 234 -> 234 ms and full outputs 1273 -> 700 ms;
sparse AW IID is 131 -> 129 ms and full outputs 1360 -> 583 ms. Graph recovery
can help even when the initial Schur probe rejects. These one-pair checks are
diagnostics, not a complete regression or graph-performance gate.

None of the 25 native pairs increases native allocation peak. The measured
AW K=6 full-output candidate peak is 227956853 bytes. No new production heap
owner is introduced.

## Scientific checks and provenance

- Twelve focused Schur tests pass, including actual-weight probes, charged
  continuation, strict final projection and overlap controls.
- Three graph-recovery tests pass: unit connected/disconnected recovery,
  exact memory admission for unit/supplied weights, and a separate weighted
  analytical normalization oracle. The last covers both component structures,
  reversed rows, selective outputs, constant/moderate/tiny weights and common
  scales 1e-100, 1 and 1e100. Inconsistent cycle targets fail even on tiny edges.
- The candidate passes the actual-plugin ABI/panic-containment check and the
  narrow weighted explicit-dummy/frequency-expansion fixture, multi-FE input
  oracle fixture and e-class transaction fixture, with terminal sentinels.
- Twenty-two large independent saved-output audits pass group constancy,
  weighted component normalization, reconstruction, RSS and normal equations.
- All 82 application calls succeed. Numerical reference-output disagreement
  is recorded separately from application status and never labeled a science pass.

Baseline is commit e9d593bdff116373fd09496d008be71d01b0a726; its reused artifact
matches all 514 captured source entries. Candidate `weightedfinish` uses this
commit plus the three core files listed in `identities.json`; all 514 captured
entries match the worktree at archival. Plugin SHA-256 is
732ec4b95ed7a37aea70abfd65485b443a5788ba7e8218d28de43ead689cf90e.
The unchanged build helper embeds September 19; measurements occurred September
20. Identity, implementation patch, source audit and scientific receipts bind
the actual artifact independently of that embedded date.

The copied harness includes subsequent graph-inventory support. Per-call
harness/controller hashes identify the actual executed versions; early calls
use the committed e9d593bd harness. Later calls explicitly request the existing
fast memory default. The staged controller remains fixed throughout this screen.
No installed package, remote state or release qualification was changed.
