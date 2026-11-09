# Plugin action and result protocol

The platform plugin exports Stata's required `pginit` and `stata_call`; the C
bridge forwards to Rust's `fereg_dispatch`. Scientific logic and result
validation remain in Rust.

## 0.5.0 versions

The action handshake, model metadata, OLS result layout, and additive PPML
request/result layouts are independently versioned:

| Contract | 0.5.0 version |
|---|---:|
| Action protocol | 1.7 |
| Model wire | 2.1 |
| OLS result wire | 1.6 |
| PPML request wire | 1.9 |
| PPML result wire | 1.5 |

Estimation requires the exact action and estimator-specific request/result
versions. Diagnostic `selftest` callers retain the action
protocol's same-major, not-newer-minor compatibility rule. ADO identity must be
exactly 0.5.0. Model wire 2.0 remains readable only for the bounded older
no-absorbed-metadata case; model wire v1, unknown majors, and newer minors are
rejected. The additive PPML wires do not alter or alias the OLS result wire.

## Actions

- `selftest` reports plugin identity, requires and validates the diagnostic
  handshake, and runs the bounded installation checks. `verbose=1` adds all
  wire versions, ABI, source commit, build date, host/worker details and individual
  check results; omitted `verbose` or `verbose=0` selects the short summary.
  Identity is printed before handshake validation. Failed checks always print
  their details and return a nonzero code. The public ado reports its own version
  before loading the plugin, including when the binary is missing.
  The former `version` and `doctor` actions are removed.
- `panic-test` is confirmation-guarded and exists only for ABI containment.
- `estimate-ols` is the one live-SPI public estimation action used by
  `fereg`.
- `prepare-ols-test` and the hidden OLS adapters (`_fereg_ols`,
  `_fereg_ols_hidden_numeric_v1`, `_fereg_estimate_ols_test`) remain developer
  qualification surfaces. Their ado files are excluded from distributions; the
  plugin binary still answers `prepare-ols-test` because the native loader test
  drives it through every distributed plugin.
- `estimate-ppml` is the live-SPI public estimation action used by `feppml`.
  The public wrapper delegates once through `_fereg_ppml` and the guarded
  `_fereg_estimate_ppml` transaction. Non-SPI dispatcher attempts to invoke
  `estimate-ppml` are rejected.

## Developer injections (hidden adapters only)

Licensed qualification fixtures exercise the shipped binary's failure and routing paths
through request keys that public `fereg`/`feppml` syntax cannot produce; the
public wrappers reject `inject()`. Failure injections do not change arithmetic;
the routing injections below select experimental algorithms for qualification.

