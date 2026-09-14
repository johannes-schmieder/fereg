# ADR 0065: retain original low-memory OLS score columns

Status: accepted for local development, 2026-09-13.

## Decision

Ordinary low-memory robust and clustered OLS may retain the initial certified
projected generated columns in private column-major scratch before finite QR
mutates the working file. The owner travels directly from the fit to its
same-call inference composer, outside the certified fit result. Inference uses
actual post-QR generated-column identities to build its bounded row scratch;
no cross-fit or weight-generation cache is introduced.

Keep independent original raw means, rank/reference-scale policy, score-design
and covariance certificates, weight semantics, sample/DoF, final saved-output
precision and transactional publication. Fast-mode policy, conventional
inference and retained PPML sessions keep their existing paths. Failed optional
file creation/copy returns to the existing uncached score construction; failed
writes return the donor buffer and remove the partial private file.

The retained file borrows the working file's existing bounded I/O bank while
projection and QR need that bank, then takes it after QR finishes. This avoids
another large I/O bank during fit and adds no resident N-by-K matrix. Inference
replaces its former column scratch and avoids its N-element extraction buffer.
File handles/path metadata remain additional bounded state; row scratch, raw-mean
blocks, transpose tiles and independent certificates retain their prior plans.

For N rows, G generated columns and finite rank R, logical scratch peak rises
from `8*N*max(G,2*R)` to `16*N*G` when the optional copy succeeds. The later
column/row overlap `8*N*(G+R)` fits that bound. Checked dimensions, fallible
reservation, capacity validation and private-file cleanup remain authoritative.
This accepts a larger disk footprint, without expanding solver admission.

## Evidence and limits

The [source-bound report](../../benchmarks/reports/development/low-score-retention-local-2026-09-13/README.md)
binds 178 inputs and isolated plugins against accepted `26d05b0`. Thirteen
untraced pairs and eight trace/cap calls preserve scientific gates. Three
same-input weak two-FE AW repeats improve command medians 0.198→0.148 seconds;
a separate trace reduces score construction 56.40→4.37 ms for 0.40 ms copy cost.
Single slope, three-FE saved and rank-deficient robust controls also improve.
Wide AW is below the 5% gain gate; PW costs 33 ms/3.55% and uses more sampled RSS.
No untraced pair exceeds both 5% and 10 ms added cost.

Both plugins pass the same 75,268,168-byte usable LSMR cap and 13,911,784-byte
CG cap. The latter produces native peaks near 11.63 MB and scientifically
matching certified results. This establishes the exercised constrained path,
not universal whole-action reporting. Public execution-base telemetry remains
smaller than route-specific joint reservations.

Thirty-eight focused core checks, 95 serial plugin tests, 49 candidate positive
licensed fits, four baseline supplement fits and universal/static/native arm64
ABI checks pass. Tests exercise all weight types, rank-selected scratch columns,
independent dummy/sandwich checks, a corrupted retained score certificate and
buffer/file recovery after a failed optional write. The original unsupported
factor-by-continuous request and initial compile failure remain preserved.
No SCC, Windows or release qualification is claimed.
