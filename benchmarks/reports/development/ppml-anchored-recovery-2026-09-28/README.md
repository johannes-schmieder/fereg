# Anchored PPML recovery candidate

The numerical repair retains the original 1e-9 reconstruction certificate. It
adds one conditional native weighted refinement of the traversal remainder,
using the accepted Hessian weights and restoring the same firm anchor in each
connected component. It does not change the PPML optimizer, sample, controls,
weights, score units, RNG streams, correction signs or ordinary fit results.

## Diagnosis

The exact failing pin is `a05882933fb0f1fc668e2a09bed8ff768b7329be`.
The frozen CZ26 sequence passes full-sample PPML, then fails Branch A raw FE
recovery on route 2, before probes. The main PPML optimizer has already passed.
First-edge traversal has maximum scaled error 1.1222825957e-9 versus 1e-9;
17 rows fail. The target varies slightly within repeated pairs, so it is only
approximately additive. The graph is connected, has 51 levels on each side,
2,234 occupied pairs and maximum distance two from the firm anchor. A long or
weakly connected graph does not explain this particular failure.

Independent weighted LSMR and dense SVD solve the same anchored problem. They
agree at the reported f64 precision, reduce maximum scaled reconstruction error
to 6.3879074850e-10 with no failing rows, and leave a normalized weighted normal
residual of 4.18e-17. The largest change to the traversal coefficient is 1.55e-9.
The column-scaled matrix has condition number 3.23. This supports refinement of
a small approximate-projection discrepancy without relaxing the certificate.

Direct full and Branch A calls on the current upstream baseline already pass.
They are not a clean failing-before/passing-after comparison. Evidence for the
repair combines the exact pinned failure, the captured failing target executed
through the candidate recovery helper, independent mathematical oracles, and
complete frozen candidate sequences.

## Implementation and failure behavior

`crates/fereg-core/src/ols/bias_linear.rs` keeps traversal as the fast path and
solves only its finite remainder when the row certificate fails. The accepted
native FE solver supplies a weighted least-squares increment; no dense
observation design is introduced. The row scratch buffer is reused. Component
anchors and retained coefficient products are included in the memory bound.
Every returned row is recertified at the unchanged threshold. Nonfinite inputs,
invalid layouts, allocation failures and inconsistent targets still fail.

Errors carry machine-readable key/value fields for `bias_channel`, `bias_block`,
`bias_probe`, `bias_route`, stage, maximum scaled and absolute errors, threshold,
scales and nonfinite counts where applicable. Block/probe indices are zero based.
Diagnostics travel through the error channel, outside the successful-result wire;
failed transactions retain their existing rollback behavior.

The conditional solve can be more expensive on a large or weak graph. It uses
the accepted solver policy and limits and occurs only when traversal fails.
There is one attempt per failing recovery, not an unbounded retry loop. A genuine
out-of-span target remains a refusal even if its weighted projection is accurate.

## Reproduce without private data

Run `cargo test -p fereg-core --lib anchored_recovery_first_edge_can_amplify_admissible_target_noise`.
The eight-row case uses a repeated 2-by-2 FE design with exact anchored
coefficients `(0.125, 0.25, 0, 0.375)` and paired perturbations of +/-0.75e-9.
Traversal exceeds the 1e-9 bound, while the known rational least-squares solution
meets it. Both native LSMR and retaining MAP refinement are checked.

Run `cargo test -p fereg-core --lib bias -- --test-threads=1` for the focused suite.
It covers cycles, repeated pairs, disconnected anchors, reversed row order,
a 129-level chain with a weak weighted link, large cancelling levels,
nonfinite/overflow inputs, invalid dimensions, genuine inconsistency, lazy
fast-path behavior, allocation errors and correction-channel context. The
private capture test remains ignored by default and runs only on the data host.

## Validation

Local candidate validation passed 28 focused Rust tests, 15 duration
reconstruction/oracle cases and ten complete derivative/curvature oracle cases.
The duration fixtures also assert unchanged ordinary coefficients, VCE, sample
and fitted means. Four licensed Stata 18 fixtures cover correction, rollback,
allocation and runtime validation. A fifth fixture verifies the extracted review
package and ordinary PPML on Mac arm64. Matched Mac/Linux plugins passed binary
and ABI checks; Linux also passed its licensed bias fixture.

The captured failing target passed native recovery in 0.18955 seconds with
maximum scaled error 6.3879074850e-10 and no reported difference from the
independent weighted LSMR/SVD coefficient solution. No row-level captures or
coefficient arrays are distributed.

The affected Branch A call passed with both 4 and 28 native threads, 256 probes
and Stata four processors. Retained rows are identical. Maximum corrected FE
difference is 8.40e-14, fitted eta difference 4.15e-13 and fitted-mean difference
1.52e-15. All nine channels per FE were compared. The predefined comparison
bounds were rtol 1e-8 and atol 2e-9.

All nine original full/A/B calls passed on CZ26, CZ22 and CZ25 in array
7774695, with the original 256 probes and 28 effective native threads. The
largest successful reconstruction diagnostic across those calls was 3.345e-11.
The frozen wrapper's common-anchor steps also completed.

Input validation confirms exact values and row order against the pinned full
and Branch A captures. Full and Branch B contain exactly the same declared
input rows and values; the frozen wrapper explicitly sorts between calls, so
constant cross-call row order is not required. Each call preserves its own
input/output row identity and all eighteen saved-effect sample masks. Retained
counts are 2,049,981 full, 1,016,421 A and 1,033,560 B. No original successful
pinned Branch B capture exists because the pinned sequence stops at A.

The first identity validator mistakenly required full and B to share one order.
Its failure is preserved. The single corrected read-only retest (7774946) checks
exact row values across calls and exact order within each call; no estimation
or data changed for that correction.

