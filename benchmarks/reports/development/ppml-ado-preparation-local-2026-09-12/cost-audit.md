# P1 PPML wrapper and completeness-pass audit

Baseline: `8be388e280eb64af5de9830204a63bee8de3b33d`.
The experiment changes three ado entry points and uses the identical native
plugin in both arms. Its native source matches the previously measured final
reference-pass snapshot across all 150 tracked native source/build inputs.

| Boundary | Baseline work | Candidate behavior and retained authority |
|---|---|---|
| `feppml` → `_fereg_ppml` → `_fereg_estimate_ppml` | Three nested `sortpreserve` scopes | Remove automatic order bookkeeping. None of these wrappers or the shared metadata parser sorts observations or constructs an ado design. Original row order and stored sort metadata are checked on success and injected failure. |
| Public weight expressions | Candidate marker, expression evaluation into one temporary raw weight variable | Unchanged. Retain arbitrary legal FW/PW expressions and `if`/`in` gating; native code receives the same raw values. |
| Candidate membership | `marksample, novarlist`, parser metadata and private sample shadow | Unchanged. Native original/candidate/active row classification and domain precedence remain authoritative. |
| Whole-candidate completeness hint | An N-byte Stata byte variable copied from `touse`, then `markout` over outcome/source/weight/adjustment and a `count` scan | Remove these passes and send the existing optional `input_complete=0` hint. No public scientific count or output validation is removed. |
| Whole-input numeric SPI | Batched source loading always classifies missing values; a true completeness hint also scans loaded missing flags and can reject disagreement | Same batched loader and missing classification. False hint skips only the redundant assertion. Native callers may still supply a true hint and retain its consistency check. |
| Restricted or string input | Candidate-gated scalar SPI and native string coding | Unchanged. Adjustment loading remains separate, scalar and candidate-gated. |
| Offset/exposure and weights | Allocation-free preflight classifies loaded values; the admitted owner materializes the exact ledger and canonical adjustment/weights | Unchanged. Missingness, finite/domain checks, exposure logarithm, original-row errors and capacity bounds are native rules. |
| Publication and failures | Held prior estimates; private output shadows; independent receipt/sample/DoF/count validation; final protected commit | Unchanged. Requested output names, saved effects and prior/null estimates remain transactional. |

The eliminated explicit Stata marker payload is N bytes when `candidate_all`
is true (983,040 bytes on the large primary). `markout` refers to every bound
outcome/source/weight/adjustment column and `count` visits its marker. These are
source-level pass/payload observations, not hardware byte-traffic measurements;
Stata's internal short-circuit behavior and automatic sort-preservation storage
are not assumed. Native live allocation peaks and sampled process RSS are
reported separately. This removes neither a native scientific check nor an IRLS
pass, matrix or certificate.

Complete-command minus native total time locates the outer Stata cost. It is
computed for each paired call; component medians are independent and need not
sum. The unchanged binary and exact scientific/solver exports prevent a native
route change from being mistaken for an ado improvement. No explicit parallel
work, solver setup, retry, fallback, generation or memory model is changed.
