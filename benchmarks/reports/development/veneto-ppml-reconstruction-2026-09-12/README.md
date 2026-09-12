# Veneto feppml reconstruction repair

The repaired `feppml` converges on all 3,159,720 Veneto observations at both four
and fourteen native threads. Both fits pass the preserved ppmlhdfe slope,
standard-error and full slope-covariance comparisons. The original failure
occurred before the first PPML update; the repaired fits complete 15 outer
iterations and 589 inner iterations with their final score and Hessian checks
intact.

## Change

The retained weighted-linear session had treated cancellation between a large
fixed-effect predictor and residual as a reconstruction failure. The repair
uses a component-scaled floating-point rounding bound derived from the number
of arithmetic operations. Canonical PPML predictors now use `offset + (z - r)`
directly, avoiding a split and recombination of the fixed-effect component.
The existing response buffer is borrowed, with no new observation-sized array.

The response-relative diagnostic and final publication tolerance remain
unchanged. Sample, model, weight semantics, rank, VCE, degrees of freedom,
solver schedule, score tolerance and line search are unchanged. Public one-shot
OLS does not use the retained session. The [numerical contract](../../../../docs/ppml_reconstruction.md)
derives the rounding bound and documents the independent exact-rational oracle,
corruption controls, and nonfinite/overflow rejection.

## Focused full-panel verification

SCC job **7536374**, on `scc-gr4`, completed with scheduler `failed=0` and
`exit_status=0`, both Stata completion sentinels, and the job completion sentinel.
Each fit starts a fresh Stata process and uses a CPU affinity limited to its
requested native thread count; Stata MP itself is capped at its four-processor
license. The job reserved 14 slots and 42 GiB, with a 22-minute job limit and
10-minute per-fit limits. Total job wall time was 238 seconds.

| Native threads | Command seconds | Native allocation peak, GiB | Largest slope gap from ppmlhdfe | Final relative score |
|---:|---:|---:|---:|---:|
| 4 | 133.324 | 5.958 | 4.59e-12 | 1.27e-14 |
| 14 | 101.878 | 7.374 | 4.55e-12 | 2.08e-14 |

Memory is the native allocator's measured peak, not whole-process RSS. Scheduler
accounting reports a job maximum virtual memory of 8.721 GiB. These are single
diagnostic fits; they do not establish a general performance or scaling claim.

The largest relative differences from ppmlhdfe are 2.37e-8 for standard errors
and 4.73e-8 for the full 4-by-4 slope covariance. Acceptance uses the existing
benchmark's slope tolerance `atol=1e-10, rtol=1e-7` and SE/covariance tolerance
`atol=1e-10, rtol=1e-6`. Cross-thread slopes differ by at most 4.37e-14.

Both fits retain every input row, report no separated observations or
singletons, use no solver fallback, and preserve the residual degrees of
freedom. Their reported likelihood is about 0.001199 higher and deviance about
0.002398 lower than the saved ppmlhdfe fit; the relative objective differences
are below 8.30e-11. The final strict solve, rank freeze, two-consecutive-iteration
condition, and PSD checks pass. This is a focused regression check against
preserved numerical results, not full qualification or an independent proof of
the large-panel optimum.

## Other validation and identities

- 94 Rust tests pass: three reconstruction/oracle tests, 31 PPML driver tests,
  and 60 retained/prepared-fit tests.
- The rebuilt universal macOS plugin passes its binary audit and actual arm64
  ABI/self-test. Four licensed fixtures pass: OLS smoke, PPML smoke, PPML
  scientific contract, and PPML weighted inference.
- Linux build, actual-plugin ABI/self-test, and the standard PPML development
  smoke pass in job **7536367**. That attempt subsequently stopped before any
  full-panel estimation because the new controller omitted `campaign/` from
  the saved-reference path. The corrected controller in **7536374** reuses
  exactly the same tested plugin; the first attempt remains preserved.
- Strict Clippy reports six pre-existing lints in unchanged OLS/FE files and
  none in the modified files. It is not reported as passing.

Base source: `79f1ba6250c539bd34f91eb551e02791f4f01758`.
Source snapshot SHA-256:
`659924a603a3b750a7478cad608d95022166b5a83956e066d9b9e29698420b96`.
All 484 source/fixture snapshot files were checked against the final working
tree. These receipts identify the tested source snapshot independently of its later commit.

Linux plugin SHA-256:
`cfcd6d457197c958b0fd4a776e511b6deb4b3a04e731c886cd24e889c1b394b0`.
Universal macOS plugin SHA-256:
`4187efdfbd6235e63c369d9b42dede6381b8a9e50b5360fd63ee237bd6c11b08`.
Frozen input SHA-256:
`92062a6bccaa7b55c3a213e288cf0d22e68668e21f454119ac168dbd938cb08d`.

Machine-readable evidence: [SCC validation](scc-validation.json),
[full numerical receipts](scc-results.json), [source manifest](source-manifest.json),
[Rust tests](local-tests.json), [Mac fixtures](local-licensed.json),
[first attempt](first-attempt.json), and [corrected submission](retry-submission.json).
Submission captures omit machine-specific run paths and retain hashes of the
original captures. Raw logs remain in the ignored `Veneto_Benchmark/work/ppml-fix/` directory and
the corresponding SCC development directories. Historical benchmark reports
and failure receipts remain unchanged; the stopped campaign was not restarted.
