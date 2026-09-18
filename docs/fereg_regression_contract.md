# Direct `fereg` regression contract

This contract protects public OLS while shared prepared-model, linear-solve,
inference, and memory mechanisms are extracted for `feppml`. It compares the
same `fereg` command in an exact baseline artifact and a candidate artifact.
External `reghdfe` and Greg comparisons remain separate acceptance evidence;
they cannot replace this direct internal comparison.

The machine-readable authority is
`benchmarks/manifests/fereg_f0_regression_v1.json`. The validator and receipt
evaluator are `scripts/fereg_f0_regression.py`.

The zero-overhead structural companion is
`benchmarks/manifests/fereg_allocation_source_contract_v34.json`, enforced by
`scripts/fereg_allocation_source_contract.py` from both repository checks and
normal Python test discovery. It does not replace measured F0 evidence.

The separately frozen public-command and staged-package surface is specified
by the [direct F0 command and package contract](fereg_f0_behavior_contract.md).

## F0 state and identity

The current F0 entry is deliberately `candidate_unsealed`, not frozen. It is
bound to:

- source commit `2dee5d8c4cd99b51bbaca600348f66f325b58ab4` and tree
  `cc10b50077f162cab8be10702a9627b99ee5eea2`;
- SHA-256 identities for `Cargo.lock` and `references.lock.toml`;
- ADO/plugin 0.5.0, action protocol 1.5, model wire 2.1, and OLS result wire
  1.4; and
- the exact detached-build universal macOS plugin, including size, SHA-256,
  and embedded source prefix.

The exact-SHA local full run is recorded under
`build_evidence/local/f0-2dee5d8/`. Its augmented receipt passed the Rust quick
record and all 35 licensed Stata/MP 19.0.115 full-profile fixtures, including
Break, and is hash-bound in the manifest. This does not seal F0. SCC
scientific/performance evidence and target-native Linux and Windows evidence
remain explicitly missing. A green comparison against an unsealed manifest
therefore returns `blocked`, never `pass`.

Changing F0 requires a reviewed manifest change. Historical receipts retain
their original manifest hash and cannot silently follow the change.

## Closed profiles

The manifest owns two non-overlapping inventories:

- `local_screen`: 46 cells at 100,000 and one million observations. In
  addition to the original dense/sparse, slope, moderate-K, memory, thread,
  fit, and saved-FE shapes, it directly covers robust VCE, fweights and
  pweights, one through five cluster dimensions, the parallel moderate-K
  clustered reducer, disconnected saved FEs, saved slopes, and every explicit
  solver/preconditioner route.
- `scc_governing`: 42 ten-million-observation cells covering the corresponding
  routes plus the governing constrained-memory case.

Evidence names one profile and must contain every cell in manifest order,
exactly once. Local evidence cannot satisfy the SCC profile, and evidence may
not add a favorable unregistered cell. Each cell carries its own complete host
and software environment because SCC array cells may land on different nodes.
Both arms within a cell use that one node and environment. The evaluator first
forms one mean log ratio per cell, so it never pools raw timing observations
across machines.

## Scientific gate comes first

Each cell contains baseline and candidate preflight records for return code,
sample and singleton identities, coefficient names and statuses, coefficients,
VCE shape and values, DoF, fit statistics, saved-output identities, solver
metadata, and independent certificates. The inventory is closed. Both arms
must return zero, converge, and satisfy every absolute certificate bound.

The active `extraction_exact` policy compares all scientific fields exactly,
including numeric arrays and certificate values. It additionally requires an
exact `numeric_hex_sha256`. Its preimage is compact sorted-key ASCII JSON, with
the canonical terminal newline, over keys `coefficients`, `vce`, `fit`, and
`certificates`. Their values are the complete row-major raw `%21x` strings from
`e(b)`, `e(V)`, `e(fereg_fit)`, and `e(fereg_certificate)`, respectively. This
is the rule for pure shared-engine moves: deterministic extraction may not hide
a numerical change or a signed-zero/last-bit difference.

The manifest also records the pre-existing fallback OLS tolerances, but leaves
them inactive:

| Field | Absolute tolerance | Relative tolerance |
| --- | ---: | ---: |
| coefficients | `1e-10` | `1e-7` |
| VCE | `1e-10` | `1e-6` |
| fit statistics | `1e-9` | `1e-7` |

Activating `registered_tolerance` requires an explicit accepted ADR recorded
in the manifest. A code extraction or PPML implementation cannot activate it
implicitly. DoF, sample, status, output, solver, and digest identities remain
exact even under that fallback.