| Key | Estimator | Effect |
|---|---|---|
| `inject=breaknative` + `break_marker_token=<32 lowercase hex>` | OLS, PPML | Arms native Break: displays `FEREG_NATIVE_BREAK_ARMED` (OLS) or `FEPPML_NATIVE_BREAK_ARMED` (PPML), creates `<tmp>/<fereg\|feppml>-native-break-<token>.marker`, and waits for Stata Break inside the native call |
| `inject=lsmr_iteration_break` + `break_marker_token=<32 lowercase hex>` | OLS, PPML | Requires explicit `solver=lsmr` and at least two categorical intercept FEs. Parks after a completed, continuing iteration (OLS: fedemean's modified LSMR, observed through its cancellation polls; PPML: the joint LSMR); the main-thread poller emits the native marker, real Break sets the cancellation signal, and all work joins before rollback |
| `inject=serial_input` | OLS, PPML | Keeps the serial readers on a qualified host; public policy and arithmetic unchanged, so equal worker counts compare loading paths exactly |
| `inject=input_break` + `break_marker_token=<32 lowercase hex>` | OLS, PPML | After the first parallel input phase has joined and closed its window, creates the native marker and polls until a real Break; the transaction rolls back |
| `inject=input_worker_panic` | OLS, PPML | Panics in one worker block of the first parallel input phase; the window closes during unwind and the FFI boundary returns r(3499) |
| `inject=input_read_failure` | OLS, PPML | One block of the first parallel input phase reports SDK code 919; the failure is returned (never retried serially) and rolls back |
| `inject=panic` | PPML | Panics after validation to prove ABI panic containment (OLS uses the `panic-test` action) |
| `inject=allocation` | PPML | Fails the FE-recovery reservation to prove atomic allocation failure |
| `inject=routing_h{4\|8\|16\|32}_k{0\|1\|2}` | OLS | Selects one of 12 eligible two-FE routing arms: the incumbent reaches iteration `h` before a bounded optional CMG build; `k` is the high-degree worker tree sample count (zero retains the hybrid graph). More-than-two-FE categorical requests fail clearly |
| `inject=routing_diagonal` | OLS | Tests the current-weight Schur diagonal on eligible two-FE systems, retaining the observation operator |
| `inject=routing_cells` | OLS | Tests an operator over aggregated worker/firm cells, retaining the incumbent diagonal preconditioner |
| `inject=routing_direct` | OLS | Tests a bounded componentwise direct Schur factorization on eligible two-FE systems |
| `inject=it_coefficients` | OLS | Tests compact coefficient-space acceleration on eligible categorical systems; requires `solver=it` |
| `inject=ownership_{moments\|scores\|both}` | OLS | Independently omits provably unused speculative moments and/or retains eligible projected score storage; preserves solver choice and exact science, including explicit solvers |
| `inject=cells_eager[_moments\|_scores\|_both]` | OLS | First-occurrence cells constructed before the diagonal pilot for eligible two-FE Auto/Auto, the public cell order; optional ownership controls, unchanged CMG policy. |
| `inject=cells_deferred[_moments\|_scores\|_both]` | OLS | The former public cell order: construction only after successful CMG commitment at an existing restart, with joint memory admission; optional ownership controls, unchanged routing policy. |
| `inject=routing_prefix_{none\|map4\|it4}` | OLS | With two categorical intercept FEs, the incumbent recurrence without an optional upgrade. Its many-FE prefix-plus-within-LSMR arm is retired: three or more FEs fail with InvalidArgument |
| `inject=routing_incremental_many` | OLS (replay wire) | Retired with the pinned within LSMR it qualified: on at least three categorical intercept FEs it fails with InvalidArgument |
| `inject=routing_many_geometry_v1` | OLS (replay wire) | Selects the public many-FE geometry policy for at least three categorical intercept FEs: fedemean's routes first, then the geometry-qualified MAP pilot and MAP, each at the requested-tolerance certificate. Unlike other injections it keeps public two-FE ownership/cell activation, so it equals uninjected Auto on every design |
| `inject=routing_many_windows_v1` | OLS (replay wire, hidden Stata adapter) | Retired with the pinned within LSMR (its bounded session windows): on at least three categorical intercept FEs it fails with InvalidArgument; it keeps public two-FE ownership/cell activation, so two-FE and ineligible designs equal uninjected Auto |
| `inject=routing_two_gates_v2` | OLS (replay wire) | Selects the public two-FE policy, gate v2: the structural gate plus an early CMG build at local step 4, 8 or 12 when the diagonal recurrence has stalled; keeps public two-FE ownership/cell activation, so it equals uninjected Auto on every design |
| `inject=routing_incremental_two` | OLS (replay wire) | Qualifies the existing direct-or-h16/k1 synchronous portfolio for exactly two categorical intercept FEs |
| `inject=routing_builder_break` + `break_marker_token=<32 lowercase hex>` | OLS | Uses the synchronous two-FE h4/k1 builder and waits for Break at its first cancellation poll after executed construction work; any worker count. More-than-two-FE categorical requests fail clearly |
| `inject=ppml_topology` | PPML | Shares admitted categorical checkpoint component indexing after acceptance; weighted numerical data is recomputed in every frame |
| `inject=ppml_separation_cg` | PPML | Comparison arm for the beta.10 default: solves admitted separation-detector projections (rectifier, Mu, numerical direction span checks) with exactly two categorical intercepts with retained-product CG, the earlier public path, instead of the OLS two-FE Auto route; detector tolerances, weights, iteration limit and decision rules are the same. Replaces `inject=ppml_separation_auto` (see VALIDATION, PPML separation routing) |

The LSMR iteration witness leaves ordinary solver policies unchanged. Its
`lsmr_break` trace must contain `iteration_break_ready` with a positive completed
iteration and `iteration_break_cancelled` from the actual upstream cancellation
error. OLS observes fedemean's modified-LSMR polls; PPML observes its joint
`mlsmr_cancellable` solve. A fit that finishes without reaching this checkpoint
fails before publication. `scripts/run_stata_break_test.py` selects these real
PTY fixtures with `--fixture-kind ols-lsmr` or `ppml-lsmr` and
`--lsmr-workers 1` or `4`; each verifies populated/null estimates, output rollback
and exact successful fits after interruption.

Each routing name is a complete literal token, for example
`routing_h8_k1`. The hidden Stata adapter accepts the `routing_h*_k*`,
`routing_builder_break`, prefix and kernel tokens and the candidate
`routing_many_windows_v1`; the plugin additionally
accepts `routing_incremental_{many,two}`, `routing_many_geometry_v1`,
`routing_two_gates_{v1,v2}`, `routing_two_remaining_*` and
`routing_portfolio_{v1,unseeded}`,
which reach it only through the replay wire (`bench/routing.py`,
`bench/qualify.py`) and Rust tests. OLS routing injections require `solver=auto`, reject
`break_marker_token` except for the builder-Break arm, and are rejected by PPML. Invalid names and explicit
solver conflicts fail before observation reads or publication. Omitted or empty
`inject` retains the qualified automatic policy; these arms do not enable a new
public default or change request/result versions or result dimensions. Explicit
injections otherwise bypass public two-FE ownership/cell activation;
only the selected `routing_many_geometry_v1` arm and the candidate
`routing_many_windows_v1` keep it, so public activation can require exact
selected-hidden/public agreement on every design.

The incremental selectors require Auto preconditioning to change computation.
Outside the selected categorical intercept family, including slopes and explicit
preconditioners, they retain the incumbent without optional routing preparation.
Finite controls do not count as FEs. Both selectors resolve to the existing
`routing_portfolio_unseeded` machinery on eligible topology; its two-FE
certificates, limits, memory admission and cancellation remain unchanged, and
its many-FE arm, like `routing_portfolio_v1`'s, ran the retired within LSMR
and fails clearly. Uninjected Auto uses the many-FE geometry policy for at
least three categorical intercept FEs and the structural gates for two.

The separate coefficient-IT injection requires explicit `solver=it` and rejects
`break_marker_token`. Ordinary explicit IT keeps its incumbent observation-space
implementation. The compact challenger has not passed promotion qualification;
it remains a developer-only comparison.

The checkpoint arms apply to eligible categorical two-FE systems. The general
more-than-two-FE pair-upgrade experiment is retired: explicit h/k or
`routing_builder_break` requests on those categorical designs return
InvalidArgument. Zero/one-FE and slope routes retain their existing behavior;
the three kernel experiments affect only eligible two-FE systems. Explicit
`preconditioner()` choices remain authoritative. Optional construction is admitted
against the joint memory budget; a memory refusal continues the incumbent.
Construction is cancellable at bounded chunks; a Break propagates as an
interrupted native estimation with atomic rollback after all tasks join, never
as a continued incumbent or a published result. Accepted results retain the
original-design certificate, requested tolerance, per-action iteration budget,
and atomic publication contract. These qualification arms build synchronously
at a numerical checkpoint. Prefix experiments are independently qualified;
their tokens cannot be combined into a Cartesian policy search.

The builder-Break arm claims a process-wide developer marker for its one
synchronous build. At the builder's first cancellation poll after executed
construction work it records readiness (solver trace
`phase=routing_builder|event=builder_break_ready|task=0|ordinal=<zero-based poll>`)
and parks the constructing worker. Stata's polling thread emits the native
Break marker only after that readiness; the requested cancellation then
propagates through the next poll.

`ppml_topology` is independent of OLS routing and does not enable a new PPML
solver policy. Its pending structural cache publishes only on acceptance;
rejected trials and support changes cannot reuse stale state. The default has
no such cache. These experiments do not constitute performance qualification.

The hidden adapters read the token from `FEREG_BREAK_MARKER_TOKEN` (OLS) or
`FEREG_PPML_BREAK_MARKER_TOKEN` (PPML); `scripts/run_stata_break_test.py`
sets both and sends Break only after the marker file exists. Fixtures:
`tests/licensed_stata/estimate_ols_eclass_break.do`,
`tests/licensed_stata/estimate_ppml_eclass_break.do`,
`parity_ppmlhdfe/estimate_ppml_hidden_transaction.do`,
`parity_ppmlhdfe/estimate_ppml_hidden_allocation.do` and the PPML public-scope
fixtures.

## Parallel numeric input

Action protocol 1.7 adds three optional public host facts to the OLS and PPML
requests, computed by `_fereg_input_host` from `c()` values: `host_edition`
(`c(edition_real)`, else from `c(MP)`/`c(SE)`), `host_interface` (`console` when
`c(console)` is `console`, otherwise `gui`) and `host_aliases` (1 when any
variable the plugin reads is a frame alias, Stata 18+). With the existing
`stata` version, the plugin's compiled host table decides whether the numeric
window may be used; absent or unknown facts keep the serial readers.

The C bridge exports `fereg_spi_numeric_window_open`, `_read` and `_close`.
Open and close run on the bound Stata thread; open validates the observation
count and every variable (in range, numeric) and captures `_stata_->vdata` and
`SV_missval`. Read is the only wrapper callable from another thread, and only
inside an open window with its generation; on success it writes every requested
row and slot (values, `>= SV_missval` flags, or the slot fills for excluded
rows). While a window is open every other wrapper, including `SF_poll`, refuses
with 498. A window found open at a later `stata_call` entry (a leaked guard) is
closed, reported once and the call fails; the next call proceeds.

## Break and cancellation

SPI polling stays on Stata's thread; during parallel numeric input it happens
between window phases. OLS preparation, inference and derived
observation outputs, including saved fixed-effect recovery, run on the action
team while Stata's thread polls Break every 20 ms; PPML polls at its
IRLS, separation, inference and publication safe points. On Break the plugin
requests cooperative cancellation and long native loops stop at their next
checkpoint (FE sweeps and iterations, LSMR/CMG entry, one-shot fit phases,
clustered subsets, and every 1024 rows of long serial inference passes). The transaction then fails with return code 1 before any
store and the ado restores prior results.

## Plugin lifetime

The plugin keeps one cached worker team per process between actions, and its
threads keep running plugin code while idle. On Windows the plugin therefore
pins its own module on the first action: a later `FreeLibrary`, by Stata or a
test harness, leaves the code mapped until the process exits, so an idle worker
never executes unmapped pages. A rebuilt or reinstalled Windows plugin takes
effect after Stata restarts. The Windows actual-plugin loader test releases the
library after the selftest action and asserts that the module is still mapped.
Every `protocol_mismatch` error (ado files and loaded plugin from different
versions) is followed by a line asking the user to restart Stata, and to
reinstall fereg if the error persists.

## Estimation request transport

OLS and PPML retain their canonical count/byte-length framed request. The ado
adapter sends `bundle=<payload>` through 4,000 payload bytes. Above that size,
it stores the identical payload in the caller-local `fereg_request_bundle` and
sends only `bundle_local=<byte-count>`. The SPI name is `_fereg_request_bundle`;
the leading underscore selects a local, not a global. The descriptor must be the
only action argument. No caller-supplied macro name is accepted.

Rust admits at most 16 MiB of metadata before the fixed main-thread macro read.
Its buffer includes one content byte beyond the declared size to detect SPI
truncation, plus a separate terminator byte. Exact NUL position, UTF-8 and the
unchanged canonical request decoder are checked before observations are read.
The buffer is dropped after parsing. Malformed/over-cap transport cannot complete
a receipt; scientific request/result versions and the one-call transaction are
unchanged. See [SPEC](SPEC.md).

## Public OLS transaction

Explicit solver requests retain their support checks and never silently become
automatic requests. In particular, the routing experiments do not alter
explicit LSMR or CMG behavior, permit unsafe CG, or make CMG-Schur available
outside its supported topology. Public `solver(auto)` retains the current
qualified routing policy unless a hidden qualification adapter supplies one of
the routing injections above.

The request wire carries native route tokens, not the public grammar. One ado
table (`_fereg_solver.ado`) maps `solver(method[, reduce() precondition()
accelerate() cmg*()])` onto `solver=auto|map|lsmr|pcg|cmg|cmgschur|it|squarem` and
`preconditioner=auto|off|diagonal|schwarz`; the plugin re-validates the tokens
(`preconditioner` other than `off` only with `auto`, `lsmr` or `pcg`). OLS
`solver=pcg` is fedemean's PCG (public `solver(cg)` with a preconditioner other
than `cmg`): its `preconditioner` also accepts `block`, and an optional
`reduction=auto|full|eliminate` field (default `auto`, rejected with every other
solver and by PPML) names the reduction. The hidden
adapters take the tokens directly. The experimental OLS CMG-Schur request
(`solver=cmgschur`, public `solver(cg, reduce(eliminate) precondition(cmg))`) accepts an optional
`cmgdegree=4|5|6` field (default 5). That field is rejected for every other
solver and by PPML. The public adapter emits it only for that route. The
existing OLS transport cap admits the field and hidden Break keys.
CMG-Schur requires exactly two categorical intercept fixed effects; unsupported
topology is rejected before observation reads in the plugin adapter.

