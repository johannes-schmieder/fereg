# ADR 0215: transfer unchanged multi-partition topology ownership

Status: accepted for local development, 2026-09-26; not qualification.

## Decision

Adopt **multitopology1**, based on **5e90bb3e** with four Rust overlays and one
licensed scientific fixture. Replace the single-source transfer constructor with
one constructor for ordinary single-source partitions, shared by OLS and PPML.
Do not retain overlapping single- and multi-partition implementations.
The [source-bound report](../../benchmarks/reports/development/ppml-multi-topology-local-2026-09-26/README.md)
contains exact artifacts, independent checks, timing distributions and failures.

ADR 0202's narrow PPML-only prototype remains rejected. Its small cases saved
0–2 ms. New million-row attribution establishes a materially larger budget:
about 16 ms final topology construction plus 2.5 ms singleton pruning, within
25 ms final categorical preparation. This successor is justified by that evidence,
consolidated ownership machinery and a predeclared public saving gate around
10 ms. It does not reopen threshold or residual-extrapolation tuning.

## Identity, sample and ownership

Preserve the existing 16,384-row eligibility floor and unweighted prepared-sample
requirement. Every bound FE recipe must contain one categorical source, and its
availability membership owner must exist. Unless singletons were explicitly kept,
check that every referenced level has more than one observation. No initial
singleton in any partition proves the joint pruning fixed point: no deletion can
start a cascade. Frequency and other supplied weights retain generic pruning;
so do interaction recipes, small samples and any singleton. PPML's coordinator
still owns separation and generation pruning.

At a fixed point, consume availability topology and move referenced code and
membership allocations. Source storage remains in candidate order; the registry
remains in first-effect-use order; repeated recipes share one registry entry.
Independently validate code ranges, complete level coverage, exact membership
and row counts before assigning unchanged original level identities. The final
immutable validated owner performs its full existing check. Malformed axes and
reservation failures fail closed, before any public result is committed.

This generalizes the former guard to compatible one- and multi-partition slope
availability as well as categorical designs. Unreferenced availability owners
are dropped; they confer no authority on the retained design. An extra large
one-FE slope oracle covers this extension and its frequency-weight exclusion.

Reuse is confined to immutable topology within the same preparation call. No
weight, weighted geometry, RHS, solver state or certificate crosses a call or
sample generation. Rank, samples, weight semantics, DoF, covariance, separation,
convergence, original-input score/model-space/objective/reconstruction checks,
final q*mu Hessian and transactional publication remain unchanged.

## Evidence and limitations

All **121 distinct focused Rust tests**, **ten distinct licensed fixtures**,
**231 public PPML states** and **12 explicit Poisson/Hessian/sandwich comparisons**
pass. All **79 native PPML pairs** and **48 OLS pairs** have exact scientific
exports. Tests cover source reorder/aliases, pointer-preserved ownership, unused
levels, malformed axes, every injected reservation failure, weighted singleton
cascades, changing generations and independent explicit-dummy inference.

Three rotating development pairs give fast million-row **875->861 ms** and low
**980->967 ms**, versus comparable stock **880/872 ms**. Fresh seed202609445
confirms **669->660** fast and **741->735** low on 737,280 rows. Six of nine fresh
families beat stock by at least 10% in every pair; the low numeric, four-slope low
and disconnected low cases do not. Retain the four-slope median loss **115->116**.
All 45 fresh stock comparisons pass. Four older-seed stock comparisons remain
unranked; no tolerance or output requirement is changed.

OLS's initial **63->67 ms** slope loss did not repeat in the predeclared five-pair
block (**65/65**). Small OLS losses remain: factors **207/209**, large categorical
**158/159**, one FE **16/17**. Both blocks are retained separately. This is not a
universal timing non-regression claim. No guard or source was changed after the
pilot or fresh validation.

Paired diagnostic model generation falls **49.30->35.28 ms** fast and
**49.05->35.77** low; containing final categorical preparation falls
**25.10->9.91 /25.01->10.39**. Full membership validation remains. Stop this
preparation experiment here; its remaining checks are not the next target.

The transfer adds only model-sized owner/mapping vectors. It removes repeated
N-sized code/membership construction for eligible multi-partition samples.
Existing admission remains conservative; no allowance is reduced or enlarged.
Whole-fit tracked peaks change **-240..0 bytes**, scratch is unchanged, and
modeled bounds vary **-48..+112 bytes**. Removed preparation storage does not
imply an equal whole-command peak saving. Physical caps, static-inventory drift,
known constrained-fit failures and deferred platform qualification remain open.

The next substantial boundary is canonical retained-rank construction, measured
at **222/276 ms** in the large fast/low diagnostic fits. Its per-call graph plan,
weighted incidence preparation and low-mode finite/reconstruction passes require
a coherent lifecycle redesign; another small preparation adjustment will not
close the remaining **69/182 ms** development deficits to 0.90 stock.
