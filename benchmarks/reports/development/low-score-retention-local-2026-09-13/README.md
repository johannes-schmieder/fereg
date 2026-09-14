# Low-memory OLS score retention: adopted local checkpoint

Adopt [ADR 0065](../../../../docs/adr/0065-low-memory-ols-score-retention.md)
against accepted `26d05b01a0bdfb2125ba502adcbd06ccb623cde2`. Ordinary low-memory
robust/clustered OLS now retains the original projected generated columns in
private column-major scratch before finite QR overwrites its working file.
Inference consumes the actual post-QR selected column identities, avoiding its
second projection. It still builds raw means independently, transposes bounded
row tiles and runs the original score-design and covariance certificates.

## Ownership and scientific boundary

The private scratch owner travels beside, rather than inside, the certified fit
result. It cannot change result equality or become a cross-fit/generation cache.
All ordinary/categorical robust and clustered entry points share this handoff;
IID, fast-mode policy and retained PPML sessions keep their existing paths.
No tolerance, rank rule, weight scale, sample, DoF, saved-output precision,
normalization or transactional publication policy changes.

The extra file borrows the working file's existing I/O bank during copying and
takes that bank after QR finishes. It adds no second large I/O buffer during
projection/QR and no resident N-by-K matrix. Inference replaces its previous
column scratch and avoids its N-element extraction buffer. Its existing row
scratch, transpose tile, raw-mean block and certificates remain. A fixed file
handle/path is additional metadata. File creation/write failure discards the
optional copy and retains the old uncached inference path. Failed writes return
the donor bank; private files retain their ordinary-error/panic cleanup rules.

For N rows, G generated columns and final finite rank R, logical scratch peak
changes from `8*N*max(G,2*R)` to `16*N*G` when retention succeeds. The retained
column/row transpose overlap is `8*N*(G+R)`, no greater than the fit peak because
R <= G. All sizes/offsets remain checked. These are file-capacity bounds, not
physical disk-traffic or resident-memory measurements. The larger disk footprint
is an explicit tradeoff; this is not an allocation-free fit.

## Source and artifact identities

[Identities](built-identities.json) bind 178 inputs and isolated universal plugins:

- Baseline: `8bfa9fc20a19de01055ac8c551266d9dba4ec6581a2f5a55364d29537a252f69`.
- Candidate: `365bee73f4b08ceacd1f6feef35b8fbb8ddeb97462f573c6a5e41bc731b24e0c`.

The [patch](implementation.patch), [source archive](source-archive.json),
[script identities](script-identities.json) and captured input hashes identify
what was tested. Both architectures were forcibly rebuilt in an isolated source;
main and installed dist were not built. All 178 candidate inputs match main.
The installed plugin and unrelated work remain preserved. These are local
M2 Ultra/Stata-MP development results, not SCC, Windows, external-comparator or
release qualification results. No push occurred.

## Complete-command and phase results

The [initial protocol](protocol.md), [follow-up](followup-protocol.md) and
[memory/rank supplement](memory-followup-protocol.md) precede their respective
calls. Original full/warm data, weights, seeds and numerical tolerances are kept.
There are 13 untraced pairs and eight separate trace/cap calls, 34 commands total.
The weak target has three rotated-order repeats on the same input, not three
independent seeds. All pairs pass full b/V, sample/status/DoF and discrete
initial/final solver comparisons; maximum paired b difference is zero and
maximum V difference is 1.58e-15. No untraced pair exceeds both 5% and 10 ms added
command cost. [Summary](adoption-summary.json), [all calls](summary.csv), and
[content-addressed captures](captures.json) retain the results.

| Case | N / K / workers | Command seconds | Native peak MB, baseline → candidate | Scratch MB, baseline → candidate |
|---|---|---:|---:|---:|
| Weak two-FE AW robust, same-input medians | 30720 / 10 / 4 | 0.198 → 0.148 | 22.95 → 22.95 | 4.92 → 5.41 |
| Wide one-FE AW robust, screen | 983040 / 50 / 16 | 40.737 → 38.702 | 553.31 → 553.28 | 784.85 → 800.55 |
| One-FE PW two-way cluster, screen | 245760 / 16 / 16 | 0.929 → 0.962 | 66.35 → 66.35 | 62.72 → 66.64 |
| Absorbed slope AW robust, control | 30720 / 16 / 4 | 0.270 → 0.229 | 9.85 → 9.67 | 7.37 → 8.36 |
| Three-FE robust with saved output, control | 30720 / 16 / 4 | 0.451 → 0.373 | 29.74 → 25.32 | 7.86 → 8.36 |
| Rank-deficient AW robust with saved output, supplement | 30720 / 50 / 4 | 1.598 → 1.422 | 40.59 → 38.20 | 24.08 → 25.07 |