Its optional `cmgsamples` unsigned 32-bit field defaults to zero, preserving the
original hybrid graph. A positive value requests that many averaged weighted
trees for each worker above `cmgdegree`. Optional `cmgseed` defaults to zero and
accepts decimal integers through 9007199254740991; an explicit seed requires
positive `cmgsamples`. Both fields are rejected for other solvers and PPML before
observation reads or publication. Omitted fields preserve existing request bytes.
Sampling changes only the fixed preconditioner; the outer operator and original
problem certificate remain exact. The public adapter publishes the effective
values as `e(fe_cmgdegree)`, `e(fe_cmgsamples)` and `e(fe_cmgseed)`.

### Route rows

Result wire 1.6 reports each projection's route in the vocabulary of
`solver()` (`crates/fereg-plugin/src/route_report.rs`). A route row has 12
columns:

| Column | Content | Codes |
|---:|---|---|
| 1 | reduction | 0 none, 1 full, 2 eliminate |
| 2 | method | 0 none, 1 direct, 2 map, 3 cg, 4 lsmr, 5 derived |
| 3 | MAP acceleration | 0 none, 1 cg, 2 irons-tuck, 3 squarem; nonzero only with method map |
| 4 | preconditioner | 0 none, 1 jacobi, 2 block, 3 schwarz, 4 cmg |
| 5–9 | local solves: with block, the local solve of design effects 1–5 (absorb order); with schwarz, the subdomains counted by local solve | block: 0 none or eliminated, 1 dense, 2 cmg, 3 jacobi, at least one nonzero; schwarz: counts of trivial, dense, skyline and approximate subdomains in columns 5–8 (all zero when unreported), column 9 zero; otherwise all zero |
| 10 | escalations | solves that switched to a stronger preconditioner |
| 11 | iterations | exact integer |
| 12 | certified fallback | 0 or 1 |