ADR 0030 separately admits local development of the wide OLS finite solve.
Its controller records intentional numerical differences with the registered
OLS envelopes and independent certificates. That controller does not activate
this manifest's fallback policy or satisfy F0 qualification. Source inventory
v13 records the reviewed wide moment kernel, Cholesky dispatch and checked
memory reserve; the hard one-shot and transaction boundaries remain frozen.
Inventory v14 adds only the reviewed production tokens and error literal for
ADR 0031's arithmetic-preserving wide raw-cache blocking. Its allocation-site
inventory and hard boundaries match v13; local science comparisons remain exact.
Inventory v15 records ADR 0032's retained low-memory categorical LSMR
preparation, including its fallible weight copy and bounded descriptor
collections. The hard one-shot and transaction boundaries remain unchanged;
local comparisons require exact science.
Inventory v16 records ADR 0033's certified automatic LSMR/CG report merge.
Allocation sites and hard boundaries match v15. The bounded regression
fixture intentionally changes the prior report-only error into a successful
certified fit; unchanged successful cases still require exact science. This
does not activate the frozen F0 manifest's numerical fallback policy.

Inventory v17 records only the reviewed dependency-wiring change for
ADR 0034's fixed LSMR reduction order. The source allocation inventory and
hard boundaries match v16. The affected local captures retain exact
self-repetition and complete pinned-reference checks; intentional changes
in last-bit arithmetic across dependency revisions are recorded separately.
This does not activate the frozen F0 manifest's numerical fallback policy.

If any scientific comparison fails, the evaluator returns at the scientific
stage without parsing timing or memory records. Timing from a scientifically
invalid arm is never reported as performance evidence. Platform capture code
can enforce the same preflight by importing
`compare_scientific(manifest, baseline, candidate)`.

## Paired performance gate

Every cell contains nine warm pairs with the fixed order
`AB, BA, AB, BA, AB, BA, AB, BA, AB`, where A is F0 and B is the candidate.
For pair (i), the evaluator forms

\[
  \ell_i = \log(t_{B,i}) - \log(t_{A,i}).
\]

It reports the complete nine log ratios, their mean and sample standard
deviation, standard error, geometric-mean ratio, and two-sided 95 percent
confidence interval. With nine pairs it uses the frozen Student-t critical
value `2.306004135` for eight degrees of freedom.

The gates are:

- equal-weight geometric mean of primary-cell ratios no greater than `1.02`;
- no cell whose paired-log-ratio confidence interval lies wholly above parity;
  and
- no confirmed individual-cell geometric mean above `1.05`.

An initial cell above `1.05` or whose interval lies wholly above parity returns
`confirmation_required`. It must be rerun on the same node with another full
nine-pair alternating sequence. The confirmation replaces the initial run for
the aggregate and memory comparison; it fails if its ratio remains above
`1.05` or its interval still excludes parity. Unrequested confirmation data is
rejected to prevent post-hoc selection.

## Allocation, leak, constrained-memory, and RSS gates

For each arm, a cell records total native allocation, the observation,
observation-column, FE-level, and cluster dimensions that determine data-sized
storage, retained bytes after repeated calls, and optional constrained-memory
results. Those four fields seal the matched model shape; they are not tagged
allocator-event counts.

- The four model dimensions must be exactly equal, so allocation and RSS
  comparisons cannot substitute a smaller candidate problem.
- Candidate native allocation may exceed F0 by at most one MiB of fixed-size
  bookkeeping.
- Candidate retained bytes after repeated calls must be zero.
- The governing constrained cell must show `memory(low)` succeeding with
  return code zero and `memory(fast)` failing preflight with `r(909)` at the
  same byte limit for both artifacts.
- Candidate peak RSS may exceed matched F0 peak RSS by no more than the larger
  of two percent or 64 MiB.

The independent static contract closes the source-level allocation-class gap.
It keeps a closed inventory of both allocation-bearing syntax and complete
production token structure across the shared OLS execution closure: the ABI
dispatch, plugin adapter/input/result paths, and the core data, FE, linear
algebra, OLS, error/return-code, memory, metadata, option, parallel, protocol,
and sandwich modules. Core and plugin module wiring plus the complete plugin source-file
inventory are closed, so a `#[path]` redirect or alternate helper module cannot
substitute unwatched implementation bytes. The workspace, Cargo configuration,
core/plugin target, feature, and dependency wiring are bound semantically, and
the core build script is part of the exact source inventory; changing a crate
root or compiled feature therefore cannot leave decoy guarded files on disk.
The exact locked dependency graph is part of the Cargo wiring. The C ABI
bridge/header, pinned Stata SPI sources, export maps, and all three
platform build source lists are a second closed native-route inventory. This
also rejects C-side `malloc`/copy work that Rust allocator telemetry cannot see.
Allocation sites also carry automatic observation,
observation-by-column, FE-level, cluster, and model/fixed scale hints. The
complete token digest catches opaque constructors, cloning, and helper
indirection even when a new allocation does not use a recognized standard
container spelling. Ordered production string and character literals are
bound separately, because action constants, `cfg` discriminants, and module
paths can change the compiled route even when the normalized token stream is
unchanged.

