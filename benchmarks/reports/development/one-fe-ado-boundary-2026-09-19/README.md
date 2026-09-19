# Refine the Stata command boundary

Two primary pairs per width reuse the exact factor candidate native artifact
with diagnostic ado timers. All eight calls preserve native scientific output
exactly. Syntax takes 0–0.5 ms and public coefficient names about 0.5–1 ms, so
neither accounts for the large remaining boundary cost.

| Timer | K=10 median | K=50 median |
|---|---:|---:|
| Before plugin | 9 ms | 28 ms |
| Plugin, including stores and cleanup | 172.5 ms | 405.5 ms |
| Validation and names | 0.5 ms | 2 ms |
| Ereturn post, including buildfvinfo | 2.5 ms | 15 ms |
| Remaining publication | 0.5 ms | 1 ms |

The full instrumented commands take medians of 189 and 473 ms. About 4 and
21.5 ms respectively lie outside the five transaction timers, including program
entry/exit and the public wrapper. Temporary-variable lifecycle costs are a
hypothesis for further profiling, not an established result.

The earlier eight `outerdetail` calls verified scientific parity but did not
export the timer values: the controller does not retain captured Stata output.
A separate `outerdetail2` snapshot writes numeric timer rows to each run's
working directory. Those first attempts remain local and are not used for the
phase table. Timer file writes are diagnostic overhead, not an adopted change.
No additional licensed fixture set or qualification was run for instrumentation.
