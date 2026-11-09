//! One-level additive Schwarz preconditioning of the full system over
//! factor-pair subdomains (feature `schwarz`).
//!
//! **Subdomains.** For each chosen pair of effects ([`Subdomains`]: the
//! star of the largest effect with every other, or all pairs), every
//! connected component of the pair's bipartite level graph is one
//! subdomain ([`pairs`]); a pair's components partition its two effects'
//! levels.
//!
//! **Action.** `z = sum_i R_i' A_i R_i r` over the subdomains, `A_i` the
//! local action of [`local`] (block elimination of the larger side, an
//! exact or approximate Schur solve, mean removal and the `tau 11'/n`
//! augmentation), without partition of unity or coarse correction. Levels
//! in no subdomain (a design with one effect) get Jacobi. Each local action
//! is symmetric positive semidefinite, positive definite on its
//! component's levels, and the components of one pair cover its effects'
//! levels, so the sum is symmetric positive definite on all of coefficient
//! space whenever every effect is in a pair: the full formulation's
//! preconditioner contract for PCG and modified LSMR.
//!
//! **Determinism.** The pairs, components and slots are a fixed function of
//! the design; the cross-tabulation, Schur complements and factorizations
//! (approximate Cholesky with a fixed seed) of the design and weights. The
//! subdomains' local actions run in parallel into disjoint slots, and every
//! level sums its subdomains' contributions in pair order, so the action
//! does not depend on the worker count.
//!
//! **Memory.** Every allocation of this module is fallible; approximate
//! Cholesky allocates inside approx-chol. The build admits the pair storage
//! from its exact sizes and each local factor from its size (approximate
//! factors by an estimate) before allocating them, within the budget the
//! memory limit leaves beyond the projection's workspace. The apply
//! workspace ([`Scratch`]) is retained by the caller; repeated applies of
//! one width allocate nothing. A preconditioner is rebuilt only for new
//! weights (or another specification), between solves.

mod local;
mod pairs;
mod skyline;

use super::kernels::PARALLEL_WORK;
use super::structure::Structure;
use super::weights::State;
use super::{refill, with_capacity};
use crate::design::Effect;
use crate::elimination::Cancel;
use crate::error::{Error, Result};
use crate::projector::{PreconditionerUsed, SchwarzSolves, SchwarzSpec, Subdomains};
use local::{Inner, Piece};
use pairs::Pair;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Why a Schwarz preconditioner was not built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    /// It exceeds the memory budget.
    MemoryLimit,
    /// An exact factorization reached the rounding floor, or approximate
    /// Cholesky refused a Schur complement.
    Numerical(&'static str),
}

/// The memory budget of one build, shared by its parallel parts. Retained
/// and transient storage are both charged and never released, so whether a
/// build fits does not depend on the order of its parts.
pub(super) struct Admit {
    limit: Option<u64>,
    used: AtomicU64,
    failed: AtomicBool,
}

impl Admit {
    fn new(budget: Option<u128>) -> Self {
        Self {
            limit: budget.map(|b| u64::try_from(b).unwrap_or(u64::MAX)),
            used: AtomicU64::new(0),
            failed: AtomicBool::new(false),
        }
    }

    /// Charge `bytes`; `false` (remembered) once the total exceeds the limit.
    pub(super) fn take(&self, bytes: u128) -> bool {
        let Some(limit) = self.limit else {
            return true;
        };
        let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
        let before = self
            .used
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                Some(used.saturating_add(bytes))
            })
            .unwrap_or(u64::MAX);
        let fits = before.saturating_add(bytes) <= limit;
        if !fits {
            self.failed.store(true, Ordering::Relaxed);
        }
        fits
    }

    fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }
}

/// The apply workspace of one column width, retained across applies.
#[derive(Default)]
pub(crate) struct Scratch {
    local: Vec<f64>,
    inner: Vec<f64>,
    partials: Vec<f64>,
}

/// A built additive Schwarz preconditioner.
pub(crate) struct Schwarz {
    spec: SchwarzSpec,
    /// The weight state it was built for.
    generation: u64,
    /// Coefficient offsets of the effects (with the total).
    offsets: Vec<usize>,
    pairs: Vec<Pair>,
    /// Every pair's components in pair order, contiguous in the slots.
    pieces: Vec<Piece>,
    slots: usize,
    inner_units: usize,
    partial_units: usize,
    /// Per effect, the `(pair, side)` holding its levels, in pair order.
    covers: Vec<Vec<(usize, usize)>>,
    used: SchwarzSolves,
    bytes: u128,
}

