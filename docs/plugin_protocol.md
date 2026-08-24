# Plugin action protocol (separate from model wire)

The C ABI exports only Stata's required `pginit` and `stata_call` symbols.
`stata_call` forwards `argc/argv` to Rust's `stfixest_dispatch`.

The plugin's current action handshake is version `1.1`. Minor 1 adds only the
guarded prepared-OLS shadow-result transport. Existing diagnostic callers at
version `1.0` remain compatible under the minor-version rule, while the hidden
transport requires exact `1.1`. The expanded-model metadata wire is
independently versioned at `2.1`, and the prepared-OLS result wire is separately
versioned at `1.1`; none of these numbers is an alias for another. An action
handshake never performs a lossy model- or result-wire conversion.

## Actions

### `version`

No handshake is required. The plugin emits machine-readable lines containing
plugin/ado versions, protocol version, SPI ABI, source commit, and build date.

### `doctor`

Required fields:

```text
ado=0.1.0-phase01
protocol_major=1
protocol_minor=0
stata=18.0
threads=0
plugin_file=<resolved path>
```

The `protocol_major`/`protocol_minor` input keys are retained for compatibility
with the Phase 0 ado loader. They identify the **action handshake**, not the
model wire. Machine-readable `version` and `doctor` output uses the unambiguous
labels:

```text
stfixest.action_protocol=1.1
stfixest.model_wire=2.1
stfixest.ols_result_wire=1.1
```

The Phase 0 ado diagnostic caller may continue to send protocol 1.0 until a
later public protocol promotion.

The action validates compatibility, resolves the local thread count, runs the
native self-test, and prints OS/architecture/CPU/thread/dependency metadata.

### `selftest`

Accepts the same handshake fields and runs protocol, factor, interaction,
metadata, and parallel-repeatability tests.

### `panic-test`

Requires `confirm=panic-boundary-test`. It injects a panic solely to verify that
`catch_unwind` returns code 3499. The action is never called by production ado.

### `estimate-ols`, `estimate-ppml`

Return `r(498)` in this phase.

## Developer-only preparation action

`prepare-ols-test` is an internal integration action, not an estimator action.
It requires `confirm=developer-preparation-test`, the standard action
handshake, model-wire 2.1 matrices and length-delimited registries, an explicit
numeric binding matrix, and three pre-created diagnostic output variables. The
binding roles are candidate (`1`), outcome (`2`), numeric model source (`3`),
optional analytic weight (`4`), optional frequency weight (`5`), and optional
probability weight (`6`). Importance-weight role `7` is deliberately undefined
and rejected before observation reads. At most one weight binding is permitted.
Inputs and outputs must cover the plugin varlist exactly.

Role `3` describes numeric storage rather than continuous term semantics. One
source can therefore serve `c.g`, `i.g`, multiple categorical interactions,
and one or more supported fixed-effect terms without a duplicate binding, SPI
read, compact column, or encoded-factor table. The action supports plain
continuous terms, numeric factor main effects, numeric all-categorical
interactions, and unnamed single-source intercept-only numeric fixed effects,
with optional analytic, frequency, or probability weights. Fixed-effect source
values may be any finite `f64`, including negative and fractional values;
ordinary factor-variable sources retain the finite nonnegative-integer rule.
Categorical-by-continuous and continuous products, time-series terms, string
sources, absorbed interactions, absorbed slopes, saved-FE outputs, clusters,
an owned solver `FeDesign`, and estimator execution remain unsupported. The
action does perform the reference-compatible singleton fixed point for its
supported absorbed terms.
The strict action-level `keep_singletons` flag defaults to false in the ado
helper and is not model-wire metadata. Pruning runs only after reasons 1--5 and
weight validation have selected the initial compact sample. Unweighted,
analytic-weight, and probability-weight requests use physical row counts;
frequency weights use checked positive-integer multiplicity, so a one-row
level with frequency weight two is not a singleton. Deleted rows receive
reason 8, and the row map, outcome, every source, and any weight are compacted
together in original order. Categorical sources are then factorized again on
the final sample so no empty level or stale code survives. `num_singletons`
counts physical deleted rows; `keepsingletons` is an exact no-op. The action
never posts `e()` results. Stata's model wire owns expanded ordinary levels,
bases, omission, and coefficient slots. Rust retains omitted-base slots without
generating numerical columns and factorizes each active categorical source
deterministically. All-categorical interactions are small ordered equality
recipes over those codes; no observation-scale interaction column or tuple
table is allocated. A level or tuple that existed in the candidate expansion
but disappears after native sample preparation becomes an explicit all-zero
recipe, not an error. Prepared fixed-effect recipes retain only the term id,
label, shared source id, and indices into the deduplicated categorical-source
and prepared-topology tables. The action builds one fallible compressed
membership index per unique final FE source, validates exact code/membership
alignment and the singleton policy, and emits only small topology counts. It
does not copy observation-scale codes or construct an owned solver `FeDesign`.
Empty final diagnostic samples retain zero-level FE topology sources and remain
valid preparation results when the numerical check below is not requested.