The repeated weak gain is **25.25%** (45–52 ms per pair). The separate trace
locates it in score storage/construction: **56.40 → 4.37 ms**, with **0.40 ms**
copy cost for 11 generated columns. Independent score certification remains
8.47/8.50 ms; covariance work remains near 11 ms. Source inspection locates two
removed five-column inference projections; these are source-derived call counts,
not a new hardware-traffic counter. The old `reabsorbed` trace field is zero on
both scratch paths and must not be read as proof that neither projected.

Wide AW's screen reduction is **4.99546%**, below the prospective 5% gate; it is
not rounded into a qualifying gain. Its candidate-only diagnostic spends 128 ms
copying 51 columns, 0.759 s in score storage, 6.147 s in score certification,
7.245 s in covariance and 6.495 s in covariance certification. The latter three
unchanged checks remain substantial. Without a paired baseline phase trace, the
entire 2.035 s screen difference is not attributed to retention. PW costs
**33 ms/3.55%**, and sampled RSS rises **0.157 → 0.207 GiB**. Wide AW sampled RSS
also rises **1.055 → 1.084 GiB**. No general native/RSS saving is claimed.

Small/narrow, no-FE, FW interacted cluster, complete fast IID and the original
saved rank-deficient IID control all pass. The latter adds 5 ms/0.69% but does
not exercise retention; that coverage error is preserved and corrected by the
separate robust request on identical data. The robust supplement retains all
51 generated columns but selects only rank 49, proves retention in a separate
trace, and saves 176 ms/11.0%. These control gains are single-pair observations.

## Memory admission and fallback

The weak route's public execution base is 9,405,384 bytes, while its native peak
is about 22.95 MB. The base is not the full joint bound. The
[scalar replay](low-admission-replay.json), linked to the candidate's isolated
tested core library, uses actual finalized levels 10240/3649, four workers and
up to five RHSs. Execution plus Schwarz LSMR is **75,268,168 usable bytes**;
execution plus CG is **13,911,784**. This is the existing absorption model, not
universal whole-action reporting or an exact allocation prediction.

Both original plugins succeed at each shared cap. At the larger cap they keep
LSMR and exact unrestricted science. At the smaller cap they select the existing
admitted CG route, retain identical baseline/candidate solver decisions and peak
at **11,634,974 / 11,632,710 bytes**. Full b/V, sample/rank/status/DoF and original
certificates pass; maximum capped-versus-unrestricted V difference is 1.92e-13.
Candidate traces confirm retention at both caps. No cap was raised after failure.
All four cap calls are excluded from timing aggregates. Broader public memory
reporting and other-route accounting remain P0 work.

## Validation and retained failures

[Validation summary](validation-summary.json) and [licensed receipts](licensed-validation-receipts.json)
record 38 focused core checks, 95 full serial plugin tests, 49 candidate positive
licensed fits and four baseline supplement fits. Coverage includes all four
weight kinds, independent dummy/sandwich and two-way CGM/PSD checks, raw means,
rank-deficient generated-column selection, slopes/three FE, bounded tiles,
file cleanup, buffer return after an injected write failure, and an independently
rejected corrupted retained score column. Universal/static and actual arm64 ABI
checks pass. Process returns and explicit terminal sentinels are retained.

The first compile failed because selective formatting lost three new imports;
it ran no tests. The imports were repaired before the final-source tests/build.
The original layout fixture passes two FE, three FE and slope cases, then fails
closed on the unsupported factor-by-continuous finite request `i.f##c.x1` with
r(498), on both plugins. It is preserved as a negative boundary. A distinct
supported categorical-interaction fixture passes both plugins. These are not
rewritten passing receipts; see [failure evidence](failure-evidence.json).
The supplemental layout checks cover finite b/V and residuals; the paired command
comparisons additionally cover full published b/V, including the intercept.

This closes P3's duplicate low-memory score residualization and P6's bounded
column/scratch handoff item. It does not close broader generated-column evaluation,
slope/three-plus-FE layout optimization, PPML state/passes/Schur, P0 reporting,
P7 scheduling or R1–R5. Complete #27 before the separately authorized agents,
upstream CMG assessment and small SCC campaign.