The contract independently hashes the current ABI/OLS adapter, all six
IID/robust/clustered prepared-fit adapters, and the one-shot weighted-linear
entry seam. Ten adapter functions that already existed at exact F0 carry their
F0 token hashes in the checker and cannot be accepted merely by regenerating
the live manifest. Their ordered F0 literal payloads are pinned separately,
as are the original OLS action and confirmation literals, so normalized string
placeholders cannot weaken that guarantee. The original OLS conditional/call
branch inside the now-expanded ABI dispatcher is pinned as its own F0 seam, so
the new PPML sibling branch cannot redirect the old action. A new file, direct or helper-hidden
allocation, changed adapter route or import alias, or transitive entry into the
retained PPML session fails the ordinary repository gate until it is explicitly
reviewed.
Comments and whitespace do not alter the token contract. Production literal
changes are conservative review events, including diagnostic wording, because
Rust literals also select actions, features, targets, and source paths.
Items whose `cfg` expression is provably false when `test=false` are excluded,
including `all(test, ...)`; unknown feature/target expressions remain guarded.
Private PPML implementation files and the retained session implementation are
boundary-only: their PPML-only internal token/allocation inventories are
excluded, while their file identities and the module wiring around them remain
closed. The allocator phase guard fragment in the retained session is an
explicit exception: public one-shot OLS executes its constructor and destructor,
so that complete fragment has its own exact token, literal, and allocation-site
inventory. Changes to any other shared file remain fully guarded even when
their author believes the change is PPML-only. That conservative review is
intentional because the same compiled module is reachable from public OLS.

This static inventory is deliberately conservative and has no production
instrumentation or runtime overhead. It proves structural continuity, while
the matched F0 matrix independently adjudicates actual native peak bytes,
retained bytes, constrained-memory behavior, RSS, science, and timing. Neither
layer can stand in for the other.

## Evidence and receipt integrity

Evidence binds the manifest SHA-256, F0 and candidate commit/tree/lockfile
identities, platform, plugin SHA-256, size, and embedded commit prefix. The
evaluator rejects pending F0 platform artifacts, cross-platform arm pairs,
missing/extra cells, malformed cadence, nonfinite values, and identity
substitution. Local scientific certificate digests also expose the exact
preparation receipt, input config, dataset, and command SHA-256 identities, so
the final evaluator receipt remains auditable without trusting a loose path.

The governing result deliberately contains two source-bound comparisons, not
one cross-platform binary comparison. Licensed direct-command/package behavior
reopens baseline and candidate bundles on one identical native host/platform
(macOS is valid), and its baseline artifact must be the exact frozen F0
artifact for that platform. SCC science, performance, and memory compare the
Linux baseline/candidate artifacts on the same node. Both comparisons must
name the same exact source commit/tree, but their platform plugin bytes are
expected to differ. The formal PPML qualification aggregate cross-binds each
candidate artifact
to its own authoritative platform build/load receipt, including the same
source archive and toolchain contract.

The direct command comparison reopens canonical USTAR bundles containing the
exact fixture sources, sanitized Stata logs, raw process/Stata status,
terminal-sentinel counts, the complete staged package/plugin inventory, and
resources; normalized result rows cannot be hand-authored independently of
those bytes. SCC task receipts likewise inventory every retained raw run file,
and the offline verifier reparses those files to reconstruct each scientific,
timing, RSS, and memory record.

An evaluation receipt contains the complete raw evidence and derived metrics.
It is canonical-JSON sealed with SHA-256. Verification recomputes the evidence
hash and the entire evaluation against the currently named manifest. An
optional external identity file binds both the receipt file hash and payload
hash, so a modified-and-resealed receipt cannot replace accepted evidence.
Keep raw licensed logs out of Git; commit only sanitized receipts and their
identity records when the relevant gate is accepted.

## Commands

Validate the audited F0 object and manifest:

```bash
python3 -I -B -S scripts/fereg_f0_regression.py validate-manifest
```

Audit the current structural allocation and one-shot route inventory:

```bash
python3 -I -B -S scripts/fereg_allocation_source_contract.py audit
```