The optional action key `run_borrowed_map=1`, exposed only as the hidden ado
option `mapcheck`, adds a developer-only pre-write numerical check. It defaults
to false. The check validates the entire prepared object and constructs one
fallibly reserved, checked row-major `N x (1 + K)` absorption batch directly:
column zero is the prepared outcome and each later column is one generated
model column in exact prepared-model order. Syntax-omitted coefficient slots
are absent; the generated intercept remains last. No intermediate
observation-scale model columns, FE-code copies, membership copies, weight
copies, or unweighted vector of ones are created. The check verifies the
coefficient-to-generated-column mapping and continuous, categorical, and
all-categorical-interaction recipe/source identities before calling the strict
borrowed-intercept MAP solver. It uses the already prepared analytic,
frequency, or probability weights as the objective; importance weights remain
unsupported.

The developer check uses fixed private settings (`tolerance=1e-10`,
`max_iterations=20000`, `check_every=1`, symmetric sweeps). These are not
public estimator defaults or user-configurable estimator options. With no
explicit fixed effects, a nonempty batch performs the virtual one-level
structural-constant projection and reports `map_iterations=1`,
`map_stop=certified`. This behavior is locked by both Rust and actual-plugin C
loader tests. An empty
prepared sample remains a successful result when the flag is absent, but
`run_borrowed_map=1` rejects it with invalid-data code 459. Allocation,
capacity, arithmetic-overflow, data/recipe validation, interruption, solver
nonconvergence, and certificate failures all occur before the first diagnostic
store and before any display. The last interruption check is also before that
first store. Success discards the residualized batch and displays only small
row/column/iteration/certificate metadata in addition to the existing three
diagnostics; it writes no coefficient, covariance, FE, or other numerical
estimation result and posts no `e()` state.

Model-wire 2.1 adds `absorb_term_matrix`, `absorb_term_count`,
`absorb_component_matrix`, `absorb_component_count`, `absorb_labels`, and
`absorb_saved_names`. Empty tables use the standard one-row all-zero sentinel;
nonempty label and saved-name registries are byte-length framed. Model-wire 2.0
is accepted only with empty absorbed tables and registries. All absorb-table
cardinality, ordering, role, source-range, registry, variable-type, binding,
and output validation occurs before any observation read. The developer ado
also rejects string sources with `r(498)` before `plugin call`, because Stata's
plugin varlist otherwise rejects them first. Unsupported FE shapes and string
FE sources therefore return `r(498)` before reads or diagnostic writes.
Weights must be passed as one numeric source binding. A negative candidate-row
analytic weight returns pinned code 402. Frequency weights must be finite,
strictly positive integral values representable as `u32`; a domain violation
returns pinned code 401. An explicit Stata-missing mask is a reason-4 exclusion.
Probability weights must be positive and finite;
retained values cross the boundary bit-for-bit, explicitly masked missing
values receive reason 4, and zero, negative, or unmasked nonfinite candidate
values fail before diagnostic writes. This developer action returns native
invalid-data code `459` for other candidate-row domain violations. It does not
compute or claim the robust VCE required by the sealed reference case and does
not claim the public command's eventual Stata-facing error code. Weight
expressions are not materialized by this developer helper.
All binding, matrix, type, and output validation precedes
observation reads, and diagnostic writes begin only after native preparation
succeeds. Both `estimate-ols` and `estimate-ppml` remain unconditionally
unsupported.

## Hidden atomic prepared-OLS transport

`estimate-ols-test` is a native integration action, not a public estimator. It
is reachable through the Rust/actual-plugin harnesses and the developer-only
`_stfixest_estimate_ols_test` ado helper, and requires before any observation
read:

- exact action protocol `1.1`;
- compatible OLS result wire `1.1`;
- the exact loaded plugin version;
- `confirm=developer-estimation-test`;
- model wire 2.1, numeric bindings, cluster metadata when requested, and exact
  nonaliasing input/output layouts;
- an initially incomplete result receipt.

