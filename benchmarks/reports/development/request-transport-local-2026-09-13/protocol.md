# Bounded local request transport: local repair protocol

Accepted native baseline 22b5106fcffb40dbc729832c45a269890daa5c0b; current main
81a96cd contains only wider-probe evidence. Baseline artifact is
ad39e2329579cf87295796379e12035805d923a3417b8021af37df0a198322f6.
The wider numerical prototype remains absent throughout this repair.

OLS/PPML adapters keep ordinary framed inline requests up to 4000 bundle bytes.
Longer requests pass the canonical byte length in one short bundle_local option;
the unchanged payload stays in fixed caller-local fereg_request_bundle. Native
uses the existing main-thread macro bridge, fallibly reserves at most 16 MiB of
metadata plus prefix/terminator, and requires the exact length, NUL placement,
UTF-8 and original closed framed-request validation before observation reads.
Reject mixed transports, malformed/zero/overflowing/over-cap lengths, missing or
truncated macros, malformed frames and stale handshakes without publication.
Release loaded transport storage after parsing. No arbitrary macro name, global
macro, new scientific wire policy, data-plane work in ado or second native call.

Validate complete ordinary/bundled/local OLS and PPML scientific equivalence,
error/store/preflight/ABI checks, and original wide public PPML case including
warm-up with its exact N/K/weight/VCE/seed. Use focused independent OLS and PPML
dummy oracles on both artifacts. Add licensed round-trip/rollback coverage for
long OLS and PPML metadata and preserve the baseline r(1003) evidence. The new
macro transport's current host behavior must be demonstrated, not inferred from
the C bridge API alone.

This is a transport repair, not a speedup claim. After functional proof, compare
small/narrow and protected weak/low/saved and selected complete K50/129 commands
in a bounded paired local set. Record full command/phase/native/model/RSS cost,
sample/rank/DoF/covariance/certificates, retries/fallbacks and result identity.
Investigate command increases above max(10ms,5%). Failed old wide requests are
unavailable baselines, not timings; never divide successful time by failure.
No builds/tests alongside timing. Isolated artifacts only, no installed-dist
replacement, agents, SCC, push or qualification. Repair first, then resume the
original full wider-moment experiment against the repaired baseline.
