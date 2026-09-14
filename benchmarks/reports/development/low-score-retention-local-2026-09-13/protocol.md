# P3 low-memory score retention: prospective evaluation

Baseline is accepted 26d05b0, isolated plugin 8bfa9fc2. Preserve all prior saved
recovery/precision/admission behavior, main installed dist and unrelated work.
No agents or cluster work while #27 remains incomplete.

The low OLS fit writes projected generated columns to column-major scratch,
then QR mutates that file. Robust/clustered inference reconstructs raw selected
columns, computes means, projects them again, writes a second column-major file,
then transposes to row scratch. Existing fast caches do not apply. Direct reuse
of the overwritten QR file is scientifically invalid.

Evaluate retaining original projected columns before QR mutation and handing
the owned scratch to the same-call inference composer. Keep file ownership
outside CertifiedPreparedOlsFit so its PartialEq/result contract need not be
changed merely to carry a private transport. Ordinary/categorical robust and
clustered production have three entry sites. Shared PPML sessions and IID must
keep their current paths. An optional caller-owned scratch output is preferable
to a global cache or weight-generation reuse.

Consume actual post-QR generated-column identities when forming row scratch;
never assume all generated columns survive, that rank equals width, or that the
normalized intercept belongs to the finite design. Keep independent raw means
and score-design/covariance certification. Compare the reused initial projection
against the prior independently projected score design, especially weak/slope,
rank-deficient and extreme-between/within cases.

Before implementation, settle the incremental I/O-buffer reservation and full
file lifetime. Current file I/O buffers are up to 1 MiB each, plus transpose
workspace. Do not infer an admission proof from the low base's single 1 MiB
transpose allowance. Two retained generated-width files can require more disk
capacity than the prior max(N*generated, 2*N*rank) payload; rank deficiency is a
protected control. Report both scratch capacity and actual I/O/pass costs. Keep
RAII removal on all errors/panics/interruption and no new resident N-by-K copy.
A low cap must retain a fitting uncached route rather than fail solely because
an optional retained file was requested.

Use focused scratch/fit/inference tests and independent dummy/full-covariance
oracles first. Then screen original low PW two-way, low robust AW and a weak
categorical multi-FE case against the accepted baseline, with complete command,
projection, score-storage, native/RSS, scratch, iterations/fallback counts. Choose
and hash original requests before timing. Adoption requires a material command
or peak-memory benefit (5% and 10 ms for time); inspect protected increases above
both 5% and 10 ms. Include small/narrow, rank-deficient, slope/three-FE and tight
cap controls; add repeat seeds only after a successful screen. Do not overlap
builds/tests/oracles/generation/archive compression with timing. No claim yet.
