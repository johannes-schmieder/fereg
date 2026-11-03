# Exceptional OLS rank verification

This note gives the mathematical obligations behind the exceptional OLS path
in [SPEC §8](SPEC.md#8-rank-and-collinearity) and
[SPEC §16](SPEC.md#16-certificates-and-errors). Its scope is the supported
structural intercept case and one or two categorical-intercept FE terms.
Higher-way and slope absorption, PPML and the ordinary path retain their
existing policy. Validation and qualification are recorded in
[VALIDATION](VALIDATION.md).

The implementation separates [source reconstruction](../crates/fereg-core/src/ols/prepared_fit/one_shot/rank_resolution.rs),
the [forest bound](../crates/fereg-core/src/ols/prepared_fit/one_shot/rank_repair.rs),
[finite rank verification](../crates/fereg-core/src/ols/prepared_fit/finite/rank_bound.rs)
and [score/residual verification](../crates/fereg-core/src/ols/prepared_fit/one_shot/rank_score.rs).

Treat the supplied floating-point source values and the positive values
returned by `NormalizedWeights::value` as exact real numbers. Write
$W=\operatorname{diag}(w_i)$,
$\langle u,v\rangle_W=u'Wv$, and $\|u\|_W=\sqrt{u'Wu}$.
For two categorical effects, changing the sign of the second effect's columns
gives an incidence matrix $A$ with the same FE span. Let $P$ be its
weighted orthogonal projector and $M=I-P$. The exact finite problem uses
$a_j=Mx_j$ and $b=My$. These definitions concern the represented weight
frame, not exact rational weight normalization or an unrounded data-generating
process. Storage rounding can also create a small nonzero cycle from intended
additive decimal values; that represented cycle belongs to the finite problem.

The ratio $\|Mx\|_W^2/\|x\|_W^2$ can approach zero under
$x\mapsto x+A\gamma$ without changing the finite problem. It therefore
triggers investigation in this scope. Equality of original values within an
included partition can prove absorption directly. Otherwise the exceptional
path reconstructs from the source values; a small computed within norm alone
does not prove absorption.

**Projection error from a feasible forest flow.** A deterministic spanning
forest supplies an arbitrary floating-point FE potential $\alpha$. Its
accuracy is immaterial to quotient preservation: subtracting any vector in
the FE span preserves $Mx$. After deflation and a fresh projection, the
implemented row reconstructions have the form

$$
 t=x-A\alpha+\eta_0,\qquad
 \widehat a=t-A\widehat\alpha+\eta_1,\qquad
 \|\eta_0\|_W\le\rho_0,\quad \|\eta_1\|_W\le\rho_1.
$$

Error-free `TwoSum` expansions enclose each row subtraction; outward weighted
norm accumulation gives $\rho_0,\rho_1$. The potential-generation error
does not enter these bounds. If a solver exports no coefficient product, its
returned rows only supply a guess for another forest potential. Fresh source
reconstruction and the following certificate still determine acceptance.

Let $g=A'W\widehat a$ be the exact moment vector of the represented
reconstructed rows. Sum each nonroot subtree's moments and route that sum
along its parent observation edge, with the incidence sign. This defines a
flow $f$ satisfying $A_T'f=g$. Component compatibility follows from
the incidence identity: the exact component moments sum to zero. The proof
does not alter rounded moments to enforce that identity. Set $z_e=f_e/w_e$
on tree edges and zero elsewhere. Then

$$
 A'Wz=g,\qquad P\widehat a=Pz,\qquad
 \|P\widehat a\|_W\le\|z\|_W
 =\left(\sum_{e\in T}f_e^2/w_e\right)^{1/2}.
$$

The implementation encloses weighted products, node sums, subtree sums and
the final norm, obtaining an upper bound $F$ on this flow norm. Since
$M\widehat a=Mx+M(\eta_0+\eta_1)$, contraction of $M$ gives the
implemented sufficient bound

$$
 \boxed{\|\widehat a-Mx\|_W\le E=F+\rho_0+\rho_1.}
$$

The structural intercept and one-effect cases use the corresponding
constant/star incidence representation. An exactly zero reconstructed
defect with zero arithmetic error proves FE membership. Nonzero uncertainty
does not establish exact membership. Small local moments by themselves are
insufficient: the tree weights and geometry enter $F$ explicitly.

**The complete finite design must be covered.** Once repair activates, every
generated column and the outcome receive a source-based bound, including
columns whose original within/raw ratio looked ordinary. Otherwise an
ordinary column's projection error could conceal its dependence on a repaired
column. Structural zeros have exact zero rows and zero error. Nonzero repaired
columns use recorded within norms $s_j>0$ as finite solve scales. Define

$$
 Z_j=W^{1/2}a_j/s_j,\qquad
 \widehat Z_j=W^{1/2}\widehat a_j/s_j.
$$

The accumulated standardized error $\Delta$ bounds
$\|Z-\widehat Z\|_F$. The code uses a conservative sum of column bounds,
with norm/division allowances, rather than relying on cancellation between
column errors. Original source scales remain available for reconstruction and
reporting.

**Retained and dropped coordinates have separate obligations.** The existing
deterministic factorization selects a basis $J$ of size $r$. Let $G$
be its computed Gram block, $H=Z_J'Z_J$, and $C$ the stored candidate
inverse. An outward bound $d_G\ge\|G-H\|_\infty$ combines Gram formation
error with the projection perturbation

$$
 \|\widehat Z_J'\widehat Z_J-Z_J'Z_J\|_\infty
 \le\sqrt r\bigl(2\|\widehat Z_J\|_F\Delta+\Delta^2\bigr).
$$

The formation enclosure includes standardization, weighting and summation.
Its arithmetic precheck rejects unresolved underflow before a later
multiplication could magnify it; remaining product/sum underflow has an
absolute allowance. If outward bounds establish

$$
 h=\|I-CG\|_\infty+\|C\|_\infty d_G<1,
$$

the Neumann-series bound gives
$\|H^{-1}\|_\infty\le\|C\|_\infty/(1-h)$. Because $H$ is a
Gram matrix,

$$
 \sigma_{\min}(Z_J)\ge
 \sqrt{(1-h)/\|C\|_\infty}.
$$

The lower singular bound must exceed the existing relative rank threshold
$\tau$ times an upper bound on the largest true column norm. The
implemented Frobenius/report-scale envelope is conservative.

For each dropped column $j$, any finite stored vector $c$ is a valid
representation witness. Its computed inverse/Gram origin is not part of the
proof. A direct row scan encloses
$u_j=\|\widehat Z_j-\widehat Z_Jc\|_2$; then

$$
 \|Z_j-Z_Jc\|_2
 \le u_j+\Delta\sqrt{1+\|c\|_2^2}
 \le u_j+\Delta(1+\|c\|_2).
$$

The last upper bound must be at most $\tau$ times a **lower** bound on
the largest true column norm. An upper scale here would loosen the omission
threshold. No difference of Gram products certifies a dropped direction.
Together these checks certify independence of the selected basis and the
specified numerical dropped-span tolerance. They neither prove exact
algebraic dependence nor identify the names an exact QR would choose at a
pivot tie. Canonical slots and cross-worker omission agreement remain separate
requirements.

**Original-quotient scores and published residuals.** For the returned finite
coefficient vector $\beta$, let $e=b-\sum_j a_j\beta_j$. Independently
form $q=\operatorname{fl}(\widehat b-\sum_j\widehat a_j\beta_j)$.
If $E_y$ bounds the outcome projection error and $\rho_q$ bounds the
kept-order FMA formation error, then

$$
 \|q-e\|_W\le\delta
 =E_y+\sum_j|\beta_j|E_j+\rho_q.
$$

An outward upper bound for each original-quotient score is

$$
 U_j=|\langle\widehat a_j,q\rangle_W|_{\rm upper}
       +E_j\|q\|_{W,\rm upper}
       +(\|\widehat a_j\|_{W,\rm upper}+E_j)\delta.
$$

Positive lower bounds
$L_j=\|\widehat a_j\|_{W,\rm lower}-E_j$ and
$S=\max(\|\widehat b\|_{W,\rm lower}-E_y,
\|q\|_{W,\rm lower}-\delta)$ bound the true column and response/residual
scales from below. Acceptance requires $U_j/(L_jS)$ to meet the unchanged
score tolerance. The implementation evaluates equivalent scaled expressions
with outward intervals and checks positive finite denominators. It covers
retained, numerically dropped and positive-within-norm syntax-omitted columns.
Proven FE-span columns are handled by the FE certificate.

The repaired path performs a terminal FE projection and additionally checks
$\delta+\|r-q\|_W\le\texttt{score\_tolerance}\,S$ for its published
residual $r$. It recomputes every categorical level's local-span moment
at the requested final FE tolerance, using
$|\sum_{i\in g} w_i r_i|/(\sqrt{\sum_{i\in g} w_i}\max(\|q\|_W,\|r\|_W))$
with outward upper numerators and lower denominators. Exact zero residuals
satisfy these moments directly. An exactly absorbed outcome is accepted as
an exact zero case only with zero outcome error, zero finite coefficients and
zero published residuals. Bread/covariance checks and requested saved-effect
reconstruction remain required. These guarantees are not a uniform relative
coefficient-error bound for arbitrarily ill-conditioned finite designs.

**A rounding adversary.** On an unweighted complete two-by-two table, in
row-major cell order, set $L=2^{54}$,
$x=(L,-1,0,-L)'$, and $c=(1,-1,-1,1)'$. All source entries are
exactly representable. The cycle contrast is $c'x=1$, so the exact
two-way within column is $Mx=c/4$. Exact centering within the first
partition gives

$$
 ((L+1)/2,-(L+1)/2,L/2,-L/2)'.
$$

Correctly rounding these four exact results to binary64 produces
$(2^{53},-2^{53},2^{53},-2^{53})'$, whose cycle contrast is zero and
which lies entirely in the second partition's span. Thus even correctly
rounded intermediate projection rows can erase a nonzero quotient direction
present in the stored source. Revisiting those sources and bounding
reconstruction is essential; a certificate of the damaged intermediate
problem cannot recover the lost contrast.

**Bounded work and limits.** The preliminary per-column allowance is
`score_tolerance / (32 sqrt(K+1))`, where `K` is the generated-column count;
the assembled rank and score checks remain authoritative. There are at most
two projection attempts per nonstructural RHS, with a tighter second
tolerance and the remaining iteration allowance. One-partition verification
uses direct weighted means. Failure to establish the bounds is an unresolved
error, not a new omission or a weaker tolerance.

With no suspect ratio, the path adds no observation scan or changed numerical
arithmetic. Suspect structural columns require source equality checks. After
activation, forest construction and each flow/reconstruction pass cost
$O(N+L_A)$, where $L_A$ is the number of FE levels; projection costs
are additional. Score scans cost $O(NK)$, the inverse check costs
$O(r^3)$, and direct dropped-span scans cost
$O(Nr(K-r))$. Extra retained data are $O(N+L_A+K)$, besides existing
finite and solver workspaces. Sequential repair parents and the residual
witness are admitted explicitly; the final one-RHS session is constructed
after releasing the old session. These are complexity statements, not timing
results.

A weak forest, small tree-edge weights, near-boundary finite rank, extreme
arithmetic or interval width can make these sufficient bounds inconclusive.
The implementation does not promise successful resolution of every such
case, restore information absent from stored input, or extend this proof to
other FE families. Two independent abstract GPT Pro mathematical audits
examined quotient invariance, forward-error witnesses and adversarial
alternatives. The local implementation was then reviewed independently;
those reviews informed the design but do not replace the explicit bounds or
the scientific and performance evidence.
