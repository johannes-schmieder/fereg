# Plugin action protocol (separate from model wire)

The C ABI exports only Stata's required `pginit` and `stata_call` symbols.
`stata_call` forwards `argc/argv` to Rust's `stfixest_dispatch`.

The action/diagnostics handshake remains version `1.0`. The expanded-model
metadata wire is independently versioned at `2.1`; neither version number is
an alias for the other. An action handshake never performs a lossy model-wire
conversion.

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
stfixest.action_protocol=1.0
stfixest.model_wire=2.1
```

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
Weights must be passed as one numeric source binding. Frequency weights must
be finite, strictly positive integral values representable as `u32`; an
explicit Stata-missing mask is a reason-4 exclusion, while an unmasked NaN or
infinity is invalid data. Probability weights must be positive and finite;
retained values cross the boundary bit-for-bit, explicitly masked missing
values receive reason 4, and zero, negative, or unmasked nonfinite candidate
values fail before diagnostic writes. This developer action returns native
invalid-data code `459` for a candidate-row domain violation. It does not
compute or claim the robust VCE required by the sealed reference case and does
not claim the public command's eventual Stata-facing error code. Weight
expressions are not materialized by this developer helper.
All binding, matrix, type, and output validation precedes
observation reads, and diagnostic writes begin only after native preparation
succeeds. Both `estimate-ols` and `estimate-ppml` remain unconditionally
unsupported.

## Return-code map

| Class | Stata code |
|---|---:|
| Syntax | 198 |
| Unsupported/invalid option | 498 |
| Protocol mismatch | 499 |
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