The action supports the current prepared continuous/numeric-factor,
intercept-only numeric-FE, unit/a/f/p-weight surface and selects exactly one of
iid, HC1 robust, or one-/multiway clustered inference. It performs one native
source-loading phase, prepares the authoritative sample once, and calls the
selected certified same-call core composition. It never calls the legacy VCE,
DoF, fit, or posting paths and never returns residualized N-by-K data to Stata.
For categorical absorbed interactions, singleton deletion may remove complete
ordinary factor levels. Wire 1.1 therefore reports a compact final-sample graph
while preserving the larger candidate graph only as caller allocation
capacity. The guarded Stata helper rejects explicit `ib...`/`ibn.` operators
and `fvset` base overrides before dispatch because model wire 2.1 cannot
distinguish all such policies; Rust independently requires the observable
one-base/minimum-level graph before rebasing.

After every core certificate passes, the plugin fallibly builds and validates a
complete Rust-owned shadow result. Rust performs every N-, K-, and K²-scale
check, including native omission-marker construction, before any store. Stata
or the test harness must have precreated these 11 private result matrices:

| Protocol key | Logical contents | Required layout |
|---|---|---|
| `output_b_matrix` | Coefficients scattered to canonical slots | `1 x P` |
| `output_v_matrix` | Full scattered covariance | `P x P` |
| `output_status_matrix` | Slot/status/generated-column metadata | `P x 4` |
| `output_fit_matrix` | Fit statistics and defined flags | `12 x 2` |
| `output_dof_matrix` | Distinct DoF values and defined flags | `20 x 2` |
| `output_solver_matrix` | Solver and certificate summaries | `2 x 12` |
| `output_fe_matrix` | Absorbed-effect metadata | `max(F,1) x 8` |
| `output_cluster_matrix` | Cluster-dimension metadata | `max(Q,1) x 4` |
| `output_subset_matrix` | Cluster-subset metadata | `max(2^Q-1,1) x 4` |
| `output_certificate_matrix` | Independent certificate diagnostics | `32 x 2` |
| `output_receipt_matrix` | Wire identity, counts, flags, completion | `1 x 16` |

Here the caller allocates `P=max(candidate-sample coefficient slots,1)`, `F` is
the prepared absorbed-effect row count, and `Q` is the declared cluster-dimension
count. After final-sample categorical rebasing, the logical coefficient count
may be smaller than `P`; Rust validates that bound and writes only the leading
logical block. The receipt reports the logical count, and Stata shrinks the
private matrices before naming or posting them. The action also requires
a private full-length sample variable and permits private residual and
drop-reason variables. All 11 matrix names, dimensions, aliases, numeric output
positions, and the receipt's initial `COMPLETE=0` state are preflighted before
observation loading. Each result name is then fallibly converted once to a
NUL-terminated borrowed C handle whose matrix dimensions fit the SPI integer
range. The cell-store loop performs no name allocation, sanitization, or
fallible coordinate conversion.

Commit is **logically atomic over private shadow destinations**, not a claim of
physical SPI rollback. SPI offers no transaction that can undo an earlier
successful matrix or variable store. Therefore a failed store may leave some
private shadow cells changed, but the result is invalid and unobservable as an
estimate because the receipt's `COMPLETE` cell remains zero. The plugin stores
sample/residual/reason shadows and the ten nonreceipt result matrices, writes
receipt metadata, and writes `COMPLETE=1` as the final operation. No allocation,
poll, display, computation, or store follows a successful completion write.

The SPI session prepares fixed numeric-store and matrix-store errors before
commit. The ABI boundary uses static preformatted C reports for those exact
errors, so even reporting an injected partial-store failure performs no Rust
allocation or second logical output store. The Fake SPI fixture preallocates
its event log and error and injects failure at all 222 store ordinals. The
actual C loader injects all 219 live-store failures, verifies one stable borrowed
name pointer for every result matrix, and observes `COMPLETE=0` in every failure
case. The successful candidate-bound universal macOS x86_64+arm64 build, binary
audit, and live loader harness pass through `d731b327`.

Stata 18 rejects the full guarded key/value list as too many plugin options.
The hidden helper therefore sends exactly one canonical bundle argument:
`count:` followed by `byte_length:payload` for every key/value field. The native
decoder accepts either this one bundle or the existing ordinary argv form,
never a mixture. It rejects zero/oversized counts, noncanonical decimals,
empty, truncated, trailing, duplicate, unknown, and UTF-8-splitting fields
before handshake validation or any SPI read/store.

## Hidden licensed eclass transaction

`_stfixest_estimate_ols_test` is the only ado consumer of a complete receipt.
It remains explicitly developer-only. It creates all private shadows, then uses
`_estimates hold ..., restore nullok varname()` before the sole substantive
plugin call. It accepts only exact receipt/table/count identities, a total
sample marker contained in the candidate marker, correctly scoped optional
residuals, unique canonical covariance coordinates, finite/symmetric covariance
with nonnegative diagonal, valid solver/FE/cluster metadata, and mandatory
independent certificates.