The OLS solver table is 2x21 with rows absorption and final residual: columns
1–12 are the route row and 13–21 the warning count, pilot iterations, pilot
contraction, setup and solve seconds, the certificate's absolute and relative
infinity and relative two norms, and the normal-equation residual. Rust
validates every code and count before stores; the ado decodes the rows into
`e(fe_*)` strings, validates them again and posts the diagnostics as
`e(fe_solve)` without codes. A plain MAP projection of one intercept channel
(or the constant alone) is reported as direct unless MAP was requested by name.
The table replaces result wire 1.5's 2x16 code table; an ado and plugin of
different result minors fail the exact version check.

Before any observation read, Rust validates the complete request, exact
action/model/result versions, source and output bindings, dimensions,
nonaliasing, memory/thread options, and initially incomplete receipt. Rust then
owns sample construction, singleton pruning, computation, inference,
certification, saved-output preparation, and result validation.

The caller precreates private shadows for coefficients, covariance, statuses,
fit/DoF/solver/FE/cluster/subset/certificate tables, resource/timing
instrumentation, `e(sample)`, optional residual/absorbed/saved-FE outputs, and
the receipt. Every N/K/K² value and output position is checked before stores.
SPI calls stay on the Stata thread.

SPI has no physical rollback, so logical completion is receipt based. The
`COMPLETE` cell is the final native operation; no allocation, polling,
display, computation, or store follows it. The ado transaction validates the
complete private receipt, posts `e()`, and commits requested output variables
and estimates only in its final protected block. Native error, store failure,
panic, validation failure, or Control-C restores prior `e()`/named estimates
and exposes no requested outputs.