## Performance scope

| CZ | Call | Command s | Native excluding correction s | Correction s | Max scaled reconstruction error |
| --- | --- | ---: | ---: | ---: | ---: |
| 26 | Full | 224.816 | 25.412 | 186.472 | 3.345e-11 |
| 26 | A | 117.443 | 13.742 | 89.342 | 1.295e-11 |
| 26 | B | 123.745 | 15.921 | 91.031 | 1.250e-11 |
| 22 | Full | 136.082 | 11.066 | 117.392 | 1.039e-11 |
| 22 | A | 78.036 | 6.874 | 63.488 | 1.224e-11 |
| 22 | B | 73.032 | 6.931 | 57.971 | 1.425e-11 |
| 25 | Full | 98.934 | 8.792 | 84.896 | 1.587e-13 |
| 25 | A | 51.722 | 4.919 | 41.682 | 2.820e-11 |
| 25 | B | 51.399 | 4.888 | 41.424 | 9.900e-14 |

All nine calls used 28 effective native threads, Stata four processors and 256
probes at seed 739391 and streams 0/1/2. Original automatic solver/memory
selection, deterministic mode and 1e-9 certificates were retained.

| CZ | Whole-job wall s | Peak RSS GiB | Peak virtual memory |
| --- | ---: | ---: | ---: |
| 26 | 1560 | 5.164 | 8.028G |
| 22 | 915 | 3.559 | 6.127G |
| 25 | 626 | 2.348 | 4.938G |

The scheduler memory peaks include frozen branch/wage preparation and the
three PPML calls. They do not isolate recovery. All three tasks returned
`failed=0` and `exit_status=0`, with independently checked Stata return codes
and the required all-three-call terminal sentinels.

The native time excluding correction includes preparation, estimation, inference
and publication work; it is not a separately instrumented optimizer-only timer.
`time_fe_recovery` measures ordinary saved-effect recovery. Bias FE recovery is
inside correction time; the captured-target 0.190-second measurement isolates
that repair separately. The thread comparison observed command times of 113.689
seconds at 28 threads and 116.130 seconds at four. Correction occupied 90.338 and
89.005 seconds respectively. These single diagnostic observations do not establish
thread scaling or a general speedup.

## Identity and downstream adoption

This candidate was built from dirty `main` based on
`94459a1420923cffe5bcb87d8e2b54bbdd2d8b4b`, including the preexisting accepted
bias-unit-support work. The build-input manifest SHA-256 is
`e0ce365c88956bdcdc02617cdb34d2e92398f34c1c3ab506adb6b9a9a4e396b6`.
The exact input file mapping and test-only fixture supplement accompany the
receipts. HEAD alone does not identify this candidate.

- Universal Mac plugin: `92a447289b6b8550b06162a81879b17fe3fa70ed3afb21343fdea7f4b6163c1f`.
- Linux x86-64 plugin: `156f63c32e7c462be4eb5ca979f575e1713b5ae20853708a2be90a1c584a3a5b`.

The two platform review bundles contain the same matching ado, compiled Stata
18 Mata helpers and help, with their respective platform plugin. They are
explicitly review-only and ineligible for publication.
PPML request/result wires are 1.8/1.5 because the candidate includes the accepted
support restriction. Do not replace only the plugin in an older ado installation.
The unchanged Cargo.lock passed cargo-audit against advisory database commit
`ef03605143a913024f864d2edf476adad5720c93`; this is a dependency audit, not
clean-source release qualification.

For a separately approved downstream integration:

1. Extract the entire bundle to a new isolated dependency directory. Verify
   `MANIFEST.sha256`, the plugin checksums and `CANDIDATE_SOURCE.json`.
2. Start a fresh Stata session so an older native plugin cannot remain loaded.
   Add that directory first with `adopath ++ "<candidate-directory>"` and confirm
   `which feppml` resolves there. Use Stata four processors and native threads
   within the SCC reservation.
3. Preserve the frozen campaign directory and its outputs. Bind a new integration
   lane to the whole candidate directory and its manifest; do not use the base
   HEAD as a repaired Git pin. Keep the original sample, controls, exposure,
   weights, sc_candidate3 split, 256 probes, seed 739391, streams 0/1/2,
   tolerance 1e-9, iterate 300, solver(auto), memory(auto), deterministic mode,
   keepsingletons, biasunit(persid), biasfe() and eta().
4. Record the new source-manifest and platform-plugin hashes with each result.
   Treat support refusals and numerical failures as errors; do not relax masks
   or tolerances to get a passing integration run.

This work does not qualify Windows, native Intel-host Mac, a formal package
release, or a broad performance matrix. Mac arm64 used Stata 18; SCC Linux used
Stata 19. FEVC and the frozen Separations campaign remain unchanged and stopped.
Failed attempts and wrapper corrections are preserved in the evidence history.

## Evidence index

- [Pinned diagnosis](pinned-diagnosis.json): failing pin, independent recovery, graph and support diagnostics.
- [Local candidate validation](candidate-local.json): unit, dense-oracle and licensed transaction fixtures.
- [Native target and thread comparison](candidate-native-and-threads.json): numerical agreement, runtimes and scheduler evidence.
- [Nine-call sequence results](candidate-sequences.json): aggregate diagnostics and accounting for every regional task.
- [Input identity](candidate-input-identity.json): frozen inputs and within-call output/sample checks.
- [Platform installation](candidate-platform-installation.json): exact Mac runtime bytes and licensed installation test.
- [Attempt adjudication](candidate-attempt-adjudication.json): preserved build/wrapper failures and corrections.
- [Build inputs](candidate-source.json) and [test-only supplement](candidate-test-inputs.json): exact candidate identities.
