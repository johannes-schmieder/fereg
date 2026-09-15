# PPML saved-effect recovery uses the admitted worker team

The completed PPML estimator called saved-effect recovery from its serial
composition boundary. Recovery's Rayon CG/normalization could consequently use
the global default or an enclosing caller pool, rather than the fit's admitted
worker count. The fix installs recovery in the cached team using the effective
count recorded by the accepted fit. A guard rejects a mismatched count before
recovery allocation; no-save calls retain their direct fast path. Interruption
and SPI stay at the serial boundaries before and after this Rust-only work.
No tolerance, Hessian weight, sample, rank or reconstruction rule changes.

[The patch](implementation.patch) is an isolated change on `4d0bd5b`, excluding
the larger uncommitted recovery/solver-memory model. [Source identities](identity.json)
include locked native inputs and test fixtures. A three-worker enclosing pool
exercises saved recovery at requested counts 1, 2, 1. The diagnostic unrouted
control, with the guard but without the fix, rejects the mismatch as expected.

The first wrapper (7586289) correctly rejected that control, then accidentally
selected its cached executable again for the candidate's single-package test.
The build had produced a different, correct combined-feature candidate artifact.
Both hashes and the failed run remain in [validation](validation.json). This
violated the repository's existing separate-target rule; it is not accepted as
a candidate result. One focused wrapper correction (7586299) verifies and runs
the correct compiler-reported candidate executables directly. All **55 prepared
estimator tests** and the isolated saved-FE allocator test pass, with explicit
sentinel and [scheduler accounting](scheduler.json). Future combined checkpoints
use a separate target directory and run only compiler-reported hashed executables.

The allocator test repeats a generated-control/frequency-weight/clustered saved
FE fixture at 16,384 rows, using fast/1, low/4 and fast/1. All nine measured
inference, recovery and publication peaks remain within the corresponding
estimates. The fake SPI cells and event log are allocated before measurement;
no licensed estimator or external empirical data is involved. This shows the
focused boundary is bounded; it does not establish full action coverage or a
complete-command speed improvement. Broader memory accounting, paired command
controls and a frozen Mac/Linux candidate remain open. The installed plugin is
unchanged; no package, push, tag or release was performed.
