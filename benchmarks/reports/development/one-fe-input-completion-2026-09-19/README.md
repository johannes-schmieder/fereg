# Reserved input and one-FE completion scheduling

The adopted candidate removes initialization and repeated traversal work and
starts independent certification earlier. In the last five-pair development
comparison, predecessor/candidate/pinned-fixest command medians are 153/133/141 ms
for ten regressors and 429/390/465 ms for fifty. Both use 983,040 observations,
one ordinary intercept FE, IID inference and 16 native threads on the M2 Ultra.
The protected-control maximum median paired ratio is 1.046802; the geometric
mean across all eleven cells is 0.980149. No control triggers the preselected
5% recheck rule.

The two separately preselected final blocks contain twenty candidate/reference
pairs. Fereg wins all twenty. Block A candidate/reference medians are 142/151 ms
and 407/467 ms; block B gives 135/146 ms and 393/468 ms. Ratios of command
medians clear 5% at both widths in both blocks, but the median paired reduction
in block A's narrow case is only 3.36%. Under the stricter paired-ratio
interpretation the target is **not met**. Keep both blocks; do not replace the
unfavorable block with another draw. The next candidate must deliver more
margin. [acceptance.json](acceptance.json) records both estimands and the
[preselected plan](acceptance-plan.json).

The changes are bounded to existing admitted paths:

- Numeric columns are reserved empty and initialized once by the worker that
  already consumes the two owned input tiles. The same main-thread SPI reader,
  capacity checks, shutdown protocol and ordinary fallback remain in place.
  Optional RHS construction uses those same owned tiles after the authoritative
  FE column is copied. No new observation-scale owner or worker is introduced.
- Full-sample calls without requested observation products skip a row loop that
  performs no stores. Final publication preflight still checks residuals, row
  mapping and drop reasons, with a fused parallel check when the full-sample
  mapping must be the identity. Partial samples retain the original checks.
- Nondecreasing FE codes construct the identical membership index directly;
  shuffled codes use the general constructor. Narrow ordinary one-FE numeric
  preparation overlaps completeness checking with FE encoding. Incomplete
  samples discard speculation before compaction and rebuild authoritative codes.
- The independent unit residual sum uses its unchanged scaled row-order
  arithmetic beside score certification. Successful scalars remain tied to the
  private immutable residual owner; failures return to the original inference
  boundary. Narrow residual projection proceeds into independent score checking
  while reported bread finishes. Exact final slot-status matching gates reuse;
  bread and projection errors retain their original precedence.

The score workspace fits the existing residual-projection overlap allowance:
projection has released its larger 512-row dot bank before the two 16,384-row
score partial banks are constructed. Instrumented allocation-phase paths keep
ordinary scheduling. The input transport keeps its existing two-tile memory
admission and native architecture. The previously documented opaque runtime
coverage gap is unchanged; no new memory qualification is claimed.

The 313 development calls preserve all intermediate candidates. Output-only,
reserved-input, residual-sum, parallel-preflight, ordered-index, preparation
factorization and early-score comparisons appear separately in summary.json.
Parallel RHS validation passed science but lost command time (144 vs 141 ms
narrow in its confirmation block), so it was removed. Its patch and receipts
are retained. The wide 390 vs 399 ms difference between early-score and its
predecessor is outside early-score's width guard and is not attributed to it.
See the separate completion-profile report for diagnostic critical-path timers.

Validation includes exact native scientific comparisons, byte-identical
coefficient/covariance/status/DoF/certificate/fit CSVs, all eight artifact ABI
checks and 24 licensed fixture passes including independent dummy and
transaction oracles. Sixteen focused native test executions pass; one belongs
to the rejected RHS experiment. These cover reserved capacity and failure
shutdown, missing-row fallback, output omission, ordered membership,
preparation thresholds, final preflight and overlap error precedence. The
final residual perturbation test covers 45 configurations. One earlier fake
SPI fixture count was corrected before its passing rerun; that failed receipt
is disclosed in validation.json. Final source hashes match all 509 entries of
the earlyscore source manifest.

This is local arm64 development evidence: 313 development calls plus 40 final
comparison calls. No remote or release qualification, push, branch or installed
package replacement occurred. identities.json binds each source snapshot and
artifact; science.json records return codes, terminal acceptance and numerical
checks. The final candidate plugin SHA-256 is
`421b92cefe2b5dcbf19290193dd0ebbbfe10ceb82060e654ef446cdfccf0a3c4`.