## Public PPML transaction

The PPML adapter implements the same control-plane/data-plane boundary used by
the public command. Stata parses syntax and model
metadata, constructs the candidate marker, allocates private shadows, and
makes exactly one guarded `estimate-ppml` call. Rust validates the complete
request, PPML request/result wire identities, source and output bindings,
dimensions, nonaliasing destinations, memory/thread controls, and initially
incomplete receipt before reading observations.

Request wire 1.9 has one closed `solver` field with values `auto`, `map`,
`lsmr`, and `cmg` (public `solver(auto|lsmr|cg)`; `map` is the canonical family
token used by the hidden adapter) and adds the required `output_route_matrix`
destination. The optional `preconditioner` field (`auto`, `off`, `diagonal`,
`schwarz`; default `auto`) is valid only with `solver=lsmr` and carries
`solver(lsmr, precondition())`; the ado sends it only when it is not `auto`,
so the largest request has 101 fields. Private solver, mode, policy, trace and
experiment keys are unsupported. An older exact
request version is a protocol mismatch; an unknown key on 1.9 is syntax error.
Recognized solver/topology combinations outside the staged public envelope
return `r(498)`. A supported route that fails its checked workspace preflight
returns `r(909)`; neither condition is converted to an implicit solver choice.

The live action owns sample construction, singleton and separation processing,
topology rebasing, IRLS, inference, certificates, saved-output preparation,
and complete result validation. All result values are certified in Rust-owned
memory before the first SPI store. Result wire 1.5 includes fixed 16-row
inference/PSD evidence, a 14x2 drop-reason count table, and a 5x13
separation-sequence table. It also contains a fixed 4x22 solver table whose rows are
MAP, fused LSMR, joint LSMR, and prepared CMG. The table binds the requested,
selected, concrete, effective, and checkpoint routes and per-engine frame,
retry, fallback, timing, and certificate evidence; the compiled validator
checks it and the ado posts its counts and evidence as `e(fe_solve)` without the
identity codes. The separate 1x12 route matrix (request wire 1.9, outside the
compiled validator) holds the route row of the final certified linear solve,
validated by Rust before stores and by the ado before posting. Its independent 33-column receipt binds the other
tables, execution/generation counts, and uint32 limbs for the initial/final
generation hashes plus deterministic digests of the complete ordered
separation trace and generation-hash sequence. Timing and process-local values
are excluded from those digests. The receipt does not alias the OLS receipt,
and its column 33 `COMPLETE` cell is the final native operation.

