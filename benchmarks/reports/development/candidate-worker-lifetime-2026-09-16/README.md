# Worker retirement checkpoint

The native cached worker team now owns worker handles, signals Rayon shutdown,
and joins retired scoped teams after releasing the cache mutex. Partial startup
failure also joins successfully started workers. An external asynchronous Rust
caller dropping the final owner on its own worker detaches handles to avoid a
self/peer deadlock; Stata uses scoped work and releases owners on its calling
thread. The [native patch](native-worker-retirement.patch) is in the working tree.

A separate [CMG draft](cmg-joined-shutdown-draft.patch) does the same for CMG's
private scoped executor. It remains isolated: production Cargo manifests and
dependency pins are unchanged. Neither change supplies a byte bound for opaque
runtime queues, TLS or native thread resources.

Job 7607710 compares `5240e58-worker-retirement1` (published CMG) with
`5240e58-runtime-draft2` (isolated CMG patch). Both contain the native retirement
change, so this comparison isolates only the dependency draft. Its copied lock
changes only the CMG source entry; all other versions remain fixed. The earlier
7607694 attempt was canceled after unrelated lock updates were discovered; no
results from it are accepted. Its lock-generation output is preserved locally.

The 28-core reservation executes on four physical cores and finishes in 532
seconds, scheduler failed=0/exit=0, maxvmem 3.343G. Seven native pool tests and
137 baseline plugin tests pass; 38 CMG and 137 plugin tests pass with the draft
(319 executions, not 319 distinct tests). Linux build/export/actual-loader checks,
18 terminal worker sentinels and 36 isolation records pass. Nine alternating
fresh-process pairs preserve exact scientific matrices, sample/order, solver
non-time fields and saved outputs.

| Case | Published CMG median seconds | Draft median seconds |
| --- | ---: | ---: |
| Small OLS | 0.073 | 0.081 |
| Frequency-weighted clustered OLS | 0.222 | 0.206 |
| PPML | 0.279 | 0.273 |

No command or reported phase exceeds both 5% and 10 ms slowdown. Detailed
allocation/RSS/scratch, phase and fallback observations are in
[phase-costs.json](phase-costs.json). This is a bounded development screen and
makes no general performance claim. Artifact SHA256 values are
`07bca07ac2be3c66d982ef5a5f4a3422dcdc31ef73e36a69b71a92807496211d`
(published CMG) and
`fd5da43cae056689f7ba372996c352c911b89abd41b498020a5f1252ff720937`
(draft). Receipts bind source, inputs, reference pins and original log hashes.

The [validation record](validation.json) explicitly keeps whole-action coverage
incomplete, its ratio undefined and publication eligibility false. No upstream
push, dependency adoption, release or installed-package replacement occurred.
