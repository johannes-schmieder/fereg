# ADR 0182: vectorized bounded estimation-request framing

Status: accepted for local development, 2026-09-25. Not qualification.

## Decision

Replace the interpreted per-field framing loop in the OLS and PPML ado
transactions with vectorized Mata operations over their internal metadata list.
Stata remains the control plane. No observations, FE codes, generated columns,
scientific products or estimation loops enter Mata. Keep the existing count and
byte-length wire format, inline/local transport crossover, native metadata cap,
decoder, single substantive plugin call and transactional result publication.

The internal list consists of unquoted key=value fields separated by semicolons;
spaces within values remain content. Trim each field and prefix its UTF-8 byte
length using nonexponential integer formatting. An empty list produces `0:` and
still faces the unchanged native required-field validation. This is not a new
general-purpose user-string parser or public interface.

Own the input string explicitly until tokenization finishes. The
[Mata parsing contract](https://www.stata.com/manuals/m-5tokenget.pdf) makes the
input lifetime significant. Drop temporary Mata objects and clear the consumed
raw Stata metadata before plugin entry. Storage is proportional to metadata
bytes and field count; there is no observation-sized bank or persistent function.

## Error lifetime

Stata tempnames alone do not dispose of Mata externals after an aborted ado call.
A focused probe demonstrated this, and a second probe demonstrated that a
multi-name drop with a missing name does not guarantee partial-prefix cleanup.
Within a bounded `nobreak` section, capture framing errors, save the original
return code, and independently attempt to drop each temporary. Propagate that
original error after cleanup. Break is deferred only over metadata framing and
cleanup; it cannot leave constructed externals behind between these statements.
The existing held-estimate transaction remains responsible for restoring public
state. No failed frame enters native computation.

The successor probe injects errors at six construction boundaries and checks
that all three external symbols are absent after each failure and after success.
It does not simulate physical memory exhaustion. The final framing block also
agrees byte-for-byte with the old independently interpreted encoder for spaces,
punctuation, embedded value quotes, UTF-8 and empty input.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/ppml-command-framing-local-2026-09-25/README.md)
preserves same-command phase attribution, every prototype, failures, public
rotations, independent oracles and exact source identities. The Rust/C inputs
and actual native plugin are unchanged. Only the two ado transactions differ.

Final fresh PPML medians improve 46 to 44 ms on narrow categorical and 101 to
100 ms on disconnected categorical. Across seven families there are 21 improving
pairs, six losses and eight ties; serial categorical loses 1 ms in its median.
The change is retained for its small common overhead benefit without a new
solver, tuning guard or data-sized allocation. It does not establish the broad
10% advantage. OLS controls retain exact scientific exports, with timing losses
reported alongside the gains and a bounded confirmation of noisy controls.

All 390 PPML public states, including diagnostic calls, pass independent score,
model-space and objective replay. All 240 baseline PPML comparisons, 120 direct
candidate/stock comparisons and 30 OLS pairs pass. Final acceptance includes
three licensed explicit-design/Poisson/Hessian/sandwich fixture executions.
The focused static screen has 25 passes and two independently reproduced parent
failures; neither is relaxed or repaired here. No Rust numerical suite or
cross-platform qualification is claimed as a new execution.

Mata's allocator may retain released capacity; no RSS or physical-cap equivalence
is inferred from symbol cleanup. The native memory model and its documented
allocation-inventory failure remain unchanged. Existing scientific discrepancies,
constrained-fit failures and platform gates remain open.

The phase evidence directs subsequent work to canonical construction and ordinary
joint solves. Interrupt round trips consume only 0.3–0.6 ms on the diagnostic
families and do not justify another optimization cycle. Keep this metadata work
closed unless a new broader change naturally affects it.