/// The pairs of `subdomains`, side 0 the effect with more levels (the
/// first of equals).
fn pair_list(
    st: &Structure,
    effects: &[Effect<'_>],
    subdomains: Subdomains,
) -> Result<Vec<[usize; 2]>> {
    let k = effects.len();
    let orient = |x: usize, y: usize| {
        if effects[y].levels() > effects[x].levels() {
            [y, x]
        } else {
            [x, y]
        }
    };
    let mut list = with_capacity(k * k.saturating_sub(1) / 2)?;
    match subdomains {
        Subdomains::Star => {
            let hub = st.largest;
            list.extend((0..k).filter(|&e| e != hub).map(|e| orient(hub, e)));
        }
        Subdomains::AllPairs => {
            for a in 0..k {
                list.extend((a + 1..k).map(|b| orient(a, b)));
            }
        }
    }
    Ok(list)
}

/// What became of one piece's factorization.
#[derive(Clone, Copy)]
enum Built {
    Pending,
    Done,
    Refused(Refusal),
    Failed(Error),
}

fn count(value: &mut u32) {
    *value = value.saturating_add(1);
}

impl Schwarz {
    /// Build for the current weights within `budget` bytes, including the
    /// apply workspace of `width` columns.
    pub(super) fn build(
        st: &Structure,
        effects: &[Effect<'_>],
        state: &State,
        (spec, generation): (SchwarzSpec, u64),
        (budget, width): (Option<u128>, usize),
        cancel: &Cancel<'_>,
    ) -> Result<std::result::Result<Self, Refusal>> {
        let admit = Admit::new(budget);
        let list = pair_list(st, effects, spec.subdomains)?;
        let mut pairs = with_capacity::<Pair>(list.len())?;
        let mut pieces = Vec::new();
        let mut first = 0;
        for sides in list {
            cancel.check()?;
            let Some((pair, parts)) =
                pairs::build(st, effects, state.weights(), (sides, first), &admit, cancel)?
            else {
                return Ok(Err(Refusal::MemoryLimit));
            };
            super::reserve(&mut pieces, parts.len())?;
            for part in parts {
                let big = part.big;
                pieces.push(Piece {
                    pair: pairs.len(),
                    big,
                    relative: part.start,
                    start: first + part.start,
                    sizes: [part.sizes[big], part.sizes[1 - big]],
                    offsets: [st.offsets[sides[big]], st.offsets[sides[1 - big]]],
                    inner: Inner::Trivial,
                    inner_first: 0,
                    partial_first: 0,
                });
            }
            first += pair.slots();
            pairs.push(pair);
        }
        if !admit.take((pieces.capacity() * std::mem::size_of::<Piece>()) as u128) {
            return Ok(Err(Refusal::MemoryLimit));
        }
        if let Some(refusal) = Self::factor(&pairs, &mut pieces, spec, &admit, cancel)? {
            return Ok(Err(refusal));
        }
        let mut schwarz = Self {
            spec,
            generation,
            offsets: with_capacity(st.offsets.len())?,
            pairs,
            pieces,
            slots: first,
            inner_units: 0,
            partial_units: 0,
            covers: with_capacity(effects.len())?,
            used: SchwarzSolves::default(),
            bytes: 0,
        };
        schwarz.offsets.extend_from_slice(&st.offsets);
        schwarz.layout()?;
        if !admit.take(schwarz.bytes + schwarz.scratch_bytes(width)) {
            return Ok(Err(Refusal::MemoryLimit));
        }
        Ok(Ok(schwarz))
    }

    /// Factor every piece's Schur complement in parallel; the refusal of the
    /// build (memory first, then the first numerical one in piece order).
    fn factor(
        pairs: &[Pair],
        pieces: &mut [Piece],
        spec: SchwarzSpec,
        admit: &Admit,
        cancel: &Cancel<'_>,
    ) -> Result<Option<Refusal>> {
        use rayon::prelude::*;
        let mut outcomes = with_capacity::<Built>(pieces.len())?;
        outcomes.resize(pieces.len(), Built::Pending);
        pieces
            .par_iter_mut()
            .zip(outcomes.par_iter_mut())
            .for_each(|(piece, outcome)| {
                if admit.failed() {
                    return;
                }
                *outcome = match local::inner(&pairs[piece.pair], piece, spec.local, admit, cancel)
                {
                    Ok(Ok(inner)) => {
                        piece.inner = inner;
                        Built::Done
                    }
                    Ok(Err(refusal)) => Built::Refused(refusal),
                    Err(error) => Built::Failed(error),
                };
            });
        if let Some(error) = outcomes.iter().find_map(|o| match o {
            Built::Failed(error) => Some(*error),
            _ => None,
        }) {
            return Err(error);
        }
        if admit.failed() {
            return Ok(Some(Refusal::MemoryLimit));
        }
        Ok(outcomes.iter().find_map(|o| match o {
            Built::Refused(refusal) => Some(*refusal),
            _ => None,
        }))
    }

    /// Scratch offsets, covers, the local solves used and retained bytes.
    fn layout(&mut self) -> Result<()> {
        let (mut inner, mut partial) = (0, 0);
        for piece in &mut self.pieces {
            piece.inner_first = inner;
            piece.partial_first = partial;
            inner += piece.inner_units();
            partial += piece.partial_units();
            match piece.inner {
                Inner::Trivial => count(&mut self.used.trivial),
                Inner::Dense { .. } => count(&mut self.used.dense),
                Inner::Skyline(_) => count(&mut self.used.skyline),
                Inner::Approximate { .. } | Inner::Component { .. } => {
                    count(&mut self.used.approximate)
                }
            }
        }
        (self.inner_units, self.partial_units) = (inner, partial);
        for e in 0..self.offsets.len() - 1 {
            let mut covers = with_capacity(self.pairs.len())?;
            for (p, pair) in self.pairs.iter().enumerate() {
                if let Some(side) = pair.effects.iter().position(|&x| x == e) {
                    covers.push((p, side));
                }
            }
            self.covers.push(covers);
        }
        self.bytes = self.pairs.iter().map(Pair::bytes).sum::<u128>()
            + self.pieces.iter().map(|p| p.inner.bytes()).sum::<u128>()
            + (self.pieces.capacity() * std::mem::size_of::<Piece>()) as u128
            + 16 * (self.pairs.len() * self.covers.len()) as u128
            + 8 * self.offsets.capacity() as u128;
        Ok(())
    }

    /// Whether this preconditioner is the one of `spec` for weight state
    /// `generation`.
    pub(super) fn current(&self, spec: SchwarzSpec, generation: u64) -> bool {
        self.spec == spec && self.generation == generation
    }

    /// Retained bytes (without the apply workspace).
    pub(super) fn bytes(&self) -> u128 {
        self.bytes
    }

    /// Bytes of the apply workspace of `width` columns.
    pub(super) fn scratch_bytes(&self, width: usize) -> u128 {
        let w = width as u128;
        8 * (self.slots as u128 * w
            + self.inner_units as u128 * w.max(2)
            + self.partial_units as u128 * w)
    }

    pub(super) fn used(&self) -> PreconditionerUsed {
        PreconditionerUsed::Schwarz {
            subdomains: u32::try_from(self.pieces.len()).unwrap_or(u32::MAX),
            local: self.used,
        }
    }

    /// Size `scratch` for `width` columns, keeping allocations.
    pub(super) fn size(&self, scratch: &mut Scratch, width: usize) -> Result<()> {
        let sizes = [
            (&mut scratch.local, self.slots * width),
            (&mut scratch.inner, self.inner_units * width.max(2)),
            (&mut scratch.partials, self.partial_units * width),
        ];
        for (values, len) in sizes {
            if values.len() != len {
                refill(values, len, 0.0)?;
            }
        }
        Ok(())
    }

    /// `z = M^-1 r` for `R` interleaved columns (`scratch` sized for `R`);
    /// `inverse_mass` gives Jacobi on levels in no subdomain.
    pub(super) fn apply<const R: usize>(
        &self,
        r: &[f64],
        z: &mut [f64],
        scratch: &mut Scratch,
        inverse_mass: &[f64],
    ) {
        let Scratch {
            local,
            inner,
            partials,
        } = scratch;
        if !self.pieces.is_empty() {
            let local = &mut local[..self.slots * R];
            self.solve::<R>(&self.pieces, r, (local, inner, partials));
        }
        let local = &local[..];
        for (e, covers) in self.covers.iter().enumerate() {
            let (lo, hi) = (self.offsets[e], self.offsets[e + 1]);
            let body = |l: usize, out: &mut [f64]| {
                if covers.is_empty() {
                    let d = inverse_mass[lo + l];
                    for j in 0..R {
                        out[j] = d * r[(lo + l) * R + j];
                    }
                    return;
                }
                let mut s = [0.0; R];
                for &(p, side) in covers {
                    let pair = &self.pairs[p];
                    let at = (pair.first + pair.slot[side][l] as usize) * R;
                    for j in 0..R {
                        s[j] += local[at + j];
                    }
                }
                out.copy_from_slice(&s);
            };
            let out = &mut z[lo * R..hi * R];
            if out.len() < PARALLEL_WORK {
                for (l, row) in out.chunks_exact_mut(R).enumerate() {
                    body(l, row);
                }
            } else {
                use rayon::prelude::*;
                out.par_chunks_mut(R)
                    .enumerate()
                    .for_each(|(l, row)| body(l, row));
            }
        }
    }

    /// The local actions of contiguous `pieces` into their slots: halves in
    /// parallel by slots, small groups serially.
    fn solve<const R: usize>(
        &self,
        pieces: &[Piece],
        r: &[f64],
        (local, inner, partials): (&mut [f64], &mut [f64], &mut [f64]),
    ) {
        let w = R.max(2);
        let first = &pieces[0];
        let last = &pieces[pieces.len() - 1];
        let span = last.start + last.n() - first.start;
        if pieces.len() == 1 || span * R < PARALLEL_WORK {
            let (mut local, mut inner, mut partials) = (local, inner, partials);
            for piece in pieces {
                let (x, rest) = std::mem::take(&mut local).split_at_mut(piece.n() * R);
                let (s, rest_inner) =
                    std::mem::take(&mut inner).split_at_mut(piece.inner_units() * w);
                let (p, rest_partials) =
                    std::mem::take(&mut partials).split_at_mut(piece.partial_units() * R);
                piece.solve::<R>(&self.pairs[piece.pair], r, x, s, p);
                (local, inner, partials) = (rest, rest_inner, rest_partials);
            }
            return;
        }
        let middle = first.start + span / 2;
        let mid = pieces
            .partition_point(|p| p.start < middle)
            .clamp(1, pieces.len() - 1);
        let (left, right) = pieces.split_at(mid);
        let at = &right[0];
        let (local_left, local_right) = local.split_at_mut((at.start - first.start) * R);
        let (inner_left, inner_right) =
            inner.split_at_mut((at.inner_first - first.inner_first) * w);
        let (partials_left, partials_right) =
            partials.split_at_mut((at.partial_first - first.partial_first) * R);
        rayon::join(
            || self.solve::<R>(left, r, (local_left, inner_left, partials_left)),
            || self.solve::<R>(right, r, (local_right, inner_right, partials_right)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::Design;
    use crate::projector::SchwarzLocal;
    use crate::weights::{validate, Weights};

    /// SplitMix64.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        }

        fn below(&mut self, bound: usize) -> usize {
            (self.next() % bound as u64) as usize
        }

        fn uniform(&mut self) -> f64 {
            (self.next() >> 11) as f64 / (1_u64 << 53) as f64 - 0.5
        }
    }

    /// Uniform codes with every level observed, and positive weights.
    fn codes(seed: u64, n: usize, levels: &[usize]) -> (Vec<Vec<u32>>, Vec<f64>) {
        let mut rng = Rng(seed);
        let codes = levels
            .iter()
            .map(|&count| {
                (0..n)
                    .map(|i| if i < count { i } else { rng.below(count) } as u32)
                    .collect()
            })
            .collect();
        let weights = (0..n).map(|_| 1.0 + rng.uniform()).collect();
        (codes, weights)
    }

    /// The preconditioner of `spec` on a design, with its coefficient count.
    fn build(
        codes: &[Vec<u32>],
        levels: &[usize],
        weights: Option<&[f64]>,
        spec: SchwarzSpec,
    ) -> (Schwarz, State) {
        let n = codes[0].len();
        let effects = codes
            .iter()
            .zip(levels)
            .map(|(codes, &levels)| Effect::new(codes, levels))
            .collect();
        let design = Design::new(n, effects).unwrap();
        let cancel = Cancel(None);
        let st = Structure::build(&design, &cancel).unwrap().unwrap();
        let validated = validate(weights.map_or(Weights::Unit, Weights::Values), n).unwrap();
        let mut state = State::default();
        let mut scatter = Vec::new();
        assert!(state
            .fill(&st, design.effects(), validated, false, &mut scatter)
            .unwrap());
        let built = Schwarz::build(&st, design.effects(), &state, (spec, 1), (None, 8), &cancel);
        (built.unwrap().unwrap(), state)
    }

    fn apply(schwarz: &Schwarz, state: &State, r: &[f64]) -> Vec<f64> {
        let mut scratch = Scratch::default();
        schwarz.size(&mut scratch, 1).unwrap();
        let mut z = vec![f64::NAN; r.len()];
        schwarz.apply::<1>(r, &mut z, &mut scratch, &state.inverse_mass);
        z
    }

    fn dot(a: &[f64], b: &[f64]) -> f64 {
        a.iter().zip(b).map(|(x, y)| x * y).sum()
    }

    /// Eigenvalues of a symmetric matrix by cyclic Jacobi rotations.
    fn eigenvalues(mut a: Vec<f64>, n: usize) -> Vec<f64> {
        for _ in 0..100 {
            let off: f64 = (0..n)
                .flat_map(|p| (0..n).filter(move |&q| q != p).map(move |q| (p, q)))
                .map(|(p, q)| a[p * n + q] * a[p * n + q])
                .sum();
            let total: f64 = a.iter().map(|x| x * x).sum();
            if off <= 1e-30 * total {
                break;
            }
            for p in 0..n {
                for q in p + 1..n {
                    let apq = a[p * n + q];
                    if apq == 0.0 {
                        continue;
                    }
                    let theta = (a[q * n + q] - a[p * n + p]) / (2.0 * apq);
                    let t = if theta == 0.0 {
                        1.0
                    } else {
                        theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt())
                    };
                    let c = 1.0 / (t * t + 1.0).sqrt();
                    let s = t * c;
                    for k in 0..n {
                        let (kp, kq) = (a[k * n + p], a[k * n + q]);
                        a[k * n + p] = c * kp - s * kq;
                        a[k * n + q] = s * kp + c * kq;
                    }
                    for k in 0..n {
                        let (pk, qk) = (a[p * n + k], a[q * n + k]);
                        a[p * n + k] = c * pk - s * qk;
                        a[q * n + k] = s * pk + c * qk;
                    }
                }
            }
        }
        (0..n).map(|i| a[i * n + i]).collect()
    }

    fn specs() -> Vec<SchwarzSpec> {
        let mut specs = Vec::new();
        for subdomains in [Subdomains::Star, Subdomains::AllPairs] {
            for local in [
                SchwarzLocal::Auto,
                SchwarzLocal::Exact,
                SchwarzLocal::Approximate,
            ] {
                specs.push(SchwarzSpec { subdomains, local });
            }
        }
        specs
    }

    /// The assembled action of a tiny design is symmetric with all
    /// eigenvalues positive (nonsingular on all of coefficient space,
    /// although `D'WD` is singular), for every rule and local solve.
    #[test]
    fn dense_action_is_symmetric_positive_definite() {
        let levels = [24, 9, 5, 3];
        let (codes, weights) = codes(11, 90, &levels);
        let p: usize = levels.iter().sum();
        for spec in specs() {
            for weights in [None, Some(&weights[..])] {
                let (schwarz, state) = build(&codes, &levels, weights, spec);
                let mut m = vec![0.0; p * p];
                for j in 0..p {
                    let mut e = vec![0.0; p];
                    e[j] = 1.0;
                    for (i, z) in apply(&schwarz, &state, &e).into_iter().enumerate() {
                        m[i * p + j] = z;
                    }
                }
                let scale = m.iter().fold(0.0_f64, |s, v| s.max(v.abs()));
                for i in 0..p {
                    for j in 0..i {
                        let gap = (m[i * p + j] - m[j * p + i]).abs();
                        assert!(gap <= 1e-12 * scale, "{spec:?} ({i}, {j}): {gap}");
                    }
                }
                let lambda = eigenvalues(m, p);
                let (lo, hi) = lambda
                    .iter()
                    .fold((f64::INFINITY, 0.0_f64), |(lo, hi), &l| {
                        (lo.min(l), hi.max(l))
                    });
                assert!(lo > 1e-6 * hi, "{spec:?}: eigenvalues in [{lo}, {hi}]");
            }
        }
    }

    /// Random vectors on a larger design: symmetric bilinear form, positive
    /// quadratic form, and the interleaved multi-column action equal to the
    /// single-column one.
    #[test]
    fn random_vectors_see_a_symmetric_positive_definite_action() {
        let levels = [1500, 1100, 40, 7, 3];
        let (codes, weights) = codes(21, 9000, &levels);
        let p: usize = levels.iter().sum();
        let mut rng = Rng(22);
        for spec in specs() {
            let (schwarz, state) = build(&codes, &levels, Some(&weights), spec);
            let vectors: Vec<Vec<f64>> = (0..6)
                .map(|_| (0..p).map(|_| rng.uniform()).collect())
                .collect();
            let images: Vec<Vec<f64>> =
                vectors.iter().map(|v| apply(&schwarz, &state, v)).collect();
            for a in 0..vectors.len() {
                let form = dot(&vectors[a], &images[a]);
                assert!(form > 0.0, "{spec:?}: {form}");
                for b in 0..a {
                    let (x, y) = (dot(&vectors[a], &images[b]), dot(&vectors[b], &images[a]));
                    assert!((x - y).abs() <= 1e-12 * (x.abs() + y.abs()), "{spec:?}");
                }
            }
            const R: usize = 3;
            let mut r = vec![0.0; p * R];
            for (j, v) in vectors[..R].iter().enumerate() {
                for (g, &x) in v.iter().enumerate() {
                    r[g * R + j] = x;
                }
            }
            let mut scratch = Scratch::default();
            schwarz.size(&mut scratch, R).unwrap();
            let mut z = vec![f64::NAN; p * R];
            schwarz.apply::<R>(&r, &mut z, &mut scratch, &state.inverse_mass);
            for (j, image) in images[..R].iter().enumerate() {
                for (g, &x) in image.iter().enumerate() {
                    let y = z[g * R + j];
                    assert!((x - y).abs() <= 1e-12 * x.abs().max(1e-300), "{spec:?} {g}");
                }
            }
        }
    }

    /// A Schur complement denser than its component's bipartite graph (each
    /// of 2000 larger-side levels in about eight of 300 smaller-side ones)
    /// is approximated on the whole component: the local action stays
    /// symmetric positive definite and its multi-column form agrees.
    #[test]
    fn dense_schur_complements_factor_the_whole_component() {
        let levels = [2000, 300];
        let (codes, weights) = codes(41, 16_000, &levels);
        let p: usize = levels.iter().sum();
        let spec = SchwarzSpec {
            subdomains: Subdomains::Star,
            local: SchwarzLocal::Approximate,
        };
        let (schwarz, state) = build(&codes, &levels, Some(&weights), spec);
        assert!(schwarz
            .pieces
            .iter()
            .any(|piece| matches!(piece.inner, Inner::Component { .. })));
        let PreconditionerUsed::Schwarz { local, .. } = schwarz.used() else {
            panic!("a Schwarz preconditioner reports Schwarz");
        };
        assert!(local.approximate >= 1);
        let mut rng = Rng(42);
        let vectors: Vec<Vec<f64>> = (0..4)
            .map(|_| (0..p).map(|_| rng.uniform()).collect())
            .collect();
        let images: Vec<Vec<f64>> = vectors.iter().map(|v| apply(&schwarz, &state, v)).collect();
        for a in 0..vectors.len() {
            assert!(dot(&vectors[a], &images[a]) > 0.0);
            for b in 0..a {
                let (x, y) = (dot(&vectors[a], &images[b]), dot(&vectors[b], &images[a]));
                assert!((x - y).abs() <= 1e-12 * (x.abs() + y.abs()));
            }
        }
        const R: usize = 2;
        let mut r = vec![0.0; p * R];
        for (j, v) in vectors[..R].iter().enumerate() {
            for (g, &x) in v.iter().enumerate() {
                r[g * R + j] = x;
            }
        }
        let mut scratch = Scratch::default();
        schwarz.size(&mut scratch, R).unwrap();
        let mut z = vec![f64::NAN; p * R];
        schwarz.apply::<R>(&r, &mut z, &mut scratch, &state.inverse_mass);
        for (j, image) in images[..R].iter().enumerate() {
            for (g, &x) in image.iter().enumerate() {
                assert!((x - z[g * R + j]).abs() <= 1e-12 * x.abs().max(1e-300));
            }
        }
    }

    /// A one-effect design has no pair: the action is Jacobi.
    #[test]
    fn one_effect_is_jacobi() {
        let (codes, weights) = codes(31, 50, &[12]);
        let (schwarz, state) = build(&codes, &[12], Some(&weights), SchwarzSpec::default());
        assert_eq!(schwarz.pieces.len(), 0);
        let r: Vec<f64> = (0..12).map(|g| g as f64 - 4.5).collect();
        let z = apply(&schwarz, &state, &r);
        for g in 0..12 {
            assert_eq!(z[g], r[g] * state.inverse_mass[g]);
        }
    }
}
