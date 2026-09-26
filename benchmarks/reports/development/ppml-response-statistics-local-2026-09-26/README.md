# Single-use canonical response statistics: rejected

Local development evidence, 2026-09-26; not qualification. **Do not adopt
responsestats1.** The implementation saves categorical work, but its repeated
slope-control loss fails the predeclared retention gate.
[ADR 0218](../../../../docs/adr/0218-canonical-response-statistics-experiment-closeout.md)
restores the accepted computational source. The broad <=0.90-stock goal remains
active; this report does not transfer candidate gains to restored production.

## Identities and implementation

Baseline **residualrows2**, accepted at **ceb8f53d**, plugin SHA-256
`ef3ee205cab739352ccc1024500e26e9c7c874c609001c58cf0bc7038c32f519`.
Candidate **responsestats1**, base **04b5b792**, with overlays in
`ols/canonical_products.rs`, `ols/canonical_products/three.rs`,
`ols/linear_session.rs`, and `ols/prepared_fit.rs`. Plugin SHA-256:
`ad4dce9a467c13bc4b5540c8ecd430eab43a601cb4c48e111daf55e8bcc8a1ea`.
All **567 computational inputs** matched that artifact before measurement.
After rejection, all 567 again match residualrows2. Complete candidate source,
patch, declarations, build identities and restoration receipt are archived.

The canonical builders already compute current-response statistics, but their
rank consumer rescans the reconstructed response norm and constant norm; the
reported-intercept identity separately computes the original response mean.
The candidate extends the existing single-use moment handoff with three scalars:

- A stable norm accumulated on the actual reconstructed response values during
  the existing independent original-input certificate pass. It does not reuse
  the scaled Gram diagonal. Worker scratch holds a scale/square-sum pair after
  row checking; the existing stable merge consumes pairs in fixed worker order.
  The three-FE response-only builder uses its single worker's completed scalar.
- The constant-column norm accumulated in original row order during current
  weight normalization, using the same stable recurrence as the former scan.
- The freshly computed original response mean, used only for the same canonical
  fit's intercept identity. The independent raw-residual mean stays fresh.

Every quantity uses the current solver-weight divisor. No response statistic is
stored in retained design state or survives a weight/response generation change.
No new bank, pool, allowance, public option or checking frequency is introduced.
Control rank policy, current-weight projection, model-space, finite/score,
objective, reconstruction, final-Hessian, samples, weight semantics, inference,
DoF, separation and transactional publication keep their requirements.
Generic paths retain their norm/mean scans. Slope-bearing views decline the
categorical product builder; the measured slope loss is therefore not evidence
that the new row-statistic arithmetic caused it. Its binary-wide cause was not
localized, and no causal explanation is claimed.

Stock fixest is unmodified, archive SHA-256
`17372a789aaf9a3b04e8e0324feaf0f3c3c16f517baeb75e46289340332f0ee9`,
with glm.tol=1e-10, fixef.tol=1e-11, matched requested threads, BLAS one, and matched
inference/outputs. Public clocks include loading, estimation and publication;
harness setup/import is outside them. Compilation, diagnostics, audits and
archiving ran outside headline blocks. All completed fit monitors are clean.

## Public distributions and decision

B=residualrows2, C=responsestats1, S=comparable stock; medians in milliseconds.
[Ordered distributions](distributions.md) retain every pilot/fresh/OLS clock.
Pilot seed202609347, three rotations:

|Case; N/K/T|B / C / S|C/S paired range|
|---|---:|---:|
|Two-FE numeric low;983040/6/8|944 /918 /864|1.045–1.077|
|Matched fast|866 /844 /867|0.963–0.981|
|Two-FE two slopes low;30720/6/4|71 /70 /120|0.583–0.587|
|Three-FE two slopes fast;61440/10/8|147 /153 /231|0.662–0.672|
|Three-FE factors, cluster, fast;153600/19/8|562 /561 /701|0.787–0.832|
|Two-FE serial fast;245760/6/1|539 /534 /937|0.569–0.576|