The independent [all-action inventory](allocation_inventory.md) additionally
watches PPML, separation and session bodies. Its passing result is structural,
not a complete memory-capacity certificate.

After an intentional reviewed allocation or route change, inspect regenerated
JSON before creating the next version of the frozen companion manifest:

```bash
python3 -I -B -S scripts/fereg_allocation_source_contract.py render
```

Capture a clean exact candidate identity:

```bash
python3 -I -B -S scripts/fereg_f0_regression.py source-identity --repository . --revision HEAD
```

Prepare the six immutable local datasets and the exact 46-cell command map
in a new directory. This is an untimed licensed-Stata phase. It verifies the
generator and OLS manifest from a clean exact-F0 checkout, adds and validates
the stable `obs_id`, hashes every dataset, and writes a canonical sealed
`preparation-receipt.json`:

```bash
python3 scripts/run_fereg_f0_local.py prepare \
  --manifest benchmarks/manifests/fereg_f0_regression_v1.json \
  --baseline-source /path/to/clean/f0 \
  --stata /path/to/StataMP \
  --output-root /path/to/new/f0-local-preparation
```

Run the closed local screen from that preparation. Both source checkouts must
be clean, both complete packages are staged into isolated resolver roots, and
every preflight and measured arm is a fresh Stata process. The runner refuses
an altered preparation receipt, dataset, command map, cell order, artifact,
source identity, or reused output path:

```bash
python3 scripts/run_fereg_f0_local.py run \
  --manifest benchmarks/manifests/fereg_f0_regression_v1.json \
  --input-config /path/to/new/f0-local-preparation/local-input.json \
  --preparation-receipt /path/to/new/f0-local-preparation/preparation-receipt.json \
  --baseline-source /path/to/clean/f0 \
  --baseline-plugin /path/to/f0/fereg_macos.plugin \
  --candidate-source /path/to/clean/candidate \
  --candidate-plugin /path/to/candidate/fereg_macos.plugin \
  --stata /path/to/StataMP \
  --toolchain 'rustc/cargo/clang exact versions' \
  --run-root /path/to/new/f0-local-run \
  --evidence /path/to/f0-local-evidence.json \
  --receipt /path/to/f0-local-receipt.json \
  --identity-output /path/to/f0-local-receipt.identity.json
```

The current unsealed F0 makes an otherwise green local receipt honestly exit
nonzero with status `blocked`; the evidence and receipt are still written for
review. A local screen is required diagnostic evidence, not a substitute for
the governing SCC profile.

Evaluate platform-produced evidence and write both receipt and external
identity:

```bash
python3 -I -B -S scripts/fereg_f0_regression.py evaluate \
  --evidence /path/to/evidence.json \
  --output /path/to/receipt.json \
  --identity-output /path/to/receipt.identity.json
```

The evaluate command exits nonzero for both failure and an honestly blocked
unsealed baseline, while still writing the diagnostic receipt. Verify a stored
receipt independently with:

```bash
python3 -I -B -S scripts/fereg_f0_regression.py verify-receipt \
  --receipt /path/to/receipt.json \
  --identity /path/to/receipt.identity.json
```

F0 advances through four explicit states; no command may skip one:

1. `candidate_unsealed` permits artifact discovery only.
2. `seal-artifacts` reopens the exact local receipt, baseline behavior bundle,
   target-native macOS/Linux/Windows load evidence, source archives, and the
   separately reviewed controller source. It writes an immutable artifact
   seal plus an `artifacts_sealed` manifest.
3. An exact `artifacts_sealed` F0-versus-F0 SCC run closes all 42 tasks, raw
   Stata captures, submitted script/qsub ledger, and live qacct records. Its
   evaluator verdict is deliberately `blocked`; it can publish only
   `complete_nonqualifying`, never `RUN.SUCCESS`. `freeze-baseline` reopens
   that entire diagnostic run and writes the qualification seal.
4. Only the resulting `frozen` manifest may govern the final exact
   candidate-versus-F0 SCC run. That run also requires the independently
   verified candidate-versus-F0 behavior comparison and can publish
   `RUN.SUCCESS` only after a recomputed passing receipt.

The SCC preparation copies the manifest-referenced external seal closure into
`input/seal-evidence` and seals a complete SHA-256/size inventory. Evaluator
code still comes from the exact harness git archive; seal bytes do not need to
be committed or smuggled into that archive. The closed verifier reopens both
roots. This separation keeps licensed raw evidence outside Git without
creating a synthetic frozen path.

Freezing is a manifest and evidence milestone, not an inference from a process
exit code. The complete executable sequence and evidence-spec schema are in
`benchmarks/scc/README.md`.