Checkpoint family code 3 identifies the current-weight LSMR accelerator
accepted by the scientific contract; individual no-work or fallback checkpoint frames remain
attributed to the MAP row when MAP produced their returned projection. Solver
fallback codes are 0 for none, 1 for CMG ordinary-frame numerical
nonconvergence, 2 for checkpoint LSMR numerical nonconvergence, and 3 when
both occurred. The fixed result-wire dimensions are unchanged.

The ado transaction validates every private shadow and the completed
receipt before posting or renaming requested outputs. Syntax/domain error,
protocol mismatch, nonconvergence, allocation/store failure, panic, or
Control-C restores the prior estimation state and exposes no partial requested
outputs. Public `feppml.ado` performs no second parse, native call, or posting
outside this protected transaction.

## Return codes

| Class | Stata code |
|---|---:|
| Syntax | 198 |
| Unsupported/invalid option | 498 |
| Protocol mismatch | 499 |
| Unsupported weight kind | 101 |
| Negative analytic weight | 402 |
| Invalid frequency-weight domain | 401 |
| Invalid ordinary factor on the available sample | 452 |
| Insufficient observations or residual DoF | 2001 |
| Invalid data | 459 |
| Numerical nonconvergence | 430 |
| Allocation | 909 |
| User interruption | 1 |
| Native self-test failure | 3498 |
| Internal error or caught panic | 3499 |