B/C: **10 wins, six losses, two ties**. Large low/fast improve 2.8/2.5%,
but the three-FE slope control loses 4.1%. Four of six families meet 0.90 stock
in every pair. The candidate is not changed after this screen.

Fresh **seed202609448**, five predeclared rotations, is consumed. All 45 direct
stock scientific comparisons pass. This does not resolve older discrepancies
on other seeds or default-reference accuracy failures.

|Fresh case; N/K/T|B / C / S|C/S paired range|
|---|---:|---:|
|Two-FE numeric fast;737280/6/8|657 /648 /762|0.839–0.867|
|Matched low|712 /698 /765|0.907–0.951|
|Two-FE numeric fast;368640/6/4|414 /408 /665|0.605–0.624|
|Unbalanced three-FE factors, cluster, fast;184320/19/8|665 /669 /3202|0.204–0.214|
|Three-FE two slopes fast;76800/10/8|190 /195 /300|0.613–0.650|
|Three-FE four slopes low;38400/6/8|114 /116 /116|0.974–1.026|
|Two-FE serial fast;245760/6/1|540 /536 /934|0.571–0.597|
|Weak_d3 fast;61440/6/4|131 /131 /777|0.166–0.171|
|Disconnected two FE low;61440/6/4|93 /91 /99|0.909–0.949|

B/C: **29 wins, 14 losses, two ties**. Six of nine families meet 0.90 in every
stock pair. Low large, four-slope and disconnected families miss that target.
Fresh fast/low gains are 1.4/2.0%; fresh two-slope loss is 2.6%. Factor and
four-slope median losses remain explicit, without claiming their cause.

One bounded repeat of the pilot three-FE two-slope control was declared before
running it. Full and warm numeric input hashes match the pilot exactly; source,
seed and definitions are unchanged. Five ordered B/C pairs in ms:
**151/151, 148/152, 146/152, 148/150, 152/151**. Medians **148/151** repeat the
loss. The pilot, fresh and repeat blocks stay separate, not pooled to erase it.

The declared gate required useful public savings without a reproducible control
regression. It fails. Reject this implementation, preserve its evidence and
restore ADR 0216 exactly. Do not tune layouts, seeds or thresholds around this
probe. Its mathematical handoff is useful evidence for a future materially
larger design, not an accepted production optimization.

## Measured attribution

Separate **rankprofile2/responsestatsprofile1** use the same phase instrumentation,
two alternating repetitions and no stock ranking. Diagnostic candidate plugin:
`45c79117cab0917c7064e258691fa610bc4722ef091b59410c8d34b400bd983d`.

|Phase, summed across canonical calls|Fast B / C ms|Low B / C ms|
|---|---:|---:|
|Canonical setup|39.553 /45.593|39.930 /45.219|
|Contained weight preparation|7.586 /12.433|7.614 /12.424|
|Fresh certificate/products|22.803 /22.506|21.852 /22.569|
|Separate finite norm scans|15.726 /0.003|15.687 /0.004|
|Reported intercept/bread|19.844 /10.575|20.008 /10.407|
|Independent finite scores|13.789 /13.555|13.643 /13.611|
|Containing canonical retained solve|221.847 /202.761|249.282 /229.555|
|Whole plugin|832.445 /812.727|911.765 /889.088|

Weight preparation adds about 5 ms. Norm/intercept consumers save about 25 ms;
complete canonical construction saves about 19–20 ms. This is a net saving,
not a claim that moved work vanished. Containing spans nest: do not add table
rows indiscriminately. The factor diagnostic's whole plugin is 489.756/485.375;
its public fresh median loss still remains in the retention decision.

## Science, memory and unsuccessful attempts