Omission stripes come from protected marker matrices passed to
`_ms_findomitted`; the plugin's numerical b/V shadows are not exposed to that
command. For a nonempty coefficient axis, Stata's cell-built matrix posting can
warn and alter the adjudicated multiway CGM restored state. The helper instead
copies each complete b/V shadow as a whole matrix through Mata, applies the
validated stripes to those copies, and posts them without changing the exact
two-/three-way negative-determinant covariance. The K=0 branch posts no b/V.
Only after the selected fit, DoF, solver, FE, cluster, certificate, weight,
sample, and command metadata have been posted does a `nobreak` block rename a
requested residual and run `_estimates unhold ..., not`.

Deterministic failure stages, real scientific failure, named and null
prior-estimate cases, and an interactive PTY Control-C after the sole plugin
call restore prior `e()` state and `e(sample)` exactly and leave no requested
residual. Licensed iid/HC1/one-/multiway cluster, unit/a/f/p, default/
`noconstant`, omitted-coordinate, K=0, sample/residual, and frozen q2/q3
covariance fixtures pass through candidate hardening `d731b327`; the Stata runner rejects the
nonsymmetric/highly-singular warning on any successful fixture.

All observation-, coefficient-, and covariance-scale result validation is
native. After the one plugin call, the helper performs only constant-size
receipt/table identity checks, transaction control, whole-matrix posting, and
the final residual/estimate commit.

## Hidden numeric candidate projection and receipt

`_stfixest_ols_hidden_numeric_v1` is a developer-only adapter over the hidden
eclass helper. It retains the private command identity in `e(cmd)` and
`e(cmdline)` while recording `stfeols` only as an intended future public
command. It projects the frozen 40-case `ols_v1` semantic manifest without
enabling public syntax: 30 cases execute and ten remain exact, named blockers.

The active candidate receipt is
`benchmarks/reports/captured/darwin-arm64/stata18/ols_hidden_numeric/20260822T132813Z-d731b32`.
Every Stata capture and supporting gate loads one identical isolated staged
plugin (SHA-256
`eda2629198429d419a476151886936e0761eca26f2a8cc34fe83c52146703980`,
universal x86_64+arm64, embedded `d731b3276eab`). The receipt's payload seal is
`1558571563006bd3f5a1ce5a39b33f1f552256a83212a865d51cebdbdfe37279`.
`benchmarks/manifests/ols_hidden_numeric_candidate_receipt_v1.json` separately
binds the complete file SHA-256, git blob, and mode, so changing and resealing
committed receipt content does not preserve eligibility. Older receipts are
historical only. The receipt explicitly records `public_activation=false` and
unmeasured performance.

Public `estimate-ols`, `stfeols`, `estimate-ppml`, and `stfepois` remain
fail-closed at `r(498)`. The next gate is broader sealed estimator-level parity
and reviewed activation wiring across the complete supported public surface.

## Return-code map

| Class | Stata code |
|---|---:|
| Syntax | 198 |
| Unsupported/invalid option | 498 |
| Protocol mismatch | 499 |
| Negative analytic weight | 402 |
| Invalid frequency-weight domain | 401 |
| Insufficient final observations/DoF | 2001 |
| Invalid data | 459 |
| Allocation | 909 |
| User interruption | 1 |
| Native self-test failure | 3498 |
| Internal error or caught panic | 3499 |

All failures are fail-closed. The ado layer must not post partial results after
a nonzero return code.

## Platform loader contract

Stata's macOS console reports `c(os)==Unix`, so `c(os)` alone is not a complete
platform discriminator. The ado loader selects one artifact with this ordered
rule:

| `c(os)` | `c(machine_type)` | Artifact |
|---|---|---|
| `Windows` | any | `stfixest_windows.plugin` |
| `MacOSX` | any | `stfixest_macos.plugin` |
| `Unix` | starts with `Mac` | `stfixest_macos.plugin` |
| `Unix` | any other value | `stfixest_linux.plugin` |
| any other value | any | fail with `r(9)` |

The macOS rule covers both observed Stata 18 labels: `Mac (Apple Silicon)` and
`Macintosh (Intel 64-bit)`. Unknown operating systems never default to Linux.

Licensed-Stata fixtures emit `END_FIXTURE|<name>|rc=0` only after their final
assertion. `scripts/run_stata_tests.py` sets the repository ado-path in an
isolated wrapper and requires that exact terminal sentinel; a zero Stata
subprocess status without the sentinel is a failure.