Licensed fixtures pass only when process status, Stata return code, status
files, and the unique terminal `END_FIXTURE|<name>|rc=0` sentinel agree.

## Shared memory result

The estimation action accepts four scalar baseline fields: `stata_used_entry`,
`stata_alloc_entry`, `stata_used_before`, `stata_alloc_before`. Each is a
nonnegative exact integer byte count or `.` for unavailable. The separate
`output_memory_matrix` is mandatory, distinct from every input and output
matrix, and has 11 rows and two columns (presence, value). Absent cells encode
`0,0`. Publication precedes COMPLETE and participates in the same rollback.

Rows are `schema_version`, `estimate_status`, `stata_used_before`,
`stata_alloc_before`, `native_live_before`, `native_peak_est`,
`stata_growth_est`, `combined_peak_est`, `additional_reserve`,
`safety_headroom`, `model_gap_bits`. Schema 2 retains status 0 unavailable,
1 partial, 2 estimated-complete-practical. Status 2 requires a known Stata-19
baseline, an expected native forecast and every action stage. It describes
the empirical engineering model in the scientific contract, not a strict allocation or RSS
bound. Other Stata versions, unknown baselines (including unmeasured frames)
and unknown attempt storage remain partial, without a combined peak or reserve.

The combined estimate is the maximum of entry allocation and pre-plugin Stata
allocation plus the larger of native execution and subsequent posting growth,
with the preexisting native owner added once. The additional reserve excludes
resident baselines. Solver admission and safety headroom remain separate.
The estimate's calibrated scope and extrapolation limits are described in
[the memory model](ARCHITECTURE.md#preparation-and-storage). The native forecast
includes admitted solver workspace peaks, whose CMG parts assume CMG's 32-level
hierarchy cap (SPEC §19).

The low 16 gap bits retain strict proof limitations even at practical status 2.
Bits 17/18 mark unknown expected use/Stata growth, bit 16 an unavailable
baseline, and bits 20–29 missing action stages. Schema 2 status 2 permits only
low-16 proof gaps; its validator checks phase composition. Schema 1 remains
readable under its original stricter status-2 rule (zero gaps). Package metadata
binds the independently versioned memory schema. Existing OLS/PPML runtime
matrices retain their layout and observation boundaries.

## Optional PPML coefficient mean correction

PPML request 1.9/result 1.5 carries an optional correction inside the same
transaction. `bias_method`, `bias_unit`, `bias_probes`, `bias_seed`, and
`bias_stream` identify its calculation. `output_bias_count`,
`output_bias_channels`, `output_bias_matrix`, `output_bias_diagnostics`, and
`output_bias_support`
identify private destinations. The saved-channel count is zero or eighteen;
coefficient and diagnostic matrices have dimensions 9 by ordinary coefficient
count and 21 by 1; the support matrix is 2 by 11 in absorb order.
All three matrix destinations are mandatory for a correction request.
Option-off requests allocate none of these outputs.

All destinations, values, dimensions, sample identities and request bindings
are validated before publication. Correction columns and matrices precede the
ordinary COMPLETE receipt; no second substantive call or partial public result
is permitted. Missing unit IDs do not select rows. The complete statistical,
normalization, RNG and channel contracts are in
[ppml_bias_correction.md](ppml_bias_correction.md).

### Hidden two-FE gate qualification selector

`inject=routing_two_gates_v2` selects gate v2, the public two-FE Auto policy:
gate v1 plus an early CMG build at local step 4, 8 or 12 when the diagonal
recurrence has stalled (see [the gate notes](two_fe_auto_gates.md)). Like the
many-FE geometry selector it keeps the public two-FE ownership and
cell activation, so it equals uninjected Auto on every design.

`inject=routing_two_gates_v1` selects the frozen structural two-gate OLS
Auto/Auto candidate on exactly two categorical intercept fixed effects. It
uses the first eligible initial-recurrence local16 boundary: packed/outcome
actions request the existing optional CMG build; final and unsupported roles
close economic proposals without a build. The second gate is constant Defer.
The owner state survives Low chunks and the final accuracy key; correction
closes economics. Existing direct selection, resource admission, cancellation,
certification and rollback remain authoritative. Public Auto/Auto used these
structural gates by default before gate v2; this selector remains available for
explicit tests.