The candidate passes **76 distinct Rust tests** and **nine licensed fixtures**
with terminal rc=0 sentinels. New norm oracles use independent hypot reductions
on actual materialized rows, zero and 1e-200..1e200 response scales, current-weight
means, two/three FE, T1/T4, changed weights and response-only construction.
Existing weighted contrasts, T1/T4/T8, disconnected/nested/rank-deficient axes,
exact/minus-one-byte admission, scratch corruption, strict rollback and
intercept/bread perturbation checks also pass.

**223 public PPML states** and **12 explicit-design Poisson/Hessian/sandwich
comparisons** pass. All **78 native PPML pairs** and **18 OLS pairs** have exact
scientific exports. Anchored comparisons pass **144/145**, direct C/stock
**66/67**. The one existing small three-FE full-output residual discrepancy
remains unranked. There are no incomplete pairs or new scientific discrepancies.

OLS B/C medians: **64/63, 75/75, 14/14, 213/207, 159/162, 18/16**. Preserve the
large OLS 3-ms loss; no repeat was needed after the PPML rejection gate failed.
No application failure, timeout, interrupted fit or detected contention occurred.
An initial test-only Vec/slice type error prevented compilation; all seven failed
selector receipts and their snapshot are preserved. It was fixed before either
measured artifact was built. ABI panic-containment self-tests passed.

Tracked whole-fit heap and scratch peak deltas are **zero**. Modeled bounds vary
**-112..+48 bytes** with unchanged allowances. The proposed scalars/worker locals
use existing fixed descriptor/stack headroom and dead certificate row scratch.
There is no physical-cap, Linux or qualification claim. Static inventory drift,
wire-minor/checkpoint-count failures, weighted constrained-fit/nonretaining-CG
failures, SCC deferral and stopped campaigns remain open. Frozen 77/16/111
benchmark definitions and pins are unchanged. No push or release.

## Current production gap and next boundary

Restored production itself meets 0.90 in all pairs of four of six pilot and six
of nine fresh families. Pilot fast/low median deficits to that target are
**85.7/166.4 ms**. Fresh large low, four-slope low and disconnected low deficits
are **23.5, 9.6 and 3.9 ms**. These are current baseline measurements, not
inherited candidate claims.

Move from canonical statistic handoffs to the **shared accepted-point and
working-state lifecycle**. Current baseline diagnostic IRLS-exclusive time is
**95.4 ms fast /76.2 low**, separate from roughly **40 ms of outer score checks**.
The exclusive total is not yet attributed entirely to row refresh and must not
be presented as wholly removable. Source inspection finds a concrete repeated
operation: accept_trial swaps a validated eta/mean pair, but the ordinary
end-of-iteration refresh computes exp(eta) again before forming current q*mu,
working response and score residuals. Trial eta formation and mean formation also
make separate passes. This boundary is shared by categorical and slope PPML.

Measure refresh_prepared_working_state, fill_means,
fill_working_state_and_scores, fill_line_search_trial_core and objective work
inside that existing total. Then design an explicit accepted eta/mean lifetime
and fused row transitions, using existing banks. Preserve invalidation after
predictor edits, trial rejection, normalization, strict rollback and sample or
bounds changes. Current-weight working state, independent scores/objectives,
two consecutive convergence passes and exact final-Hessian replay remain
mandatory. The roughly 49–53 ms of separation digest work is a separate measured
boundary; do not conflate it with working-state savings or cache changing inputs.

## Receipts

Work root: `target/ppml-response-statistics-20260926`. Restore the content-addressed
archive using `restore_receipts.py` and a new destination directory. All
**9,732 logical receipts** were restored and hash/privacy-verified. Archive:
`a6f4b6c3fa084f4e6dd32945618a46967f80bc23d1a664acb6858a56d5b0c073`.
The archive retains all artifacts, tests, controllers, monitors, distributions,
audits, diagnostics, candidate patch and exact restoration proof. Generated
binary datasets/output arrays are hashed and omitted; raw licensed logs are not
committed. Remaining path literals are reviewed shared-reference locations and
synthetic privacy fixtures, listed separately.
