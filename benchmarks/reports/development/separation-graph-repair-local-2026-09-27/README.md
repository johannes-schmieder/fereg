# Sparse-zero PPML separation: bounded repair experiments

**Decision: all production candidates in this report are rejected.** Main retains
only the independently validated Schwarz memory correction from e4317d8e. No
sample, tolerance, certificate, weight or routing change from these experiments
is installed. This report is development diagnosis, not qualification or a
fixest timing comparison.

## What is proved about the original failures

The original two-FE sparse-zero N61440/K10/T4 input has **631 observations with
an exact two-intercept recession direction**, including **81 beyond the 550
zero-only-FE rows**. The three-FE counterpart has **580**, including **70 beyond
510 zero-only-FE rows**. This is an independent proof on the literal original
inputs, not a conclusion inferred from small fitted means.

`independent_graph_oracle.py` constructs integer FE potentials. Positive rows
identify their two endpoint potentials. Each zero row imposes an ordering
constraint between the resulting components. Collapse directed cycles; a
potential increasing along the remaining directed acyclic graph gives an exact
FE-only direction that is zero on every positive outcome and nonpositive on
all zero outcomes, with a strict negative value on each reported row. Additional
finite and third-FE coefficients are zero. This direction proves deletions; it
does not prove absence of other separation in a larger model.

`separation-graph-results.json` preserves every proven row, direction digest and
integer sign check. The DTA SHA-256 identities are:

- Two FE: `95171454f4642fc920f47440e3fef5d6bc22d28d3f9edda469e5ca65b1ea34b3`.
- Three FE: `dbbf2893d9f4f793e630d8605ce07c1d0b2cdc92262ca76353ee96a48276a174`.

Local regeneration initially differed in one control's numerical hash. The
inputs were therefore regenerated in the original pinned SCC environment and
transferred after both complete DTA hashes matched the frozen receipts. The
repair experiments below use those exact inputs. Fresh local cases have their
own recorded input identities.

## Attempts, including rejected results

The base is e4317d8e. Separate manifests, patches and plugin hashes bind the
experiments; original SCC receipts and the accepted computational snapshot are
unchanged. Raw licensed logs and binaries remain outside Git under
`target/performance-repair-20260927/`.

1. **Retained CG for numerical detector projections.** Separately budgeted CG
   gets both original cases beyond the initial separation solve. Publication
   subsequently fails r(430), with a joint-LSMR normal-equation certificate
   failure. Before failure, strict diagnostics show 81 and 70 zero rows still
   moving toward zero means. Their counts agree with the independent missing
   FE-direction rows above. The six-row analytic probe passes, but that is not
   adequate coverage. This replacement was rejected.
2. **Externally applied exact graph proof, diagnostic only.** Excluding the
   independently proven rows before invoking the CG prototype allows both fits
   to complete (1.292 and 2.930 process seconds on this Mac). This is a causal
   experiment on a deliberately changed input sample, not an accepted native
   repair or comparable timing claim. The estimator still performs its remaining
   sample processing. `separation-graph-pruned-results.json` records this scope.
3. **Graph proposal followed by the original MAP projections.** The native
   graph implementation matches exhaustive independent binary-potential
   solutions on all 512 three-by-three graph outcome patterns, also with reversed
   row order. The existing full-span/sign/ledger checks remain in place. An
   independent Poisson/Hessian/sandwich fixture passes 12 fits over two/three FE,
   unit/FW/PW weights, robust/cluster VCE, T1/T4, fast/low, offset and saved means,
   eta and residuals. Nevertheless both original large inputs still exhaust MAP
   (40.763 and 75.224 process seconds). Four predeclared fresh cases pass; their
   process clocks are 19.506, 50.488, 0.319 and 0.985 seconds. This incomplete
   repair was rejected.
4. **Graph proposal plus retained CG projections.** A final combined candidate
   has 38 passing separation tests and 54 passing memory tests. The new public
   oracle rejects its first fit with r(459): the prepared projection finite-score
   certificate does not pass. Categorical PPML, three-FE OLS and multipartition
   slope oracle controls pass. The six-case combined replay block was not run
   after that failed scientific gate. This candidate was rejected and removed.

A first graph build also used an unregistered diagnostic note. The publication
validator correctly rejected it with r(3499); the subsequent build used the
existing registered IR proposal note without changing the numerical algorithm.
This failure and all substantive failures remain in `failed-attempts.json`,
`decision.json` and the standard licensed fixture receipts.

The graph-only and combined implementations are preserved in the patches and
`prepared_separation_graph.rs`. The experimental Stata oracle is preserved here,
not registered as a passing production fixture. The source manifests identify
parent commits plus modified-file hashes; their parent field is not a claim
that the experiment equals that clean commit.

## Scientific interpretation and next implementation boundary

This is a native separation problem, not merely a comparator discrepancy. The
initial MAP failure is reproducible on a six-row exactly representable response
with tiny boundary weights. Plain CG can return a numerical no-separation result
while explicit FE directions still exist on the original full inputs; stronger
publication checks prevent acceptance in these observed cases.

The graph proof solves one part of that problem efficiently and exactly. The
shared finite-plus-FE weighted projection remains a blocker, including validation
of directions already known to lie in an FE subspace. The graph-only public
error does not establish whether the failing MAP call is its span check or a
later IR projection. The combined error localizes to the finite-score check;
coefficient rounding, residual normalization and response/control error allocation
have not yet been individually adjudicated.

The next coherent candidate should carry explicit FE coefficient products into
an independently checked model-span proof, then implement a reliable current-
weight finite-plus-FE projector for any remaining numerical IR work. It must
handle near-zero projected responses without relaxing rank or finite-score
criteria. Reusing the current generic CG entry point is not sufficient evidence
of that capability. A graph submodel can propose deletions but must never stand
in for full-model terminal separation certification.

The explored memory design admitted the maximum of graph traversal and retained
CG workspace before use and kept detector storage separate from later IRLS.
It was removed with the rejected candidate. No per-fit memory improvement or
current production repair is claimed. Known nonretaining-CG span instability and
static allocation-inventory drift remain unresolved.

`closeout-verification.json` records exact reproduction of both integer proofs,
restoration of all 210 accepted computational inputs, and the accepted Mac plugin
identity. No experimental production source remains.
